use core::fmt;
use std::io::{BufRead, BufReader, Read, Write};

use super::{Action, Error, Inquiry, Packet, Response, ResponseKind, Result};

fn header_for_address(address: u8) -> Result<u8> {
    if address <= 7 {
        Ok(0x80 | address)
    } else {
        Err(Error::InvalidAddress)
    }
}

pub struct ViscaPort<T: Read + Write> {
    reader: BufReader<Box<T>>,
}

impl<T> ViscaPort<T>
where
    T: Read + Write,
{
    pub fn new(port: T) -> Self {
        Self {
            reader: BufReader::new(Box::new(port)),
        }
    }

    fn send_packet_with_response(
        &mut self,
        address: u8,
        packet_type: u8,
        category: u8,
        id: u8,
        data: Option<&[u8]>,
    ) -> Result<Response> {
        let reader = self.reader.get_mut();

        #[cfg(debug_assertions)]
        {
            let mut bytes: Packet = vec![header_for_address(address)?, packet_type, category, id];

            if let Some(data) = data {
                bytes.extend_from_slice(data);
            }

            bytes.push(0xFF);

            println!("Sending: {:02X?}", bytes);
        }

        reader.write_all(&[header_for_address(address)?, packet_type, category, id])?;

        if let Some(data) = data {
            reader.write_all(data)?;
        }

        reader.write_all(&[0xFF])?;

        let response = self.receive_response(address)?;
        if let ResponseKind::Completion = response.kind() {
            return Ok(response);
        }

        let response = self.receive_response(address)?;
        if let ResponseKind::Completion = response.kind() {
            Ok(response)
        } else {
            Err(Error::InvalidResponse)
        }
    }

    fn receive_response(&mut self, address: u8) -> Result<Response> {
        loop {
            let mut bytes: Packet = vec![];
            self.reader.read_until(0xFF, &mut bytes)?;

            #[cfg(debug_assertions)]
            println!("Received: {:02X?}", bytes);

            let response: Response = bytes.try_into()?;
            if response.address() == address {
                return Ok(response);
            }
        }
    }

    pub fn execute<const P: usize, A: Action<P>>(&mut self, address: u8, action: A) -> Result<()> {
        let response = self.send_packet_with_response(
            address,
            0x01,
            A::COMMAND_CATEGORY,
            A::COMMAND_ID,
            Some(&action.data()?),
        )?;

        if response.payload().is_empty() {
            Ok(())
        } else {
            Err(Error::InvalidResponse)
        }
    }

    pub fn inquire<R: Inquiry>(&mut self, address: u8) -> Result<R> {
        let response = self.send_packet_with_response(
            address,
            0x09,
            R::COMMAND_CATEGORY,
            R::COMMAND_ID,
            None,
        )?;
        R::transform_inquiry_response(&response)
    }
}

impl<T> fmt::Debug for ViscaPort<T>
where
    T: Read + Write + fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ViscaPort ( {:?} )", self.reader.get_ref())
    }
}
