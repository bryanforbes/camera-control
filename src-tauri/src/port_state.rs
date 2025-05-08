use pelcodrs::Message;
use serde::{Deserialize, Serialize};
use serialport::{DataBits, FlowControl, Parity, SerialPort, StopBits};
use specta::Type;
use std::sync::Mutex;
use tauri_plugin_store::StoreExt;
use tauri_specta::Event;

use crate::error::{Error, Result};

#[derive(Default)]
pub struct Port {
    port: Option<Box<dyn SerialPort>>,
}

impl Port {
    pub fn name(&self) -> Option<String> {
        self.port.as_ref().and_then(|port| port.name().clone())
    }

    pub fn initialize<R>(&mut self, app: &tauri::AppHandle<R>) -> Result<()>
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
        println!("{path:?}");

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

    pub fn set<R>(&mut self, app: &tauri::AppHandle<R>, path: Option<&str>) -> Result<()>
    where
        R: tauri::Runtime,
    {
        self.set_port(path)?;

        let store = app.store("config.json")?;
        store.set("port", path);
        store.save()?;
        store.close_resource();

        self.emit_all(app)?;

        Ok(())
    }

    pub fn send_message(&mut self, message: Message) -> Result<()> {
        if let Some(port) = self.port.as_mut() {
            Ok(port.write_all(message.as_ref())?)
        } else {
            Err(Error::NoPortSet)
        }
    }

    pub fn emit_all<R>(&self, handle: &tauri::AppHandle<R>) -> Result<()>
    where
        R: tauri::Runtime,
    {
        PortStateEvent::new(self).emit(handle)?;
        Ok(())
    }

    pub fn emit_to<R>(&self, handle: &tauri::AppHandle<R>, label: &str) -> Result<()>
    where
        R: tauri::Runtime,
    {
        PortStateEvent::new(self).emit_to(handle, label)?;
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Type, Event)]
pub struct PortStateEvent {
    port: Option<String>,
}

impl PortStateEvent {
    pub fn new(port: &Port) -> Self {
        Self { port: port.name() }
    }
}

#[derive(Default)]
pub struct PortState {
    port: Mutex<Port>,
}

pub fn with_port<T, F>(port_state: tauri::State<PortState>, func: F) -> Result<T>
where
    F: FnOnce(&mut Port) -> Result<T>,
{
    let mut port = port_state.port.lock().expect("mutext poisoned");

    func(&mut port)
}
