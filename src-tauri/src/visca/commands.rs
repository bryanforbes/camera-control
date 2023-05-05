use super::{Error, Response, Result};

fn header_for_address(address: u8) -> Result<u8> {
    if address <= 7 {
        Ok(0x80 | address)
    } else {
        Err(Error::InvalidAddress)
    }
}

fn validate_speed(speed: u8, max: u8) -> Result<u8> {
    if speed > 0 && speed <= max {
        Ok(speed)
    } else {
        Err(Error::InvalidSpeed)
    }
}

fn validate_preset(preset: u8) -> Result<u8> {
    if preset <= 0x0F {
        Ok(preset)
    } else {
        Err(Error::InvalidPreset)
    }
}

pub trait Command: Sized {
    const COMMAND_CATEGORY: u8;
    const COMMAND_ID: u8;

    fn command_payload(self) -> Result<Vec<u8>>;

    fn to_command_bytes(self, address: u8) -> Result<Vec<u8>> {
        let mut packet = vec![
            header_for_address(address)?,
            0x01,
            Self::COMMAND_CATEGORY,
            Self::COMMAND_ID,
        ];

        packet.extend(self.command_payload()?.iter());
        packet.push(0xFF);

        Ok(packet)
    }
}

pub trait Inquiry: Command {
    fn to_inquiry_bytes(address: u8) -> Result<[u8; 5]> {
        Ok([
            header_for_address(address)?,
            0x09,
            Self::COMMAND_CATEGORY,
            Self::COMMAND_ID,
            0xFF,
        ])
    }
    fn transform_inquiry_response(response: &Response) -> Result<Self>;
}

#[derive(Clone, Copy, Debug)]
pub enum Power {
    On = 0x02,
    Off = 0x03,
}

impl Command for Power {
    const COMMAND_CATEGORY: u8 = 0x04;
    const COMMAND_ID: u8 = 0x00;

    fn command_payload(self) -> Result<Vec<u8>> {
        Ok(vec![self as u8])
    }
}

impl Inquiry for Power {
    fn transform_inquiry_response(response: &Response) -> Result<Self> {
        let value = response.payload()[0];
        match value {
            0x02 => Ok(Self::On),
            0x03 => Ok(Self::Off),
            _ => Err(Error::InvalidResponse),
        }
    }
}

impl From<bool> for Power {
    fn from(value: bool) -> Self {
        if value {
            Power::On
        } else {
            Power::Off
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub enum Zoom {
    Tele = 0x02,
    Wide = 0x03,
    Stop = 0x00,
}

impl Command for Zoom {
    const COMMAND_CATEGORY: u8 = 0x04;
    const COMMAND_ID: u8 = 0x07;

    fn command_payload(self) -> Result<Vec<u8>> {
        Ok(vec![self as u8])
    }
}

impl From<&str> for Zoom {
    fn from(value: &str) -> Self {
        match value {
            "in" => Self::Tele,
            "out" => Self::Wide,
            _ => Self::Stop,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub enum Autofocus {
    Auto = 0x02,
    Manual = 0x03,
}

impl Command for Autofocus {
    const COMMAND_CATEGORY: u8 = 0x04;
    const COMMAND_ID: u8 = 0x38;

    fn command_payload(self) -> Result<Vec<u8>> {
        Ok(vec![self as u8])
    }
}

impl Inquiry for Autofocus {
    fn transform_inquiry_response(response: &Response) -> Result<Self> {
        match response.payload()[0] {
            0x02 => Ok(Self::Auto),
            0x03 => Ok(Self::Manual),
            _ => Err(Error::InvalidResponse),
        }
    }
}

impl From<bool> for Autofocus {
    fn from(value: bool) -> Self {
        if value {
            Autofocus::Auto
        } else {
            Autofocus::Manual
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub enum Focus {
    Stop = 0x00,
    Far = 0x02,
    Near = 0x03,
}

impl Command for Focus {
    const COMMAND_CATEGORY: u8 = 0x04;
    const COMMAND_ID: u8 = 0x08;

    fn command_payload(self) -> Result<Vec<u8>> {
        Ok(vec![self as u8])
    }
}

impl From<&str> for Focus {
    fn from(value: &str) -> Self {
        match value {
            "far" => Self::Far,
            "near" => Self::Near,
            _ => Self::Stop,
        }
    }
}

#[derive(Clone, Copy)]
pub enum Preset {
    Set(u8),
    Recall(u8),
}

impl Command for Preset {
    const COMMAND_CATEGORY: u8 = 0x04;
    const COMMAND_ID: u8 = 0x3F;

    fn command_payload(self) -> Result<Vec<u8>> {
        Ok(vec![
            match self {
                Self::Set(_) => 0x01,
                Self::Recall(_) => 0x02,
            },
            match self {
                Self::Set(preset) | Self::Recall(preset) => validate_preset(preset)?,
            },
        ])
    }
}

pub enum Move {
    Up(u8),
    Down(u8),
    Left(u8),
    Right(u8),
    Stop,
}

impl Command for Move {
    const COMMAND_CATEGORY: u8 = 0x06;
    const COMMAND_ID: u8 = 0x01;

    fn command_payload(self) -> Result<Vec<u8>> {
        Ok(vec![
            match self {
                Self::Left(speed) | Self::Right(speed) => validate_speed(speed, 0x18)?,
                _ => 0x00,
            },
            match self {
                Self::Up(speed) | Self::Down(speed) => validate_speed(speed, 0x14)?,
                _ => 0x00,
            },
            match self {
                Self::Up(_) | Self::Down(_) | Self::Stop => 0x03,
                Self::Left(_) => 0x01,
                Self::Right(_) => 0x02,
            },
            match self {
                Self::Up(_) => 0x01,
                Self::Down(_) => 0x02,
                Self::Left(_) | Self::Right(_) | Self::Stop => 0x03,
            },
        ])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_power_command() -> Result<()> {
        assert_eq!(
            Power::On.to_command_bytes(1)?,
            vec![0x81, 0x01, 0x04, 0x00, 0x02, 0xFF]
        );
        assert_eq!(
            Power::Off.to_command_bytes(2)?,
            vec![0x82, 0x01, 0x04, 0x00, 0x03, 0xFF]
        );
        Ok(())
    }

    #[test]
    fn test_power_command_invalid_address() {
        assert_matches!(
            Power::On.to_command_bytes(8).unwrap_err(),
            Error::InvalidAddress
        );
    }

    #[test]
    fn test_power_inquiry() -> Result<()> {
        assert_eq!(Power::to_inquiry_bytes(1)?, [0x81, 0x09, 0x04, 0x00, 0xFF]);
        Ok(())
    }

    #[test]
    fn test_power_inquiry_transform_response() -> Result<()> {
        assert_matches!(
            Power::transform_inquiry_response(&Response::new(vec![0x90, 0x50, 0x02, 0xFF])?)?,
            Power::On
        );
        assert_matches!(
            Power::transform_inquiry_response(&Response::new(vec![0x90, 0x50, 0x03, 0xFF])?)?,
            Power::Off
        );
        Ok(())
    }

    #[test]
    fn test_power_inquiry_transform_response_invalid_response() -> Result<()> {
        assert_matches!(
            Power::transform_inquiry_response(&Response::new(vec![0x90, 0x50, 0x00, 0xFF])?)
                .unwrap_err(),
            Error::InvalidResponse
        );
        Ok(())
    }

    #[test]
    fn test_power_from() {
        assert_matches!(Power::from(true), Power::On);
        assert_matches!(Power::from(false), Power::Off);
    }

    #[test]
    fn test_zoom_command() -> Result<()> {
        assert_eq!(
            Zoom::Stop.to_command_bytes(1)?,
            vec![0x81, 0x01, 0x04, 0x07, 0x00, 0xFF]
        );
        assert_eq!(
            Zoom::Tele.to_command_bytes(2)?,
            vec![0x82, 0x01, 0x04, 0x07, 0x02, 0xFF]
        );
        assert_eq!(
            Zoom::Wide.to_command_bytes(3)?,
            vec![0x83, 0x01, 0x04, 0x07, 0x03, 0xFF]
        );
        Ok(())
    }

    #[test]
    fn test_zoom_command_invalid_address() {
        assert_matches!(
            Zoom::Tele.to_command_bytes(8).unwrap_err(),
            Error::InvalidAddress
        );
    }

    #[test]
    fn test_zoom_from() {
        assert_matches!(Zoom::from("in"), Zoom::Tele);
        assert_matches!(Zoom::from("out"), Zoom::Wide);
        assert_matches!(Zoom::from("anything else"), Zoom::Stop);
    }

    #[test]
    fn test_autofocus_command() -> Result<()> {
        assert_eq!(
            Autofocus::Auto.to_command_bytes(1)?,
            vec![0x81, 0x01, 0x04, 0x38, 0x02, 0xFF]
        );
        assert_eq!(
            Autofocus::Manual.to_command_bytes(2)?,
            vec![0x82, 0x01, 0x04, 0x38, 0x03, 0xFF]
        );
        Ok(())
    }

    #[test]
    fn test_autofocus_command_invalid_address() {
        assert_matches!(
            Autofocus::Auto.to_command_bytes(8).unwrap_err(),
            Error::InvalidAddress
        );
    }

    #[test]
    fn test_autofocus_inquiry_transform_response() -> Result<()> {
        assert_matches!(
            Autofocus::transform_inquiry_response(&Response::new(vec![0x90, 0x50, 0x02, 0xFF])?)?,
            Autofocus::Auto
        );
        assert_matches!(
            Autofocus::transform_inquiry_response(&Response::new(vec![0x90, 0x50, 0x03, 0xFF])?)?,
            Autofocus::Manual
        );
        Ok(())
    }

    #[test]
    fn test_autofocus_inquiry_transform_response_invalid_response() -> Result<()> {
        assert_matches!(
            Autofocus::transform_inquiry_response(&Response::new(vec![0x90, 0x50, 0x00, 0xFF])?)
                .unwrap_err(),
            Error::InvalidResponse
        );
        Ok(())
    }

    #[test]
    fn test_autofocus_from() {
        assert_matches!(Autofocus::from(true), Autofocus::Auto);
        assert_matches!(Autofocus::from(false), Autofocus::Manual);
    }

    #[test]
    fn test_focus_command() -> Result<()> {
        assert_eq!(
            Focus::Stop.to_command_bytes(1)?,
            vec![0x81, 0x01, 0x04, 0x08, 0x00, 0xFF]
        );
        assert_eq!(
            Focus::Far.to_command_bytes(2)?,
            vec![0x82, 0x01, 0x04, 0x08, 0x02, 0xFF]
        );
        assert_eq!(
            Focus::Near.to_command_bytes(3)?,
            vec![0x83, 0x01, 0x04, 0x08, 0x03, 0xFF]
        );
        Ok(())
    }

    #[test]
    fn test_focus_command_invalid_address() {
        assert_matches!(
            Focus::Stop.to_command_bytes(8).unwrap_err(),
            Error::InvalidAddress
        );
    }

    #[test]
    fn test_focus_from() {
        assert_matches!(Focus::from("near"), Focus::Near);
        assert_matches!(Focus::from("far"), Focus::Far);
        assert_matches!(Focus::from("anything else"), Focus::Stop);
    }

    #[test]
    fn test_preset_command() -> Result<()> {
        assert_eq!(
            Preset::Set(3).to_command_bytes(1)?,
            vec![0x81, 0x01, 0x04, 0x3F, 0x01, 0x03, 0xFF]
        );
        assert_eq!(
            Preset::Set(4).to_command_bytes(2)?,
            vec![0x82, 0x01, 0x04, 0x3F, 0x01, 0x04, 0xFF]
        );
        assert_eq!(
            Preset::Recall(3).to_command_bytes(3)?,
            vec![0x83, 0x01, 0x04, 0x3F, 0x02, 0x03, 0xFF]
        );
        assert_eq!(
            Preset::Recall(4).to_command_bytes(4)?,
            vec![0x84, 0x01, 0x04, 0x3F, 0x02, 0x04, 0xFF]
        );
        Ok(())
    }

    #[test]
    fn test_preset_command_invalid_address() {
        assert_matches!(
            Preset::Set(1).to_command_bytes(8).unwrap_err(),
            Error::InvalidAddress
        );
    }

    #[test]
    fn test_preset_command_invalid_preset() {
        assert_matches!(
            Preset::Set(0x10).to_command_bytes(1).unwrap_err(),
            Error::InvalidPreset
        );
    }

    #[test]
    fn test_move_command() -> Result<()> {
        assert_eq!(
            Move::Up(0x01).to_command_bytes(1)?,
            vec![0x81, 0x01, 0x06, 0x01, 0x00, 0x01, 0x03, 0x01, 0xFF]
        );
        assert_eq!(
            Move::Down(0x14).to_command_bytes(2)?,
            vec![0x82, 0x01, 0x06, 0x01, 0x00, 0x14, 0x03, 0x02, 0xFF]
        );
        assert_eq!(
            Move::Left(0x01).to_command_bytes(3)?,
            vec![0x83, 0x01, 0x06, 0x01, 0x01, 0x00, 0x01, 0x03, 0xFF]
        );
        assert_eq!(
            Move::Right(0x14).to_command_bytes(4)?,
            vec![0x84, 0x01, 0x06, 0x01, 0x14, 0x00, 0x02, 0x03, 0xFF]
        );
        assert_eq!(
            Move::Stop.to_command_bytes(5)?,
            vec![0x85, 0x01, 0x06, 0x01, 0x00, 0x00, 0x03, 0x03, 0xFF]
        );
        Ok(())
    }

    #[test]
    fn test_move_command_invalid_address() {
        assert_matches!(
            Move::Stop.to_command_bytes(8).unwrap_err(),
            Error::InvalidAddress
        );
    }

    #[test]
    fn test_move_command_invalid_speed() {
        assert_matches!(
            Move::Up(0).to_command_bytes(1).unwrap_err(),
            Error::InvalidSpeed
        );
        assert_matches!(
            Move::Up(0x15).to_command_bytes(1).unwrap_err(),
            Error::InvalidSpeed
        );
        assert_matches!(
            Move::Down(0).to_command_bytes(1).unwrap_err(),
            Error::InvalidSpeed
        );
        assert_matches!(
            Move::Down(0x15).to_command_bytes(1).unwrap_err(),
            Error::InvalidSpeed
        );
        assert_matches!(
            Move::Left(0).to_command_bytes(1).unwrap_err(),
            Error::InvalidSpeed
        );
        assert_matches!(
            Move::Left(0x19).to_command_bytes(1).unwrap_err(),
            Error::InvalidSpeed
        );
        assert_matches!(
            Move::Right(0).to_command_bytes(1).unwrap_err(),
            Error::InvalidSpeed
        );
        assert_matches!(
            Move::Right(0x19).to_command_bytes(1).unwrap_err(),
            Error::InvalidSpeed
        );
    }
}
