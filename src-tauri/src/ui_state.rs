use crate::{
    camera::{
        CameraAction, CameraEvent, CameraEventSender, CameraHandle, CameraMotion, CameraSession,
        CameraSessionBuilder, CameraSessionResult, CommandOutcome, MotionHandle, PendingMotion,
        PendingMotionResult, SessionId,
    },
    error::{Error, Result},
    pelco_camera::PelcoCamera,
};
use serde::Serialize;
use specta::Type;
use std::sync::Mutex;
use tauri::Manager;
use tauri_plugin_store::StoreExt;
use tauri_specta::Event;
use uuid::Uuid;

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

struct RegisteredMotion {
    token: String,
    handle: MotionHandle,
}

#[derive(Default)]
pub struct UIState {
    session: Option<CameraSession>,
    motion: Option<RegisteredMotion>,
    ports: Option<Vec<String>>,
    status: Status,
    exiting: bool,
}

impl UIState {
    pub fn app_state() -> Mutex<Self> {
        Mutex::new(Self::default())
    }

    pub fn session_id(&self) -> Option<SessionId> {
        self.session.as_ref().map(|session| session.id())
    }

    pub fn camera(&self) -> Result<CameraHandle> {
        self.session
            .as_ref()
            .map(CameraSession::handle)
            .ok_or(Error::NoPortSet)
    }

    pub fn start_motion(
        &mut self,
        do_motion: impl FnOnce(&CameraHandle) -> PendingMotionResult,
    ) -> Result<String> {
        let camera = self.camera()?;
        let PendingMotion { motion, response } = do_motion(&camera)?;

        let token = Uuid::new_v4().to_string();
        self.motion = Some(RegisteredMotion {
            token: token.clone(),
            handle: motion,
        });

        // The worker publishes the outcome independently of this reply.
        drop(response);
        Ok(token)
    }

    pub fn set_status(&mut self, status: &str) {
        self.status = Status::Success(String::from(status));
    }

    pub fn set_error(&mut self, error: &str) {
        self.status = Status::Error(String::from(error));
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

pub type AppUIState = Mutex<UIState>;

#[derive(Debug, Clone, Serialize, Type, Event)]
pub struct UIStateEvent {
    port: Option<String>,
    ports: Option<Vec<String>>,
    status: Status,
}

impl From<&UIState> for UIStateEvent {
    fn from(state: &UIState) -> Self {
        Self {
            port: state
                .session
                .as_ref()
                .and_then(|session| session.name())
                .map(|name| name.to_owned()),
            ports: state.ports.clone(),
            status: state.status.clone(),
        }
    }
}

pub fn with_ui<R: tauri::Runtime, T, F>(app: &tauri::AppHandle<R>, op: F) -> Result<T>
where
    F: FnOnce(&mut UIState) -> Result<T>,
{
    let state = app.state::<AppUIState>();
    let mut ui = state.lock().expect("mutext poisoned");

    op(&mut ui)
}

pub fn with_ui_emit<R: tauri::Runtime, T, F>(app: &tauri::AppHandle<R>, op: F) -> Result<T>
where
    F: FnOnce(&mut UIState) -> Result<T>,
{
    with_ui(app, |ui| {
        let result = op(ui);

        if let Err(error) = &result {
            ui.set_error(&format!(r#"Error: {error}"#));
        }

        let _ = UIStateEvent::from(&*ui).emit(app);

        result
    })
}

fn status_for(action: &CameraAction, outcome: &CommandOutcome) -> String {
    match action {
        CameraAction::RecallPreset(_, preset) => match outcome {
            CommandOutcome::Sent => format!("Recalling preset {preset}"),
            CommandOutcome::Completed => format!("Recalled preset {preset}"),
            CommandOutcome::Superceded => format!("Preset {preset} recall superceded"),
        },
        CameraAction::SetPreset(_, preset) => match outcome {
            CommandOutcome::Sent => format!("Setting preset {preset}"),
            CommandOutcome::Completed => format!("Set preset {preset}"),
            CommandOutcome::Superceded => format!("Preset {preset} set superceded"),
        },
        CameraAction::Motion(motion) => match motion {
            CameraMotion::PanTilt(direction) => match outcome {
                CommandOutcome::Sent | CommandOutcome::Completed => format!("Moving {direction}"),
                CommandOutcome::Superceded => format!("Moving {direction} superceded"),
            },
            CameraMotion::Zoom(zoom) => match outcome {
                CommandOutcome::Sent | CommandOutcome::Completed => format!("Zooming {zoom}"),
                CommandOutcome::Superceded => format!("Zooming {zoom} superceded"),
            },
        },
        CameraAction::StopMotion => match outcome {
            CommandOutcome::Sent | CommandOutcome::Completed | CommandOutcome::Superceded => {
                "Done moving".into()
            }
        },
    }
}

pub fn publish_camera_event<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    event: CameraEvent,
) -> Result<()> {
    with_ui(app, |ui| {
        if ui.session_id() != Some(event.session()) {
            return Ok(());
        }

        match event.result() {
            Ok(CommandOutcome::Superceded) => return Ok(()),
            Ok(outcome) => {
                ui.set_status(&status_for(event.action(), outcome));
            }
            Err(error) => {
                ui.set_error(error);
            }
        }

        let _ = UIStateEvent::from(&*ui).emit(app);
        Ok(())
    })
}

#[derive(Default)]
pub struct PortChangeLock {
    mutex: tokio::sync::Mutex<()>,
}

pub async fn set_ui_state_session<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    path: Option<&str>,
) -> Result<()> {
    set_ui_state_session_with(app, path, async |path| open_session(app, path).await).await
}

pub type CameraEventSenderState = Mutex<CameraEventSender>;

async fn open_session<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    path: &str,
) -> CameraSessionResult {
    let events_tx_state = app.state::<CameraEventSenderState>();
    let events_tx = events_tx_state.lock().expect("mutex poisoned").clone();
    CameraSessionBuilder::new(path, events_tx)
        .open(PelcoCamera::from_serial_port)
        .await
}

async fn set_ui_state_session_with<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    path: Option<&str>,
    open: impl AsyncFnOnce(&str) -> CameraSessionResult,
) -> Result<()> {
    let port_changes = app.state::<PortChangeLock>();
    let _port_change_guard = port_changes.mutex.lock().await;

    let state = app.state::<AppUIState>();

    let old_session = {
        let mut ui = state.lock().expect("mutex poisoned");

        if ui.exiting {
            return Ok(());
        }

        ui.motion = None;
        ui.session.take()
    };

    if let Some(session) = old_session
        && let Err(error) = session.shutdown().await
    {
        log::warn!("Failed to stop previous camera session: {error}");
    }

    let result: Result<()> = async {
        let new_session = match path {
            Some(path) => Some(open(path).await?),
            None => None,
        };

        state.lock().expect("mutex poisoned").session = new_session;

        let store = app.store("config.json")?;
        store.set("port", path);
        let saved = store.save();
        store.close_resource();
        saved?;
        Ok(())
    }
    .await;

    with_ui(app, |ui| {
        match (&result, path) {
            (Ok(()), _) => ui.set_status(if ui.session.is_some() {
                "Connected"
            } else {
                "Disconnected"
            }),
            (Err(Error::Camera(error)), Some(path)) => {
                ui.set_error(&format!("Could not open camera port {path}: {error}"));
            }
            (Err(error), _) => {
                ui.set_error(&format!("Could not save camera port selection: {error}"));
            }
        }
        let _ = UIStateEvent::from(&*ui).emit(app);
        result
    })
}

pub async fn initialize_ui_state<R: tauri::Runtime>(app: &tauri::AppHandle<R>) -> Result<()> {
    initialize_ui_state_with(app, async |path| open_session(app, path).await).await
}

async fn initialize_ui_state_with<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    open: impl AsyncFnOnce(&str) -> CameraSessionResult,
) -> Result<()> {
    let store = app.store("config.json")?;

    let path = store
        .get("port")
        .and_then(|value| value.as_str().map(str::to_owned));

    store.close_resource();

    set_ui_state_session_with(app, path.as_deref(), open).await
}

pub async fn stop_ui_state_motion<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    token: &str,
) -> Result<()> {
    let Some(motion) = with_ui(app, |ui| {
        Ok(ui
            .motion
            .as_ref()
            .filter(|motion| motion.token == token)
            .map(|motion| motion.handle.clone()))
    })?
    else {
        return Ok(());
    };

    motion.stop().await?;

    with_ui(app, |ui| {
        if ui
            .motion
            .as_ref()
            .is_some_and(|current| current.token == token)
        {
            ui.motion = None;
        }
        Ok(())
    })
}

pub async fn shutdown_ui_state<R: tauri::Runtime>(app: &tauri::AppHandle<R>) -> Result<()> {
    let port_changes = app.state::<PortChangeLock>();
    let _port_change_guard = port_changes.mutex.lock().await;

    let state = app.state::<AppUIState>();

    let session = {
        let mut ui = state.lock().expect("mutex poisoned");
        ui.exiting = true;
        ui.motion = None;
        ui.session.take()
    };

    if let Some(session) = session {
        session.shutdown().await?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::camera::Error as CameraError;
    use crate::camera::{
        CameraAction, CameraSessionAdapter, CommandOutcome, CommandResult, Direction,
    };
    use serde_json::{Value, json, to_value};
    use serialport::{SerialPortInfo, SerialPortType};
    use std::{
        fs,
        path::Path,
        sync::Arc,
        task::{Context, Poll, Waker},
        time::Duration,
    };
    use tauri::{
        Listener,
        test::{MockRuntime, mock_builder, mock_context, noop_assets},
    };
    use test_case::test_case;
    use tokio::sync::{mpsc, oneshot};

    type Calls = Arc<Mutex<Vec<String>>>;

    struct FakeAdapter {
        name: String,
        calls: Calls,
        fail_close: bool,
        close_gate: Option<(oneshot::Sender<()>, std::sync::mpsc::Receiver<()>)>,
        motion_started: Option<oneshot::Sender<()>>,
        fail_stop_once: bool,
        stop_gate: Option<(oneshot::Sender<()>, std::sync::mpsc::Receiver<()>)>,
    }

    impl FakeAdapter {
        fn new(name: &str, calls: &Calls) -> Self {
            Self {
                name: name.into(),
                calls: calls.clone(),
                fail_close: false,
                close_gate: None,
                motion_started: None,
                fail_stop_once: false,
                stop_gate: None,
            }
        }

        fn record(&self, action: &str) {
            self.calls
                .lock()
                .unwrap()
                .push(format!("{action}:{}", self.name));
        }
    }

    impl Drop for FakeAdapter {
        fn drop(&mut self) {
            self.record("drop");
        }
    }

    impl CameraSessionAdapter for FakeAdapter {
        fn name(&self) -> Option<String> {
            Some(self.name.clone())
        }
        fn handle_action(&mut self, action: &CameraAction) -> CommandResult {
            if matches!(action, CameraAction::StopMotion) {
                self.record("stop");
                if let Some((entered, release)) = self.stop_gate.take() {
                    entered.send(()).unwrap();
                    release.recv_timeout(Duration::from_secs(5)).unwrap();
                }
                if std::mem::take(&mut self.fail_stop_once) {
                    return Err(std::io::Error::from(std::io::ErrorKind::BrokenPipe).into());
                }
            } else {
                self.record("action");
                if matches!(action, CameraAction::Motion(_))
                    && let Some(started) = self.motion_started.take()
                {
                    let _ = started.send(());
                }
            }
            Ok(CommandOutcome::Completed)
        }
        fn stop_motion(&mut self) -> crate::camera::Result<()> {
            Ok(())
        }
        fn close(&mut self) -> crate::camera::Result<()> {
            self.record("close");
            if let Some((entered, release)) = self.close_gate.take() {
                entered.send(()).unwrap();
                release.recv_timeout(Duration::from_secs(5)).unwrap();
            }
            if self.fail_close {
                Err(std::io::Error::from(std::io::ErrorKind::BrokenPipe).into())
            } else {
                Ok(())
            }
        }
    }

    async fn open_adapter(adapter: FakeAdapter) -> CameraSessionResult {
        let path = adapter.name.clone();
        let (events, _received) = mpsc::unbounded_channel();
        CameraSessionBuilder::new(path, events)
            .open_with_adapter(move |path| {
                assert_eq!(path, adapter.name);
                adapter.record("open");
                Ok(adapter)
            })
            .await
    }

    async fn open_fake(path: &str, calls: &Calls) -> CameraSessionResult {
        open_adapter(FakeAdapter::new(path, calls)).await
    }

    fn run(future: impl Future<Output = ()>) {
        tauri::async_runtime::block_on(async {
            tokio::time::timeout(Duration::from_secs(5), future)
                .await
                .expect("test timed out");
        });
    }

    fn mock_app(data_dir: &Path) -> tauri::App<MockRuntime> {
        let mut context = mock_context(noop_assets());
        context.config_mut().app.app_directories_override =
            serde_json::from_value(json!({ "data": data_dir })).unwrap();
        let app = mock_builder()
            .plugin(tauri_plugin_store::Builder::default().build())
            .manage(UIState::app_state())
            .manage(PortChangeLock::default())
            .build(context)
            .unwrap();
        tauri_specta::Builder::new()
            .events(tauri_specta::collect_events![UIStateEvent])
            .mount_events(&app);
        app
    }

    fn snapshot(state: &mut UIState) -> Value {
        to_value(UIStateEvent::from(&*state)).unwrap()
    }

    fn app_snapshot(app: &tauri::AppHandle<MockRuntime>) -> Value {
        to_value(with_ui(app, |ui| Ok(UIStateEvent::from(&*ui))).unwrap()).unwrap()
    }

    fn listen(app: &tauri::App<MockRuntime>) -> Arc<Mutex<Vec<Value>>> {
        let payloads = Arc::new(Mutex::new(Vec::new()));
        let received = payloads.clone();
        app.listen(UIStateEvent::NAME, move |event| {
            received
                .lock()
                .unwrap()
                .push(serde_json::from_str(event.payload()).unwrap());
        });
        payloads
    }

    fn saved_port(dir: &Path) -> Value {
        let config: Value =
            serde_json::from_slice(&fs::read(dir.join("config.json")).unwrap()).unwrap();
        config["port"].clone()
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
        run(async {
            let calls = Calls::default();
            let mut state = UIState {
                session: Some(open_fake("selected", &calls).await.unwrap()),
                ports: Some(vec!["selected".into(), "other".into()]),
                ..UIState::default()
            };
            state.set_error("Movement failed");
            let event = UIStateEvent::from(&state);
            state.ports = Some(vec![]);
            state.set_status("Ready");
            state.session.take().unwrap().shutdown().await.unwrap();
            assert_eq!(
                to_value(event).unwrap(),
                json!({"port":"selected", "ports":["selected","other"], "status":{"text":"Movement failed","isError":true}})
            );
            assert_eq!(
                snapshot(&mut state),
                json!({"port":null, "ports":[], "status":{"text":"Ready","isError":false}})
            );
        });
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
        run(async {
            let dir = tempfile::tempdir().unwrap();
            fs::write(dir.path().join("config.json"), config.to_string()).unwrap();
            let app = mock_app(dir.path());
            initialize_ui_state_with(app.handle(), async |_| panic!("must not open a port"))
                .await
                .unwrap();
            assert_eq!(
                app_snapshot(app.handle()),
                json!({"port":null,"ports":null,"status":{"text":"Disconnected","isError":false}})
            );
        });
    }

    #[test_case(false; "opens saved port")]
    #[test_case(true; "saved port fails to open")]
    fn initialize_from_saved_path(fail_open: bool) {
        run(async {
            let dir = tempfile::tempdir().unwrap();
            fs::write(dir.path().join("config.json"), r#"{"port":"saved"}"#).unwrap();
            let app = mock_app(dir.path());
            let calls = Calls::default();
            let result = initialize_ui_state_with(app.handle(), async |path| {
                assert_eq!(path, "saved");
                if fail_open {
                    Err(CameraError::Io(std::io::ErrorKind::NotFound.into()))
                } else {
                    open_fake(path, &calls).await
                }
            })
            .await;
            if fail_open {
                assert!(
                    matches!(result, Err(Error::Camera(CameraError::Io(error))) if error.kind() == std::io::ErrorKind::NotFound)
                );
                assert_eq!(app_snapshot(app.handle())["port"], Value::Null);
            } else {
                result.unwrap();
                assert_eq!(app_snapshot(app.handle())["port"], "saved");
                assert_eq!(
                    app_snapshot(app.handle())["status"],
                    json!({"text":"Connected","isError":false})
                );
            }
            assert_eq!(saved_port(dir.path()), "saved");
            shutdown_ui_state(app.handle()).await.unwrap();
        });
    }

    #[test]
    fn replacement_closes_and_drops_old_adapter_before_opening_and_persists_selection() {
        run(async {
            let dir = tempfile::tempdir().unwrap();
            let app = mock_app(dir.path());
            let calls = Calls::default();
            for path in ["old", "new"] {
                set_ui_state_session_with(app.handle(), Some(path), async |path| {
                    open_fake(path, &calls).await
                })
                .await
                .unwrap();
                assert_eq!(saved_port(dir.path()), path);
                assert_eq!(app_snapshot(app.handle())["port"], path);
            }
            assert_eq!(
                *calls.lock().unwrap(),
                ["open:old", "close:old", "drop:old", "open:new"]
            );
            set_ui_state_session_with(app.handle(), None, async |_| {
                panic!("disconnect must not open")
            })
            .await
            .unwrap();
            assert_eq!(saved_port(dir.path()), Value::Null);
            assert_eq!(
                app_snapshot(app.handle()),
                json!({"port":null,"ports":null,"status":{"text":"Disconnected","isError":false}})
            );
            assert_eq!(
                *calls.lock().unwrap(),
                [
                    "open:old",
                    "close:old",
                    "drop:old",
                    "open:new",
                    "close:new",
                    "drop:new"
                ]
            );
        });
    }

    #[test]
    fn failed_replacement_disconnects_without_overwriting_saved_port() {
        run(async {
            let dir = tempfile::tempdir().unwrap();
            let app = mock_app(dir.path());
            let calls = Calls::default();
            set_ui_state_session_with(app.handle(), Some("old"), async |path| {
                open_fake(path, &calls).await
            })
            .await
            .unwrap();
            let payloads = listen(&app);
            let result = set_ui_state_session_with(app.handle(), Some("missing"), async |_| {
                Err(CameraError::Io(std::io::ErrorKind::NotFound.into()))
            })
            .await;
            assert!(
                matches!(result, Err(Error::Camera(CameraError::Io(error))) if error.kind() == std::io::ErrorKind::NotFound)
            );
            let snapshot = app_snapshot(app.handle());
            assert_eq!(snapshot["port"], Value::Null);
            assert_eq!(snapshot["status"]["isError"], true);
            assert!(
                snapshot["status"]["text"]
                    .as_str()
                    .unwrap()
                    .starts_with("Could not open camera port missing:")
            );
            assert_eq!(payloads.lock().unwrap().as_slice(), &[snapshot]);
            assert_eq!(saved_port(dir.path()), "old");
            assert_eq!(
                *calls.lock().unwrap(),
                ["open:old", "close:old", "drop:old"]
            );
        });
    }

    #[test_case(None; "disconnect")]
    #[test_case(Some("new"); "replacement")]
    fn close_failure_releases_old_adapter_and_completes_switch(path: Option<&str>) {
        run(async {
            let dir = tempfile::tempdir().unwrap();
            fs::write(dir.path().join("config.json"), r#"{"port":"old"}"#).unwrap();
            let app = mock_app(dir.path());
            let calls = Calls::default();
            let mut adapter = FakeAdapter::new("old", &calls);
            adapter.fail_close = true;
            let session = open_adapter(adapter).await.unwrap();
            with_ui(app.handle(), |ui| {
                ui.session = Some(session);
                Ok(())
            })
            .unwrap();
            let payloads = listen(&app);
            set_ui_state_session_with(app.handle(), path, async |path| {
                assert_eq!(
                    *calls.lock().unwrap(),
                    ["open:old", "close:old", "drop:old"]
                );
                open_fake(path, &calls).await
            })
            .await
            .unwrap();
            let snapshot = app_snapshot(app.handle());
            assert_eq!(snapshot["port"], json!(path));
            assert_eq!(saved_port(dir.path()), json!(path));
            assert_eq!(payloads.lock().unwrap().as_slice(), &[snapshot]);
            shutdown_ui_state(app.handle()).await.unwrap();
        });
    }

    #[test]
    fn set_session_returns_store_failure() {
        run(async {
            let dir = tempfile::tempdir().unwrap();
            fs::create_dir(dir.path().join("config.json")).unwrap();
            let app = mock_app(dir.path());
            let calls = Calls::default();
            let payloads = listen(&app);
            let result = set_ui_state_session_with(app.handle(), Some("selected"), async |path| {
                open_fake(path, &calls).await
            })
            .await;
            assert!(matches!(result, Err(Error::Store(_))));
            let snapshot = app_snapshot(app.handle());
            assert_eq!(snapshot["port"], "selected");
            assert_eq!(snapshot["status"]["isError"], true);
            assert!(
                snapshot["status"]["text"]
                    .as_str()
                    .unwrap()
                    .starts_with("Could not save camera port selection:")
            );
            assert_eq!(payloads.lock().unwrap().as_slice(), &[snapshot]);
            shutdown_ui_state(app.handle()).await.unwrap();
        });
    }

    #[test]
    fn shutdown_failure_still_drops_session_and_blocks_reopening() {
        run(async {
            let dir = tempfile::tempdir().unwrap();
            let app = mock_app(dir.path());
            let calls = Calls::default();
            let mut adapter = FakeAdapter::new("old", &calls);
            adapter.fail_close = true;
            let session = open_adapter(adapter).await.unwrap();
            with_ui(app.handle(), |ui| {
                ui.session = Some(session);
                Ok(())
            })
            .unwrap();
            assert!(matches!(
                shutdown_ui_state(app.handle()).await,
                Err(Error::Camera(CameraError::Io(error))) if error.kind() == std::io::ErrorKind::BrokenPipe
            ));
            with_ui(app.handle(), |ui| {
                assert!(ui.exiting);
                assert!(ui.session.is_none());
                Ok(())
            })
            .unwrap();
            set_ui_state_session_with(app.handle(), Some("new"), async |_| {
                panic!("must not reopen")
            })
            .await
            .unwrap();
            shutdown_ui_state(app.handle()).await.unwrap();
            assert_eq!(
                *calls.lock().unwrap(),
                ["open:old", "close:old", "drop:old"]
            );
        });
    }

    fn start_test_motion(app: &tauri::AppHandle<MockRuntime>) -> String {
        with_ui(app, |ui| {
            ui.start_motion(|camera| camera.pan_tilt(Direction::Up))
        })
        .unwrap()
    }

    #[test]
    fn failed_stop_retains_motion_for_retry() {
        run(async {
            let dir = tempfile::tempdir().unwrap();
            let app = mock_app(dir.path());
            let calls = Calls::default();
            let mut adapter = FakeAdapter::new("selected", &calls);
            let (entered, started) = oneshot::channel();
            adapter.motion_started = Some(entered);
            adapter.fail_stop_once = true;
            set_ui_state_session_with(app.handle(), Some("selected"), async |_| {
                open_adapter(adapter).await
            })
            .await
            .unwrap();
            let token = start_test_motion(app.handle());
            started.await.unwrap();

            assert!(matches!(
                stop_ui_state_motion(app.handle(), &token).await,
                Err(Error::Camera(CameraError::Io(error))) if error.kind() == std::io::ErrorKind::BrokenPipe
            ));
            stop_ui_state_motion(app.handle(), &token).await.unwrap();
            stop_ui_state_motion(app.handle(), &token).await.unwrap();

            assert_eq!(
                *calls.lock().unwrap(),
                [
                    "open:selected",
                    "action:selected",
                    "stop:selected",
                    "stop:selected"
                ]
            );
            shutdown_ui_state(app.handle()).await.unwrap();
        });
    }

    #[test_case(false; "newer motion in same session")]
    #[test_case(true; "first motion in replacement session")]
    fn delayed_stop_does_not_clear_new_motion(replace_session: bool) {
        run(async {
            let dir = tempfile::tempdir().unwrap();
            let app = mock_app(dir.path());
            let calls = Calls::default();
            let (entered, stopping) = oneshot::channel();
            let (release, released) = std::sync::mpsc::channel();
            let mut adapter = FakeAdapter::new("old", &calls);
            adapter.stop_gate = Some((entered, released));
            let (entered, started) = oneshot::channel();
            adapter.motion_started = Some(entered);
            set_ui_state_session_with(app.handle(), Some("old"), async |_| {
                open_adapter(adapter).await
            })
            .await
            .unwrap();
            let old_token = start_test_motion(app.handle());
            started.await.unwrap();

            let mut stop = std::pin::pin!(stop_ui_state_motion(app.handle(), &old_token));
            assert!(matches!(
                stop.as_mut().poll(&mut Context::from_waker(Waker::noop())),
                Poll::Pending
            ));
            stopping.await.unwrap();
            release.send(()).unwrap();

            if replace_session {
                set_ui_state_session_with(app.handle(), Some("new"), async |path| {
                    open_fake(path, &calls).await
                })
                .await
                .unwrap();
            }
            // Process the old stop result after the new handle is installed.
            let new_token = start_test_motion(app.handle());
            assert_ne!(old_token, new_token);
            stop.await.unwrap();
            stop_ui_state_motion(app.handle(), &old_token)
                .await
                .unwrap();
            assert_eq!(
                calls
                    .lock()
                    .unwrap()
                    .iter()
                    .filter(|call| call.starts_with("stop:"))
                    .count(),
                1
            );
            stop_ui_state_motion(app.handle(), &new_token)
                .await
                .unwrap();
            let expected = if replace_session {
                "stop:new"
            } else {
                "stop:old"
            };
            {
                let calls = calls.lock().unwrap();
                assert_eq!(
                    calls
                        .iter()
                        .filter(|call| call.starts_with("stop:"))
                        .count(),
                    2
                );
                assert_eq!(calls.last().unwrap(), expected);
            }
            shutdown_ui_state(app.handle()).await.unwrap();
        });
    }

    #[test_case(false; "disconnect")]
    #[test_case(true; "shutdown")]
    fn ending_session_clears_motion_and_invalidates_old_camera_handle(shutdown: bool) {
        run(async {
            let dir = tempfile::tempdir().unwrap();
            let app = mock_app(dir.path());
            let calls = Calls::default();
            let (entered, started) = oneshot::channel();
            let mut adapter = FakeAdapter::new("old", &calls);
            adapter.motion_started = Some(entered);
            set_ui_state_session_with(app.handle(), Some("old"), async |_| {
                open_adapter(adapter).await
            })
            .await
            .unwrap();
            let camera = with_ui(app.handle(), |ui| ui.camera()).unwrap();
            start_test_motion(app.handle());
            started.await.unwrap();
            with_ui(app.handle(), |ui| {
                assert!(ui.motion.is_some());
                Ok(())
            })
            .unwrap();
            if shutdown {
                shutdown_ui_state(app.handle()).await.unwrap();
            } else {
                set_ui_state_session_with(app.handle(), None, async |_| panic!("must not open"))
                    .await
                    .unwrap();
            }
            with_ui(app.handle(), |ui| {
                assert!(ui.motion.is_none());
                assert!(matches!(ui.camera(), Err(Error::NoPortSet)));
                Ok(())
            })
            .unwrap();
            assert!(camera.pan_tilt(Direction::Down).is_err());
            assert_eq!(
                *calls.lock().unwrap(),
                ["open:old", "action:old", "close:old", "drop:old"]
            );
        });
    }

    #[test]
    fn overlapping_switches_are_serialized_without_holding_ui_mutex() {
        run(async {
            let dir = tempfile::tempdir().unwrap();
            let app = mock_app(dir.path());
            let calls = Calls::default();
            let (release, released) = oneshot::channel();
            let first = set_ui_state_session_with(app.handle(), Some("first"), async |path| {
                released.await.unwrap();
                open_fake(path, &calls).await
            });
            let second = set_ui_state_session_with(app.handle(), Some("second"), async |path| {
                open_fake(path, &calls).await
            });
            let mut first = std::pin::pin!(first);
            let mut second = std::pin::pin!(second);
            let mut cx = Context::from_waker(Waker::noop());
            assert!(matches!(first.as_mut().poll(&mut cx), Poll::Pending));
            assert!(matches!(second.as_mut().poll(&mut cx), Poll::Pending));
            assert!(calls.lock().unwrap().is_empty());
            let state = app.state::<AppUIState>();
            assert!(state.try_lock().is_ok(), "UI mutex held while opening");

            release.send(()).unwrap();
            first.await.unwrap();
            second.await.unwrap();
            assert_eq!(
                *calls.lock().unwrap(),
                ["open:first", "close:first", "drop:first", "open:second"]
            );
            assert_eq!(saved_port(dir.path()), "second");
            assert_eq!(app_snapshot(app.handle())["port"], "second");
            shutdown_ui_state(app.handle()).await.unwrap();
        });
    }

    #[test]
    fn shutdown_waits_for_in_flight_switch_then_closes_new_session() {
        run(async {
            let dir = tempfile::tempdir().unwrap();
            let app = mock_app(dir.path());
            let calls = Calls::default();
            let (release, released) = oneshot::channel();
            let switching = set_ui_state_session_with(app.handle(), Some("new"), async |path| {
                released.await.unwrap();
                open_fake(path, &calls).await
            });
            let shutdown = shutdown_ui_state(app.handle());
            let mut switching = std::pin::pin!(switching);
            let mut shutdown = std::pin::pin!(shutdown);
            let mut cx = Context::from_waker(Waker::noop());
            assert!(matches!(switching.as_mut().poll(&mut cx), Poll::Pending));
            assert!(matches!(shutdown.as_mut().poll(&mut cx), Poll::Pending));
            release.send(()).unwrap();
            switching.await.unwrap();
            shutdown.await.unwrap();
            with_ui(app.handle(), |ui| {
                assert!(ui.exiting);
                assert!(ui.session.is_none());
                Ok(())
            })
            .unwrap();
            assert_eq!(
                *calls.lock().unwrap(),
                ["open:new", "close:new", "drop:new"]
            );
            assert_eq!(saved_port(dir.path()), "new");
            // Shutdown is idempotent and must preserve the saved selection.
            shutdown_ui_state(app.handle()).await.unwrap();
            assert_eq!(calls.lock().unwrap().len(), 3);
        });
    }

    #[test]
    fn switch_waiting_behind_shutdown_cannot_reopen_port() {
        run(async {
            let dir = tempfile::tempdir().unwrap();
            fs::write(dir.path().join("config.json"), r#"{"port":"old"}"#).unwrap();
            let app = mock_app(dir.path());
            let calls = Calls::default();
            let (entered, closing) = oneshot::channel();
            let (release, released) = std::sync::mpsc::channel();
            let mut adapter = FakeAdapter::new("old", &calls);
            adapter.close_gate = Some((entered, released));
            let session = open_adapter(adapter).await.unwrap();
            with_ui(app.handle(), |ui| {
                ui.session = Some(session);
                Ok(())
            })
            .unwrap();

            let shutdown = shutdown_ui_state(app.handle());
            let switching = set_ui_state_session_with(app.handle(), Some("new"), async |_| {
                panic!("must not open after shutdown")
            });
            let mut shutdown = std::pin::pin!(shutdown);
            let mut switching = std::pin::pin!(switching);
            let mut cx = Context::from_waker(Waker::noop());
            assert!(matches!(shutdown.as_mut().poll(&mut cx), Poll::Pending));
            closing.await.unwrap();
            assert!(matches!(switching.as_mut().poll(&mut cx), Poll::Pending));
            let state = app.state::<AppUIState>();
            assert!(state.try_lock().is_ok(), "UI mutex held while closing");
            release.send(()).unwrap();
            shutdown.await.unwrap();
            switching.await.unwrap();
            assert_eq!(
                *calls.lock().unwrap(),
                ["open:old", "close:old", "drop:old"]
            );
            assert_eq!(saved_port(dir.path()), "old");
            assert_eq!(app_snapshot(app.handle())["port"], Value::Null);
        });
    }

    #[test]
    fn state_wrappers_emit_success_and_error_snapshots() {
        let dir = tempfile::tempdir().unwrap();
        let app = mock_app(dir.path());
        let payloads = Arc::new(Mutex::new(Vec::new()));
        let received = payloads.clone();
        app.listen(UIStateEvent::NAME, move |event| {
            received
                .lock()
                .unwrap()
                .push(serde_json::from_str::<Value>(event.payload()).unwrap());
        });
        with_ui_emit(app.handle(), |ui| {
            ui.set_status("Ready");
            Ok(())
        })
        .unwrap();
        let result: Result<()> = with_ui_emit(app.handle(), |_| Err(Error::NoPortSet));
        assert!(matches!(result, Err(Error::NoPortSet)));
        let payloads = payloads.lock().unwrap();
        assert_eq!(
            *payloads,
            [
                json!({"port":null,"ports":null,"status":{"text":"Ready","isError":false}}),
                json!({"port":null,"ports":null,"status":{"text":"Error: No port set","isError":true}})
            ]
        );
        assert_eq!(app_snapshot(app.handle()), payloads[1]);
    }

    mod event_publication {
        use super::*;
        use crate::camera::{CameraEvent, CameraMotion, Direction, Zoom};
        use test_case::test_case;

        struct EventAdapter {
            name: String,
            outcome: Option<CommandOutcome>,
        }

        impl CameraSessionAdapter for EventAdapter {
            fn name(&self) -> Option<String> {
                Some(self.name.clone())
            }
            fn handle_action(&mut self, _: &CameraAction) -> CommandResult {
                self.outcome.clone().ok_or_else(|| {
                    CameraError::Protocol(Box::new(std::io::Error::other(
                        "simulated command failure",
                    )))
                })
            }
            fn stop_motion(&mut self) -> crate::camera::Result<()> {
                Ok(())
            }
            fn close(&mut self) -> crate::camera::Result<()> {
                Ok(())
            }
        }

        async fn session_and_event(
            action: CameraAction,
            outcome: Option<CommandOutcome>,
        ) -> (CameraSession, CameraEvent) {
            let (events, mut received) = mpsc::unbounded_channel();
            let session = CameraSessionBuilder::new("selected", events)
                .open_with_adapter(move |path| {
                    Ok(EventAdapter {
                        name: path.into(),
                        outcome,
                    })
                })
                .await
                .unwrap();
            let handle = session.handle();
            match action {
                CameraAction::RecallPreset(number, name) => {
                    let _ = handle.recall_preset(number, &name).await;
                }
                CameraAction::SetPreset(number, name) => {
                    let _ = handle.set_preset(number, &name).await;
                }
                CameraAction::Motion(CameraMotion::PanTilt(direction)) => {
                    let PendingMotion {
                        motion: _,
                        response: started,
                    } = handle.pan_tilt(direction).unwrap();
                    let _ = started.wait().await;
                }
                CameraAction::Motion(CameraMotion::Zoom(direction)) => {
                    let PendingMotion {
                        motion: _,
                        response: started,
                    } = handle.zoom(direction).unwrap();
                    let _ = started.wait().await;
                }
                CameraAction::StopMotion => {
                    let PendingMotion {
                        motion,
                        response: started,
                    } = handle.zoom(Zoom::In).unwrap();
                    let _ = started.wait().await;
                    received.recv().await.unwrap();
                    let _ = motion.stop().await;
                }
            }
            (session, received.recv().await.unwrap())
        }

        fn install_session(app: &tauri::AppHandle<MockRuntime>, session: CameraSession) {
            with_ui(app, |ui| {
                ui.session = Some(session);
                ui.ports = Some(vec!["selected".into(), "other".into()]);
                ui.set_status("Ready");
                Ok(())
            })
            .unwrap();
        }

        #[test_case(CameraAction::RecallPreset(7, "Stage".into()), CommandOutcome::Sent, "Recalling preset Stage"; "recall_sent")]
        #[test_case(CameraAction::RecallPreset(7, "Stage".into()), CommandOutcome::Completed, "Recalled preset Stage"; "recall_completed")]
        #[test_case(CameraAction::SetPreset(7, "Lectern".into()), CommandOutcome::Sent, "Setting preset Lectern"; "save_sent")]
        #[test_case(CameraAction::SetPreset(7, "Lectern".into()), CommandOutcome::Completed, "Set preset Lectern"; "save_completed")]
        #[test_case(CameraAction::Motion(CameraMotion::PanTilt(Direction::Left)), CommandOutcome::Sent, "Moving left"; "pan_sent")]
        #[test_case(CameraAction::Motion(CameraMotion::PanTilt(Direction::Left)), CommandOutcome::Completed, "Moving left"; "pan_completed")]
        #[test_case(CameraAction::Motion(CameraMotion::Zoom(Zoom::In)), CommandOutcome::Sent, "Zooming in"; "zoom_in_sent")]
        #[test_case(CameraAction::Motion(CameraMotion::Zoom(Zoom::In)), CommandOutcome::Completed, "Zooming in"; "zoom_in_completed")]
        #[test_case(CameraAction::Motion(CameraMotion::Zoom(Zoom::Out)), CommandOutcome::Sent, "Zooming out"; "zoom_out_sent")]
        #[test_case(CameraAction::Motion(CameraMotion::Zoom(Zoom::Out)), CommandOutcome::Completed, "Zooming out"; "zoom_out_completed")]
        #[test_case(CameraAction::StopMotion, CommandOutcome::Sent, "Done moving"; "stop_sent")]
        #[test_case(CameraAction::StopMotion, CommandOutcome::Completed, "Done moving"; "stop_completed")]
        fn current_session_success_updates_status_and_emits_one_snapshot(
            action: CameraAction,
            outcome: CommandOutcome,
            message: &str,
        ) {
            run(async {
                let dir = tempfile::tempdir().unwrap();
                let app = mock_app(dir.path());
                let (session, event) = session_and_event(action, Some(outcome)).await;
                let session_id = session.id();
                install_session(app.handle(), session);
                // Success must also clear a previously published error.
                with_ui(app.handle(), |ui| {
                    ui.set_error("Previous failure");
                    Ok(())
                })
                .unwrap();
                let payloads = listen(&app);
                publish_camera_event(app.handle(), event).unwrap();
                let expected = json!({
                    "port":"selected", "ports":["selected","other"],
                    "status":{"text":message,"isError":false}
                });
                assert_eq!(
                    payloads.lock().unwrap().as_slice(),
                    std::slice::from_ref(&expected)
                );
                assert_eq!(app_snapshot(app.handle()), expected);
                with_ui(app.handle(), |ui| {
                    assert!(ui.session_id() == Some(session_id));
                    Ok(())
                })
                .unwrap();
                shutdown_ui_state(app.handle()).await.unwrap();
            });
        }

        #[test_case(CameraAction::RecallPreset(7, "Stage".into()); "recall")]
        #[test_case(CameraAction::SetPreset(7, "Lectern".into()); "save")]
        #[test_case(CameraAction::Motion(CameraMotion::PanTilt(Direction::Left)); "pan")]
        #[test_case(CameraAction::Motion(CameraMotion::Zoom(Zoom::In)); "zoom_in")]
        #[test_case(CameraAction::Motion(CameraMotion::Zoom(Zoom::Out)); "zoom_out")]
        #[test_case(CameraAction::StopMotion; "stop")]
        fn current_session_failure_updates_error_and_emits_one_snapshot(action: CameraAction) {
            run(async {
                let dir = tempfile::tempdir().unwrap();
                let app = mock_app(dir.path());
                let (session, event) = session_and_event(action, None).await;
                install_session(app.handle(), session);
                let payloads = listen(&app);
                publish_camera_event(app.handle(), event).unwrap();
                let expected = json!({
                    "port":"selected", "ports":["selected","other"],
                    "status":{"text":"Camera protocol error: simulated command failure","isError":true}
                });
                assert_eq!(
                    payloads.lock().unwrap().as_slice(),
                    std::slice::from_ref(&expected)
                );
                assert_eq!(app_snapshot(app.handle()), expected);
                shutdown_ui_state(app.handle()).await.unwrap();
            });
        }

        #[test_case(CameraAction::RecallPreset(7, "Stage".into()); "recall")]
        #[test_case(CameraAction::SetPreset(7, "Lectern".into()); "save")]
        #[test_case(CameraAction::Motion(CameraMotion::PanTilt(Direction::Left)); "pan")]
        #[test_case(CameraAction::Motion(CameraMotion::Zoom(Zoom::In)); "zoom_in")]
        #[test_case(CameraAction::Motion(CameraMotion::Zoom(Zoom::Out)); "zoom_out")]
        #[test_case(CameraAction::StopMotion; "stop")]
        fn superseded_current_session_result_preserves_state_and_emits_nothing(
            action: CameraAction,
        ) {
            run(async {
                let dir = tempfile::tempdir().unwrap();
                let app = mock_app(dir.path());
                let (session, event) =
                    session_and_event(action, Some(CommandOutcome::Superceded)).await;
                install_session(app.handle(), session);
                let before = app_snapshot(app.handle());
                let payloads = listen(&app);
                publish_camera_event(app.handle(), event).unwrap();
                assert_eq!(app_snapshot(app.handle()), before);
                assert!(payloads.lock().unwrap().is_empty());
                shutdown_ui_state(app.handle()).await.unwrap();
            });
        }

        #[test_case(false, Some(CommandOutcome::Sent); "disconnected_sent")]
        #[test_case(false, Some(CommandOutcome::Completed); "disconnected_completed")]
        #[test_case(false, None; "disconnected_error")]
        #[test_case(true, Some(CommandOutcome::Sent); "replaced_sent")]
        #[test_case(true, Some(CommandOutcome::Completed); "replaced_completed")]
        #[test_case(true, None; "replaced_error")]
        fn old_session_event_preserves_state_and_emits_nothing(
            replaced: bool,
            outcome: Option<CommandOutcome>,
        ) {
            run(async {
                let dir = tempfile::tempdir().unwrap();
                let app = mock_app(dir.path());
                let (old, event) =
                    session_and_event(CameraAction::RecallPreset(1, "Old".into()), outcome).await;
                let old_id = old.id();
                old.shutdown().await.unwrap();
                if replaced {
                    // Reopening the same path must still reject events from the previous session.
                    let current = open_fake("selected", &Calls::default()).await.unwrap();
                    assert!(current.id() != old_id);
                    install_session(app.handle(), current);
                } else {
                    with_ui(app.handle(), |ui| {
                        ui.set_status("Disconnected");
                        Ok(())
                    })
                    .unwrap();
                }
                let before = app_snapshot(app.handle());
                let payloads = listen(&app);
                publish_camera_event(app.handle(), event).unwrap();
                assert_eq!(app_snapshot(app.handle()), before);
                assert!(payloads.lock().unwrap().is_empty());
                shutdown_ui_state(app.handle()).await.unwrap();
            });
        }
    }
}
