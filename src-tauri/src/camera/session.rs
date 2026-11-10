use super::{
    error::{Error, Result},
    handle::{
        CameraAction, CameraHandle, CameraRequest, CommandOutcome, CommandResult, MotionId,
        RequestAction,
    },
};
use serialport::{DataBits, FlowControl, Parity, SerialPort, StopBits};
use tokio::sync::{mpsc, oneshot};
use uuid::Uuid;

#[derive(Default)]
struct MotionTracker {
    newest_id: Option<MotionId>,
}

impl MotionTracker {
    fn can_start(&self, id: MotionId) -> bool {
        match self.newest_id {
            Some(newest_id) => id > newest_id,
            None => true,
        }
    }

    fn start_motion(&mut self, id: MotionId) -> bool {
        if let Some(newest_id) = self.newest_id
            && id <= newest_id
        {
            return false;
        }

        self.newest_id = Some(id);
        true
    }

    fn stop_motion(&mut self, id: MotionId) -> bool {
        if let Some(newest_id) = self.newest_id
            && id < newest_id
        {
            return false;
        }

        self.newest_id = Some(id);
        true
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct SessionId(Uuid);

pub struct CameraSession {
    id: SessionId,
    name: Option<String>,
    handle: CameraHandle,
    shutdown: oneshot::Sender<()>,
    closed: oneshot::Receiver<Result<()>>,
}

impl CameraSession {
    pub fn id(&self) -> SessionId {
        self.id
    }

    pub fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    pub fn handle(&self) -> CameraHandle {
        self.handle.clone()
    }

    pub async fn shutdown(self) -> Result<()> {
        let _ = self.shutdown.send(());
        self.closed.await.map_err(|_| Error::CameraSessionClosed)?
    }
}

pub type CameraSessionResult = Result<CameraSession>;

pub struct CameraEvent {
    session: SessionId,
    action: CameraAction,
    result: std::result::Result<CommandOutcome, String>,
}

impl CameraEvent {
    pub fn session(&self) -> SessionId {
        self.session
    }

    pub fn action(&self) -> &CameraAction {
        &self.action
    }

    pub fn result(&self) -> &std::result::Result<CommandOutcome, String> {
        &self.result
    }
}

pub type CameraEventSender = mpsc::UnboundedSender<CameraEvent>;

fn finish_request(
    session: SessionId,
    events: &CameraEventSender,
    request: CameraRequest,
    action: CameraAction,
    result: CommandResult,
) {
    let event_result = match &result {
        Ok(outcome) => Ok(outcome.clone()),
        Err(error) => Err(error.to_string()),
    };

    let _ = events.send(CameraEvent {
        session,
        action,
        result: event_result,
    });

    request.reply(result)
}

#[derive(Clone, Copy)]
pub enum RequestLane {
    Action,
    Stop,
}

pub trait CameraSessionAdapter {
    fn name(&self) -> Option<String>;
    fn handle_action(&mut self, action: &CameraAction) -> Result<CommandOutcome>;
    fn stop_motion(&mut self) -> Result<()>;
    fn close(&mut self) -> Result<()>;

    /// Return an immediate result or reserve the lane for a later completion.
    fn dispatch(&mut self, action: &CameraAction, _: RequestLane) -> Option<CommandResult> {
        Some(self.handle_action(action))
    }

    fn can_dispatch_stop(&self) -> bool {
        true
    }

    /// Poll a pending request. An outer error ends the session.
    fn poll_completion(&mut self) -> Result<Option<(RequestLane, CommandResult)>> {
        Ok(None)
    }
}

pub struct CameraSessionBuilder {
    path: String,
    events: CameraEventSender,
    baud_rate: u32,
}

impl CameraSessionBuilder {
    pub fn new(path: impl Into<String>, events: CameraEventSender) -> Self {
        Self {
            path: path.into(),
            events,
            baud_rate: 9600,
        }
    }

    #[allow(dead_code)]
    pub fn baud_rate(mut self, baud_rate: u32) -> Self {
        self.baud_rate = baud_rate;
        self
    }

    pub async fn open<C>(
        self,
        adapter_creator: impl FnOnce(Box<dyn SerialPort>) -> C + Send + 'static,
    ) -> CameraSessionResult
    where
        C: CameraSessionAdapter,
    {
        self.try_open(move |port| Ok(adapter_creator(port))).await
    }

    pub(crate) async fn try_open<C>(
        self,
        adapter_creator: impl FnOnce(Box<dyn SerialPort>) -> Result<C> + Send + 'static,
    ) -> CameraSessionResult
    where
        C: CameraSessionAdapter,
    {
        self.open_with(move |path, baud_rate| {
            let port = serialport::new(path, baud_rate)
                .data_bits(DataBits::Eight)
                .flow_control(FlowControl::None)
                .parity(Parity::None)
                .stop_bits(StopBits::One)
                .open()?;

            adapter_creator(port)
        })
        .await
    }

    /// Opens a fake adapter on the worker without accessing a serial port.
    #[cfg(test)]
    pub(crate) async fn open_with_adapter<C>(
        self,
        adapter_creator: impl FnOnce(&str) -> Result<C> + Send + 'static,
    ) -> CameraSessionResult
    where
        C: CameraSessionAdapter,
    {
        self.open_with(move |path, _baud_rate| adapter_creator(path))
            .await
    }

    async fn open_with<C>(
        self,
        opener: impl FnOnce(&str, u32) -> Result<C> + Send + 'static,
    ) -> CameraSessionResult
    where
        C: CameraSessionAdapter,
    {
        let baud_rate = self.baud_rate;

        create_camera_session(&self.path, self.events, move |path| opener(path, baud_rate)).await
    }
}

async fn create_camera_session<C>(
    path: &str,
    events_tx: CameraEventSender,
    adapter_creator: impl FnOnce(&str) -> Result<C> + Send + 'static,
) -> CameraSessionResult
where
    C: CameraSessionAdapter,
{
    let path = path.to_owned();
    let session_id = SessionId(Uuid::new_v4());

    let (ready_tx, ready_rx) = oneshot::channel();
    let (requests_tx, mut requests_rx) = mpsc::channel::<CameraRequest>(32);
    let (stops_tx, mut stops_rx) = mpsc::unbounded_channel::<CameraRequest>();
    let (shutdown_tx, mut shutdown_rx) = oneshot::channel();
    let (closed_tx, closed_rx) = oneshot::channel();

    std::thread::Builder::new()
        .name("camera-session-worker".into())
        .spawn(move || {
            let mut adapter = match adapter_creator(&path) {
                Ok(adapter) => adapter,
                Err(error) => {
                    let _ = ready_tx.send(Err(error));
                    return;
                }
            };

            let name = adapter.name();

            if ready_tx.send(Ok(name)).is_err() {
                let _ = adapter.close();
                drop(adapter);
                return;
            }

            let mut motion_tracker = MotionTracker::default();

            enum Work {
                Request(RequestLane, CameraRequest),
                Poll,
                Shutdown,
            }

            let mut pending_action = None;
            let mut pending_stop = None;
            let worker_result = loop {
                let idle = pending_action.is_none() && pending_stop.is_none();
                let work = tauri::async_runtime::block_on(async {
                    tokio::select! {
                        biased;

                        _ = &mut shutdown_rx => Work::Shutdown,
                        Some(request) = stops_rx.recv(),
                            if pending_stop.is_none() && adapter.can_dispatch_stop() =>
                            Work::Request(RequestLane::Stop, request),
                        request = requests_rx.recv(), if idle => match request {
                            Some(request) => Work::Request(RequestLane::Action, request),
                            None => Work::Shutdown,
                        },
                        _ = async {}, if !idle => Work::Poll,
                    }
                });

                let (lane, request) = match work {
                    Work::Shutdown => break Ok(()),
                    Work::Poll => {
                        match adapter.poll_completion() {
                            Ok(Some((lane, result))) => {
                                let slot = match lane {
                                    RequestLane::Action => &mut pending_action,
                                    RequestLane::Stop => &mut pending_stop,
                                };
                                if let Some((request, action)) = slot.take() {
                                    finish_request(session_id, &events_tx, request, action, result);
                                }
                            }
                            Ok(None) => {}
                            Err(error) => break Err(error),
                        }
                        continue;
                    }
                    Work::Request(lane, request) => (lane, request),
                };

                let action = request.action().camera_action();
                let early_result = match request.action() {
                    RequestAction::StartMotion { id, .. } if motion_tracker.can_start(*id) => {
                        match adapter.stop_motion() {
                            Ok(()) => {
                                motion_tracker.start_motion(*id);
                                None
                            }
                            Err(error) => Some(Err(error)),
                        }
                    }
                    RequestAction::StopMotion { id } if motion_tracker.stop_motion(*id) => None,
                    RequestAction::StartMotion { .. } | RequestAction::StopMotion { .. } => {
                        Some(Ok(CommandOutcome::Superceded))
                    }
                    RequestAction::Command(_) => None,
                };

                match early_result.or_else(|| adapter.dispatch(&action, lane)) {
                    Some(result) => finish_request(session_id, &events_tx, request, action, result),
                    None => match lane {
                        RequestLane::Action => pending_action = Some((request, action)),
                        RequestLane::Stop => pending_stop = Some((request, action)),
                    },
                }
            };

            for (request, action) in [pending_action, pending_stop].into_iter().flatten() {
                let error = match &worker_result {
                    Ok(()) => Error::CameraSessionClosed,
                    Err(error) => {
                        Error::Protocol(Box::new(std::io::Error::other(error.to_string())))
                    }
                };
                finish_request(session_id, &events_tx, request, action, Err(error));
            }

            drop(requests_rx);
            let result = adapter.close();
            drop(adapter);
            let _ = closed_tx.send(worker_result.and(result));
        })
        .map_err(|_| Error::CameraWorkerStart)?;

    let name = ready_rx.await.map_err(|_| Error::CameraSessionClosed)??;

    Ok(CameraSession {
        id: session_id,
        name,
        handle: CameraHandle::new(requests_tx, stops_tx),
        shutdown: shutdown_tx,
        closed: closed_rx,
    })
}

#[cfg(test)]
mod tests {
    use super::super::handle::{CameraMotion, Direction, PendingMotion, Zoom};
    use super::*;
    use std::sync::Mutex;
    use std::{
        sync::{
            Arc,
            atomic::{AtomicBool, Ordering},
        },
        task::{Context, Poll, Waker},
        time::Duration,
    };
    use test_case::test_case;

    fn run<F: Future>(future: F) -> F::Output {
        tauri::async_runtime::block_on(async {
            tokio::time::timeout(Duration::from_secs(5), future)
                .await
                .expect("session test timed out")
        })
    }

    struct RecordingCamera {
        calls: Arc<Mutex<Vec<&'static str>>>,
        fail_stop: Arc<AtomicBool>,
        name: String,
    }

    impl CameraSessionAdapter for RecordingCamera {
        fn name(&self) -> Option<String> {
            Some(self.name.clone())
        }

        fn handle_action(&mut self, action: &CameraAction) -> Result<CommandOutcome> {
            let call = match action {
                CameraAction::RecallPreset(255, _) => {
                    return Err(Error::Io(std::io::ErrorKind::BrokenPipe.into()));
                }
                CameraAction::RecallPreset(_, _) => "recall",
                CameraAction::SetPreset(_, _) => {
                    self.calls.lock().unwrap().push("save");
                    return Ok(CommandOutcome::Completed);
                }
                CameraAction::Motion(CameraMotion::PanTilt(_)) => "pan",
                CameraAction::Motion(CameraMotion::Zoom(_)) => "zoom",
                CameraAction::StopMotion => "release",
            };
            self.calls.lock().unwrap().push(call);
            Ok(CommandOutcome::Sent)
        }

        fn stop_motion(&mut self) -> Result<()> {
            self.calls.lock().unwrap().push("stop-before-start");
            if self.fail_stop.load(Ordering::Relaxed) {
                return Err(Error::Io(std::io::ErrorKind::BrokenPipe.into()));
            }
            Ok(())
        }

        fn close(&mut self) -> Result<()> {
            self.calls.lock().unwrap().push("closed");
            Ok(())
        }
    }

    impl Drop for RecordingCamera {
        fn drop(&mut self) {
            self.calls.lock().unwrap().push("dropped");
        }
    }

    struct TestSession {
        session: CameraSession,
        events: mpsc::UnboundedReceiver<CameraEvent>,
        calls: Arc<Mutex<Vec<&'static str>>>,
        fail_stop: Arc<AtomicBool>,
    }

    impl TestSession {
        async fn open() -> Self {
            let calls = Arc::new(Mutex::new(Vec::new()));
            let camera_calls = calls.clone();
            let fail_stop = Arc::new(AtomicBool::new(false));
            let worker_fail_stop = fail_stop.clone();
            let (events_tx, events) = mpsc::unbounded_channel();
            let session = create_camera_session("test-port", events_tx, move |path| {
                assert_eq!(path, "test-port");
                camera_calls.lock().unwrap().push("opened");
                Ok(RecordingCamera {
                    calls: camera_calls,
                    fail_stop: worker_fail_stop,
                    name: path.to_owned(),
                })
            })
            .await
            .unwrap();
            Self {
                session,
                events,
                calls,
                fail_stop,
            }
        }
    }

    #[test]
    fn builder_returns_open_error_without_constructing_adapter() {
        run(async {
            let directory = tempfile::tempdir().unwrap();
            let path = directory.path().join("missing-port");
            let (events_tx, mut events) = mpsc::unbounded_channel();

            let result = CameraSessionBuilder::new(path.to_str().unwrap(), events_tx)
                .open(|_| -> RecordingCamera {
                    panic!("adapter must not be constructed when port opening fails")
                })
                .await;

            assert!(matches!(result, Err(Error::SerialPort(_))));
            assert!(events.recv().await.is_none());
        });
    }

    #[test]
    fn builder_owns_path_and_supports_baud_rate_override() {
        let (events_tx, _events) = mpsc::unbounded_channel();
        let mut path = String::from("test-port");
        let builder = CameraSessionBuilder::new(path.as_str(), events_tx);
        path.clear();

        assert_eq!(builder.path, "test-port");
        assert_eq!(builder.baud_rate, 9600);

        let builder = builder.baud_rate(19200);
        assert_eq!(builder.path, "test-port");
        assert_eq!(builder.baud_rate, 19200);
    }

    #[test_case(None, 9600; "default_baud_rate")]
    #[test_case(Some(19200), 19200; "custom_baud_rate")]
    fn builder_opens_adapter_on_worker_with_configured_path_and_baud_rate(
        baud_rate: Option<u32>,
        expected_baud_rate: u32,
    ) {
        run(async {
            let calls = Arc::new(Mutex::new(Vec::new()));
            let adapter_calls = calls.clone();
            let caller_thread = std::thread::current().id();
            let (events_tx, mut events) = mpsc::unbounded_channel();
            let mut builder = CameraSessionBuilder::new("test-port", events_tx);
            if let Some(baud_rate) = baud_rate {
                builder = builder.baud_rate(baud_rate);
            }

            let session = builder
                .open_with(move |path, baud_rate| {
                    assert_ne!(std::thread::current().id(), caller_thread);
                    assert_eq!(path, "test-port");
                    assert_eq!(baud_rate, expected_baud_rate);
                    adapter_calls.lock().unwrap().push("opened");
                    Ok(RecordingCamera {
                        calls: adapter_calls,
                        fail_stop: Arc::new(AtomicBool::new(false)),
                        name: path.to_owned(),
                    })
                })
                .await
                .unwrap();

            assert_eq!(session.name(), Some("test-port"));
            assert_eq!(*calls.lock().unwrap(), ["opened"]);
            assert!(matches!(
                session.handle().set_preset(2, "Stage").await,
                Ok(CommandOutcome::Completed)
            ));
            let event = events.recv().await.unwrap();
            assert!(event.session() == session.id());
            assert!(matches!(event.action(), CameraAction::SetPreset(2, name) if name == "Stage"));
            assert!(matches!(event.result(), Ok(CommandOutcome::Completed)));

            let handle = session.handle();
            session.shutdown().await.unwrap();
            assert_eq!(
                *calls.lock().unwrap(),
                ["opened", "save", "closed", "dropped"]
            );
            assert!(events.recv().await.is_none());
            assert!(matches!(
                handle.recall_preset(1, "Stage").await,
                Err(Error::CameraSessionClosed)
            ));
        });
    }

    #[test]
    fn builder_propagates_injected_opener_failure() {
        run(async {
            let (events_tx, mut events) = mpsc::unbounded_channel();
            let caller_thread = std::thread::current().id();
            let result = CameraSessionBuilder::new("missing-port", events_tx)
                .baud_rate(19200)
                .open_with(move |path, baud_rate| -> Result<RecordingCamera> {
                    assert_ne!(std::thread::current().id(), caller_thread);
                    assert_eq!(path, "missing-port");
                    assert_eq!(baud_rate, 19200);
                    Err(Error::Io(std::io::ErrorKind::BrokenPipe.into()))
                })
                .await;

            assert!(
                matches!(result, Err(Error::Io(error)) if error.kind() == std::io::ErrorKind::BrokenPipe)
            );
            assert!(events.recv().await.is_none());
        });
    }

    #[test]
    fn startup_returns_name_and_unique_identity_and_shutdown_closes_before_returning() {
        run(async {
            let first = TestSession::open().await;
            let second = TestSession::open().await;
            assert_eq!(first.session.name(), Some("test-port"));
            assert!(first.session.id() != second.session.id());
            assert_eq!(*first.calls.lock().unwrap(), ["opened"]);

            let old_handle = first.session.handle();
            first.session.shutdown().await.unwrap();
            assert_eq!(
                *first.calls.lock().unwrap(),
                ["opened", "closed", "dropped"]
            );
            assert!(matches!(
                old_handle.recall_preset(1, "Stage").await,
                Err(Error::CameraSessionClosed)
            ));
            second.session.shutdown().await.unwrap();
        });
    }

    #[test]
    fn open_failure_is_returned_without_dispatch_or_close() {
        run(async {
            let (events_tx, mut events) = mpsc::unbounded_channel();
            let result =
                create_camera_session::<RecordingCamera>("missing-port", events_tx, |path| {
                    assert_eq!(path, "missing-port");
                    Err(Error::Io(std::io::ErrorKind::BrokenPipe.into()))
                })
                .await;
            assert!(
                matches!(result, Err(Error::Io(error)) if error.kind() == std::io::ErrorKind::BrokenPipe)
            );
            assert!(events.recv().await.is_none());
        });
    }

    #[test]
    fn requests_publish_ordered_events_with_session_action_and_outcome() {
        run(async {
            let mut test = TestSession::open().await;
            let handle = test.session.handle();
            assert!(matches!(
                handle.recall_preset(4, "Baptistry").await,
                Ok(CommandOutcome::Sent)
            ));
            assert!(matches!(
                handle.set_preset(2, "Stage").await,
                Ok(CommandOutcome::Completed)
            ));
            let recall = test.events.recv().await.unwrap();
            let save = test.events.recv().await.unwrap();
            assert!(recall.session() == test.session.id());
            assert!(save.session() == test.session.id());
            assert!(
                matches!(recall.action(), CameraAction::RecallPreset(4, name) if name == "Baptistry")
            );
            assert!(matches!(save.action(), CameraAction::SetPreset(2, name) if name == "Stage"));
            assert!(matches!(recall.result(), Ok(CommandOutcome::Sent)));
            assert!(matches!(save.result(), Ok(CommandOutcome::Completed)));
            test.session.shutdown().await.unwrap();
            assert!(test.events.recv().await.is_none());
        });
    }

    #[test]
    fn action_failure_publishes_error_and_worker_accepts_next_request() {
        run(async {
            let mut test = TestSession::open().await;
            let handle = test.session.handle();
            assert!(matches!(
                handle.recall_preset(255, "Invalid").await,
                Err(Error::Io(error)) if error.kind() == std::io::ErrorKind::BrokenPipe
            ));
            let event = test.events.recv().await.unwrap();
            assert!(event.session() == test.session.id());
            assert!(
                matches!(event.result(), Err(message) if *message == Error::Io(std::io::ErrorKind::BrokenPipe.into()).to_string())
            );
            assert!(matches!(
                handle.recall_preset(1, "Stage").await,
                Ok(CommandOutcome::Sent)
            ));
            test.session.shutdown().await.unwrap();
        });
    }

    #[test]
    fn newer_motion_stops_previous_motion_and_supersedes_older_release() {
        run(async {
            let mut test = TestSession::open().await;
            let handle = test.session.handle();
            let PendingMotion {
                motion: first,
                response: started,
            } = handle.pan_tilt(Direction::Left).unwrap();
            assert!(matches!(started.wait().await, Ok(CommandOutcome::Sent)));
            let PendingMotion {
                motion: second,
                response: started,
            } = handle.zoom(Zoom::In).unwrap();
            assert!(matches!(started.wait().await, Ok(CommandOutcome::Sent)));
            assert!(matches!(first.stop().await, Ok(CommandOutcome::Superceded)));
            assert!(matches!(second.stop().await, Ok(CommandOutcome::Sent)));
            assert_eq!(
                *test.calls.lock().unwrap(),
                [
                    "opened",
                    "stop-before-start",
                    "pan",
                    "stop-before-start",
                    "zoom",
                    "release",
                ]
            );
            let _first = test.events.recv().await.unwrap();
            let _second = test.events.recv().await.unwrap();
            let stale_stop = test.events.recv().await.unwrap();
            assert!(matches!(stale_stop.action(), CameraAction::StopMotion));
            assert!(matches!(
                stale_stop.result(),
                Ok(CommandOutcome::Superceded)
            ));
            test.session.shutdown().await.unwrap();
        });
    }

    #[test]
    fn failed_replacement_stop_preserves_previous_motion_id_and_publishes_failure() {
        run(async {
            let mut test = TestSession::open().await;
            let handle = test.session.handle();
            let PendingMotion {
                motion: first,
                response: started,
            } = handle.pan_tilt(Direction::Right).unwrap();
            assert!(matches!(started.wait().await, Ok(CommandOutcome::Sent)));
            test.fail_stop.store(true, Ordering::Relaxed);
            let PendingMotion {
                motion: _second,
                response: started,
            } = handle.zoom(Zoom::Out).unwrap();
            assert!(
                matches!(started.wait().await, Err(Error::Io(error)) if error.kind() == std::io::ErrorKind::BrokenPipe)
            );
            assert!(matches!(first.stop().await, Ok(CommandOutcome::Sent)));
            assert_eq!(
                *test.calls.lock().unwrap(),
                [
                    "opened",
                    "stop-before-start",
                    "pan",
                    "stop-before-start",
                    "release",
                ]
            );
            let _first = test.events.recv().await.unwrap();
            let failure = test.events.recv().await.unwrap();
            assert!(matches!(
                failure.action(),
                CameraAction::Motion(CameraMotion::Zoom(Zoom::Out))
            ));
            assert!(
                matches!(failure.result(), Err(message) if *message == Error::Io(std::io::ErrorKind::BrokenPipe.into()).to_string())
            );
            test.session.shutdown().await.unwrap();
        });
    }

    #[test_case(false; "priority_release")]
    #[test_case(true; "priority_shutdown")]
    fn control_requests_take_priority_over_queued_motion(shutdown: bool) {
        struct BlockingAdapter {
            calls: Arc<Mutex<Vec<&'static str>>>,
            entered: Option<oneshot::Sender<()>>,
            release: std::sync::mpsc::Receiver<()>,
        }

        impl CameraSessionAdapter for BlockingAdapter {
            fn name(&self) -> Option<String> {
                None
            }

            fn handle_action(&mut self, action: &CameraAction) -> Result<CommandOutcome> {
                match action {
                    CameraAction::RecallPreset(_, _) => {
                        let _ = self.entered.take().unwrap().send(());
                        self.release.recv_timeout(Duration::from_secs(5)).unwrap();
                        self.calls.lock().unwrap().push("recall");
                    }
                    CameraAction::StopMotion => self.calls.lock().unwrap().push("release"),
                    _ => panic!("released or shutdown motion must not execute"),
                }
                Ok(CommandOutcome::Sent)
            }

            fn stop_motion(&mut self) -> Result<()> {
                panic!("queued motion must not reach pre-start stop")
            }

            fn close(&mut self) -> Result<()> {
                Ok(())
            }
        }

        impl Drop for BlockingAdapter {
            fn drop(&mut self) {
                self.calls.lock().unwrap().push("dropped");
            }
        }

        run(async {
            let (events_tx, mut events) = mpsc::unbounded_channel();
            let (entered_tx, entered_rx) = oneshot::channel();
            let (release_tx, release_rx) = std::sync::mpsc::sync_channel(1);
            let calls = Arc::new(Mutex::new(Vec::new()));
            let camera_calls = calls.clone();
            let session = create_camera_session("priority-port", events_tx, move |path| {
                assert_eq!(path, "priority-port");
                Ok(BlockingAdapter {
                    calls: camera_calls,
                    entered: Some(entered_tx),
                    release: release_rx,
                })
            })
            .await
            .unwrap();
            let handle = session.handle();
            let busy_handle = handle.clone();
            let busy =
                tauri::async_runtime::spawn(
                    async move { busy_handle.recall_preset(1, "Stage").await },
                );
            entered_rx.await.unwrap();
            let PendingMotion {
                motion,
                response: started,
            } = handle.zoom(Zoom::In).unwrap();

            // Poll once to enqueue the control request before unblocking the worker.
            // This establishes ordering without sleeps or scheduler assumptions.
            if shutdown {
                let mut closing = std::pin::pin!(session.shutdown());
                assert!(matches!(
                    closing
                        .as_mut()
                        .poll(&mut Context::from_waker(Waker::noop())),
                    Poll::Pending
                ));
                release_tx.send(()).unwrap();
                closing.await.unwrap();
                assert!(matches!(
                    started.wait().await,
                    Err(Error::CameraResponseReceiveFailed)
                ));
                assert_eq!(*calls.lock().unwrap(), ["recall", "dropped"]);
                assert!(matches!(
                    handle.set_preset(1, "Stage").await,
                    Err(Error::CameraSessionClosed)
                ));
            } else {
                let mut stopping = std::pin::pin!(motion.stop());
                assert!(matches!(
                    stopping
                        .as_mut()
                        .poll(&mut Context::from_waker(Waker::noop())),
                    Poll::Pending
                ));
                release_tx.send(()).unwrap();
                assert!(matches!(stopping.await, Ok(CommandOutcome::Sent)));
                assert!(matches!(
                    started.wait().await,
                    Ok(CommandOutcome::Superceded)
                ));
                session.shutdown().await.unwrap();
                assert_eq!(*calls.lock().unwrap(), ["recall", "release", "dropped"]);
                let _recall = events.recv().await.unwrap();
                assert!(matches!(
                    events.recv().await.unwrap().action(),
                    CameraAction::StopMotion
                ));
                assert!(matches!(
                    events.recv().await.unwrap().result(),
                    Ok(CommandOutcome::Superceded)
                ));
            }
            assert!(matches!(busy.await.unwrap(), Ok(CommandOutcome::Sent)));
        });
    }

    #[test_case(false; "close_error")]
    #[test_case(true; "lost_close_acknowledgment")]
    fn shutdown_reports_cleanup_failure(close_panics: bool) {
        struct FailingCloseAdapter {
            close_panics: bool,
            dropped: Arc<AtomicBool>,
        }

        impl CameraSessionAdapter for FailingCloseAdapter {
            fn name(&self) -> Option<String> {
                None
            }

            fn handle_action(&mut self, _: &CameraAction) -> Result<CommandOutcome> {
                panic!("no action expected")
            }

            fn stop_motion(&mut self) -> Result<()> {
                panic!("no motion expected")
            }

            fn close(&mut self) -> Result<()> {
                if self.close_panics {
                    panic!("simulated cleanup panic");
                }
                Err(Error::Io(std::io::ErrorKind::BrokenPipe.into()))
            }
        }

        impl Drop for FailingCloseAdapter {
            fn drop(&mut self) {
                self.dropped.store(true, Ordering::Relaxed);
            }
        }

        run(async {
            let (events_tx, _events) = mpsc::unbounded_channel();
            let dropped = Arc::new(AtomicBool::new(false));
            let adapter_dropped = dropped.clone();
            let session = create_camera_session("close-test-port", events_tx, move |_| {
                Ok(FailingCloseAdapter {
                    close_panics,
                    dropped: adapter_dropped,
                })
            })
            .await
            .unwrap();
            let result = session.shutdown().await;
            match close_panics {
                true => assert!(matches!(result, Err(Error::CameraSessionClosed))),
                false => assert!(
                    matches!(result, Err(Error::Io(error)) if error.kind() == std::io::ErrorKind::BrokenPipe)
                ),
            }
            assert!(dropped.load(Ordering::Relaxed));
        });
    }

    #[test]
    fn dropped_event_consumer_does_not_prevent_reply_or_shutdown() {
        run(async {
            let test = TestSession::open().await;
            drop(test.events);
            assert!(matches!(
                test.session.handle().recall_preset(1, "Stage").await,
                Ok(CommandOutcome::Sent)
            ));
            test.session.shutdown().await.unwrap();
        });
    }

    #[test]
    fn cancelling_startup_closes_and_drops_adapter_when_open_finishes() {
        run(async {
            let (events_tx, mut events) = mpsc::unbounded_channel();
            let (opening_tx, opening_rx) = oneshot::channel();
            let (release_tx, release_rx) = std::sync::mpsc::sync_channel(1);
            let calls = Arc::new(Mutex::new(Vec::new()));
            let camera_calls = calls.clone();
            // Box::pin lets us drop the actual future, not just a pinned reference.
            let mut opening = Box::pin(create_camera_session(
                "cancelled-startup",
                events_tx,
                move |_| {
                    opening_tx.send(()).unwrap();
                    release_rx.recv_timeout(Duration::from_secs(5)).unwrap();
                    camera_calls.lock().unwrap().push("opened");
                    Ok(RecordingCamera {
                        calls: camera_calls,
                        fail_stop: Arc::new(AtomicBool::new(false)),
                        name: "cancelled-startup".into(),
                    })
                },
            ));
            assert!(matches!(
                opening
                    .as_mut()
                    .poll(&mut Context::from_waker(Waker::noop())),
                Poll::Pending
            ));
            opening_rx.await.unwrap();
            drop(opening);
            release_tx.send(()).unwrap();

            // Sender closure establishes that the worker has finished cleanup.
            assert!(events.recv().await.is_none());
            assert_eq!(*calls.lock().unwrap(), ["opened", "closed", "dropped"]);
        });
    }

    #[test]
    fn dropping_session_shuts_down_worker_even_with_live_camera_handles() {
        run(async {
            let TestSession {
                session,
                mut events,
                calls,
                ..
            } = TestSession::open().await;
            let handle = session.handle();
            let cloned_handle = handle.clone();
            drop(session);

            assert!(events.recv().await.is_none());
            assert_eq!(*calls.lock().unwrap(), ["opened", "closed", "dropped"]);
            assert!(matches!(
                handle.recall_preset(1, "Stage").await,
                Err(Error::CameraSessionClosed)
            ));
            assert!(matches!(
                cloned_handle.zoom(Zoom::In),
                Err(Error::CameraSessionClosed)
            ));
            assert!(matches!(
                handle.pan_tilt(Direction::Left),
                Err(Error::CameraSessionClosed)
            ));
        });
    }

    #[test]
    fn shutdown_finishes_in_flight_request_and_cancels_both_pending_queues() {
        struct BlockingCamera {
            camera: RecordingCamera,
            entered: Option<oneshot::Sender<()>>,
            release: std::sync::mpsc::Receiver<()>,
        }

        impl CameraSessionAdapter for BlockingCamera {
            fn name(&self) -> Option<String> {
                self.camera.name()
            }

            fn handle_action(&mut self, action: &CameraAction) -> Result<CommandOutcome> {
                assert!(
                    matches!(action, CameraAction::RecallPreset(1, _)),
                    "queued request must not execute"
                );
                self.entered.take().unwrap().send(()).unwrap();
                self.release.recv_timeout(Duration::from_secs(5)).unwrap();
                self.camera.handle_action(action)
            }

            fn stop_motion(&mut self) -> Result<()> {
                panic!("queued motion must not execute");
            }

            fn close(&mut self) -> Result<()> {
                self.camera.close()
            }
        }

        run(async {
            let (events_tx, mut events) = mpsc::unbounded_channel();
            let (entered_tx, entered_rx) = oneshot::channel();
            let (release_tx, release_rx) = std::sync::mpsc::sync_channel(1);
            let calls = Arc::new(Mutex::new(Vec::new()));
            let camera_calls = calls.clone();
            let session = create_camera_session("pending-requests", events_tx, move |path| {
                Ok(BlockingCamera {
                    camera: RecordingCamera {
                        calls: camera_calls,
                        fail_stop: Arc::new(AtomicBool::new(false)),
                        name: path.into(),
                    },
                    entered: Some(entered_tx),
                    release: release_rx,
                })
            })
            .await
            .unwrap();
            let session_id = session.id();
            let handle = session.handle();
            let mut active = std::pin::pin!(handle.recall_preset(1, "Stage"));
            let mut cx = Context::from_waker(Waker::noop());
            assert!(matches!(active.as_mut().poll(&mut cx), Poll::Pending));
            entered_rx.await.unwrap();

            let mut preset = std::pin::pin!(handle.set_preset(2, "Lectern"));
            assert!(matches!(preset.as_mut().poll(&mut cx), Poll::Pending));
            let PendingMotion {
                motion,
                response: started,
            } = handle.zoom(Zoom::In).unwrap();
            let mut stopping = std::pin::pin!(motion.stop());
            assert!(matches!(stopping.as_mut().poll(&mut cx), Poll::Pending));
            let mut closing = std::pin::pin!(session.shutdown());
            assert!(matches!(closing.as_mut().poll(&mut cx), Poll::Pending));
            assert!(
                calls.lock().unwrap().is_empty(),
                "cleanup must wait for in-flight action"
            );

            release_tx.send(()).unwrap();
            closing.await.unwrap();
            assert!(matches!(active.await, Ok(CommandOutcome::Sent)));
            assert!(matches!(
                preset.await,
                Err(Error::CameraResponseReceiveFailed)
            ));
            assert!(matches!(
                started.wait().await,
                Err(Error::CameraResponseReceiveFailed)
            ));
            assert!(matches!(
                stopping.await,
                Err(Error::CameraResponseReceiveFailed)
            ));
            assert_eq!(*calls.lock().unwrap(), ["recall", "closed", "dropped"]);

            let event = events.recv().await.unwrap();
            assert!(event.session() == session_id);
            assert!(matches!(event.action(), CameraAction::RecallPreset(1, _)));
            assert!(matches!(event.result(), Ok(CommandOutcome::Sent)));
            // Discarded requests get no completion event because they never executed.
            assert!(events.recv().await.is_none());
            assert!(matches!(
                handle.set_preset(3, "After shutdown").await,
                Err(Error::CameraSessionClosed)
            ));
            assert!(matches!(
                motion.stop().await,
                Err(Error::CameraSessionClosed)
            ));
        });
    }

    #[test]
    fn startup_and_shutdown_wait_for_their_callbacks() {
        struct GatedCloseAdapter {
            closing: Option<oneshot::Sender<()>>,
            allow_close: std::sync::mpsc::Receiver<()>,
            dropped: Arc<AtomicBool>,
        }

        impl CameraSessionAdapter for GatedCloseAdapter {
            fn name(&self) -> Option<String> {
                Some("ready".into())
            }

            fn handle_action(&mut self, _: &CameraAction) -> Result<CommandOutcome> {
                panic!("no action expected")
            }

            fn stop_motion(&mut self) -> Result<()> {
                panic!("no motion expected")
            }

            fn close(&mut self) -> Result<()> {
                let _ = self.closing.take().unwrap().send(());
                self.allow_close
                    .recv_timeout(Duration::from_secs(5))
                    .unwrap();
                Ok(())
            }
        }

        impl Drop for GatedCloseAdapter {
            fn drop(&mut self) {
                self.dropped.store(true, Ordering::Relaxed);
            }
        }

        run(async {
            let (events_tx, _events) = mpsc::unbounded_channel();
            let (opening_tx, opening_rx) = oneshot::channel();
            let (allow_open_tx, allow_open_rx) = std::sync::mpsc::sync_channel(1);
            let (closing_tx, closing_rx) = oneshot::channel();
            let (allow_close_tx, allow_close_rx) = std::sync::mpsc::sync_channel(1);
            let dropped = Arc::new(AtomicBool::new(false));
            let adapter_dropped = dropped.clone();
            let mut opening = std::pin::pin!(create_camera_session(
                "lifecycle-port",
                events_tx,
                move |path| {
                    assert_eq!(path, "lifecycle-port");
                    let _ = opening_tx.send(());
                    allow_open_rx.recv_timeout(Duration::from_secs(5)).unwrap();
                    Ok(GatedCloseAdapter {
                        closing: Some(closing_tx),
                        allow_close: allow_close_rx,
                        dropped: adapter_dropped,
                    })
                },
            ));

            assert!(matches!(
                opening
                    .as_mut()
                    .poll(&mut Context::from_waker(Waker::noop())),
                Poll::Pending
            ));
            opening_rx.await.unwrap();
            assert!(matches!(
                opening
                    .as_mut()
                    .poll(&mut Context::from_waker(Waker::noop())),
                Poll::Pending
            ));
            allow_open_tx.send(()).unwrap();
            let session = opening.await.unwrap();
            assert_eq!(session.name(), Some("ready"));

            let mut closing = std::pin::pin!(session.shutdown());
            assert!(matches!(
                closing
                    .as_mut()
                    .poll(&mut Context::from_waker(Waker::noop())),
                Poll::Pending
            ));
            closing_rx.await.unwrap();
            assert!(!dropped.load(Ordering::Relaxed));
            assert!(matches!(
                closing
                    .as_mut()
                    .poll(&mut Context::from_waker(Waker::noop())),
                Poll::Pending
            ));
            allow_close_tx.send(()).unwrap();
            closing.await.unwrap();
            assert!(dropped.load(Ordering::Relaxed));
        });
    }
}
