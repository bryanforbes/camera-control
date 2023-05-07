use super::{Error, Response, Result};

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

pub trait Action<const P: usize>: Command {
    fn data(&self) -> Result<[u8; P]>;
}

pub trait Inquiry: Command + Sized {
    fn transform_inquiry_response(response: &Response) -> Result<Self>;
}

#[derive(Clone, Copy, Debug)]
pub enum Power {
    On = 0x02,
    Off = 0x03,
}

impl Command for Power {
    const COMMAND_ID: u8 = 0x00;
}

impl Action<1> for Power {
    fn data(&self) -> Result<[u8; 1]> {
        Ok([*self as u8])
    }
}

impl Inquiry for Power {
    fn transform_inquiry_response(response: &Response) -> Result<Self> {
        response.payload()[0].try_into()
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

impl TryFrom<u8> for Power {
    type Error = Error;

    fn try_from(value: u8) -> std::result::Result<Self, Self::Error> {
        match value {
            0x02 => Ok(Self::On),
            0x03 => Ok(Self::Off),
            _ => Err(Error::InvalidPowerValue),
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

impl Action<1> for Zoom {
    fn data(&self) -> Result<[u8; 1]> {
        Ok([(*self).into()])
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

impl Action<1> for Autofocus {
    fn data(&self) -> Result<[u8; 1]> {
        Ok([*self as u8])
    }
}

impl Inquiry for Autofocus {
    fn transform_inquiry_response(response: &Response) -> Result<Self> {
        response.payload()[0].try_into()
    }
}

impl TryFrom<u8> for Autofocus {
    type Error = Error;

    fn try_from(value: u8) -> std::result::Result<Self, Self::Error> {
        match value {
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

impl Action<1> for Focus {
    fn data(&self) -> Result<[u8; 1]> {
        Ok([*self as u8])
    }
}

impl TryFrom<&str> for Focus {
    type Error = Error;

    fn try_from(value: &str) -> std::result::Result<Self, Self::Error> {
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

impl Action<2> for Preset {
    fn data(&self) -> Result<[u8; 2]> {
        Ok([
            match *self {
                Self::Set(_) => 0x01,
                Self::Recall(_) => 0x02,
            },
            match *self {
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
}

impl Action<4> for Move {
    fn data(&self) -> Result<[u8; 4]> {
        Ok([
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
        ])
    }
}

#[cfg(test)]
mod tests {
    use crate::visca::Packet;

    use super::*;

    use test_case::test_case;

    #[test_case(Power::On => matches [0x04, 0x00]; "power")]
    #[test_case(Zoom::Tele => matches [0x04, 0x07]; "zoom")]
    #[test_case(Autofocus::Auto => matches [0x04, 0x38]; "autofocus")]
    #[test_case(Preset::Set(1) => matches [0x04, 0x3F]; "preset")]
    #[test_case(Move::Up(1) => matches [0x06, 0x01]; " move")]
    fn test_category_and_id<P: Command>(_: P) -> [u8; 2] {
        [P::COMMAND_CATEGORY, P::COMMAND_ID]
    }

    #[test_case(Power::On => matches Ok([0x02]); "on")]
    #[test_case(Power::Off => matches Ok([0x03]); "off")]
    fn test_power_data(command: Power) -> Result<[u8; 1]> {
        command.data()
    }

    #[test_case(vec![0x90, 0x50, 0x02, 0xFF] => matches Ok(Power::On); "on")]
    #[test_case(vec![0x90, 0x50, 0x03, 0xFF] => matches Ok(Power::Off); "off")]
    #[test_case(vec![0x90, 0x50, 0x00, 0xFF] => matches Err(Error::InvalidPowerValue); "invalid power value")]
    fn test_power_inquiry_transform_response(response: Packet) -> Result<Power> {
        Power::transform_inquiry_response(&Response::try_from(response)?)
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

    #[test_case(Zoom::Stop => matches Ok([0x00]); "stop")]
    #[test_case(Zoom::Tele => matches Ok([0x02]); "tele")]
    #[test_case(Zoom::Wide => matches Ok([0x03]); "wide")]
    fn test_zoom_data(command: Zoom) -> Result<[u8; 1]> {
        command.data()
    }

    #[test_case("in" => matches Ok(Zoom::Tele); "in => Tele")]
    #[test_case("out" => matches Ok(Zoom::Wide); "out => Wide")]
    #[test_case("stop" => matches Ok(Zoom::Stop); "stop => Stop")]
    #[test_case("anything else" => matches Err(Error::InvalidZoomValue); "invalid zoom value")]
    fn test_zoom_try_from(value: &str) -> Result<Zoom> {
        Zoom::try_from(value)
    }

    #[test_case(Autofocus::Auto => matches Ok([0x02]); "auto")]
    #[test_case(Autofocus::Manual => matches Ok([0x03]); "manual")]
    fn test_autofocus_data(autofocus: Autofocus) -> Result<[u8; 1]> {
        autofocus.data()
    }

    #[test_case(vec![0x90, 0x50, 0x02, 0xFF] => matches Ok(Autofocus::Auto); "auto")]
    #[test_case(vec![0x90, 0x50, 0x03, 0xFF] => matches Ok(Autofocus::Manual); "manual")]
    #[test_case(
        vec![0x90, 0x50, 0x00, 0xFF] => matches Err(Error::InvalidAutofocusValue);
        "invalid autofocus value"
    )]
    fn test_autofocus_inquiry_transform_response(response: Packet) -> Result<Autofocus> {
        Autofocus::transform_inquiry_response(&Response::try_from(response)?)
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

    #[test_case(Focus::Stop => matches Ok([0x00]); "stop")]
    #[test_case(Focus::Far => matches Ok([0x02]); "far")]
    #[test_case(Focus::Near => matches Ok([0x03]); "near")]
    fn test_focus_data(focus: Focus) -> Result<[u8; 1]> {
        focus.data()
    }

    #[test_case("far" => matches Ok(Focus::Far); "far")]
    #[test_case("near" => matches Ok(Focus::Near); "near")]
    #[test_case("stop" => matches Ok(Focus::Stop); "stop")]
    #[test_case("anything else" => matches Err(Error::InvalidFocusValue); "invalid focus value")]
    fn test_focus_try_from(value: &str) -> Result<Focus> {
        Focus::try_from(value)
    }

    #[test_case(Preset::Set(3) => matches Ok([0x01, 0x03]); "set 3")]
    #[test_case(Preset::Set(4) => matches Ok([0x01, 0x04]); "set 4")]
    #[test_case(Preset::Set(0x10) => matches Err(Error::InvalidPreset); "set invalid preset")]
    #[test_case(Preset::Recall(3) => matches Ok([0x02, 0x03]); "recall 3")]
    #[test_case(Preset::Recall(4) => matches Ok([0x02, 0x04]); "recall 4")]
    #[test_case(Preset::Recall(0x10) => matches Err(Error::InvalidPreset); "recall invalid preset")]
    fn test_preset_data(preset: Preset) -> Result<[u8; 2]> {
        preset.data()
    }

    #[test_case(Move::Up(0x01) => matches Ok([0x00, 0x01, 0x03, 0x01]); "up 1")]
    #[test_case(Move::Up(0x14) => matches Ok([0x00, 0x14, 0x03, 0x01]); "up 20")]
    #[test_case(Move::Up(0x00) => matches Err(Error::InvalidSpeed); "up invalid speed low")]
    #[test_case(Move::Up(0x15) => matches Err(Error::InvalidSpeed); "up invalid speed high")]
    #[test_case(Move::Down(0x01) => matches Ok([0x00, 0x01, 0x03, 0x02]); "down 1")]
    #[test_case(Move::Down(0x14) => matches Ok([0x00, 0x14, 0x03, 0x02]); "down 20")]
    #[test_case(Move::Down(0x00) => matches Err(Error::InvalidSpeed); "down invalid speed low")]
    #[test_case(Move::Down(0x15) => matches Err(Error::InvalidSpeed); "down invalid speed high")]
    #[test_case(Move::Left(0x01) => matches Ok([0x01, 0x00, 0x01, 0x03]); "left 1")]
    #[test_case(Move::Left(0x18) => matches Ok([0x18, 0x00, 0x01, 0x03]); "left 24")]
    #[test_case(Move::Left(0x00) => matches Err(Error::InvalidSpeed); "left invalid speed low")]
    #[test_case(Move::Left(0x19) => matches Err(Error::InvalidSpeed); "left invalid speed high")]
    #[test_case(Move::Right(0x01) => matches Ok([0x01, 0x00, 0x02, 0x03]); "right 1")]
    #[test_case(Move::Right(0x18) => matches Ok([0x18, 0x00, 0x02, 0x03]); "right 24")]
    #[test_case(Move::Right(0x00) => matches Err(Error::InvalidSpeed); "right invalid speed low")]
    #[test_case(Move::Right(0x19) => matches Err(Error::InvalidSpeed); "right invalid speed high")]
    #[test_case(Move::Stop => matches Ok([0x00, 0x00, 0x03, 0x03]); "stop")]
    fn test_move_data(command: Move) -> Result<[u8; 4]> {
        command.data()
    }
}
