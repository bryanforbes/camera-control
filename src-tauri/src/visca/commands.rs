use bytes::{Bytes, BytesMut};

use super::{Error, Result};

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

pub trait Command {
    const COMMAND_CATEGORY: u8 = 0x04;
    const COMMAND_ID: u8;
}

pub trait Action: Command {
    fn data(&self) -> Result<Bytes>;

    fn to_bytes(&self) -> Result<Bytes> {
        let mut bytes = BytesMut::from([0x01, Self::COMMAND_CATEGORY, Self::COMMAND_ID].as_ref());

        bytes.extend(self.data()?);

        Ok(bytes.freeze())
    }
}

pub trait Inquiry: Command + Sized {
    fn from_response_payload(payload: Bytes) -> Result<Self>;

    fn to_bytes() -> Bytes {
        Bytes::copy_from_slice(&[0x09, Self::COMMAND_CATEGORY, Self::COMMAND_ID])
    }
}

#[derive(Clone, Copy, Debug)]
pub enum Power {
    On = 0x02,
    Off = 0x03,
}

impl Command for Power {
    const COMMAND_ID: u8 = 0x00;
}

impl Action for Power {
    fn data(&self) -> Result<Bytes> {
        Ok(Bytes::copy_from_slice(&[*self as u8]))
    }
}

impl Inquiry for Power {
    fn from_response_payload(payload: Bytes) -> Result<Self> {
        match payload[0] & 0x0F {
            0x02 => Ok(Self::On),
            0x03 => Ok(Self::Off),
            _ => Err(Error::InvalidPowerValue),
        }
    }
}

impl From<bool> for Power {
    fn from(value: bool) -> Self {
        if value {
            Self::On
        } else {
            Self::Off
        }
    }
}

impl From<Power> for bool {
    fn from(value: Power) -> Self {
        match value {
            Power::On => true,
            Power::Off => false,
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
    const COMMAND_ID: u8 = 0x07;
}

impl Action for Zoom {
    fn data(&self) -> Result<Bytes> {
        Ok(Bytes::copy_from_slice(&[(*self).into()]))
    }
}

impl TryFrom<&str> for Zoom {
    type Error = Error;

    fn try_from(value: &str) -> std::result::Result<Self, Self::Error> {
        match value {
            "in" => Ok(Self::Tele),
            "out" => Ok(Self::Wide),
            "stop" => Ok(Self::Stop),
            _ => Err(Error::InvalidZoomValue),
        }
    }
}

impl From<Zoom> for u8 {
    fn from(value: Zoom) -> Self {
        value as u8
    }
}

#[derive(Clone, Copy, Debug)]
pub enum Autofocus {
    Auto = 0x02,
    Manual = 0x03,
}

impl Command for Autofocus {
    const COMMAND_ID: u8 = 0x38;
}

impl Action for Autofocus {
    fn data(&self) -> Result<Bytes> {
        Ok(Bytes::copy_from_slice(&[*self as u8]))
    }
}

impl Inquiry for Autofocus {
    fn from_response_payload(payload: Bytes) -> Result<Self> {
        match payload[0] & 0x0F {
            0x02 => Ok(Self::Auto),
            0x03 => Ok(Self::Manual),
            _ => Err(Error::InvalidAutofocusValue),
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

impl From<Autofocus> for bool {
    fn from(value: Autofocus) -> Self {
        match value {
            Autofocus::Auto => true,
            Autofocus::Manual => false,
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
    const COMMAND_ID: u8 = 0x08;
}

impl Action for Focus {
    fn data(&self) -> Result<Bytes> {
        Ok(Bytes::copy_from_slice(&[*self as u8]))
    }
}

impl TryFrom<&str> for Focus {
    type Error = Error;

    fn try_from(value: &str) -> Result<Self> {
        match value {
            "far" => Ok(Self::Far),
            "near" => Ok(Self::Near),
            "stop" => Ok(Self::Stop),
            _ => Err(Error::InvalidFocusValue),
        }
    }
}

#[derive(Clone, Copy)]
pub enum Preset {
    Set(u8),
    Recall(u8),
}

impl Command for Preset {
    const COMMAND_ID: u8 = 0x3F;
}

impl Action for Preset {
    fn data(&self) -> Result<Bytes> {
        Ok(Bytes::copy_from_slice(&[
            match *self {
                Self::Set(_) => 0x01,
                Self::Recall(_) => 0x02,
            },
            match *self {
                Self::Set(preset) | Self::Recall(preset) => validate_preset(preset)?,
            },
        ]))
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
}

impl Action for Move {
    fn data(&self) -> Result<Bytes> {
        Ok(Bytes::copy_from_slice(&[
            match *self {
                Self::Left(speed) | Self::Right(speed) => validate_speed(speed, 0x18)?,
                _ => 0x00,
            },
            match *self {
                Self::Up(speed) | Self::Down(speed) => validate_speed(speed, 0x14)?,
                _ => 0x00,
            },
            match *self {
                Self::Up(_) | Self::Down(_) | Self::Stop => 0x03,
                Self::Left(_) => 0x01,
                Self::Right(_) => 0x02,
            },
            match *self {
                Self::Up(_) => 0x01,
                Self::Down(_) => 0x02,
                Self::Left(_) | Self::Right(_) | Self::Stop => 0x03,
            },
        ]))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use bytes::Bytes;
    use test_case::test_case;

    fn matches_bytes(expected: &'static [u8]) -> impl Fn(Result<Bytes>) {
        move |actual| match actual {
            Ok(value) => assert_eq!(value, expected),
            Err(_) => panic!("Error returned"),
        }
    }

    #[test_case(Power::On => using matches_bytes(b"\x01\x04\x00\x02"); "on")]
    #[test_case(Power::Off => using matches_bytes(b"\x01\x04\x00\x03"); "off")]
    fn test_power_to_bytes(command: Power) -> Result<Bytes> {
        command.to_bytes()
    }

    #[test]
    fn test_power_inquiry_to_bytes() {
        assert_eq!(
            <Power as Inquiry>::to_bytes(),
            Bytes::from_static(b"\x09\x04\x00")
        );
    }

    #[test_case(b"\x02" => matches Ok(Power::On); "on")]
    #[test_case(b"\x03" => matches Ok(Power::Off); "off")]
    #[test_case(b"\x00" => matches Err(Error::InvalidPowerValue); "invalid power value")]
    fn test_power_inquiry_from_response_payload(payload: &'static [u8]) -> Result<Power> {
        Power::from_response_payload(Bytes::from_static(payload))
    }

    #[test_case(true => matches Power::On; "on")]
    #[test_case(false => matches Power::Off; "off")]
    fn test_power_from_bool(value: bool) -> Power {
        Power::from(value)
    }

    #[test_case(Power::On => true; "on")]
    #[test_case(Power::Off => false; "off")]
    fn test_power_into_bool(power: Power) -> bool {
        power.into()
    }

    #[test_case(Zoom::Stop => using matches_bytes(b"\x01\x04\x07\x00"); "stop")]
    #[test_case(Zoom::Tele => using matches_bytes(b"\x01\x04\x07\x02"); "tele")]
    #[test_case(Zoom::Wide => using matches_bytes(b"\x01\x04\x07\x03"); "wide")]
    fn test_zoom_to_bytes(command: Zoom) -> Result<Bytes> {
        command.to_bytes()
    }

    #[test_case("in" => matches Ok(Zoom::Tele); "in => Tele")]
    #[test_case("out" => matches Ok(Zoom::Wide); "out => Wide")]
    #[test_case("stop" => matches Ok(Zoom::Stop); "stop => Stop")]
    #[test_case("anything else" => matches Err(Error::InvalidZoomValue); "invalid zoom value")]
    fn test_zoom_try_from(value: &str) -> Result<Zoom> {
        Zoom::try_from(value)
    }

    #[test_case(Autofocus::Auto => using matches_bytes(b"\x01\x04\x38\x02"); "auto")]
    #[test_case(Autofocus::Manual => using matches_bytes(b"\x01\x04\x38\x03"); "manual")]
    fn test_autofocus_to_bytes(autofocus: Autofocus) -> Result<Bytes> {
        autofocus.to_bytes()
    }

    #[test]
    fn test_autofocus_inquiry_to_bytes() {
        assert_eq!(
            <Autofocus as Inquiry>::to_bytes(),
            Bytes::from_static(b"\x09\x04\x38")
        );
    }

    #[test_case(b"\x02" => matches Ok(Autofocus::Auto); "auto")]
    #[test_case(b"\x03" => matches Ok(Autofocus::Manual); "manual")]
    #[test_case(
        b"\x00" => matches Err(Error::InvalidAutofocusValue);
        "invalid autofocus value"
    )]
    fn test_autofocus_from_response_payload(payload: &'static [u8]) -> Result<Autofocus> {
        Autofocus::from_response_payload(Bytes::from_static(payload))
    }

    #[test_case(true => matches Autofocus::Auto; "auto")]
    #[test_case(false => matches Autofocus::Manual; "manual")]
    fn test_autofocus_from_bool(value: bool) -> Autofocus {
        Autofocus::from(value)
    }

    #[test_case(Autofocus::Auto => true; "auto")]
    #[test_case(Autofocus::Manual => false; "manual")]
    fn test_autofocus_into_bool(value: Autofocus) -> bool {
        value.into()
    }

    #[test_case(Focus::Stop => using matches_bytes(b"\x01\x04\x08\x00"); "stop")]
    #[test_case(Focus::Far => using matches_bytes(b"\x01\x04\x08\x02"); "far")]
    #[test_case(Focus::Near => using matches_bytes(b"\x01\x04\x08\x03"); "near")]
    fn test_focus_to_bytes(focus: Focus) -> Result<Bytes> {
        focus.to_bytes()
    }

    #[test_case("far" => matches Ok(Focus::Far); "far")]
    #[test_case("near" => matches Ok(Focus::Near); "near")]
    #[test_case("stop" => matches Ok(Focus::Stop); "stop")]
    #[test_case("anything else" => matches Err(Error::InvalidFocusValue); "invalid focus value")]
    fn test_focus_try_from(value: &str) -> Result<Focus> {
        Focus::try_from(value)
    }

    #[test_case(Preset::Set(3) => using matches_bytes(b"\x01\x04\x3F\x01\x03"); "set 3")]
    #[test_case(Preset::Set(4) => using matches_bytes(b"\x01\x04\x3F\x01\x04"); "set 4")]
    #[test_case(Preset::Set(0x10) => matches Err(Error::InvalidPreset); "set invalid preset")]
    #[test_case(Preset::Recall(3) => using matches_bytes(b"\x01\x04\x3F\x02\x03"); "recall 3")]
    #[test_case(Preset::Recall(4) => using matches_bytes(b"\x01\x04\x3F\x02\x04"); "recall 4")]
    #[test_case(Preset::Recall(0x10) => matches Err(Error::InvalidPreset); "recall invalid preset")]
    fn test_preset_to_bytes(preset: Preset) -> Result<Bytes> {
        preset.to_bytes()
    }

    #[test_case(Move::Up(0x01) => using matches_bytes(b"\x01\x06\x01\x00\x01\x03\x01"); "up 1")]
    #[test_case(Move::Up(0x14) => using matches_bytes(b"\x01\x06\x01\x00\x14\x03\x01"); "up 20")]
    #[test_case(Move::Up(0x00) => matches Err(Error::InvalidSpeed); "up invalid speed low")]
    #[test_case(Move::Up(0x15) => matches Err(Error::InvalidSpeed); "up invalid speed high")]
    #[test_case(Move::Down(0x01) => using matches_bytes(b"\x01\x06\x01\x00\x01\x03\x02"); "down 1")]
    #[test_case(Move::Down(0x14) => using matches_bytes(b"\x01\x06\x01\x00\x14\x03\x02"); "down 20")]
    #[test_case(Move::Down(0x00) => matches Err(Error::InvalidSpeed); "down invalid speed low")]
    #[test_case(Move::Down(0x15) => matches Err(Error::InvalidSpeed); "down invalid speed high")]
    #[test_case(Move::Left(0x01) => using matches_bytes(b"\x01\x06\x01\x01\x00\x01\x03"); "left 1")]
    #[test_case(Move::Left(0x18) => using matches_bytes(b"\x01\x06\x01\x18\x00\x01\x03"); "left 24")]
    #[test_case(Move::Left(0x00) => matches Err(Error::InvalidSpeed); "left invalid speed low")]
    #[test_case(Move::Left(0x19) => matches Err(Error::InvalidSpeed); "left invalid speed high")]
    #[test_case(Move::Right(0x01) => using matches_bytes(b"\x01\x06\x01\x01\x00\x02\x03"); "right 1")]
    #[test_case(Move::Right(0x18) => using matches_bytes(b"\x01\x06\x01\x18\x00\x02\x03"); "right 24")]
    #[test_case(Move::Right(0x00) => matches Err(Error::InvalidSpeed); "right invalid speed low")]
    #[test_case(Move::Right(0x19) => matches Err(Error::InvalidSpeed); "right invalid speed high")]
    #[test_case(Move::Stop => using matches_bytes(b"\x01\x06\x01\x00\x00\x03\x03"); "stop")]
    fn test_move_to_bytes(command: Move) -> Result<Bytes> {
        command.to_bytes()
    }
}
