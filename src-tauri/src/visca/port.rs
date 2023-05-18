use std::io::{BufRead, BufReader, ErrorKind, Write};

use serialport::SerialPort;

use super::{Action, Error, Inquiry, Response, ResponseKind, Result};

fn header_for_address(address: u8) -> Result<u8> {
    if address <= 7 {
        Ok(0x80 | address)
    } else {
        Err(Error::InvalidAddress)
    }
}

pub struct ViscaPort {
    reader: BufReader<Box<dyn SerialPort>>,
    writer: Box<dyn SerialPort>,
}

impl ViscaPort {
    pub fn new(port: Box<dyn SerialPort>) -> Self {
        Self {
            writer: port.try_clone().unwrap(),
            reader: BufReader::new(port),
        }
    }

    fn send_packet_with_response(&mut self, address: u8, bytes: Vec<u8>) -> Result<Response> {
        let address = header_for_address(address)?;

        let mut output: Vec<u8> = Vec::with_capacity(16);

        output.push(address);
        output.extend(bytes);
        output.push(0xFF);

        #[cfg(debug_assertions)]
        {
            debug!("Sending: {:02X?}", output);
        }

        self.writer.write_all(&output)?;

        let response = self.receive_response()?;
        if let ResponseKind::Completion = response.kind() {
            return Ok(response);
        }

        let response = self.receive_response()?;
        if let ResponseKind::Completion = response.kind() {
            Ok(response)
        } else {
            Err(Error::InvalidResponse)
        }
    }

    fn receive_response(&mut self) -> Result<Response> {
        loop {
            let mut bytes: Vec<u8> = Vec::with_capacity(16);
            match self.reader.read_until(0xFF, &mut bytes) {
                Ok(_) => {
                    #[cfg(debug_assertions)]
                    debug!("Received: {:02X?}", bytes.to_vec());

                    return bytes.try_into();
                }
                Err(error) if error.kind() == ErrorKind::Interrupted => continue,
                Err(error) => return Err(Error::Io(error)),
            }
        }
    }

    pub fn execute(&mut self, address: u8, action: impl Action) -> Result<()> {
        let response = self.send_packet_with_response(address, action.to_bytes()?)?;

        if response.payload().is_empty() {
            Ok(())
        } else {
            Err(Error::InvalidResponse)
        }
    }

    pub fn inquire<R>(&mut self, address: u8) -> Result<R>
    where
        R: Inquiry,
    {
        let response = self.send_packet_with_response(address, R::to_bytes())?;
        R::from_response_payload(response.payload())
    }
}
