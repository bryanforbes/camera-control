use std::fmt;

use crate::error::Result;

pub enum Direction {
    Down,
    Up,
    Left,
    Right,
}

pub trait Camera: Send {
    fn new(path: &str) -> Result<Self>
    where
        Self: Sized;
    fn name(&self) -> Option<String>;
    fn power_on(&mut self) -> Result<()>;
    fn power_off(&mut self) -> Result<()>;
    fn autofocus(&mut self, state: bool) -> Result<()>;
    fn zoom_in(&mut self) -> Result<()>;
    fn zoom_out(&mut self) -> Result<()>;
    fn pan_tilt(&mut self, direction: Direction) -> Result<()>;
    fn stop(&mut self) -> Result<()>;
    fn set_preset(&mut self, preset: u8) -> Result<()>;
    fn go_to_preset(&mut self, preset: u8) -> Result<()>;
}

impl fmt::Debug for dyn Camera {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Camera ( ")?;

        if let Some(n) = self.name().as_ref() {
            write!(f, "name: {} ", n)?;
        };

        write!(f, ")")
    }
}
