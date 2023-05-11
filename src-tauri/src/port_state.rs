use serialport::{DataBits, FlowControl, Parity, SerialPort, StopBits};
use std::{sync::Mutex, time::Duration};

use crate::{
    error::{Error, Result},
    visca::{Action, Inquiry, ViscaPort},
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

    pub fn execute(&self, address: u8, action: impl Action) -> Result<()> {
        match self.port.lock().unwrap().as_mut() {
            Some(port) => Ok(port.execute(address, action)?),
            None => Err(Error::NoPortSet),
        }
    }

    pub fn inquire<R>(&self, address: u8) -> Result<R>
    where
        R: Inquiry,
    {
        match self.port.lock().unwrap().as_mut() {
            Some(port) => Ok(port.inquire::<R>(address)?),
            None => Err(Error::NoPortSet),
        }
    }

    pub fn set_port(&self, path: Option<&str>) -> Result<()> {
        debug!("{:?}", path);

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
