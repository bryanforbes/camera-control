use super::{
    Result, ViscaError,
    packet::{RequestCategory, Response, ViscaAction, ViscaCommand, ViscaInquiry},
};

fn validate_speed(speed: u8, max: u8) -> Result<u8> {
    if speed > 0 && speed <= max {
        Ok(speed)
    } else {
        Err(ViscaError::InvalidSpeed)
    }
}

fn validate_preset(preset: u8) -> Result<u8> {
    if preset <= 0x0F {
        Ok(preset)
    } else {
        Err(ViscaError::InvalidPreset)
    }
}

#[derive(Clone, Copy, Debug)]
pub enum Power {
    On = 0x02,
    Off = 0x03,
}

impl ViscaCommand for Power {
    const ID: u8 = 0x00;
    const CATEGORY: RequestCategory = RequestCategory::Camera;
}

impl ViscaAction for Power {
    fn visca_action_data(&self) -> Result<Vec<u8>> {
        Ok(vec![*self as u8])
    }
}

impl ViscaInquiry for Power {
    fn from_response(response: &Response) -> Result<Self> {
        match response.data()[0] & 0x0F {
            0x02 => Ok(Self::On),
            0x03 => Ok(Self::Off),
            _ => Err(ViscaError::InvalidPowerValue),
        }
    }
}

impl From<bool> for Power {
    fn from(value: bool) -> Self {
        if value { Self::On } else { Self::Off }
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

impl ViscaCommand for Zoom {
    const ID: u8 = 0x07;
    const CATEGORY: RequestCategory = RequestCategory::Camera;
}

impl ViscaAction for Zoom {
    fn visca_action_data(&self) -> Result<Vec<u8>> {
        Ok(vec![*self as u8])
    }
}

impl TryFrom<&str> for Zoom {
    type Error = ViscaError;

    fn try_from(value: &str) -> std::result::Result<Self, Self::Error> {
        match value {
            "in" => Ok(Self::Tele),
            "out" => Ok(Self::Wide),
            "stop" => Ok(Self::Stop),
            _ => Err(Self::Error::InvalidZoomValue),
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

impl ViscaCommand for Autofocus {
    const ID: u8 = 0x38;
    const CATEGORY: RequestCategory = RequestCategory::Camera;
}

impl ViscaAction for Autofocus {
    fn visca_action_data(&self) -> Result<Vec<u8>> {
        Ok(vec![*self as u8])
    }
}

impl ViscaInquiry for Autofocus {
    fn from_response(response: &Response) -> Result<Self> {
        match response.data()[0] & 0x0F {
            0x02 => Ok(Self::Auto),
            0x03 => Ok(Self::Manual),
            _ => Err(ViscaError::InvalidAutofocusValue),
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

impl ViscaCommand for Focus {
    const ID: u8 = 0x08;
    const CATEGORY: RequestCategory = RequestCategory::Camera;
}

impl ViscaAction for Focus {
    fn visca_action_data(&self) -> Result<Vec<u8>> {
        Ok(vec![*self as u8])
    }
}

impl TryFrom<&str> for Focus {
    type Error = ViscaError;

    fn try_from(value: &str) -> Result<Self> {
        match value {
            "far" => Ok(Self::Far),
            "near" => Ok(Self::Near),
            "stop" => Ok(Self::Stop),
            _ => Err(Self::Error::InvalidFocusValue),
        }
    }
}

#[derive(Clone, Copy)]
pub enum Preset {
    Set(u8),
    Recall(u8),
}

impl ViscaCommand for Preset {
    const ID: u8 = 0x3F;
    const CATEGORY: RequestCategory = RequestCategory::Camera;
}

impl ViscaAction for Preset {
    fn visca_action_data(&self) -> Result<Vec<u8>> {
        Ok(vec![
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

impl ViscaCommand for Move {
    const ID: u8 = 0x01;
    const CATEGORY: RequestCategory = RequestCategory::PanTilt;
}

impl ViscaAction for Move {
    fn visca_action_data(&self) -> Result<Vec<u8>> {
        Ok(vec![
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
    use super::*;

    use crate::visca::InquiryRequestBuilder;
    use deku::{DekuContainerRead, DekuContainerWrite};
    use test_case::test_case;

    fn matches_bytes(expected: &'static [u8]) -> impl Fn(Result<Vec<u8>>) {
        move |actual| match actual {
            Ok(value) => assert_eq!(value, expected),
            Err(_) => panic!("Error returned"),
        }
    }

    #[test_case(Power::On => using matches_bytes(b"\x81\x01\x04\x00\x02\xFF"); "on")]
    #[test_case(Power::Off => using matches_bytes(b"\x81\x01\x04\x00\x03\xFF"); "off")]
    fn test_power_to_bytes(command: Power) -> Result<Vec<u8>> {
        let bytes = command.action(1).build()?.to_bytes()?;
        Ok(bytes)
    }

    #[test]
    fn test_power_inquiry_to_bytes() -> Result<()> {
        assert_eq!(
            InquiryRequestBuilder::new(1).build::<Power>()?.to_bytes(),
            Ok(b"\x81\x09\x04\x00\xFF".into())
        );
        Ok(())
    }

    #[test_case(b"\x90\x50\x02\xFF" => matches Ok(Power::On); "on")]
    #[test_case(b"\x90\x50\x03\xFF" => matches Ok(Power::Off); "off")]
    #[test_case(b"\x90\x50\x00\xFF" => matches Err(ViscaError::InvalidPowerValue); "invalid power value")]
    fn test_power_inquiry_from_response_payload(payload: &'static [u8]) -> Result<Power> {
        let (_, response) = Response::from_bytes((payload, 0))?;
        Power::from_response(&response)
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

    #[test_case(Zoom::Stop => using matches_bytes(b"\x81\x01\x04\x07\x00\xFF"); "stop")]
    #[test_case(Zoom::Tele => using matches_bytes(b"\x81\x01\x04\x07\x02\xFF"); "tele")]
    #[test_case(Zoom::Wide => using matches_bytes(b"\x81\x01\x04\x07\x03\xFF"); "wide")]
    fn test_zoom_to_bytes(command: Zoom) -> Result<Vec<u8>> {
        let bytes = command.action(1).build()?.to_bytes()?;
        Ok(bytes)
    }

    #[test_case("in" => matches Ok(Zoom::Tele); "in => Tele")]
    #[test_case("out" => matches Ok(Zoom::Wide); "out => Wide")]
    #[test_case("stop" => matches Ok(Zoom::Stop); "stop => Stop")]
    #[test_case("anything else" => matches Err(ViscaError::InvalidZoomValue); "invalid zoom value")]
    fn test_zoom_try_from(value: &str) -> Result<Zoom> {
        Zoom::try_from(value)
    }

    #[test_case(Autofocus::Auto => using matches_bytes(b"\x81\x01\x04\x38\x02\xFF"); "auto")]
    #[test_case(Autofocus::Manual => using matches_bytes(b"\x81\x01\x04\x38\x03\xFF"); "manual")]
    fn test_autofocus_to_bytes(autofocus: Autofocus) -> Result<Vec<u8>> {
        let bytes = autofocus.action(1).build()?.to_bytes()?;
        Ok(bytes)
    }

    #[test]
    fn test_autofocus_inquiry_to_bytes() -> Result<()> {
        assert_eq!(
            InquiryRequestBuilder::new(1)
                .build::<Autofocus>()?
                .to_bytes(),
            Ok(b"\x81\x09\x04\x38\xFF".into())
        );
        Ok(())
    }

    #[test_case(b"\x90\x50\x02\xFF" => matches Ok(Autofocus::Auto); "auto")]
    #[test_case(b"\x90\x50\x03\xFF" => matches Ok(Autofocus::Manual); "manual")]
    #[test_case(
        b"\x90\x50\x00\xFF" => matches Err(ViscaError::InvalidAutofocusValue);
        "invalid autofocus value"
    )]
    fn test_autofocus_from_response_payload(payload: &'static [u8]) -> Result<Autofocus> {
        let (_, response) = Response::from_bytes((payload, 0))?;
        Autofocus::from_response(&response)
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

    #[test_case(Focus::Stop => using matches_bytes(b"\x81\x01\x04\x08\x00\xFF"); "stop")]
    #[test_case(Focus::Far => using matches_bytes(b"\x81\x01\x04\x08\x02\xFF"); "far")]
    #[test_case(Focus::Near => using matches_bytes(b"\x81\x01\x04\x08\x03\xFF"); "near")]
    fn test_focus_to_bytes(focus: Focus) -> Result<Vec<u8>> {
        let bytes = focus.action(1).build()?.to_bytes()?;
        Ok(bytes)
    }

    #[test_case("far" => matches Ok(Focus::Far); "far")]
    #[test_case("near" => matches Ok(Focus::Near); "near")]
    #[test_case("stop" => matches Ok(Focus::Stop); "stop")]
    #[test_case("anything else" => matches Err(ViscaError::InvalidFocusValue); "invalid focus value")]
    fn test_focus_try_from(value: &str) -> Result<Focus> {
        Focus::try_from(value)
    }

    #[test_case(Preset::Set(3) => using matches_bytes(b"\x81\x01\x04\x3F\x01\x03\xFF"); "set 3")]
    #[test_case(Preset::Set(4) => using matches_bytes(b"\x81\x01\x04\x3F\x01\x04\xFF"); "set 4")]
    #[test_case(Preset::Set(0x10) => matches Err(ViscaError::InvalidPreset); "set invalid preset")]
    #[test_case(Preset::Recall(3) => using matches_bytes(b"\x81\x01\x04\x3F\x02\x03\xFF"); "recall 3")]
    #[test_case(Preset::Recall(4) => using matches_bytes(b"\x81\x01\x04\x3F\x02\x04\xFF"); "recall 4")]
    #[test_case(Preset::Recall(0x10) => matches Err(ViscaError::InvalidPreset); "recall invalid preset")]
    fn test_preset_to_bytes(preset: Preset) -> Result<Vec<u8>> {
        let bytes = preset.action(1).build()?.to_bytes()?;
        Ok(bytes)
    }

    #[test_case(Move::Up(0x01) => using matches_bytes(b"\x81\x01\x06\x01\x00\x01\x03\x01\xFF"); "up 1")]
    #[test_case(Move::Up(0x14) => using matches_bytes(b"\x81\x01\x06\x01\x00\x14\x03\x01\xFF"); "up 20")]
    #[test_case(Move::Up(0x00) => matches Err(ViscaError::InvalidSpeed); "up invalid speed low")]
    #[test_case(Move::Up(0x15) => matches Err(ViscaError::InvalidSpeed); "up invalid speed high")]
    #[test_case(Move::Down(0x01) => using matches_bytes(b"\x81\x01\x06\x01\x00\x01\x03\x02\xFF"); "down 1")]
    #[test_case(Move::Down(0x14) => using matches_bytes(b"\x81\x01\x06\x01\x00\x14\x03\x02\xFF"); "down 20")]
    #[test_case(Move::Down(0x00) => matches Err(ViscaError::InvalidSpeed); "down invalid speed low")]
    #[test_case(Move::Down(0x15) => matches Err(ViscaError::InvalidSpeed); "down invalid speed high")]
    #[test_case(Move::Left(0x01) => using matches_bytes(b"\x81\x01\x06\x01\x01\x00\x01\x03\xFF"); "left 1")]
    #[test_case(Move::Left(0x18) => using matches_bytes(b"\x81\x01\x06\x01\x18\x00\x01\x03\xFF"); "left 24")]
    #[test_case(Move::Left(0x00) => matches Err(ViscaError::InvalidSpeed); "left invalid speed low")]
    #[test_case(Move::Left(0x19) => matches Err(ViscaError::InvalidSpeed); "left invalid speed high")]
    #[test_case(Move::Right(0x01) => using matches_bytes(b"\x81\x01\x06\x01\x01\x00\x02\x03\xFF"); "right 1")]
    #[test_case(Move::Right(0x18) => using matches_bytes(b"\x81\x01\x06\x01\x18\x00\x02\x03\xFF"); "right 24")]
    #[test_case(Move::Right(0x00) => matches Err(ViscaError::InvalidSpeed); "right invalid speed low")]
    #[test_case(Move::Right(0x19) => matches Err(ViscaError::InvalidSpeed); "right invalid speed high")]
    #[test_case(Move::Stop => using matches_bytes(b"\x81\x01\x06\x01\x00\x00\x03\x03\xFF"); "stop")]
    fn test_move_to_bytes(command: Move) -> Result<Vec<u8>> {
        let bytes = command.action(1).build()?.to_bytes()?;
        Ok(bytes)
    }
}
