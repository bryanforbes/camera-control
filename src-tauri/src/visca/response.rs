use bytes::Bytes;

use super::Error;

#[derive(Clone, Copy, Debug)]
pub enum ResponseKind {
    Ack,
    Completion,
}

impl TryFrom<&Bytes> for ResponseKind {
    type Error = Error;

    fn try_from(bytes: &Bytes) -> std::result::Result<Self, Self::Error> {
        match (bytes[1] >> 4) & 0b0111 {
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
    bytes: Bytes,
}

impl Response {
    pub fn kind(&self) -> ResponseKind {
        self.kind
    }

    pub fn address(&self) -> u8 {
        (self.bytes[0] >> 4) - 8
    }

    pub fn payload(&self) -> Bytes {
        self.bytes.slice(2..self.bytes.len() - 1)
    }
}

impl TryFrom<Bytes> for Response {
    type Error = Error;

    fn try_from(bytes: Bytes) -> std::result::Result<Self, Self::Error> {
        debug!("bytes: {:?}", bytes);

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

    fn matches_packet<const N: usize>(expected: [u8; N]) -> impl Fn(Result<Vec<u8>>) {
        let expected = Vec::from(expected);
        move |actual| match actual {
            Ok(value) => assert_eq!(value, expected),
            Err(_) => panic!("Error returned"),
        }
    }

    #[test_case(b"\x90\x40\xFF" => matches Ok(ResponseKind::Ack); "ack 0")]
    #[test_case(b"\x90\x41\xFF" => matches Ok(ResponseKind::Ack); "ack 1")]
    #[test_case(b"\x90\xC1\xFF" => matches Ok(ResponseKind::Ack); "ack C1")]
    #[test_case(b"\x90\x50\xFF" => matches Ok(ResponseKind::Completion); "completion 0")]
    #[test_case(b"\x90\x51\xFF" => matches Ok(ResponseKind::Completion); "completion 1")]
    #[test_case(b"\x90\xD1\xFF" => matches Ok(ResponseKind::Completion); "completion D1")]
    #[test_case(b"" => matches Err(Error::InvalidResponse); "empty packet")]
    #[test_case(b"\x90" => matches Err(Error::InvalidResponse); "1 item packet")]
    #[test_case(b"\x90\xFF" => matches Err(Error::InvalidResponse); "2 item packet")]
    #[test_case(b"\x90\x00\xFF" => matches Err(Error::InvalidResponse); "wrong packet type")]
    #[test_case(b"\x90\x60\xFF" => matches Err(Error::InvalidResponse); "missing error code")]
    #[test_case(b"\x90\x60\x01\xFF" => matches Err(Error::InvalidMessageLength); "invalid message length")]
    #[test_case(b"\x90\x60\x02\xFF" => matches Err(Error::Syntax); "syntax error")]
    #[test_case(b"\x90\x60\x03\xFF" => matches Err(Error::CommandBufferFull); "command buffer full")]
    #[test_case(b"\x90\x60\x04\xFF" => matches Err(Error::CommandCanceled); "command canceled")]
    #[test_case(b"\x90\x60\x05\xFF" => matches Err(Error::NoSocket); "no socket")]
    #[test_case(b"\x90\x60\x41\xFF" => matches Err(Error::CommandNotExecutable); "command not executable")]
    #[test_case(b"\x90\x60\x06\xFF" => matches Err(Error::Unknown); "unknown error")]
    fn test_try_from(packet: &'static [u8]) -> Result<ResponseKind> {
        Ok(Response::try_from(Bytes::from_static(packet))?.kind())
    }

    #[test_case(b"\x90\x40\xFF" => matches Ok(1); "address 1")]
    #[test_case(b"\xA0\x40\xFF" => matches Ok(2); "address 2")]
    #[test_case(b"\xB0\x40\xFF" => matches Ok(3); "address 3")]
    #[test_case(b"\xC0\x40\xFF" => matches Ok(4); "address 4")]
    #[test_case(b"\xD0\x40\xFF" => matches Ok(5); "address 5")]
    #[test_case(b"\xE0\x40\xFF" => matches Ok(6); "address 6")]
    #[test_case(b"\xF0\x40\xFF" => matches Ok(7); "address 7")]
    fn test_address(packet: &'static [u8]) -> Result<u8> {
        Ok(Response::try_from(Bytes::from_static(packet))?.address())
    }

    #[test_case(b"\x90\x41\xFF" => using matches_packet([]); "empty")]
    #[test_case(b"\x90\x41\x02\xFF" => using matches_packet([0x02]); "one item")]
    #[test_case(b"\x90\x41\x02\x03\xFF" => using matches_packet([0x02, 0x03]); "two items")]
    fn test_payload(packet: &'static [u8]) -> Result<Vec<u8>> {
        Ok(Response::try_from(Bytes::from_static(packet))?
            .payload()
            .to_vec())
    }
}
