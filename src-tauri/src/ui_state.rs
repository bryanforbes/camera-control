use std::sync::Mutex;

use log::debug;
use pelcodrs::Message;
use serde::{Deserialize, Serialize};
use serialport::{DataBits, FlowControl, Parity, SerialPort, StopBits};
use tauri::{Emitter, Manager};
use tauri_plugin_store::StoreExt;

use crate::error::{Error, Result};

#[derive(Default)]
pub struct Port {
    port: Option<Box<dyn SerialPort>>,
}

impl Port {
    fn name(&self) -> Option<String> {
        self.port.as_ref().and_then(|port| port.name().clone())
    }

    fn initialize<R>(&mut self, app: &tauri::AppHandle<R>) -> Result<()>
    where
        R: tauri::Runtime,
    {
        let store = app.store("config.json")?;
        if let Some(port_name) = store.get("port") {
            if self.set_port(port_name.as_str()).is_err() {
                store.set("port", serde_json::Value::Null);
                store.save()?;
            }
        }
        store.close_resource();
        Ok(())
    }

    fn set_port(&mut self, path: Option<&str>) -> Result<()> {
        debug!("{path:?}");

        // Drop the previous port implicitly before setting a new one
        self.port = None;

        if let Some(path) = path {
            self.port = Some(
                serialport::new(path, 9000)
                    .stop_bits(StopBits::One)
                    .data_bits(DataBits::Eight)
                    .flow_control(FlowControl::None)
                    .parity(Parity::None)
                    .open()?,
            );
        }

        Ok(())
    }

    pub fn send_message(&mut self, message: Message) -> Result<()> {
        if let Some(port) = self.port.as_mut() {
            Ok(port.write_all(message.as_ref())?)
        } else {
            Err(Error::NoPortSet)
        }
    }
}

#[derive(Default)]
pub struct UIState {
    pub port: Port,
    ports: Option<Vec<String>>,
    status: String,
}

impl UIState {
    pub fn initialize<R>(&mut self, app: &tauri::AppHandle<R>) -> Result<()>
    where
        R: tauri::Runtime,
    {
        self.port.initialize(app)?;

        if self.port.name().is_some() {
            self.set_status("Connected")?;
        } else {
            self.set_status("Disconnected")?;
        }

        Ok(())
    }

    pub fn port_name(&self) -> Option<String> {
        self.port.name()
    }

    pub fn set_port(&mut self, app_handle: &tauri::AppHandle, path: Option<&str>) -> Result<()> {
        self.port.set_port(path)?;

        if self.port.name().is_some() {
            self.set_status("Connected")?;
        } else {
            self.set_status("Disconnected")?;
        }

        let store = app_handle.store("config.json")?;
        store.set("port", path);
        store.save()?;
        store.close_resource();

        Ok(())
    }

    pub fn set_status(&mut self, status: &str) -> Result<()> {
        self.status = String::from(status);
        Ok(())
    }

    pub fn populate_ports(&mut self) -> Result<()> {
        self.ports = Some(
            serialport::available_ports()?
                .into_iter()
                .map(|port| port.port_name)
                .collect(),
        );
        Ok(())
    }
}

pub type UIStateHandle = Mutex<UIState>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UIStateEvent {
    port: Option<String>,
    ports: Option<Vec<String>>,
    status: String,
}

impl UIStateEvent {
    pub fn new(state: &UIState) -> Self {
        Self {
            port: state.port_name(),
            ports: state.ports.clone(),
            status: state.status.clone(),
        }
    }
}

impl TryFrom<&tauri::AppHandle> for UIStateEvent {
    type Error = crate::error::Error;

    fn try_from(app_handle: &tauri::AppHandle) -> Result<Self> {
        let state = app_handle.state::<UIStateHandle>();
        let state = state.lock().expect("mutext poisoned");

        Ok(UIStateEvent::new(&state))
    }
}

pub fn with_ui_state<T, F>(app_handle: &tauri::AppHandle, func: F) -> Result<T>
where
    F: FnOnce(&mut UIState) -> Result<T>,
{
    let state = app_handle.state::<UIStateHandle>();
    let mut state = state.lock().expect("mutext poisoned");

    let result = func(&mut state);

    app_handle.emit("ui-state", UIStateEvent::new(&state))?;

    result
}
