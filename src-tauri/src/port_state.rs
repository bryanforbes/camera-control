use pelcodrs::Message;
use serde::Serialize;
use serialport::{DataBits, FlowControl, Parity, SerialPort, StopBits};
use std::sync::Mutex;
use tauri::Manager;
use tauri_plugin_store::{with_store, StoreCollection};

use crate::error::{Error, Result};

#[derive(Default)]
pub struct Port {
    port: Option<Box<dyn SerialPort>>,
}

impl Port {
    pub fn name(&self) -> Option<String> {
        self.port.as_ref().and_then(|port| port.name().clone())
    }

    pub fn initialize<R>(
        &mut self,
        app: tauri::AppHandle<R>,
        collection: tauri::State<'_, StoreCollection<R>>,
    ) -> Result<()>
    where
        R: tauri::Runtime,
    {
        with_store(app.app_handle(), collection, "config.json", |store| {
            if let Some(port_name) = store.get("port").cloned() {
                if self.set_port(port_name.as_str()).is_err() {
                    store.insert("port".into(), serde_json::Value::Null)?;
                    store.save()?;
                }
            }

            Ok(())
        })?;

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

    pub fn set<R>(
        &mut self,
        app: tauri::AppHandle<R>,
        collection: tauri::State<'_, StoreCollection<R>>,
        path: Option<&str>,
    ) -> Result<()>
    where
        R: tauri::Runtime,
    {
        self.set_port(path)?;

        with_store(app.app_handle(), collection, "config.json", |store| {
            store.insert("port".into(), path.into())?;
            store.save()
        })?;

        self.emit_all(&app)?;

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
        handle.emit_all("port-state", PortStateEvent::new(self))?;
        Ok(())
    }

    pub fn emit_to<R>(&self, handle: &tauri::AppHandle<R>, label: &str) -> Result<()>
    where
        R: tauri::Runtime,
    {
        handle.emit_to(label, "port-state", PortStateEvent::new(self))?;
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize)]
struct PortStateEvent {
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
