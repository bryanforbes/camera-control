use std::{ops::DerefMut, sync::Mutex, time::Duration};

use serde::{Serialize, Serializer};
use serialport::{DataBits, FlowControl, Parity, SerialPort, StopBits};
use tauri::Manager;

use crate::{
    error::{Error, Result},
    visca::{Action, Autofocus, Inquiry, Power, ViscaPort},
};

fn serialize_as_bool<I, S>(value: &I, s: S) -> std::result::Result<S::Ok, S::Error>
where
    I: Into<bool> + Copy,
    S: Serializer,
{
    s.serialize_bool((*value).into())
}

#[derive(Serialize)]
pub struct CameraState {
    #[serde(skip)]
    port: Option<ViscaPort<Box<dyn SerialPort>>>,

    #[serde(rename = "port")]
    port_name: Option<String>,
    #[serde(serialize_with = "serialize_as_bool")]
    autofocus: Autofocus,
    #[serde(serialize_with = "serialize_as_bool")]
    power: Power,
    status: String,
}

impl CameraState {
    pub fn send(&self, app_handle: &tauri::AppHandle) {
        app_handle.emit_all("camera-state", self).ok();
    }

    pub fn set_port(&mut self, path: Option<&str>) -> Result<()> {
        debug!("{:?}", path);

        // Drop the previous port implicitly before setting a new one
        self.port = None;
        self.port_name = None;
        self.status = "Disconnected".into();

        if let Some(path) = path {
            self.port = Some(ViscaPort::new(
                serialport::new(path, 9000)
                    .data_bits(DataBits::Eight)
                    .flow_control(FlowControl::None)
                    .parity(Parity::None)
                    .stop_bits(StopBits::One)
                    .timeout(Duration::from_secs(1))
                    .open()?,
            ));
            self.port_name = Some(path.into());
            self.status = "Connecting".into();

            let power = self.inquire::<Power>(1)?;

            debug!("Power: {:?}", power);

            self.power = power;
            self.status = "Connected".into();

            if let Power::On = power {
                let autofocus = self.inquire::<Autofocus>(1)?;

                debug!("Autofocus: {:?}", autofocus);
                self.autofocus = autofocus;
            }
        }

        Ok(())
    }

    pub fn set_power(&mut self, address: u8, power: Power) -> Result<()> {
        self.execute(address, power)?;
        self.power = power;
        Ok(())
    }

    pub fn set_autofocus(&mut self, address: u8, autofocus: Autofocus) -> Result<()> {
        self.execute(address, autofocus)?;
        self.autofocus = autofocus;
        Ok(())
    }

    pub fn set_status<T>(&mut self, status: T)
    where
        T: Into<String>,
    {
        self.status = status.into();
    }

    pub fn execute(&mut self, address: u8, action: impl Action) -> Result<()> {
        match self.port.as_mut() {
            Some(port) => Ok(port.execute(address, action)?),
            None => Err(Error::NoPortSet),
        }
    }

    pub fn inquire<R>(&mut self, address: u8) -> Result<R>
    where
        R: Inquiry,
    {
        match self.port.as_mut() {
            Some(port) => Ok(port.inquire::<R>(address)?),
            None => Err(Error::NoPortSet),
        }
    }
}

impl Default for CameraState {
    fn default() -> Self {
        Self {
            autofocus: Autofocus::Manual,
            port_name: None,
            port: None,
            power: Power::Off,
            status: "Disconnected".into(),
        }
    }
}

pub trait MutexCameraState {
    fn with_state<F>(&self, handle: &tauri::AppHandle, func: F)
    where
        F: FnOnce(&mut CameraState) -> Result<()>;
    fn with_state_and_status<F>(&self, handle: &tauri::AppHandle, func: F)
    where
        F: FnOnce(&mut CameraState) -> Result<String>;
}

impl MutexCameraState for Mutex<CameraState> {
    fn with_state<F>(&self, handle: &tauri::AppHandle, func: F)
    where
        F: FnOnce(&mut CameraState) -> Result<()>,
    {
        let mut state = self.lock().unwrap();

        if let Err(error) = func(state.deref_mut()) {
            state.set_status(error.to_string());
        };

        state.send(handle);
    }

    fn with_state_and_status<F>(&self, handle: &tauri::AppHandle, func: F)
    where
        F: FnOnce(&mut CameraState) -> Result<String>,
    {
        let mut state = self.lock().unwrap();

        let status: String = match func(state.deref_mut()) {
            Ok(status) => status,
            Err(error) => error.to_string(),
        };

        state.set_status(status);

        state.send(handle);
    }
}
