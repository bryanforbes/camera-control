use core::fmt;
use std::io::{BufRead, BufReader, Read, Write};

use super::{Command, Error, Inquiry, Response, ResponseKind, Result};

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

    fn send_packet_with_response(&mut self, address: u8, packet: &[u8]) -> Result<Response> {
        self.reader.get_mut().write_all(packet)?;

        let response = self.receive_response(address)?;
        if let ResponseKind::Completion = response.kind()? {
            return Ok(response);
        }

        let response = self.receive_response(address)?;
        if let ResponseKind::Completion = response.kind()? {
            Ok(response)
        } else {
            Err(Error::InvalidResponse)
        }
    }

    fn receive_response(&mut self, address: u8) -> Result<Response> {
        loop {
            let mut bytes: Vec<u8> = vec![];
            self.reader.read_until(0xFF, &mut bytes)?;

            let response = Response::new(bytes);
            if response.address() == address {
                return Ok(response);
            }
        }
    }

    pub fn execute<C: Command>(&mut self, address: u8, command: C) -> Result<()> {
        let response =
            self.send_packet_with_response(address, command.to_command_bytes(address)?.as_slice())?;

        if response.payload().len() == 0 {
            Ok(())
        } else {
            Err(Error::InvalidResponse)
        }
    }

    pub fn inquire<R: Inquiry>(&mut self, address: u8) -> Result<R> {
        let response = self.send_packet_with_response(address, &R::to_inquiry_bytes(address)?)?;
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
