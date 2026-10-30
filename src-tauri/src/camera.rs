use serde::{Deserialize, Serialize};
use specta::Type;
use std::fmt;

use crate::error::Result;

#[derive(Serialize, Deserialize, Type)]
#[serde(rename_all = "lowercase")]
pub enum Direction {
    Down,
    Up,
    Left,
    Right,
}

impl std::fmt::Display for Direction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Direction::Down => write!(f, "down"),
            Direction::Up => write!(f, "up"),
            Direction::Left => write!(f, "left"),
            Direction::Right => write!(f, "right"),
        }
    }
}

#[derive(Serialize, Deserialize, Type)]
#[serde(rename_all = "lowercase")]
pub enum Zoom {
    In,
    Out,
}

impl std::fmt::Display for Zoom {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Zoom::In => write!(f, "in"),
            Zoom::Out => write!(f, "out"),
        }
    }
}

pub trait Camera: Send {
    fn new(path: &str) -> Result<Self>
    where
        Self: Sized;
    fn name(&self) -> Option<String>;
    fn power_on(&mut self) -> Result<()>;
    fn power_off(&mut self) -> Result<()>;
    fn autofocus(&mut self, state: bool) -> Result<()>;
    fn zoom(&mut self, zoom: Zoom) -> Result<()>;
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
