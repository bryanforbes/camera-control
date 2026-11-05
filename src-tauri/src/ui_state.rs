use std::sync::Mutex;

use log::{debug, error};
use serde::Serialize;
use specta::Type;
use tauri::Manager;
use tauri_plugin_store::StoreExt;
use tauri_specta::Event;

use crate::{
    camera::Camera,
    error::{Error, Result},
    pelco_camera::PelcoCamera,
};

#[derive(Serialize, Type)]
#[serde(rename_all = "camelCase")]
#[specta(inline)]
struct StatusPayload {
    text: String,
    is_error: bool,
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(into = "StatusPayload")]
#[specta(type = StatusPayload)]
pub enum Status {
    Success(String),
    Error(String),
}

impl From<Status> for StatusPayload {
    fn from(status: Status) -> Self {
        match status {
            Status::Success(message) => Self {
                text: message,
                is_error: false,
            },
            Status::Error(message) => Self {
                text: message,
                is_error: true,
            },
        }
    }
}

impl Default for Status {
    fn default() -> Self {
        Status::Success(String::new())
    }
}

#[derive(Default)]
pub struct UIState {
    camera: Option<Box<dyn Camera>>,
    ports: Option<Vec<String>>,
    status: Status,
}

fn open_camera(path: &str) -> Result<Box<dyn Camera>> {
    Ok(Box::new(PelcoCamera::new(path)?))
}

impl UIState {
    fn set_camera(
        &mut self,
        path: Option<&str>,
        open: impl FnOnce(&str) -> Result<Box<dyn Camera>>,
    ) -> Result<()> {
        debug!("{path:?}");

        if let Some(camera) = self.camera.as_mut()
            && let Err(error) = camera.stop()
        {
            error!("Failed to stop camera before changing ports: {error}");
        }

        // Drop the previous camera implicitly before setting a new one
        self.camera = None;

        if let Some(path) = path {
            self.camera = Some(open(path)?);
        }

        Ok(())
    }

    pub fn initialize<R: tauri::Runtime>(&mut self, app: &tauri::AppHandle<R>) -> Result<()> {
        self.initialize_with(app, open_camera)
    }

    fn initialize_with<R: tauri::Runtime>(
        &mut self,
        app: &tauri::AppHandle<R>,
        open: impl FnOnce(&str) -> Result<Box<dyn Camera>>,
    ) -> Result<()> {
        let store = app.store("config.json")?;
        let saved_port = store.get("port");
        store.close_resource();

        let Some(path) = saved_port.as_ref().and_then(|value| value.as_str()) else {
            return self.set_status("Disconnected");
        };

        match self.set_camera(Some(path), open) {
            Ok(()) => self.set_status("Connected"),
            Err(error) => {
                error!("Failed to connect to saved port {path}: {error}");

                self.set_error(&format!("Could not open {path}. Connect the camera, then refresh and select its port in Settings."))
            }
        }
    }

    pub fn camera(&mut self) -> Result<&mut dyn Camera> {
        let camera = self.camera.as_mut().ok_or(Error::NoPortSet)?;
        Ok(camera.as_mut())
    }

    pub fn set_camera_port<R: tauri::Runtime>(
        &mut self,
        app_handle: &tauri::AppHandle<R>,
        path: Option<&str>,
    ) -> Result<()> {
        self.set_camera_port_with(app_handle, path, open_camera)
    }

    fn set_camera_port_with<R: tauri::Runtime>(
        &mut self,
        app_handle: &tauri::AppHandle<R>,
        path: Option<&str>,
        open: impl FnOnce(&str) -> Result<Box<dyn Camera>>,
    ) -> Result<()> {
        self.set_camera(path, open)?;

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
        self.status = Status::Success(String::from(status));
        Ok(())
    }

    pub fn set_error(&mut self, error: &str) -> Result<()> {
        self.status = Status::Error(String::from(error));
        Ok(())
    }

    pub fn refresh_ports(&mut self) -> Result<()> {
        self.update_ports(serialport::available_ports())
    }

    fn update_ports(
        &mut self,
        ports: serialport::Result<Vec<serialport::SerialPortInfo>>,
    ) -> Result<()> {
        self.ports = Some(ports?.into_iter().map(|port| port.port_name).collect());
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Type, Event)]
pub struct UIStateEvent {
    port: Option<String>,
    ports: Option<Vec<String>>,
    status: Status,
}

impl From<&mut UIState> for UIStateEvent {
    fn from(state: &mut UIState) -> Self {
        Self {
            port: state.camera().ok().and_then(|camera| camera.name()),
            ports: state.ports.clone(),
            status: state.status.clone(),
        }
    }
}

impl<R: tauri::Runtime> TryFrom<&tauri::AppHandle<R>> for UIStateEvent {
    type Error = crate::error::Error;

    fn try_from(app_handle: &tauri::AppHandle<R>) -> Result<Self> {
        let state = app_handle.state::<Mutex<UIState>>();
        let mut state = state.lock().expect("mutext poisoned");

        Ok(UIStateEvent::from(&mut *state))
    }
}

pub fn with_ui_state<R: tauri::Runtime, T, F>(app_handle: &tauri::AppHandle<R>, func: F)
where
    F: FnOnce(&mut UIState) -> Result<T>,
{
    let state = app_handle.state::<Mutex<UIState>>();
    let mut state = state.lock().expect("mutext poisoned");

    let result = func(&mut state);

    if let Err(error) = result {
        let status = format!(r#"Error: {error}"#);
        let _ = state.set_error(&status);
    }

    let _ = UIStateEvent::from(&mut *state).emit(app_handle);
}

pub fn with_ui_state_status<R: tauri::Runtime, T, F>(
    app_handle: &tauri::AppHandle<R>,
    status: &str,
    func: F,
) where
    F: FnOnce(&mut UIState) -> Result<T>,
{
    with_ui_state(app_handle, |ui| {
        func(ui)?;
        ui.set_status(status)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::camera::{Direction, Zoom};
    use serde_json::{Value, json, to_value};
    use serialport::{SerialPortInfo, SerialPortType};
    use std::{fs, path::Path, sync::Arc};
    use tauri::{
        Listener,
        test::{MockRuntime, mock_builder, mock_context, noop_assets},
    };
    use test_case::test_case;

    struct FakeCamera {
        name: String,
        calls: Arc<Mutex<Vec<&'static str>>>,
        fail_stop: bool,
    }

    impl FakeCamera {
        fn new(name: &str) -> Self {
            Self {
                name: name.into(),
                calls: Arc::default(),
                fail_stop: false,
            }
        }
    }

    impl Drop for FakeCamera {
        fn drop(&mut self) {
            self.calls.lock().unwrap().push("drop");
        }
    }

    impl Camera for FakeCamera {
        fn new(_: &str) -> Result<Self> {
            unreachable!("inject the fake camera directly")
        }

        fn name(&self) -> Option<String> {
            Some(self.name.clone())
        }

        fn stop(&mut self) -> Result<()> {
            self.calls.lock().unwrap().push("stop");
            if self.fail_stop {
                Err(std::io::Error::from(std::io::ErrorKind::BrokenPipe).into())
            } else {
                Ok(())
            }
        }

        fn power_on(&mut self) -> Result<()> {
            unreachable!()
        }

        fn power_off(&mut self) -> Result<()> {
            unreachable!()
        }

        fn autofocus(&mut self, _: bool) -> Result<()> {
            unreachable!()
        }

        fn zoom(&mut self, _: Zoom) -> Result<()> {
            unreachable!()
        }

        fn pan_tilt(&mut self, _: Direction) -> Result<()> {
            unreachable!()
        }

        fn set_preset(&mut self, _: u8) -> Result<()> {
            unreachable!()
        }

        fn go_to_preset(&mut self, _: u8) -> Result<()> {
            unreachable!()
        }
    }

    fn mock_app(data_dir: &Path) -> tauri::App<MockRuntime> {
        let mut context = mock_context(noop_assets());

        context.config_mut().app.app_directories_override =
            serde_json::from_value(json!({ "data": data_dir })).unwrap();

        let app = mock_builder()
            .plugin(tauri_plugin_store::Builder::default().build())
            .manage(Mutex::new(UIState::default()))
            .build(context)
            .unwrap();

        tauri_specta::Builder::new()
            .events(tauri_specta::collect_events![UIStateEvent])
            .mount_events(&app);

        app
    }

    fn snapshot(state: &mut UIState) -> Value {
        to_value(UIStateEvent::from(state)).unwrap()
    }

    #[test]
    fn default_state_has_no_camera_or_port_list() {
        let mut state = UIState::default();

        assert!(matches!(state.camera(), Err(Error::NoPortSet)));
        assert_eq!(
            snapshot(&mut state),
            json!({"port": null, "ports": null, "status": {"text": "", "isError": false}})
        );
    }

    #[test]
    fn snapshot_owns_its_port_list_and_status() {
        let mut state = UIState {
            camera: Some(Box::new(FakeCamera::new("selected"))),
            ports: Some(vec!["selected".into(), "other".into()]),
            ..UIState::default()
        };
        state.set_error("Movement failed").unwrap();
        let event = UIStateEvent::from(&mut state);

        state.ports = Some(vec![]);
        state.set_status("Ready").unwrap();

        assert_eq!(
            to_value(event).unwrap(),
            json!({
                "port": "selected",
                "ports": ["selected", "other"],
                "status": {"text": "Movement failed", "isError": true}
            })
        );
        assert_eq!(
            snapshot(&mut state),
            json!({"port": "selected", "ports": [], "status": {"text": "Ready", "isError": false}})
        );
    }

    #[test_case(false; "stop succeeds")]
    #[test_case(true; "stop fails")]
    fn disconnect_stops_and_drops_camera(fail_stop: bool) {
        let mut camera = FakeCamera::new("old");
        camera.fail_stop = fail_stop;
        let calls = Arc::clone(&camera.calls);
        let mut state = UIState {
            camera: Some(Box::new(camera)),
            ..UIState::default()
        };

        state.set_camera(None, |_| unreachable!()).unwrap();

        assert_eq!(*calls.lock().unwrap(), ["stop", "drop"]);
        assert!(matches!(state.camera(), Err(Error::NoPortSet)));
    }

    #[test]
    fn replacement_opens_after_previous_camera_is_stopped_and_dropped() {
        let old = FakeCamera::new("old");
        let calls = Arc::clone(&old.calls);
        let mut state = UIState {
            camera: Some(Box::new(old)),
            ..UIState::default()
        };

        state
            .set_camera(Some("new"), |path| {
                assert_eq!(path, "new");
                assert_eq!(*calls.lock().unwrap(), ["stop", "drop"]);
                Ok(Box::new(FakeCamera::new(path)))
            })
            .unwrap();

        assert_eq!(state.camera().unwrap().name().as_deref(), Some("new"));
    }

    #[test]
    fn port_refresh_replaces_names_and_keeps_them_on_error() {
        let mut state = UIState {
            ports: Some(vec!["stale".into()]),
            ..UIState::default()
        };

        state
            .update_ports(Ok(vec![
                SerialPortInfo {
                    port_name: "first".into(),
                    port_type: SerialPortType::Unknown,
                },
                SerialPortInfo {
                    port_name: "second".into(),
                    port_type: SerialPortType::Unknown,
                },
            ]))
            .unwrap();

        assert_eq!(snapshot(&mut state)["ports"], json!(["first", "second"]));

        let error = serialport::Error::new(serialport::ErrorKind::Unknown, "enumeration failed");
        assert!(matches!(
            state.update_ports(Err(error)),
            Err(Error::SerialPort(_))
        ));
        assert_eq!(snapshot(&mut state)["ports"], json!(["first", "second"]));

        state.update_ports(Ok(vec![])).unwrap();
        assert_eq!(snapshot(&mut state)["ports"], json!([]));
    }

    #[test_case(json!({}); "missing port")]
    #[test_case(json!({"port": null}); "null port")]
    #[test_case(json!({"port": 42}); "non-string port")]
    fn initialize_without_saved_path_is_disconnected(config: Value) {
        let dir = tempfile::tempdir().unwrap();

        fs::write(dir.path().join("config.json"), config.to_string()).unwrap();

        let app = mock_app(dir.path());
        let mut state = UIState::default();

        state
            .initialize_with(app.handle(), |_| unreachable!())
            .unwrap();

        assert!(matches!(state.camera(), Err(Error::NoPortSet)));
        assert_eq!(
            snapshot(&mut state)["status"],
            json!({"text": "Disconnected", "isError": false})
        );
    }

    #[test_case(false; "opens saved port")]
    #[test_case(true; "saved port fails to open")]
    fn initialize_from_saved_path(fail_open: bool) {
        let dir = tempfile::tempdir().unwrap();

        fs::write(dir.path().join("config.json"), r#"{"port":"saved"}"#).unwrap();

        let app = mock_app(dir.path());
        let mut state = UIState::default();

        state
            .initialize_with(app.handle(), |path| {
                assert_eq!(path, "saved");
                if fail_open {
                    Err(
                        serialport::Error::new(serialport::ErrorKind::NoDevice, "missing port")
                            .into(),
                    )
                } else {
                    Ok(Box::new(FakeCamera::new(path)))
                }
            })
            .unwrap();

        let event = snapshot(&mut state);

        if fail_open {
            assert_eq!(event["port"], Value::Null);
            assert_eq!(
                event["status"],
                json!({
                    "text": "Could not open saved. Connect the camera, then refresh and select its port in Settings.",
                    "isError": true
                })
            );
        } else {
            assert_eq!(event["port"], "saved");
            assert_eq!(
                event["status"],
                json!({"text": "Connected", "isError": false})
            );
        }
    }

    fn saved_port(dir: &Path) -> Value {
        let contents = fs::read(dir.join("config.json")).unwrap();
        let config: Value = serde_json::from_slice(&contents).unwrap();

        config.get("port").expect("store contains port key").clone()
    }

    #[test]
    fn setting_and_clearing_port_persists_the_selection() {
        let dir = tempfile::tempdir().unwrap();
        let app = mock_app(dir.path());
        let mut state = UIState::default();

        state
            .set_camera_port_with(app.handle(), Some("selected"), |path| {
                Ok(Box::new(FakeCamera::new(path)))
            })
            .unwrap();

        assert_eq!(saved_port(dir.path()), "selected");
        assert_eq!(
            snapshot(&mut state)["status"],
            json!({"text": "Connected", "isError": false})
        );

        state.set_camera_port(app.handle(), None).unwrap();

        assert_eq!(saved_port(dir.path()), Value::Null);
        assert!(matches!(state.camera(), Err(Error::NoPortSet)));
        assert_eq!(
            snapshot(&mut state)["status"],
            json!({"text": "Disconnected", "isError": false})
        );
    }

    #[test]
    fn failed_replacement_disconnects_without_overwriting_saved_port() {
        let dir = tempfile::tempdir().unwrap();

        fs::write(dir.path().join("config.json"), r#"{"port":"old"}"#).unwrap();

        let app = mock_app(dir.path());
        let mut state = UIState {
            camera: Some(Box::new(FakeCamera::new("old"))),
            ..UIState::default()
        };

        let result = state.set_camera_port_with(app.handle(), Some("missing"), |_| {
            Err(serialport::Error::new(serialport::ErrorKind::NoDevice, "missing port").into())
        });

        assert!(matches!(result, Err(Error::SerialPort(_))));
        assert!(matches!(state.camera(), Err(Error::NoPortSet)));
        assert_eq!(saved_port(dir.path()), "old");
    }

    #[test]
    fn set_camera_port_returns_store_save_failure() {
        let dir = tempfile::tempdir().unwrap();

        fs::create_dir(dir.path().join("config.json")).unwrap();

        let app = mock_app(dir.path());
        let mut state = UIState::default();

        let result = state.set_camera_port_with(app.handle(), Some("selected"), |path| {
            Ok(Box::new(FakeCamera::new(path)))
        });

        assert!(matches!(result, Err(Error::Store(_))));
        assert_eq!(
            snapshot(&mut state)["status"],
            json!({"text": "", "isError": false})
        );
    }

    #[test]
    fn state_wrappers_emit_success_and_error_snapshots() {
        let dir = tempfile::tempdir().unwrap();
        let app = mock_app(dir.path());
        let handle = app.handle();
        let payloads = Arc::new(Mutex::new(Vec::new()));
        let received = Arc::clone(&payloads);

        app.listen(UIStateEvent::NAME, move |event| {
            let payload: Value = serde_json::from_str(event.payload()).unwrap();
            received.lock().unwrap().push(payload);
        });

        with_ui_state_status(handle, "Ready", |_| Ok(()));
        with_ui_state_status(handle, "Moved", |state| state.camera()?.stop());

        let payloads = payloads.lock().unwrap();

        assert_eq!(
            *payloads,
            [
                json!({"port": null, "ports": null, "status": {"text": "Ready", "isError": false}}),
                json!({"port": null, "ports": null, "status": {"text": "Error: No port set", "isError": true}})
            ]
        );
        assert_eq!(
            to_value(UIStateEvent::try_from(handle).unwrap()).unwrap(),
            payloads[1]
        );
    }
}
