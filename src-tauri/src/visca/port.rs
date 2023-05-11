use core::fmt;
use std::io::{self, BufRead, BufReader, ErrorKind, Read, Write};

use bytes::{BufMut, Bytes, BytesMut};

use super::{Action, Error, Inquiry, Response, ResponseKind, Result};

fn header_for_address(address: u8) -> Result<u8> {
    if address <= 7 {
        Ok(0x80 | address)
    } else {
        Err(Error::InvalidAddress)
    }
}

// Copied from BufRead::read_until and adapted to use BytesMut
fn read_until<R: BufRead + ?Sized>(r: &mut R, delim: u8, buf: &mut BytesMut) -> io::Result<usize> {
    let mut read = 0;
    loop {
        let (done, used) = {
            let available = match r.fill_buf() {
                Ok(n) => n,
                Err(ref e) if e.kind() == ErrorKind::Interrupted => continue,
                Err(e) => return Err(e),
            };
            match memchr::memchr(delim, available) {
                Some(i) => {
                    buf.extend_from_slice(&available[..=i]);
                    (true, i + 1)
                }
                None => {
                    buf.extend_from_slice(available);
                    (false, available.len())
                }
            }
        };
        r.consume(used);
        read += used;
        if done || used == 0 {
            return Ok(read);
        }
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

    fn send_packet_with_response(&mut self, address: u8, bytes: Bytes) -> Result<Response> {
        let reader = self.reader.get_mut();
        let address = header_for_address(address)?;

        #[cfg(debug_assertions)]
        {
            let mut output_bytes = BytesMut::with_capacity(16);
            output_bytes.put_u8(address);
            output_bytes.extend(&bytes);
            output_bytes.put_u8(0xFF);
            debug!("Sending: {:02X?}", output_bytes.to_vec());
        }

        reader.write_all(&[address])?;
        reader.write_all(&bytes)?;
        reader.write_all(&[0xFF])?;

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
            let mut bytes = BytesMut::with_capacity(16);
            match read_until(&mut self.reader, 0xFF, &mut bytes) {
                Ok(_) => {
                    #[cfg(debug_assertions)]
                    debug!("Received: {:02X?}", bytes.to_vec());

                    return bytes.freeze().try_into();
                }
                Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
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

impl<T> fmt::Debug for ViscaPort<T>
where
    T: Read + Write + fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ViscaPort ( {:?} )", self.reader.get_ref())
    }
}
