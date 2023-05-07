use super::{Error, Packet};

#[derive(Clone, Copy, Debug)]
pub enum ResponseKind {
    Ack,
    Completion,
}

impl TryFrom<&Packet> for ResponseKind {
    type Error = Error;

    fn try_from(bytes: &Packet) -> std::result::Result<Self, Self::Error> {
        match bytes[1] >> 4 {
            4 => Ok(Self::Ack),
            5 => Ok(Self::Completion),
            6 => {
                if bytes.len() < 4 {
                    Err(Error::InvalidResponse)
                } else {
                    Err(match bytes[2] {
                        0x01 => Error::InvalidMessageLength,
                        0x02 => Error::Syntax,
                        0x03 => Error::CommandBufferFull,
                        0x04 => Error::CommandCanceled,
                        0x05 => Error::NoSocket,
                        0x41 => Error::CommandNotExecutable,
                        _ => Error::Unknown,
                    })
                }
            }
            _ => Err(Error::InvalidResponse),
        }
    }
}

#[derive(Debug)]
pub struct Response {
    kind: ResponseKind,
    bytes: Packet,
}

impl Response {
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

impl TryFrom<Packet> for Response {
    type Error = Error;

    fn try_from(bytes: Packet) -> std::result::Result<Self, Self::Error> {
        println!("bytes: {:?}", bytes);

        if bytes.len() < 3 {
            return Err(Error::InvalidResponse);
        }

        Ok(Self {
            kind: (&bytes).try_into()?,
            bytes,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::super::Result;
    use super::*;

    use test_case::test_case;

    fn matches_packet<const N: usize>(expected: [u8; N]) -> impl Fn(Result<Packet>) {
        let expected = Vec::from(expected);
        move |actual| match actual {
            Ok(value) => assert_eq!(value, expected),
            Err(_) => panic!("Error returned"),
        }
    }

    #[test_case(vec![0x90, 0x40, 0xFF] => matches Ok(ResponseKind::Ack); "ack 0")]
    #[test_case(vec![0x90, 0x41, 0xFF] => matches Ok(ResponseKind::Ack); "ack 1")]
    #[test_case(vec![0x90, 0x50, 0xFF] => matches Ok(ResponseKind::Completion); "completion 0")]
    #[test_case(vec![0x90, 0x51, 0xFF] => matches Ok(ResponseKind::Completion); "completion 1")]
    #[test_case(vec![] => matches Err(Error::InvalidResponse); "empty packet")]
    #[test_case(vec![0x90] => matches Err(Error::InvalidResponse); "1 item packet")]
    #[test_case(vec![0x90, 0xFF] => matches Err(Error::InvalidResponse); "2 item packet")]
    #[test_case(vec![0x90, 0x00, 0xFF] => matches Err(Error::InvalidResponse); "wrong packet type")]
    #[test_case(vec![0x90, 0x60, 0xFF] => matches Err(Error::InvalidResponse); "missing error code")]
    #[test_case(vec![0x90, 0x60, 0x01, 0xFF] => matches Err(Error::InvalidMessageLength); "invalid message length")]
    #[test_case(vec![0x90, 0x60, 0x02, 0xFF] => matches Err(Error::Syntax); "syntax error")]
    #[test_case(vec![0x90, 0x60, 0x03, 0xFF] => matches Err(Error::CommandBufferFull); "command buffer full")]
    #[test_case(vec![0x90, 0x60, 0x04, 0xFF] => matches Err(Error::CommandCanceled); "command canceled")]
    #[test_case(vec![0x90, 0x60, 0x05, 0xFF] => matches Err(Error::NoSocket); "no socket")]
    #[test_case(vec![0x90, 0x60, 0x41, 0xFF] => matches Err(Error::CommandNotExecutable); "command not executable")]
    #[test_case(vec![0x90, 0x60, 0x06, 0xFF] => matches Err(Error::Unknown); "unknown error")]
    fn test_try_from(packet: Packet) -> Result<ResponseKind> {
        Ok(Response::try_from(packet)?.kind())
    }

    #[test_case(vec![0x90, 0x40, 0xFF] => matches Ok(1); "address 1")]
    #[test_case(vec![0xA0, 0x40, 0xFF] => matches Ok(2); "address 2")]
    #[test_case(vec![0xB0, 0x40, 0xFF] => matches Ok(3); "address 3")]
    #[test_case(vec![0xC0, 0x40, 0xFF] => matches Ok(4); "address 4")]
    #[test_case(vec![0xD0, 0x40, 0xFF] => matches Ok(5); "address 5")]
    #[test_case(vec![0xE0, 0x40, 0xFF] => matches Ok(6); "address 6")]
    #[test_case(vec![0xF0, 0x40, 0xFF] => matches Ok(7); "address 7")]
    fn test_address(packet: Packet) -> Result<u8> {
        Ok(Response::try_from(packet)?.address())
    }

    #[test_case(vec![0x90, 0x41, 0xFF] => using matches_packet([]); "empty")]
    #[test_case(vec![0x90, 0x41, 0x02, 0xFF] => using matches_packet([0x02]); "one item")]
    #[test_case(vec![0x90, 0x41, 0x02, 0x03, 0xFF] => using matches_packet([0x02, 0x03]); "two items")]
    fn test_payload(packet: Packet) -> Result<Packet> {
        Ok(Response::try_from(packet)?.payload().to_vec())
    }
}
