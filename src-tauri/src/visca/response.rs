use super::{Error, Result};

#[derive(Clone, Copy, Debug)]
pub enum ResponseKind {
    Ack,
    Completion,
}

#[derive(Debug)]
pub struct Response {
    kind: ResponseKind,
    bytes: Vec<u8>,
}

impl Response {
    pub fn new(bytes: Vec<u8>) -> Result<Self> {
        if bytes.len() < 3 {
            return Err(Error::InvalidResponse);
        }

        match bytes[1] & 0xF0 {
            kind @ (0x40 | 0x50) => Ok(Self {
                bytes,
                kind: if kind == 0x40 {
                    ResponseKind::Ack
                } else {
                    ResponseKind::Completion
                },
            }),
            0x60 => {
                if bytes.len() < 4 {
                    Err(Error::InvalidResponse)
                } else {
                    Err(match bytes[2] {
                        0x01 => Error::InvalidMessageLength,
                        0x02 => Error::SyntaxError,
                        0x03 => Error::CommandBufferFull,
                        0x04 => Error::CommandCanceled,
                        0x05 => Error::NoSocket,
                        0x41 => Error::CommandNotExecutable,
                        _ => Error::UnknownError,
                    })
                }
            }
            _ => Err(Error::InvalidResponse),
        }
    }

    pub fn kind(&self) -> ResponseKind {
        self.kind
    }

    pub fn address(&self) -> u8 {
        (self.bytes[0] >> 4) - 8
    }

    pub fn payload(&self) -> &[u8] {
        self.bytes[2..self.bytes.len() - 1].as_ref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new() {
        assert_matches!(
            Response::new(vec![0x90, 0x40, 0xFF]).unwrap().kind(),
            ResponseKind::Ack
        );
        assert_matches!(
            Response::new(vec![0x90, 0x41, 0xFF]).unwrap().kind(),
            ResponseKind::Ack
        );
        assert_matches!(
            Response::new(vec![0x90, 0x50, 0xFF]).unwrap().kind(),
            ResponseKind::Completion
        );
        assert_matches!(
            Response::new(vec![0x90, 0x51, 0xFF]).unwrap().kind(),
            ResponseKind::Completion
        );
    }

    #[test]
    fn test_new_errors() {
        assert_matches!(
            Response::new(Vec::new()).unwrap_err(),
            Error::InvalidResponse
        );
        assert_matches!(
            Response::new(vec![0x90]).unwrap_err(),
            Error::InvalidResponse
        );
        assert_matches!(
            Response::new(vec![0x90, 0xFF]).unwrap_err(),
            Error::InvalidResponse
        );
        assert_matches!(
            Response::new(vec![0x90, 0x00, 0xFF]).unwrap_err(),
            Error::InvalidResponse
        );
        assert_matches!(
            Response::new(vec![0x90, 0x60, 0xFF]).unwrap_err(),
            Error::InvalidResponse
        );
        assert_matches!(
            Response::new(vec![0x90, 0x60, 0x01, 0xFF]).unwrap_err(),
            Error::InvalidMessageLength
        );
        assert_matches!(
            Response::new(vec![0x90, 0x60, 0x02, 0xFF]).unwrap_err(),
            Error::SyntaxError
        );
        assert_matches!(
            Response::new(vec![0x90, 0x60, 0x03, 0xFF]).unwrap_err(),
            Error::CommandBufferFull
        );
        assert_matches!(
            Response::new(vec![0x90, 0x60, 0x04, 0xFF]).unwrap_err(),
            Error::CommandCanceled
        );
        assert_matches!(
            Response::new(vec![0x90, 0x60, 0x05, 0xFF]).unwrap_err(),
            Error::NoSocket
        );
        assert_matches!(
            Response::new(vec![0x90, 0x60, 0x41, 0xFF]).unwrap_err(),
            Error::CommandNotExecutable
        );
        assert_matches!(
            Response::new(vec![0x90, 0x60, 0x06, 0xFF]).unwrap_err(),
            Error::UnknownError
        );
    }

    #[test]
    fn test_address() {
        assert_eq!(Response::new(vec![0x90, 0x40, 0xFF]).unwrap().address(), 1);
        assert_eq!(Response::new(vec![0xA0, 0x40, 0xFF]).unwrap().address(), 2);
        assert_eq!(Response::new(vec![0xB0, 0x40, 0xFF]).unwrap().address(), 3);
        assert_eq!(Response::new(vec![0xC0, 0x40, 0xFF]).unwrap().address(), 4);
        assert_eq!(Response::new(vec![0xD0, 0x40, 0xFF]).unwrap().address(), 5);
        assert_eq!(Response::new(vec![0xE0, 0x40, 0xFF]).unwrap().address(), 6);
        assert_eq!(Response::new(vec![0xF0, 0x40, 0xFF]).unwrap().address(), 7);
    }

    #[test]
    fn test_payload() {
        assert_eq!(
            Vec::from(Response::new(vec![0x90, 0x41, 0xFF]).unwrap().payload()),
            Vec::<u8>::new()
        );
        assert_eq!(
            Vec::from(
                Response::new(vec![0x90, 0x50, 0x02, 0xFF])
                    .unwrap()
                    .payload()
            ),
            Vec::<u8>::from([0x02])
        );
        assert_eq!(
            Vec::from(
                Response::new(vec![0x90, 0x50, 0x02, 0x03, 0xFF])
                    .unwrap()
                    .payload()
            ),
            Vec::<u8>::from([0x02, 0x03])
        );
    }
}
