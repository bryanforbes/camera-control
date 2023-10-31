use pelcodrs::{Message, PelcoDPort};
use serialport::{DataBits, FlowControl, Parity, SerialPort, StopBits};
use std::{
    ops::{Deref, DerefMut},
    sync::Mutex,
};
use tauri_specta::Event;

use crate::error::{Error, Result};
use serde::{Deserialize, Serialize};

pub struct PortState {
    port: Option<PelcoDPort<Box<dyn SerialPort>>>,
    port_name: Option<String>,
    status: String,
}

impl PortState {
    pub fn send_message(&mut self, message: Message) -> Result<()> {
        match self.port.as_mut() {
            Some(port) => Ok(port.send_message(message)?),
            None => Err(Error::NoPortSet),
        }
    }

    pub fn set_port(&mut self, path: Option<&str>) -> Result<()> {
        println!("{:?}", path);

        // Drop the previous port implicitly before setting a new one
        self.port = None;
        self.port_name = None;
        self.status = "Disconnected".into();

        if let Some(path) = path {
            self.port = Some(PelcoDPort::new(
                serialport::new(path, 9000)
                    .stop_bits(StopBits::One)
                    .data_bits(DataBits::Eight)
                    .flow_control(FlowControl::None)
                    .parity(Parity::None)
                    .open()?,
            ));
            self.port_name = Some(path.into());
            self.status = "Connected".into();
        }

        Ok(())
    }

    pub fn set_status<T>(&mut self, status: T)
    where
        T: Into<String>,
    {
        self.status = status.into();
    }
}

impl Default for PortState {
    fn default() -> Self {
        Self {
            port: None,
            port_name: None,
            status: "Disconnected".into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type, tauri_specta::Event)]
pub struct PortStateEvent {
    port: Option<String>,
    status: String,
}

impl From<&PortState> for PortStateEvent {
    fn from(value: &PortState) -> Self {
        Self {
            port: value.port_name.clone(),
            status: value.status.clone(),
        }
    }
}

pub trait MutexPortState {
    fn with_state<F>(&self, handle: &tauri::AppHandle, func: F)
    where
        F: FnOnce(&mut PortState) -> Result<()>;
    fn with_state_and_status<F>(&self, handle: &tauri::AppHandle, func: F)
    where
        F: FnOnce(&mut PortState) -> Result<String>;
}

impl MutexPortState for Mutex<PortState> {
    fn with_state<F>(&self, handle: &tauri::AppHandle, func: F)
    where
        F: FnOnce(&mut PortState) -> Result<()>,
    {
        let mut state = self.lock().unwrap();

        if let Err(error) = func(state.deref_mut()) {
            state.set_status(error.to_string());
        };

        PortStateEvent::from(state.deref()).emit_all(handle).ok();
    }

    fn with_state_and_status<F>(&self, handle: &tauri::AppHandle, func: F)
    where
        F: FnOnce(&mut PortState) -> Result<String>,
    {
        let mut state = self.lock().unwrap();

        let status: String = match func(state.deref_mut()) {
            Ok(status) => status,
            Err(error) => error.to_string(),
        };

        state.set_status(status);

        PortStateEvent::from(state.deref()).emit_all(handle).ok();
    }
}
