use std::{fmt::Debug, io};

use pelcodrs::{Direction as PelcoDirection, Message, MessageBuilder, Speed};
use serialport::SerialPort;

use crate::camera::{
    CameraAction,
    CameraMotion,
    CameraSessionAdapter,
    CommandOutcome,
    CommandResult,
    Direction,
    Result,
    Zoom,
};

trait PelcoPort: Debug + Send {
    fn name(&self) -> Option<String>;
    fn write_all(&mut self, bytes: &[u8]) -> io::Result<()>;
}

impl PelcoPort for Box<dyn SerialPort> {
    fn name(&self) -> Option<String> {
        self.as_ref().name()
    }

    fn write_all(&mut self, bytes: &[u8]) -> io::Result<()> {
        io::Write::write_all(self, bytes)
    }
}

impl From<pelcodrs::Error> for crate::camera::Error {
    fn from(error: pelcodrs::Error) -> Self {
        Self::Protocol(Box::new(error))
    }
}

pub struct PelcoCamera {
    port: Box<dyn PelcoPort>,
}

impl PelcoCamera {
    pub fn from_serial_port(port: Box<dyn SerialPort>) -> Self {
        Self::for_port(Box::new(port))
    }

    fn for_port(port: Box<dyn PelcoPort>) -> Self {
        Self { port }
    }

    fn send_message(&mut self, message: Message) -> Result<()> {
        self.port.write_all(message.as_ref())?;
        Ok(())
    }

    fn stop(&mut self) -> Result<()> {
        self.send_message(MessageBuilder::new(1).stop().finalize()?)
    }
}

impl CameraSessionAdapter for PelcoCamera {
    fn name(&self) -> Option<String> {
        self.port.name()
    }

    fn handle_action(&mut self, action: &CameraAction) -> CommandResult {
        let result = match action {
            CameraAction::RecallPreset(preset, _) => {
                self.send_message(Message::go_to_preset(1, *preset)?)
            }
            CameraAction::SetPreset(preset, _) => {
                self.send_message(Message::set_preset(1, *preset)?)
            }
            CameraAction::Motion(motion) => match motion {
                CameraMotion::PanTilt(direction) => self.send_message(
                    MessageBuilder::new(1)
                        .pan(Speed::Range(0.01))
                        .tilt(Speed::Range(0.01))
                        .direction(match direction {
                            Direction::Down => PelcoDirection::DOWN,
                            Direction::Up => PelcoDirection::UP,
                            Direction::Left => PelcoDirection::LEFT,
                            Direction::Right => PelcoDirection::RIGHT,
                        })
                        .finalize()?,
                ),
                CameraMotion::Zoom(zoom) => self.send_message(match zoom {
                    Zoom::In => MessageBuilder::new(1).zoom_in().finalize()?,
                    Zoom::Out => MessageBuilder::new(1).zoom_out().finalize()?,
                }),
            },
            CameraAction::StopMotion => self.stop(),
        };

        result.map(|()| CommandOutcome::Completed)
    }

    fn stop_motion(&mut self) -> Result<()> {
        self.stop()
    }

    fn close(&mut self) -> Result<()> {
        self.stop()
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{
        Arc,
        Mutex,
        atomic::{AtomicBool, Ordering},
    };

    use test_case::test_case;

    use super::*;
    use crate::camera::Error;

    #[derive(Debug, Default)]
    struct RecordingPort {
        name: Option<String>,
        bytes: Arc<Mutex<Vec<u8>>>,
        write_error: Option<io::ErrorKind>,
        dropped: Arc<AtomicBool>,
    }

    impl PelcoPort for RecordingPort {
        fn name(&self) -> Option<String> {
            self.name.clone()
        }

        fn write_all(&mut self, bytes: &[u8]) -> io::Result<()> {
            if let Some(kind) = self.write_error {
                return Err(io::Error::from(kind));
            }
            self.bytes.lock().unwrap().extend_from_slice(bytes);
            Ok(())
        }
    }

    impl Drop for RecordingPort {
        fn drop(&mut self) {
            self.dropped.store(true, Ordering::Relaxed);
        }
    }

    fn assert_packet(action: CameraAction, expected: [u8; 7]) {
        let port = RecordingPort::default();
        let bytes = Arc::clone(&port.bytes);
        let mut camera = PelcoCamera::for_port(Box::new(port));

        assert!(matches!(
            camera.handle_action(&action),
            Ok(CommandOutcome::Completed)
        ));

        assert_eq!(bytes.lock().unwrap().as_slice(), &expected);
    }

    #[test_case(Some("test-port"); "named port")]
    #[test_case(None; "unnamed port")]
    fn name_is_forwarded_from_the_port(name: Option<&str>) {
        let camera = PelcoCamera::for_port(Box::new(RecordingPort {
            name: name.map(str::to_owned),
            bytes: Arc::default(),
            write_error: None,
            dropped: Arc::default(),
        }));

        assert_eq!(camera.name().as_deref(), name);
    }

    #[test]
    fn stop_action_sends_stop_packet() {
        assert_packet(
            CameraAction::StopMotion,
            [0xff, 0x01, 0x00, 0x00, 0x00, 0x00, 0x01],
        );
    }

    #[test_case(PelcoCamera::stop_motion; "pre_start_stop")]
    #[test_case(PelcoCamera::close; "close")]
    fn lifecycle_methods_send_stop_packet(
        command: fn(&mut PelcoCamera) -> crate::camera::Result<()>,
    ) {
        let port = RecordingPort::default();
        let bytes = port.bytes.clone();
        let mut camera = PelcoCamera::for_port(Box::new(port));

        command(&mut camera).unwrap();

        assert_eq!(
            *bytes.lock().unwrap(),
            [0xff, 0x01, 0x00, 0x00, 0x00, 0x00, 0x01]
        );
    }

    #[test_case(Zoom::In, [0xff, 0x01, 0x00, 0x20, 0x00, 0x00, 0x21]; "in")]
    #[test_case(Zoom::Out, [0xff, 0x01, 0x00, 0x40, 0x00, 0x00, 0x41]; "out")]
    fn zoom_sends_packet(zoom: Zoom, expected: [u8; 7]) {
        assert_packet(CameraAction::Motion(CameraMotion::Zoom(zoom)), expected);
    }

    #[test_case(Direction::Down, [0xff, 0x01, 0x00, 0x10, 0x01, 0x01, 0x13]; "down")]
    #[test_case(Direction::Up, [0xff, 0x01, 0x00, 0x08, 0x01, 0x01, 0x0b]; "up")]
    #[test_case(Direction::Left, [0xff, 0x01, 0x00, 0x04, 0x01, 0x01, 0x07]; "left")]
    #[test_case(Direction::Right, [0xff, 0x01, 0x00, 0x02, 0x01, 0x01, 0x05]; "right")]
    fn pan_tilt_sends_packet(direction: Direction, expected: [u8; 7]) {
        assert_packet(
            CameraAction::Motion(CameraMotion::PanTilt(direction)),
            expected,
        );
    }

    #[test_case(CameraAction::SetPreset(1, "Stage".into()), [0xff, 0x01, 0x00, 0x03, 0x00, 0x01, 0x05]; "set lowest")]
    #[test_case(CameraAction::SetPreset(u8::MAX, "Stage".into()), [0xff, 0x01, 0x00, 0x03, 0x00, 0xff, 0x03]; "set highest")]
    #[test_case(CameraAction::RecallPreset(1, "Stage".into()), [0xff, 0x01, 0x00, 0x07, 0x00, 0x01, 0x09]; "recall lowest")]
    #[test_case(CameraAction::RecallPreset(u8::MAX, "Stage".into()), [0xff, 0x01, 0x00, 0x07, 0x00, 0xff, 0x07]; "recall highest")]
    fn preset_sends_packet(action: CameraAction, expected: [u8; 7]) {
        assert_packet(action, expected);
    }

    #[test_case(CameraAction::SetPreset(0, "Stage".into()); "set")]
    #[test_case(CameraAction::RecallPreset(0, "Stage".into()); "recall")]
    fn preset_zero_is_rejected_before_writing(action: CameraAction) {
        let port = RecordingPort::default();
        let bytes = Arc::clone(&port.bytes);
        let mut camera = PelcoCamera::for_port(Box::new(port));

        assert!(matches!(
            camera.handle_action(&action),
            Err(Error::Protocol(_))
        ));
        assert!(bytes.lock().unwrap().is_empty());
    }

    #[test_case(CameraAction::RecallPreset(1, "Stage".into()); "recall")]
    #[test_case(CameraAction::SetPreset(1, "Stage".into()); "save")]
    #[test_case(CameraAction::Motion(CameraMotion::PanTilt(Direction::Left)); "pan_tilt")]
    #[test_case(CameraAction::Motion(CameraMotion::Zoom(Zoom::In)); "zoom")]
    #[test_case(CameraAction::StopMotion; "stop")]
    fn command_propagates_port_write_error(action: CameraAction) {
        let port = RecordingPort {
            write_error: Some(io::ErrorKind::BrokenPipe),
            name: None,
            bytes: Arc::default(),
            dropped: Arc::default(),
        };
        let bytes = port.bytes.clone();
        let mut camera = PelcoCamera::for_port(Box::new(port));

        assert!(
            matches!(camera.handle_action(&action), Err(Error::Io(error)) if error.kind() == io::ErrorKind::BrokenPipe)
        );
        assert!(bytes.lock().unwrap().is_empty());
    }

    #[test_case(PelcoCamera::stop_motion; "pre_start_stop")]
    #[test_case(PelcoCamera::close; "close")]
    fn lifecycle_errors_preserve_port_until_adapter_is_dropped(
        command: fn(&mut PelcoCamera) -> Result<()>,
    ) {
        let port = RecordingPort {
            write_error: Some(io::ErrorKind::BrokenPipe),
            name: None,
            bytes: Arc::default(),
            dropped: Arc::default(),
        };
        let dropped = port.dropped.clone();
        let mut camera = PelcoCamera::for_port(Box::new(port));

        assert!(
            matches!(command(&mut camera), Err(Error::Io(error)) if error.kind() == io::ErrorKind::BrokenPipe)
        );
        assert!(!dropped.load(Ordering::Relaxed));
        drop(camera);
        assert!(dropped.load(Ordering::Relaxed));
    }
}
