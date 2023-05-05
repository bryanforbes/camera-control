use serialport::{DataBits, FlowControl, Parity, SerialPort, StopBits};
use std::{sync::Mutex, time::Duration};

use crate::{
    error::{Error, Result},
    visca::{Command, Inquiry, ViscaPort},
};

pub struct PortState {
    port: Mutex<Option<ViscaPort<Box<dyn SerialPort>>>>,
}

impl PortState {
    pub fn new() -> Self {
        Self {
            port: Default::default(),
        }
    }

    pub fn execute<C: Command>(&self, address: u8, command: C) -> Result<()> {
        match self.port.lock().unwrap().as_mut() {
            Some(port) => Ok(port.execute(address, command)?),
            None => Err(Error::NoPortSet),
        }
    }

    pub fn inquire<R: Inquiry>(&self, address: u8) -> Result<R> {
        match self.port.lock().unwrap().as_mut() {
            Some(port) => Ok(port.inquire::<R>(address)?),
            None => Err(Error::NoPortSet),
        }
    }

    pub fn set_port(&self, path: Option<&str>) -> Result<()> {
        println!("{:?}", path);

        // Drop the previous port implicitly before setting a new one
        *self.port.lock().unwrap() = None;

        if let Some(path) = path {
            *self.port.lock().unwrap() = Some(ViscaPort::new(
                serialport::new(path, 9000)
                    .data_bits(DataBits::Eight)
                    .flow_control(FlowControl::None)
                    .parity(Parity::None)
                    .stop_bits(StopBits::One)
                    .timeout(Duration::from_secs(1))
                    .open()?,
            ));
        }

        Ok(())
    }
}
