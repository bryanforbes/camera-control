use super::error::{Error, Result};
use serde::{Deserialize, Serialize};
use specta::Type;
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};
use tokio::sync::{
    mpsc::{self, error::TrySendError},
    oneshot,
};

#[derive(Copy, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "lowercase")]
pub enum Direction {
    Down,
    Up,
    Left,
    Right,
}

impl std::fmt::Display for Direction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Direction::Down => write!(f, "down"),
            Direction::Up => write!(f, "up"),
            Direction::Left => write!(f, "left"),
            Direction::Right => write!(f, "right"),
        }
    }
}

#[derive(Copy, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "lowercase")]
pub enum Zoom {
    In,
    Out,
}

impl std::fmt::Display for Zoom {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Zoom::In => write!(f, "in"),
            Zoom::Out => write!(f, "out"),
        }
    }
}

#[derive(Clone)]
pub enum CommandOutcome {
    #[allow(dead_code)]
    Sent,
    Completed,
    Superceded,
}

pub type CommandResult = Result<CommandOutcome>;

#[derive(Copy, Clone)]
pub enum CameraMotion {
    PanTilt(Direction),
    Zoom(Zoom),
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct MotionId(u64);

#[derive(Clone)]
pub enum CameraAction {
    RecallPreset(u8, String),
    SetPreset(u8, String),
    Motion(CameraMotion),
    StopMotion,
}

pub(super) enum RequestAction {
    Command(CameraAction),
    StartMotion { id: MotionId, motion: CameraMotion },
    StopMotion { id: MotionId },
}

impl RequestAction {
    pub(super) fn camera_action(&self) -> CameraAction {
        match self {
            Self::Command(action) => action.clone(),
            Self::StartMotion { motion, .. } => CameraAction::Motion(*motion),
            Self::StopMotion { .. } => CameraAction::StopMotion,
        }
    }
}

pub(super) struct CameraRequest {
    action: RequestAction,
    reply: oneshot::Sender<CommandResult>,
}

impl CameraRequest {
    pub(super) fn action(&self) -> &RequestAction {
        &self.action
    }

    pub(super) fn reply(self, outcome: CommandResult) {
        let _ = self.reply.send(outcome);
    }
}

#[derive(Clone)]
pub struct CameraHandle {
    requests: mpsc::Sender<CameraRequest>,
    stops: mpsc::UnboundedSender<CameraRequest>,
    next_motion_id: Arc<AtomicU64>,
}

#[derive(Clone)]
pub struct MotionHandle {
    id: MotionId,
    camera: CameraHandle,
}

impl MotionHandle {
    pub async fn stop(&self) -> CommandResult {
        let (reply_tx, reply_rx) = oneshot::channel();

        self.camera
            .stops
            .send(CameraRequest {
                action: RequestAction::StopMotion { id: self.id },
                reply: reply_tx,
            })
            .map_err(|_| Error::CameraSessionClosed)?;

        reply_rx
            .await
            .map_err(|_| Error::CameraResponseReceiveFailed)?
    }
}

// Motion registration uses worker events. Tests can also await the direct reply.
#[cfg_attr(not(test), allow(dead_code))]
pub struct CommandResponse {
    receiver: oneshot::Receiver<CommandResult>,
}

#[cfg(test)]
impl CommandResponse {
    pub async fn wait(self) -> CommandResult {
        self.receiver
            .await
            .map_err(|_| Error::CameraResponseReceiveFailed)?
    }
}

pub struct PendingMotion {
    pub motion: MotionHandle,
    pub response: CommandResponse,
}

pub type PendingMotionResult = Result<PendingMotion>;

impl CameraHandle {
    pub(super) fn new(
        requests: mpsc::Sender<CameraRequest>,
        stops: mpsc::UnboundedSender<CameraRequest>,
    ) -> Self {
        CameraHandle {
            requests,
            stops,
            next_motion_id: Arc::new(AtomicU64::new(1)),
        }
    }

    async fn send(&self, action: CameraAction) -> CommandResult {
        let (reply_tx, reply_rx) = oneshot::channel();

        self.requests
            .send(CameraRequest {
                action: RequestAction::Command(action),
                reply: reply_tx,
            })
            .await
            .map_err(|_| Error::CameraSessionClosed)?;

        reply_rx
            .await
            .map_err(|_| Error::CameraResponseReceiveFailed)?
    }

    pub async fn recall_preset(&self, preset: u8, name: &str) -> CommandResult {
        self.send(CameraAction::RecallPreset(preset, name.to_owned()))
            .await
    }

    pub async fn set_preset(&self, preset: u8, name: &str) -> CommandResult {
        self.send(CameraAction::SetPreset(preset, name.to_owned()))
            .await
    }

    fn start_motion(&self, motion: CameraMotion) -> Result<PendingMotion> {
        let id = self
            .next_motion_id
            .try_update(Ordering::Relaxed, Ordering::Relaxed, |id| id.checked_add(1))
            .map_err(|_| Error::CameraMotionIdsExhausted)?;

        let (reply_tx, reply_rx) = oneshot::channel();

        self.requests
            .try_send(CameraRequest {
                action: RequestAction::StartMotion {
                    id: MotionId(id),
                    motion,
                },
                reply: reply_tx,
            })
            .map_err(|error| match error {
                TrySendError::Full(_) => Error::CameraBusy,
                TrySendError::Closed(_) => Error::CameraSessionClosed,
            })?;

        let handle = MotionHandle {
            id: MotionId(id),
            camera: self.clone(),
        };

        let response = CommandResponse { receiver: reply_rx };

        Ok(PendingMotion {
            motion: handle,
            response,
        })
    }

    pub fn pan_tilt(&self, direction: Direction) -> Result<PendingMotion> {
        self.start_motion(CameraMotion::PanTilt(direction))
    }

    pub fn zoom(&self, zoom: Zoom) -> Result<PendingMotion> {
        self.start_motion(CameraMotion::Zoom(zoom))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    use test_case::test_case;

    fn channels(
        capacity: usize,
    ) -> (
        CameraHandle,
        mpsc::Receiver<CameraRequest>,
        mpsc::UnboundedReceiver<CameraRequest>,
    ) {
        let (requests, incoming) = mpsc::channel(capacity);
        let (stops, incoming_stops) = mpsc::unbounded_channel();
        (CameraHandle::new(requests, stops), incoming, incoming_stops)
    }

    fn run<F: Future>(future: F) -> F::Output {
        tauri::async_runtime::block_on(async {
            tokio::time::timeout(Duration::from_secs(5), future)
                .await
                .expect("channel exchange timed out")
        })
    }

    #[test_case(true; "recall")]
    #[test_case(false; "save")]
    fn preset_requests_preserve_number_name_and_worker_outcome(recall: bool) {
        let (handle, mut incoming, _stops) = channels(1);

        run(async {
            let operation = async {
                if recall {
                    handle.recall_preset(4, "Baptistry").await
                } else {
                    handle.set_preset(4, "Baptistry").await
                }
            };
            let worker = async {
                let request = incoming.recv().await.unwrap();
                match (recall, request.action()) {
                    (true, RequestAction::Command(CameraAction::RecallPreset(4, name)))
                    | (false, RequestAction::Command(CameraAction::SetPreset(4, name))) => {
                        assert_eq!(name, "Baptistry");
                    }
                    _ => panic!("incorrect preset action"),
                }
                request.reply(Ok(CommandOutcome::Completed));
            };

            let (result, ()) = tokio::join!(operation, worker);
            assert!(matches!(result, Ok(CommandOutcome::Completed)));
        });
    }

    #[test]
    fn motion_starts_enqueue_immediately_and_share_ids_across_handle_clones() {
        let (handle, mut incoming, _stops) = channels(3);
        let clone = handle.clone();

        let PendingMotion {
            motion: first,
            response: first_started,
        } = handle.pan_tilt(Direction::Left).unwrap();
        let PendingMotion {
            motion: second,
            response: second_started,
        } = clone.zoom(Zoom::In).unwrap();
        let PendingMotion {
            motion: third,
            response: third_started,
        } = handle.pan_tilt(Direction::Up).unwrap();

        // No start-result future has been polled yet.
        let first_request = incoming.try_recv().unwrap();
        let second_request = incoming.try_recv().unwrap();
        let third_request = incoming.try_recv().unwrap();
        assert!(matches!(
            first_request.action(),
            RequestAction::StartMotion { id, motion: CameraMotion::PanTilt(Direction::Left) }
                if *id == first.id
        ));
        assert!(matches!(
            second_request.action(),
            RequestAction::StartMotion { id, motion: CameraMotion::Zoom(Zoom::In) }
                if *id == second.id
        ));
        assert!(matches!(
            third_request.action(),
            RequestAction::StartMotion { id, motion: CameraMotion::PanTilt(Direction::Up) }
                if *id == third.id
        ));
        assert!(first.id < second.id && second.id < third.id);

        first_request.reply(Ok(CommandOutcome::Sent));
        second_request.reply(Ok(CommandOutcome::Superceded));
        third_request.reply(Err(Error::Io(std::io::ErrorKind::BrokenPipe.into())));
        run(async {
            assert!(matches!(
                first_started.wait().await,
                Ok(CommandOutcome::Sent)
            ));
            assert!(matches!(
                second_started.wait().await,
                Ok(CommandOutcome::Superceded)
            ));
            assert!(
                matches!(third_started.wait().await, Err(Error::Io(error)) if error.kind() == std::io::ErrorKind::BrokenPipe)
            );
        });
    }

    #[test]
    fn stop_uses_separate_channel_while_start_is_pending_and_request_queue_is_full() {
        let (handle, mut incoming, mut stops) = channels(1);
        let PendingMotion {
            motion,
            response: started,
        } = handle.zoom(Zoom::Out).unwrap();

        run(async {
            let worker = async {
                let stop = stops.recv().await.unwrap();
                assert!(
                    matches!(stop.action(), RequestAction::StopMotion { id } if *id == motion.id)
                );
                stop.reply(Ok(CommandOutcome::Sent));

                // Start was still queued when Stop was received.
                incoming
                    .try_recv()
                    .unwrap()
                    .reply(Ok(CommandOutcome::Superceded));
            };
            let (result, ()) = tokio::join!(motion.stop(), worker);
            assert!(matches!(result, Ok(CommandOutcome::Sent)));
            assert!(matches!(
                started.wait().await,
                Ok(CommandOutcome::Superceded)
            ));
        });
    }

    #[test_case(false; "pan_tilt")]
    #[test_case(true; "zoom")]
    fn full_request_queue_reports_busy_without_invalidating_existing_start(zoom: bool) {
        let (handle, mut incoming, _stops) = channels(1);
        let PendingMotion {
            motion: _motion,
            response: started,
        } = handle.pan_tilt(Direction::Right).unwrap();

        if zoom {
            assert!(matches!(handle.zoom(Zoom::In), Err(Error::CameraBusy)));
        } else {
            assert!(matches!(
                handle.pan_tilt(Direction::Left),
                Err(Error::CameraBusy)
            ));
        }
        incoming.try_recv().unwrap().reply(Ok(CommandOutcome::Sent));
        run(async {
            assert!(matches!(started.wait().await, Ok(CommandOutcome::Sent)));
            // A full queue is temporary: the same handle works once capacity is available.
            let PendingMotion {
                motion: _motion,
                response: started,
            } = handle.zoom(Zoom::Out).unwrap();
            incoming
                .try_recv()
                .unwrap()
                .reply(Ok(CommandOutcome::Completed));
            assert!(matches!(
                started.wait().await,
                Ok(CommandOutcome::Completed)
            ));
        });
    }

    #[test]
    fn exhausted_motion_ids_do_not_wrap_or_enqueue() {
        let (handle, mut incoming, _stops) = channels(1);
        handle.next_motion_id.store(u64::MAX - 1, Ordering::Relaxed);

        let PendingMotion {
            motion,
            response: _started,
        } = handle.zoom(Zoom::In).unwrap();
        assert_eq!(motion.id.0, u64::MAX - 1);
        let _request = incoming.try_recv().unwrap();

        assert!(matches!(
            handle.pan_tilt(Direction::Down),
            Err(Error::CameraMotionIdsExhausted)
        ));
        assert!(matches!(
            incoming.try_recv(),
            Err(mpsc::error::TryRecvError::Empty)
        ));
        assert_eq!(handle.next_motion_id.load(Ordering::Relaxed), u64::MAX);
    }

    #[test_case(false; "receiver_dropped")]
    #[test_case(true; "receiver_closed")]
    fn closed_request_channel_rejects_presets_and_motion(close_receiver: bool) {
        let (handle, mut incoming, _stops) = channels(1);
        if close_receiver {
            incoming.close();
        } else {
            drop(incoming);
        }

        run(async {
            assert!(matches!(
                handle.recall_preset(1, "Stage").await,
                Err(Error::CameraSessionClosed)
            ));
            assert!(matches!(
                handle.set_preset(1, "Stage").await,
                Err(Error::CameraSessionClosed)
            ));
        });
        assert!(matches!(
            handle.zoom(Zoom::In),
            Err(Error::CameraSessionClosed)
        ));
        assert!(matches!(
            handle.pan_tilt(Direction::Down),
            Err(Error::CameraSessionClosed)
        ));
    }

    #[test_case(false; "worker_error")]
    #[test_case(true; "worker_drops_reply")]
    fn preset_propagates_worker_failures(drop_reply: bool) {
        let (handle, mut incoming, _stops) = channels(1);

        run(async {
            let worker = async {
                let request = incoming.recv().await.unwrap();
                if drop_reply {
                    drop(request);
                } else {
                    request.reply(Err(Error::Io(std::io::ErrorKind::BrokenPipe.into())));
                }
            };
            let (result, ()) = tokio::join!(handle.recall_preset(1, "Stage"), worker);
            match drop_reply {
                true => assert!(matches!(result, Err(Error::CameraResponseReceiveFailed))),
                false => assert!(
                    matches!(result, Err(Error::Io(error)) if error.kind() == std::io::ErrorKind::BrokenPipe)
                ),
            }
        });
    }

    #[test]
    fn dropped_start_reply_is_reported() {
        let (handle, mut incoming, _stops) = channels(1);
        let PendingMotion {
            motion: _motion,
            response: started,
        } = handle.pan_tilt(Direction::Down).unwrap();
        drop(incoming.try_recv().unwrap());

        run(async {
            assert!(matches!(
                started.wait().await,
                Err(Error::CameraResponseReceiveFailed)
            ));
        });
    }

    #[test]
    fn closed_stop_channel_is_reported() {
        let (handle, _incoming, stops) = channels(1);
        let PendingMotion {
            motion,
            response: _started,
        } = handle.zoom(Zoom::In).unwrap();
        drop(stops);

        run(async {
            assert!(matches!(
                motion.stop().await,
                Err(Error::CameraSessionClosed)
            ));
        });
    }

    #[test_case(false; "worker_error")]
    #[test_case(true; "worker_drops_reply")]
    fn stop_propagates_worker_failures_and_can_be_retried(drop_reply: bool) {
        let (handle, _incoming, mut stops) = channels(1);
        let PendingMotion {
            motion,
            response: _started,
        } = handle.zoom(Zoom::In).unwrap();

        run(async {
            let worker = async {
                let request = stops.recv().await.unwrap();
                if drop_reply {
                    drop(request);
                } else {
                    request.reply(Err(Error::Io(std::io::ErrorKind::BrokenPipe.into())));
                }
            };
            let (result, ()) = tokio::join!(motion.stop(), worker);
            match drop_reply {
                true => assert!(matches!(result, Err(Error::CameraResponseReceiveFailed))),
                false => assert!(
                    matches!(result, Err(Error::Io(error)) if error.kind() == std::io::ErrorKind::BrokenPipe)
                ),
            }

            let worker = async {
                let request = stops.recv().await.unwrap();
                assert!(
                    matches!(request.action(), RequestAction::StopMotion { id } if *id == motion.id)
                );
                request.reply(Ok(CommandOutcome::Completed));
            };
            let (result, ()) = tokio::join!(motion.stop(), worker);
            assert!(matches!(result, Ok(CommandOutcome::Completed)));
        });
    }
}
