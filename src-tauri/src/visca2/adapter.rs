use std::{
    io::{Read, Write},
    time::{Duration, Instant},
};

use serialport::SerialPort;
use thiserror::Error as ThisError;

use crate::camera::{
    CameraAction, CameraMotion, CameraSessionAdapter, CameraSessionBuilder, CameraSessionResult,
    CommandOutcome, CommandResult, Direction, Error, RequestLane, Result, Zoom as CameraZoom,
};

use super::{
    codec::{AxisDirection, Command, ReplyError, ReplyKind, Zoom},
    transport::Transport,
};

const IO_TIMEOUT: Duration = Duration::from_millis(10);
const REPLY_TIMEOUT: Duration = Duration::from_secs(1);

#[derive(Debug, ThisError)]
enum SessionError {
    #[error("VISCA reply timed out")]
    Timeout,
    #[error("VISCA reply did not match an outstanding command")]
    UnexpectedReply,
    #[error("VISCA camera rejected the command: {0:?}")]
    Rejected(ReplyError),
    #[error("VISCA command stream lost synchronization")]
    Desynchronized,
}

fn protocol_error(error: impl std::error::Error + Send + Sync + 'static) -> Error {
    Error::Protocol(Box::new(error))
}

struct Pending {
    lane: RequestLane,
    deadline: Instant,
    clears_motion: bool,
}

struct ViscaCamera<P> {
    name: Option<String>,
    transport: Transport<P>,
    address: u8,
    awaiting_ack: Option<Pending>,
    sockets: Vec<(u8, Pending)>,
    active_stop: Option<Command>,
    faulted: bool,
}

/// Open a VISCA session for explicit callers. The UI currently selects Pelco-D.
pub async fn open(builder: CameraSessionBuilder, address: u8) -> CameraSessionResult {
    // Validate before opening the port.
    Command::Zoom(Zoom::Stop)
        .encode(address)
        .map_err(protocol_error)?;
    builder
        .try_open(move |port: Box<dyn SerialPort>| {
            let name = port.name();
            let transport = Transport::from_serial_port(port, IO_TIMEOUT)?;
            Ok(ViscaCamera::new(name, transport, address))
        })
        .await
}

impl<P: Read + Write> ViscaCamera<P> {
    fn new(name: Option<String>, transport: Transport<P>, address: u8) -> Self {
        Self {
            name,
            transport,
            address,
            awaiting_ack: None,
            sockets: Vec::with_capacity(2),
            active_stop: None,
            faulted: false,
        }
    }

    fn send(&mut self, command: Command, lane: RequestLane, clears_motion: bool) -> Result<()> {
        if self.faulted {
            return Err(protocol_error(SessionError::Desynchronized));
        }
        if self.awaiting_ack.is_some() || self.sockets.len() == 2 {
            return Err(Error::CameraBusy);
        }

        if let Err(error) = self.transport.send(self.address, command) {
            if matches!(error, super::transport::TransportError::Io(_)) {
                self.faulted = true;
            }
            return Err(protocol_error(error));
        }
        self.awaiting_ack = Some(Pending {
            lane,
            deadline: Instant::now() + REPLY_TIMEOUT,
            clears_motion,
        });
        Ok(())
    }

    fn read_completion(&mut self) -> Result<Option<(RequestLane, CommandResult)>> {
        let now = Instant::now();
        if self
            .awaiting_ack
            .iter()
            .chain(self.sockets.iter().map(|(_, pending)| pending))
            .any(|pending| now >= pending.deadline)
        {
            return Err(protocol_error(SessionError::Timeout));
        }

        let Some(reply) = self.transport.poll_reply().map_err(protocol_error)? else {
            return Ok(None);
        };
        if reply.camera_address != self.address {
            return Ok(None);
        }

        let socket = reply.socket;
        let unexpected = || protocol_error(SessionError::UnexpectedReply);
        if reply.kind == ReplyKind::Ack {
            if self.sockets.iter().any(|(assigned, _)| *assigned == socket) {
                return Err(unexpected());
            }
            let pending = self.awaiting_ack.take().ok_or_else(unexpected)?;
            self.sockets.push((socket, pending));
            return Ok(None);
        }

        let result = match reply.kind {
            ReplyKind::Completion([]) if socket != 0 => Ok(CommandOutcome::Completed),
            ReplyKind::Error(error) => Err(protocol_error(SessionError::Rejected(error))),
            _ => return Err(unexpected()),
        };
        let pending = if socket == 0 {
            self.awaiting_ack.take().ok_or_else(unexpected)?
        } else {
            let index = self
                .sockets
                .iter()
                .position(|(assigned, _)| *assigned == socket)
                .ok_or_else(unexpected)?;
            self.sockets.swap_remove(index).1
        };

        if result.is_ok() && pending.clears_motion {
            self.active_stop = None;
        }
        Ok(Some((pending.lane, result)))
    }

    fn start_action(&mut self, action: &CameraAction, lane: RequestLane) -> Result<bool> {
        let command = match action {
            CameraAction::SetPreset(preset, _) => Command::SetPreset(*preset),
            CameraAction::RecallPreset(preset, _) => Command::RecallPreset(*preset),
            CameraAction::Motion(CameraMotion::PanTilt(direction)) => {
                use AxisDirection::{Negative, Positive, Stop};
                let (pan, tilt) = match direction {
                    Direction::Left => (Negative, Stop),
                    Direction::Right => (Positive, Stop),
                    Direction::Up => (Stop, Negative),
                    Direction::Down => (Stop, Positive),
                };
                // Retain a stop even if the write fails after emitting some bytes.
                self.active_stop = Some(pan_tilt(Stop, Stop));
                pan_tilt(pan, tilt)
            }
            CameraAction::Motion(CameraMotion::Zoom(zoom)) => {
                self.active_stop = Some(Command::Zoom(Zoom::Stop));
                Command::Zoom(match zoom {
                    CameraZoom::In => Zoom::Tele,
                    CameraZoom::Out => Zoom::Wide,
                })
            }
            CameraAction::StopMotion => {
                let Some(command) = self.active_stop else {
                    return Ok(false);
                };
                self.send(command, lane, true)?;
                return Ok(true);
            }
        };
        self.send(command, lane, false)?;
        Ok(true)
    }
}

fn pan_tilt(pan: AxisDirection, tilt: AxisDirection) -> Command {
    Command::PanTilt {
        pan,
        tilt,
        pan_speed: 1,
        tilt_speed: 1,
    }
}

impl<P: Read + Write> CameraSessionAdapter for ViscaCamera<P> {
    fn name(&self) -> Option<String> {
        self.name.clone()
    }

    fn handle_action(&mut self, action: &CameraAction) -> CommandResult {
        if self.start_action(action, RequestLane::Action)? {
            loop {
                if let Some((_, result)) = self.poll_completion()? {
                    return result;
                }
            }
        }
        Ok(CommandOutcome::Completed)
    }

    fn stop_motion(&mut self) -> Result<()> {
        self.handle_action(&CameraAction::StopMotion).map(|_| ())
    }

    fn close(&mut self) -> Result<()> {
        let result = (|| {
            while !self.faulted && (self.awaiting_ack.is_some() || !self.sockets.is_empty()) {
                self.poll_completion()?;
            }
            self.stop_motion()
        })();
        if result.is_err()
            && let Some(command) = self.active_stop
        {
            // A failed or ambiguous exchange can leave motion running.
            let _ = self.transport.send(self.address, command);
        }
        result
    }

    fn dispatch(&mut self, action: &CameraAction, lane: RequestLane) -> Option<CommandResult> {
        match self.start_action(action, lane) {
            Ok(true) => None,
            Ok(false) => Some(Ok(CommandOutcome::Completed)),
            Err(error) => Some(Err(error)),
        }
    }

    fn can_dispatch_stop(&self) -> bool {
        self.awaiting_ack.is_none() || self.faulted
    }

    fn poll_completion(&mut self) -> Result<Option<(RequestLane, CommandResult)>> {
        if self.faulted {
            return Err(protocol_error(SessionError::Desynchronized));
        }
        let result = self.read_completion();
        if result.is_err() {
            self.faulted = true;
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use std::{io, sync::mpsc};

    use tokio::sync::mpsc as async_mpsc;

    use super::*;

    struct ControlledPort {
        replies: mpsc::Receiver<[u8; 4]>,
        writes: async_mpsc::UnboundedSender<Vec<u8>>,
    }

    impl Read for ControlledPort {
        fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
            let reply = self
                .replies
                .recv_timeout(IO_TIMEOUT)
                .map_err(|error| match error {
                    mpsc::RecvTimeoutError::Timeout => io::ErrorKind::TimedOut,
                    mpsc::RecvTimeoutError::Disconnected => io::ErrorKind::BrokenPipe,
                })?;
            let len = reply.iter().position(|&byte| byte == 0xff).unwrap() + 1;
            buffer[..len].copy_from_slice(&reply[..len]);
            Ok(len)
        }
    }

    impl Write for ControlledPort {
        fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
            self.writes
                .send(buffer.to_vec())
                .map_err(|_| io::ErrorKind::BrokenPipe)?;
            Ok(buffer.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn worker_sends_stop_after_ack_and_matches_replies_out_of_order() {
        tauri::async_runtime::block_on(async {
            tokio::time::timeout(Duration::from_secs(3), async {
                let (reply_tx, replies) = mpsc::channel();
                let (writes, mut write_rx) = async_mpsc::unbounded_channel();
                let (events, mut event_rx) = async_mpsc::unbounded_channel();
                let session = CameraSessionBuilder::new("scripted", events)
                    .open_with_adapter(move |_| {
                        let transport = Transport::new(ControlledPort { replies, writes });
                        Ok(ViscaCamera::new(None, transport, 1))
                    })
                    .await
                    .unwrap();
                let start = session.handle().pan_tilt(Direction::Left).unwrap();
                assert_eq!(
                    write_rx.recv().await.unwrap(),
                    [0x81, 1, 6, 1, 1, 1, 1, 3, 0xff]
                );

                let stop = tauri::async_runtime::spawn(async move { start.motion.stop().await });
                reply_tx.send([0x90, 0x41, 0xff, 0]).unwrap();

                // The start hasn't completed. Its ACK is enough to send stop.
                assert_eq!(
                    write_rx.recv().await.unwrap(),
                    [0x81, 1, 6, 1, 1, 1, 3, 3, 0xff]
                );
                reply_tx.send([0x90, 0x42, 0xff, 0]).unwrap();
                reply_tx.send([0x90, 0x52, 0xff, 0]).unwrap();
                assert!(matches!(stop.await.unwrap(), Ok(CommandOutcome::Completed)));
                assert!(matches!(
                    event_rx.recv().await.unwrap().action(),
                    CameraAction::StopMotion
                ));
                assert!(event_rx.try_recv().is_err());

                reply_tx.send([0x90, 0x61, 0x41, 0xff]).unwrap();
                let error = start.response.wait().await.err().unwrap();
                assert!(error.to_string().contains("CommandNotExecutable"));
                let event = event_rx.recv().await.unwrap();
                assert!(matches!(event.action(), CameraAction::Motion(_)));
                assert!(event.result().is_err());

                session.shutdown().await.unwrap();
                assert!(write_rx.recv().await.is_none());
            })
            .await
            .expect("VISCA session did not finish");
        });
    }

    #[test]
    fn expired_command_faults_the_exchange_before_reading_a_late_reply() {
        let (reply_tx, replies) = mpsc::channel();
        let (writes, mut write_rx) = async_mpsc::unbounded_channel();
        let transport = Transport::new(ControlledPort { replies, writes });
        let mut camera = ViscaCamera::new(None, transport, 1);
        camera
            .send(Command::SetPreset(1), RequestLane::Action, false)
            .unwrap();
        camera.awaiting_ack.as_mut().unwrap().deadline = Instant::now();
        reply_tx.send([0x90, 0x41, 0xff, 0]).unwrap();
        assert!(
            camera
                .poll_completion()
                .err()
                .unwrap()
                .to_string()
                .contains("timed out")
        );
        assert!(
            camera
                .send(Command::SetPreset(2), RequestLane::Action, false)
                .is_err()
        );
        assert!(write_rx.try_recv().is_ok());
        assert!(write_rx.try_recv().is_err());
    }
}
