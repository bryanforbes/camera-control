use std::io::{BufRead, BufReader, ErrorKind, Write};

use deku::{DekuContainerRead, DekuContainerWrite};
use log::debug;
use serialport::SerialPort;

use super::{
    InquiryRequestBuilder, Request, Response, ResponseKind, Result, ViscaAction, ViscaError,
    ViscaInquiry,
};

fn header_for_address(address: u8) -> Result<u8> {
    if address <= 7 {
        Ok(0x80 | address)
    } else {
        Err(ViscaError::InvalidAddress)
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

    fn send_packet_with_response(&mut self, address: u8, request: &Request) -> Result<Response> {
        let output: Vec<u8> = request.to_bytes()?;

        #[cfg(debug_assertions)]
        {
            debug!("Sending: {:02X?}", output);
        }

        self.writer.write_all(&output)?;

        let response = self.receive_response()?;
        if let ResponseKind::Completion(_) = response.kind() {
            return Ok(response);
        }

        let response = self.receive_response()?;
        if let ResponseKind::Completion(_) = response.kind() {
            Ok(response)
        } else {
            Err(ViscaError::InvalidResponse)
        }
    }

    fn receive_response(&mut self) -> Result<Response> {
        loop {
            let mut bytes: Vec<u8> = Vec::with_capacity(16);
            match self.reader.read_until(0xFF, &mut bytes) {
                Ok(_) => {
                    #[cfg(debug_assertions)]
                    debug!("Received: {:02X?}", bytes.to_vec());

                    let ((_, _), response) = Response::from_bytes((bytes.as_ref(), 0))?;
                    return Ok(response);
                }
                Err(error) if error.kind() == ErrorKind::Interrupted => continue,
                Err(error) => return Err(ViscaError::Io(error)),
            }
        }
    }

    pub fn execute(&mut self, address: u8, action: impl ViscaAction) -> Result<()> {
        let request = action.action(address).build()?;
        let response = self.send_packet_with_response(address, &request)?;

        if response.data().is_empty() {
            Ok(())
        } else {
            Err(ViscaError::InvalidResponse)
        }
    }

    pub fn inquire<R: ViscaInquiry>(&mut self, address: u8) -> Result<R> {
        let request = InquiryRequestBuilder::new(address).build::<R>()?;
        let response = self.send_packet_with_response(address, &request)?;
        R::from_response(&response)
    }
}
