//! Byte codec for addressed RS-232 VISCA.
//! Command parameters follow the AV-1082W manual.

use thiserror::Error;

pub(super) const TERMINATOR: u8 = 0xff;
pub(super) const MAX_PACKET_LEN: usize = 16;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum CodecError {
    #[error("VISCA camera address {0} is outside 1..=7")]
    InvalidAddress(u8),
    #[error("VISCA pan speed {0} is outside 0x01..=0x18")]
    InvalidPanSpeed(u8),
    #[error("VISCA tilt speed {0} is outside 0x01..=0x14")]
    InvalidTiltSpeed(u8),
    #[error("VISCA preset {0} is outside 0..=254")]
    InvalidPreset(u8),
    #[error("invalid VISCA packet length")]
    InvalidPacketLength,
    #[error("invalid VISCA packet terminator")]
    InvalidTerminator,
    #[error("invalid VISCA reply header {0:#04x}")]
    InvalidReplyHeader(u8),
    #[error("invalid VISCA reply body")]
    InvalidReplyBody,
}

#[derive(Debug, Clone, Copy)]
#[repr(u8)]
pub enum AxisDirection {
    Negative = 0x01, // Left or up
    Positive = 0x02, // Right or down
    Stop = 0x03,
}

#[derive(Debug, Clone, Copy)]
#[repr(u8)]
pub enum Zoom {
    Stop = 0x00,
    Tele = 0x02,
    Wide = 0x03,
}

#[derive(Debug, Clone, Copy)]
pub enum Command {
    PanTilt {
        pan: AxisDirection,
        tilt: AxisDirection,
        pan_speed: u8,
        tilt_speed: u8,
    },
    Zoom(Zoom),
    SetPreset(u8),
    RecallPreset(u8),
}

impl Command {
    /// Encode a command from controller address zero to a single camera.
    pub fn encode(self, camera_address: u8) -> Result<Vec<u8>, CodecError> {
        if !(1..=7).contains(&camera_address) {
            return Err(CodecError::InvalidAddress(camera_address));
        }

        let header = 0x80 | camera_address;
        match self {
            Self::PanTilt {
                pan,
                tilt,
                pan_speed,
                tilt_speed,
            } => {
                if !(1..=0x18).contains(&pan_speed) {
                    return Err(CodecError::InvalidPanSpeed(pan_speed));
                }
                if !(1..=0x14).contains(&tilt_speed) {
                    return Err(CodecError::InvalidTiltSpeed(tilt_speed));
                }

                Ok(vec![
                    header, 0x01, 0x06, 0x01, pan_speed, tilt_speed, pan as u8, tilt as u8,
                    TERMINATOR,
                ])
            }
            Self::Zoom(zoom) => Ok(vec![header, 0x01, 0x04, 0x07, zoom as u8, TERMINATOR]),
            Self::SetPreset(preset) | Self::RecallPreset(preset) => {
                if preset == TERMINATOR {
                    return Err(CodecError::InvalidPreset(preset));
                }

                let operation = match self {
                    Self::SetPreset(_) => 0x01,
                    _ => 0x02,
                };
                Ok(vec![
                    header, 0x01, 0x04, 0x3f, operation, preset, TERMINATOR,
                ])
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReplyError {
    MessageLength,
    Syntax,
    CommandBufferFull,
    CommandCanceled,
    NoSocket,
    CommandNotExecutable,
    Unknown(u8),
}

impl From<u8> for ReplyError {
    fn from(code: u8) -> Self {
        match code {
            0x01 => Self::MessageLength,
            0x02 => Self::Syntax,
            0x03 => Self::CommandBufferFull,
            0x04 => Self::CommandCanceled,
            0x05 => Self::NoSocket,
            0x41 => Self::CommandNotExecutable,
            _ => Self::Unknown(code),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReplyKind<'a> {
    Ack,
    /// Command completions have an empty payload. Inquiry replies carry data.
    Completion(&'a [u8]),
    Error(ReplyError),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Reply<'a> {
    pub camera_address: u8,
    pub socket: u8,
    pub kind: ReplyKind<'a>,
}

impl<'a> Reply<'a> {
    /// Decode exactly one complete reply addressed to controller zero.
    pub fn decode(packet: &'a [u8]) -> Result<Self, CodecError> {
        if !(3..=MAX_PACKET_LEN).contains(&packet.len()) {
            return Err(CodecError::InvalidPacketLength);
        }

        let (body, terminator) = packet.split_at(packet.len() - 1);
        if terminator[0] != TERMINATOR || body.contains(&TERMINATOR) {
            return Err(CodecError::InvalidTerminator);
        }

        let header = body[0];
        if header < 0x90 || header & 0x0f != 0 {
            return Err(CodecError::InvalidReplyHeader(header));
        }

        let socket = body[1] & 0x0f;
        let payload = &body[2..];
        let kind = match (body[1] & 0xf0, payload) {
            (0x40, []) if socket != 0 => ReplyKind::Ack,
            (0x50, _) => ReplyKind::Completion(payload),
            (0x60, [code]) => ReplyKind::Error((*code).into()),
            _ => return Err(CodecError::InvalidReplyBody),
        };

        Ok(Self {
            camera_address: (header >> 4) - 8,
            socket,
            kind,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use test_case::test_case;

    #[test_case(AxisDirection::Stop, AxisDirection::Negative, [3, 1])]
    #[test_case(AxisDirection::Stop, AxisDirection::Positive, [3, 2])]
    #[test_case(AxisDirection::Negative, AxisDirection::Stop, [1, 3])]
    #[test_case(AxisDirection::Positive, AxisDirection::Stop, [2, 3])]
    #[test_case(AxisDirection::Stop, AxisDirection::Stop, [3, 3])]
    fn encodes_pan_tilt(pan: AxisDirection, tilt: AxisDirection, directions: [u8; 2]) {
        assert_eq!(
            Command::PanTilt {
                pan,
                tilt,
                pan_speed: 0x18,
                tilt_speed: 0x14,
            }
            .encode(1)
            .unwrap(),
            [
                0x81,
                1,
                6,
                1,
                0x18,
                0x14,
                directions[0],
                directions[1],
                0xff
            ]
        );
    }

    #[test_case(Command::Zoom(Zoom::Stop), &[0x87, 1, 4, 7, 0, 0xff])]
    #[test_case(Command::Zoom(Zoom::Tele), &[0x87, 1, 4, 7, 2, 0xff])]
    #[test_case(Command::Zoom(Zoom::Wide), &[0x87, 1, 4, 7, 3, 0xff])]
    #[test_case(Command::SetPreset(0), &[0x87, 1, 4, 0x3f, 1, 0, 0xff])]
    #[test_case(Command::RecallPreset(254), &[0x87, 1, 4, 0x3f, 2, 0xfe, 0xff])]
    fn encodes_camera_commands(command: Command, expected: &[u8]) {
        assert_eq!(command.encode(7).unwrap(), expected);
    }

    #[test]
    fn rejects_invalid_command_parameters() {
        assert_eq!(
            Command::Zoom(Zoom::Stop).encode(0),
            Err(CodecError::InvalidAddress(0))
        );
        assert_eq!(
            Command::Zoom(Zoom::Stop).encode(8),
            Err(CodecError::InvalidAddress(8))
        );
        assert_eq!(
            Command::SetPreset(255).encode(1),
            Err(CodecError::InvalidPreset(255))
        );
        for (pan_speed, tilt_speed, error) in [
            (0, 1, CodecError::InvalidPanSpeed(0)),
            (0x19, 1, CodecError::InvalidPanSpeed(0x19)),
            (1, 0, CodecError::InvalidTiltSpeed(0)),
            (1, 0x15, CodecError::InvalidTiltSpeed(0x15)),
        ] {
            assert_eq!(
                Command::PanTilt {
                    pan: AxisDirection::Negative,
                    tilt: AxisDirection::Stop,
                    pan_speed,
                    tilt_speed,
                }
                .encode(1),
                Err(error)
            );
        }
    }

    #[test_case(&[0x90, 0x41, 0xff], 1, 1, ReplyKind::Ack)]
    #[test_case(&[0xa0, 0x52, 0xff], 2, 2, ReplyKind::Completion(&[]))]
    #[test_case(&[0xf0, 0x50, 0x02, 0xff], 7, 0, ReplyKind::Completion(&[0x02]))]
    #[test_case(&[0x90, 0x60, 0x03, 0xff], 1, 0, ReplyKind::Error(ReplyError::CommandBufferFull))]
    #[test_case(&[0x90, 0x61, 0x04, 0xff], 1, 1, ReplyKind::Error(ReplyError::CommandCanceled))]
    #[test_case(&[0x90, 0x61, 0x42, 0xff], 1, 1, ReplyKind::Error(ReplyError::Unknown(0x42)))]
    fn decodes_replies(packet: &[u8], camera_address: u8, socket: u8, kind: ReplyKind<'_>) {
        assert_eq!(
            Reply::decode(packet),
            Ok(Reply {
                camera_address,
                socket,
                kind,
            })
        );
    }

    #[test_case(&[0x90, 0xff], CodecError::InvalidPacketLength)]
    #[test_case(&[0x90, 0x50, 0], CodecError::InvalidTerminator)]
    #[test_case(&[0x90, 0x41, 0xff, 0x90, 0x51, 0xff], CodecError::InvalidTerminator)]
    #[test_case(&[0x81, 0x41, 0xff], CodecError::InvalidReplyHeader(0x81))]
    #[test_case(&[0x91, 0x41, 0xff], CodecError::InvalidReplyHeader(0x91))]
    #[test_case(&[0x90, 0x41, 1, 0xff], CodecError::InvalidReplyBody)]
    #[test_case(&[0x90, 0x40, 0xff], CodecError::InvalidReplyBody)]
    #[test_case(&[0x90, 0x61, 0xff], CodecError::InvalidReplyBody)]
    #[test_case(&[0x90, 0x61, 0x04, 0, 0xff], CodecError::InvalidReplyBody)]
    #[test_case(&[0x90, 0x71, 0xff], CodecError::InvalidReplyBody)]
    fn rejects_invalid_replies(packet: &[u8], error: CodecError) {
        assert_eq!(Reply::decode(packet), Err(error));
    }

    #[test]
    fn bounds_reply_length() {
        let mut packet = [0; MAX_PACKET_LEN + 1];
        packet[0] = 0x90;
        packet[1] = 0x50;
        packet[MAX_PACKET_LEN] = TERMINATOR;
        assert_eq!(Reply::decode(&packet), Err(CodecError::InvalidPacketLength));

        packet[MAX_PACKET_LEN - 1] = TERMINATOR;
        let reply = Reply::decode(&packet[..MAX_PACKET_LEN]).unwrap();
        assert_eq!(reply.kind, ReplyKind::Completion(&[0; MAX_PACKET_LEN - 3]));
    }
}
