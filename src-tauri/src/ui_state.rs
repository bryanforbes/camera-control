use std::sync::Mutex;

use log::debug;
use serde::Serialize;
use specta::Type;
use tauri::Manager;
use tauri_plugin_store::StoreExt;
use tauri_specta::Event;

use crate::{
    camera::Camera,
    error::{Error, Result},
};

#[derive(Default)]
pub struct UIState {
    camera: Option<Camera>,
    ports: Option<Vec<String>>,
    status: String,
}

impl UIState {
    fn set_camera(&mut self, path: Option<&str>) -> Result<()> {
        debug!("{path:?}");

        // Drop the previous camera implicitly before setting a new one
        self.camera = None;

        if let Some(path) = path {
            self.camera = Some(Camera::new(path)?);
        }

        Ok(())
    }

    pub fn initialize<R: tauri::Runtime>(&mut self, app: &tauri::AppHandle<R>) -> Result<()> {
        let store = app.store("config.json")?;
        if let Some(port_name) = store.get("port") {
            if self.set_camera(port_name.as_str()).is_err() {
                store.set("port", serde_json::Value::Null);
                store.save()?;
            }
        }
        store.close_resource();

        if self.camera.is_some() {
            self.set_status("Connected")?;
        } else {
            self.set_status("Disconnected")?;
        }

        Ok(())
    }

    pub fn camera(&mut self) -> Result<&mut Camera> {
        let camera = self.camera.as_mut().ok_or(Error::NoPortSet)?;
        Ok(camera)
    }

    pub fn set_camera_port<R: tauri::Runtime>(
        &mut self,
        app_handle: &tauri::AppHandle<R>,
        path: Option<&str>,
    ) -> Result<()> {
        self.set_camera(path)?;

        let store = app_handle.store("config.json")?;
        store.set("port", path);
        store.save()?;
        store.close_resource();

        if self.camera.is_some() {
            self.set_status("Connected")?;
        } else {
            self.set_status("Disconnected")?;
        }

        Ok(())
    }

    pub fn set_status(&mut self, status: &str) -> Result<()> {
        self.status = String::from(status);
        Ok(())
    }

    pub fn refresh_ports(&mut self) -> Result<()> {
        self.ports = Some(
            serialport::available_ports()?
                .into_iter()
                .map(|port| port.port_name)
                .collect(),
        );
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Type, Event)]
pub struct UIStateEvent {
    port: Option<String>,
    ports: Option<Vec<String>>,
    status: String,
}

impl UIStateEvent {
    pub fn new(state: &mut UIState) -> Self {
        Self {
            port: state.camera().ok().and_then(|camera| camera.name()),
            ports: state.ports.clone(),
            status: state.status.clone(),
        }
    }
}

impl TryFrom<&tauri::AppHandle> for UIStateEvent {
    type Error = crate::error::Error;

    fn try_from(app_handle: &tauri::AppHandle) -> Result<Self> {
        let state = app_handle.state::<Mutex<UIState>>();
        let mut state = state.lock().expect("mutext poisoned");

        Ok(UIStateEvent::new(&mut state))
    }
}

pub fn with_ui_state<T, F>(app_handle: &tauri::AppHandle, func: F)
where
    F: FnOnce(&mut UIState) -> Result<T>,
{
    let state = app_handle.state::<Mutex<UIState>>();
    let mut state = state.lock().expect("mutext poisoned");

    let result = func(&mut state);

    if let Err(error) = result {
        let status = format!(r#"Error: {error}"#);
        let _ = state.set_status(&status);
    }

    let _ = UIStateEvent::new(&mut state).emit(app_handle);
}

pub fn with_ui_state_status<T, F>(app_handle: &tauri::AppHandle, status: &str, func: F)
where
    F: FnOnce(&mut UIState) -> Result<T>,
{
    with_ui_state(app_handle, |ui| {
        func(ui)?;
        ui.set_status(status)
    })
}
