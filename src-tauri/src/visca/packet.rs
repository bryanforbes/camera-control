use super::Result;
use deku::prelude::*;

#[derive(Debug, PartialEq, DekuRead, DekuWrite)]
pub struct Header {
    #[deku(bits = 1)]
    start: u8,

    #[deku(bits = 3)]
    sender: u8,

    #[deku(bits = 3, pad_bits_before = "1")]
    receiver: u8,
}

impl Header {
    fn new(sender: u8, receiver: u8) -> Self {
        Self {
            start: 1,
            sender,
            receiver,
        }
    }

    pub fn sender(&self) -> u8 {
        self.sender
    }

    pub fn receiver(&self) -> u8 {
        self.receiver
    }
}

#[derive(Debug, Copy, Clone, PartialEq, DekuRead)]
#[deku(id_type = "u8")]
pub enum ResponseErrorKind {
    #[deku(id = 0x01)]
    InvalidMessageLength,

    #[deku(id = 0x02)]
    Syntax,

    #[deku(id = 0x03)]
    CommandBufferFull,

    #[deku(id = 0x04)]
    CommandCanceled,

    #[deku(id = 0x05)]
    NoSocket,

    #[deku(id = 0x41)]
    CommandNotExecutable,
}

#[derive(Debug, Copy, Clone, PartialEq, DekuRead)]
#[deku(id_type = "u8", bits = 3)]
pub enum ResponseKind {
    #[deku(id = 0x4)]
    Ack(#[deku(bits = 4)] u8),

    #[deku(id = 0x5)]
    Completion(#[deku(bits = 4)] u8),

    #[deku(id = 0x6)]
    Err(#[deku(bits = 4)] u8, ResponseErrorKind),
}

#[derive(Debug, PartialEq, DekuRead)]
pub struct Response {
    header: Header,

    #[deku(pad_bits_before = "1")]
    kind: ResponseKind,

    #[deku(until = "|v: &u8| *v == 0xFF")]
    data: Vec<u8>,
}

impl Response {
    pub fn kind(&self) -> ResponseKind {
        self.kind
    }

    pub fn data(&self) -> &[u8] {
        &self.data[0..self.data.len() - 1]
    }
}

#[derive(Debug, PartialEq, DekuWrite)]
#[deku(id_type = "u8")]
pub enum RequestKind {
    #[deku(id = "0x01")]
    Command,
    #[deku(id = "0x09")]
    Inquiry,
}

#[derive(Debug, PartialEq, DekuWrite)]
#[deku(id_type = "u8")]
pub enum RequestCategory {
    #[deku(id = "0x04")]
    Camera,
    #[deku(id = "0x06")]
    PanTilt,
}

#[derive(Debug, PartialEq, DekuWrite)]
pub struct Request {
    header: Header,

    kind: RequestKind,
    category: RequestCategory,
    id: u8,
    data: Vec<u8>,
    last: u8,
}

impl Request {
    fn new(
        sender: u8,
        receiver: u8,
        kind: RequestKind,
        category: RequestCategory,
        id: u8,
        data: Vec<u8>,
    ) -> Self {
        Self {
            header: Header::new(sender, receiver),
            kind,
            category,
            id,
            data,
            last: 0xFF,
        }
    }
}

pub struct ActionRequestBuilder<A: ViscaAction> {
    sender: Option<u8>,
    receiver: u8,
    action: A,
}

impl<A: ViscaAction> ActionRequestBuilder<A> {
    pub fn sender(mut self, sender: u8) -> Self {
        self.sender = Some(sender);
        self
    }

    pub fn build(self) -> Result<Request> {
        Ok(Request::new(
            self.sender.unwrap_or(0),
            self.receiver,
            RequestKind::Command,
            A::CATEGORY,
            A::ID,
            self.action.visca_action_data()?,
        ))
    }
}

pub struct InquiryRequestBuilder {
    sender: Option<u8>,
    receiver: u8,
}

impl InquiryRequestBuilder {
    pub fn new(receiver: u8) -> Self {
        Self {
            sender: None,
            receiver,
        }
    }

    pub fn sender(mut self, sender: u8) -> Self {
        self.sender = Some(sender);
        self
    }

    pub fn build<I: ViscaInquiry>(self) -> Result<Request> {
        Ok(Request::new(
            self.sender.unwrap_or(0),
            self.receiver,
            RequestKind::Inquiry,
            I::CATEGORY,
            I::ID,
            vec![],
        ))
    }
}

pub trait ViscaCommand {
    const ID: u8;
    const CATEGORY: RequestCategory;
}

pub trait ViscaAction: ViscaCommand + Sized {
    fn visca_action_data(&self) -> Result<Vec<u8>>;

    fn action(self, receiver: u8) -> ActionRequestBuilder<Self> {
        ActionRequestBuilder {
            sender: Default::default(),
            receiver,
            action: self,
        }
    }
}

pub trait ViscaInquiry: ViscaCommand + Sized {
    fn from_response(response: &Response) -> Result<Self>;
}

#[cfg(test)]
mod tests {
    use crate::visca::ViscaError;

    use super::*;

    use test_case::test_case;

    fn matches_bytes(expected: &'static [u8]) -> impl Fn(Result<Vec<u8>>) {
        move |actual| match actual {
            Ok(value) => assert_eq!(value, expected),
            Err(_) => panic!("Error returned"),
        }
    }

    #[test_case(b"\x90\x41\xFF" => Response {
        header: Header {
            start: 1, sender: 1, receiver: 0
        },
        kind: ResponseKind::Ack(1),
        data: vec![0xFF]
    })]
    #[test_case(b"\xA0\xC2\xFF" => Response {
        header: Header {
            start: 1, sender: 2, receiver: 0
        },
        kind: ResponseKind::Ack(2),
        data: vec![0xFF]
    })]
    #[test_case(b"\xB0\x53\xFF" => Response {
        header: Header {
            start: 1, sender: 3, receiver: 0
        },
        kind: ResponseKind::Completion(3),
        data: vec![0xFF]
    })]
    #[test_case(b"\xC0\xD4\xFF" => Response {
        header: Header {
            start: 1, sender: 4, receiver: 0
        },
        kind: ResponseKind::Completion(4),
        data: vec![0xFF]
    })]
    #[test_case(b"\xD0\x50\x02\xFF" => Response {
        header: Header {
            start: 1, sender: 5, receiver: 0
        },
        kind: ResponseKind::Completion(0),
        data: vec![0x02, 0xFF]
    })]
    #[test_case(b"\xE0\x60\x01\xFF" => Response {
        header: Header {
            start: 1, sender: 6, receiver: 0
        },
        kind: ResponseKind::Err(0, ResponseErrorKind::InvalidMessageLength),
        data: vec![0xFF]
    })]
    #[test_case(b"\xF0\x60\x02\xFF" => Response {
        header: Header {
            start: 1, sender: 7, receiver: 0
        },
        kind: ResponseKind::Err(0, ResponseErrorKind::Syntax),
        data: vec![0xFF]
    })]
    #[test_case(b"\x90\x60\x03\xFF" => Response {
        header: Header {
            start: 1, sender: 1, receiver: 0
        },
        kind: ResponseKind::Err(0, ResponseErrorKind::CommandBufferFull),
        data: vec![0xFF]
    })]
    #[test_case(b"\x90\x61\x04\xFF" => Response {
        header: Header {
            start: 1, sender: 1, receiver: 0
        },
        kind: ResponseKind::Err(1, ResponseErrorKind::CommandCanceled),
        data: vec![0xFF]
    })]
    #[test_case(b"\x90\x62\x05\xFF" => Response {
        header: Header {
            start: 1, sender: 1, receiver: 0
        },
        kind: ResponseKind::Err(2, ResponseErrorKind::NoSocket),
        data: vec![0xFF]
    })]
    #[test_case(b"\x90\x63\x41\xFF" => Response {
        header: Header {
            start: 1, sender: 1, receiver: 0
        },
        kind: ResponseKind::Err(3, ResponseErrorKind::CommandNotExecutable),
        data: vec![0xFF]
    })]
    fn test_response_try_from(data: &'static [u8]) -> Response {
        Response::try_from(data).unwrap()
    }

    #[test_case(Request::new(
        0, 1, RequestKind::Command, RequestCategory::Camera, 0x01, vec![]
    ) => using matches_bytes(b"\x81\x01\x04\x01\xFF"))]
    #[test_case(Request::new(
        1, 1, RequestKind::Command, RequestCategory::Camera, 0x01, vec![]
    ) => using matches_bytes(b"\x91\x01\x04\x01\xFF"))]
    #[test_case(Request::new(
        2, 1, RequestKind::Command, RequestCategory::Camera, 0x01, vec![]
    ) => using matches_bytes(b"\xA1\x01\x04\x01\xFF"))]
    #[test_case(Request::new(
        0, 2, RequestKind::Command, RequestCategory::Camera, 0x01, vec![]
    ) => using matches_bytes(b"\x82\x01\x04\x01\xFF"))]
    #[test_case(Request::new(
        1, 3, RequestKind::Command, RequestCategory::Camera, 0x01, vec![]
    ) => using matches_bytes(b"\x93\x01\x04\x01\xFF"))]
    #[test_case(Request::new(
        2, 4, RequestKind::Command, RequestCategory::Camera, 0x01, vec![]
    ) => using matches_bytes(b"\xA4\x01\x04\x01\xFF"))]
    #[test_case(Request::new(
        0, 1, RequestKind::Inquiry, RequestCategory::Camera, 0x01, vec![]
    ) => using matches_bytes(b"\x81\x09\x04\x01\xFF"))]
    #[test_case(Request::new(
        0, 1, RequestKind::Command, RequestCategory::PanTilt, 0x01, vec![]
    ) => using matches_bytes(b"\x81\x01\x06\x01\xFF"))]
    #[test_case(Request::new(
        0, 1, RequestKind::Command, RequestCategory::Camera, 0x09, vec![]
    ) => using matches_bytes(b"\x81\x01\x04\x09\xFF"))]
    #[test_case(Request::new(
        0, 1, RequestKind::Command, RequestCategory::Camera, 0x01, vec![0x20, 0x42]
    ) => using matches_bytes(b"\x81\x01\x04\x01\x20\x42\xFF"))]
    fn test_request_to_bytes(data: Request) -> Result<Vec<u8>> {
        let result = data.to_bytes()?;
        Ok(result)
    }

    #[derive(Clone, Copy)]
    enum TestCommand {
        Up(u8),
        Stop,
    }

    impl ViscaCommand for TestCommand {
        const ID: u8 = 0x99;
        const CATEGORY: RequestCategory = RequestCategory::PanTilt;
    }

    fn validate_up(up: u8) -> Result<u8> {
        if up < 0x23 {
            Ok(up)
        } else {
            Err(ViscaError::InvalidSpeed)
        }
    }

    impl ViscaAction for TestCommand {
        fn visca_action_data(&self) -> Result<Vec<u8>> {
            Ok(vec![
                match *self {
                    Self::Up(speed) => validate_up(speed)?,
                    _ => 0x00,
                },
                match *self {
                    Self::Stop => 0x42,
                    _ => 0x00,
                },
            ])
        }
    }

    #[test_case(TestCommand::Up(0x22).action(1) => using matches_bytes(b"\x81\x01\x06\x99\x22\x00\xFF"))]
    #[test_case(TestCommand::Up(0x22).action(3).sender(3) => using matches_bytes(b"\xB3\x01\x06\x99\x22\x00\xFF"))]
    #[test_case(TestCommand::Up(0x23).action(3).sender(3) => matches Err(ViscaError::InvalidSpeed))]
    #[test_case(TestCommand::Stop.action(1) => using matches_bytes(b"\x81\x01\x06\x99\x00\x42\xFF"))]
    fn test_action_request_builder(data: ActionRequestBuilder<TestCommand>) -> Result<Vec<u8>> {
        let result = data.build()?.to_bytes()?;
        Ok(result)
    }
}
