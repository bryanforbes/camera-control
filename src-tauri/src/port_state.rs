use pelcodrs::{Message, PelcoDPort};
use serialport::{DataBits, SerialPort, StopBits};
use std::sync::Mutex;

use crate::error::{Error, Result};

pub struct PortState {
    port: Mutex<Option<PelcoDPort<Box<dyn SerialPort>>>>,
}

impl PortState {
    pub fn new() -> Self {
        Self {
            port: Default::default(),
        }
    }
    pub fn send_message(&self, message: Message) -> Result<()> {
        match self.port.lock().unwrap().as_mut() {
            Some(port) => Ok(port.send_message(message)?),
            None => Err(Error::NoPortSet),
        }
    }

    pub fn set_port(&self, path: Option<&str>) -> Result<()> {
        println!("{:?}", path);

        // Drop the previous port implicitly before setting a new one
        *self.port.lock().unwrap() = None;

        if let Some(path) = path {
            *self.port.lock().unwrap() = Some(PelcoDPort::new(
                serialport::new(path, 9000)
                    .stop_bits(StopBits::One)
                    .data_bits(DataBits::Eight)
                    .open()?,
            ));
        }

        Ok(())
    }
}
