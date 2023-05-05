use super::{Error, Result};

pub enum ResponseKind {
    Ack,
    Completion,
}

pub struct Response {
    bytes: Vec<u8>,
}

impl Response {
    pub fn new(bytes: Vec<u8>) -> Self {
        Self { bytes }
    }

    pub fn address(&self) -> u8 {
        (self.bytes[0] >> 4) - 8
    }

    pub fn payload(&self) -> &[u8] {
        self.bytes[2..self.bytes.len() - 1].as_ref()
    }

    pub fn kind(&self) -> Result<ResponseKind> {
        match self.bytes[1] & 0xF0 {
            0x40 => Ok(ResponseKind::Ack),
            0x50 => Ok(ResponseKind::Completion),
            0x60 => Err(match self.bytes[2] {
                0x01 => Error::InvalidMessageLength,
                0x02 => Error::SyntaxError,
                0x03 => Error::CommandBufferFull,
                0x04 => Error::CommandCanceled,
                0x05 => Error::NoSocket,
                0x41 => Error::CommandNotExecutable,
                _ => Error::UnknownMessageError,
            }),
            _ => Err(Error::UnknownMessageError),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_address() {
        assert_eq!(Response::new(vec![0x90]).address(), 1);
        assert_eq!(Response::new(vec![0xA0]).address(), 2);
        assert_eq!(Response::new(vec![0xB0]).address(), 3);
        assert_eq!(Response::new(vec![0xC0]).address(), 4);
        assert_eq!(Response::new(vec![0xD0]).address(), 5);
        assert_eq!(Response::new(vec![0xE0]).address(), 6);
        assert_eq!(Response::new(vec![0xF0]).address(), 7);
    }

    #[test]
    fn test_payload() {
        assert_eq!(
            Vec::from(Response::new(vec![0x90, 0x41, 0xFF]).payload()),
            Vec::<u8>::new()
        );
        assert_eq!(
            Vec::from(Response::new(vec![0x90, 0x50, 0x02, 0xFF]).payload()),
            Vec::<u8>::from([0x02])
        );
        assert_eq!(
            Vec::from(Response::new(vec![0x90, 0x50, 0x02, 0x03, 0xFF]).payload()),
            Vec::<u8>::from([0x02, 0x03])
        );
    }
}
