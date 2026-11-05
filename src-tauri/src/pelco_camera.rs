use crate::camera::{Camera, Direction, Zoom};
use crate::error::Result;
use pelcodrs::{AutoCtrl, Direction as PelcoDirection, Message, MessageBuilder, Speed};
use serialport::{DataBits, FlowControl, Parity, SerialPort, StopBits};
use std::{fmt::Debug, io};

trait CameraPort: Debug + Send {
    fn name(&self) -> Option<String>;
    fn write_all(&mut self, bytes: &[u8]) -> io::Result<()>;
}

impl CameraPort for Box<dyn SerialPort> {
    fn name(&self) -> Option<String> {
        self.as_ref().name()
    }

    fn write_all(&mut self, bytes: &[u8]) -> io::Result<()> {
        io::Write::write_all(self, bytes)
    }
}

#[derive(Debug)]
pub struct PelcoCamera {
    port: Box<dyn CameraPort>,
}

impl PelcoCamera {
    fn send_message(&mut self, message: Message) -> Result<()> {
        self.port.write_all(message.as_ref())?;
        Ok(())
    }
}

impl Camera for PelcoCamera {
    fn new(path: &str) -> Result<Self>
    where
        Self: Sized,
    {
        let port = serialport::new(path, 9600)
            .data_bits(DataBits::Eight)
            .flow_control(FlowControl::None)
            .parity(Parity::None)
            .stop_bits(StopBits::One)
            .open()?;
        Ok(Self {
            port: Box::new(port),
        })
    }

    fn name(&self) -> Option<String> {
        self.port.name()
    }

    fn power_on(&mut self) -> Result<()> {
        self.send_message(MessageBuilder::new(1).camera_on().finalize()?)
    }

    fn power_off(&mut self) -> Result<()> {
        self.send_message(MessageBuilder::new(1).camera_off().finalize()?)
    }

    fn autofocus(&mut self, state: bool) -> Result<()> {
        self.send_message(Message::auto_focus(
            1,
            if state { AutoCtrl::Auto } else { AutoCtrl::Off },
        )?)
    }

    fn zoom(&mut self, zoom: crate::camera::Zoom) -> Result<()> {
        self.send_message(match zoom {
            Zoom::In => MessageBuilder::new(1).zoom_in().finalize()?,
            Zoom::Out => MessageBuilder::new(1).zoom_out().finalize()?,
        })
    }

    fn pan_tilt(&mut self, direction: crate::camera::Direction) -> Result<()> {
        self.send_message(
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
        )
    }

    fn stop(&mut self) -> Result<()> {
        self.send_message(MessageBuilder::new(1).stop().finalize()?)
    }

    fn set_preset(&mut self, preset: u8) -> Result<()> {
        self.send_message(Message::set_preset(1, preset)?)
    }

    fn go_to_preset(&mut self, preset: u8) -> Result<()> {
        self.send_message(Message::go_to_preset(1, preset)?)
    }
}

impl AsRef<PelcoCamera> for PelcoCamera {
    fn as_ref(&self) -> &PelcoCamera {
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::Error;
    use std::sync::{Arc, Mutex};
    use test_case::test_case;

    #[derive(Debug, Default)]
    struct RecordingPort {
        name: Option<String>,
        bytes: Arc<Mutex<Vec<u8>>>,
        write_error: Option<io::ErrorKind>,
    }

    impl CameraPort for RecordingPort {
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

    fn assert_packet(command: impl FnOnce(&mut PelcoCamera) -> Result<()>, expected: [u8; 7]) {
        let port = RecordingPort::default();
        let bytes = Arc::clone(&port.bytes);
        let mut camera = PelcoCamera {
            port: Box::new(port),
        };

        command(&mut camera).unwrap();

        assert_eq!(bytes.lock().unwrap().as_slice(), &expected);
    }

    #[test_case(Some("test-port"); "named port")]
    #[test_case(None; "unnamed port")]
    fn name_is_forwarded_from_the_port(name: Option<&str>) {
        let camera = PelcoCamera {
            port: Box::new(RecordingPort {
                name: name.map(str::to_owned),
                ..RecordingPort::default()
            }),
        };

        assert_eq!(camera.name().as_deref(), name);
    }

    #[test_case(PelcoCamera::power_on, [0xff, 0x01, 0x88, 0x00, 0x00, 0x00, 0x89]; "power on")]
    #[test_case(PelcoCamera::power_off, [0xff, 0x01, 0x08, 0x00, 0x00, 0x00, 0x09]; "power off")]
    #[test_case(PelcoCamera::stop, [0xff, 0x01, 0x00, 0x00, 0x00, 0x00, 0x01]; "stop")]
    fn sends_packet_for_command_without_arguments(
        command: fn(&mut PelcoCamera) -> Result<()>,
        expected: [u8; 7],
    ) {
        assert_packet(command, expected);
    }

    #[test_case(true, [0xff, 0x01, 0x00, 0x2b, 0x00, 0x00, 0x2c]; "on")]
    #[test_case(false, [0xff, 0x01, 0x00, 0x2b, 0x00, 0x01, 0x2d]; "off")]
    fn autofocus_sends_packet(enabled: bool, expected: [u8; 7]) {
        assert_packet(|camera| camera.autofocus(enabled), expected);
    }

    #[test_case(Zoom::In, [0xff, 0x01, 0x00, 0x20, 0x00, 0x00, 0x21]; "in")]
    #[test_case(Zoom::Out, [0xff, 0x01, 0x00, 0x40, 0x00, 0x00, 0x41]; "out")]
    fn zoom_sends_packet(zoom: Zoom, expected: [u8; 7]) {
        assert_packet(|camera| camera.zoom(zoom), expected);
    }

    #[test_case(Direction::Down, [0xff, 0x01, 0x00, 0x10, 0x01, 0x01, 0x13]; "down")]
    #[test_case(Direction::Up, [0xff, 0x01, 0x00, 0x08, 0x01, 0x01, 0x0b]; "up")]
    #[test_case(Direction::Left, [0xff, 0x01, 0x00, 0x04, 0x01, 0x01, 0x07]; "left")]
    #[test_case(Direction::Right, [0xff, 0x01, 0x00, 0x02, 0x01, 0x01, 0x05]; "right")]
    fn pan_tilt_sends_packet(direction: Direction, expected: [u8; 7]) {
        assert_packet(|camera| camera.pan_tilt(direction), expected);
    }

    #[test_case(PelcoCamera::set_preset, 1, [0xff, 0x01, 0x00, 0x03, 0x00, 0x01, 0x05]; "set lowest")]
    #[test_case(PelcoCamera::set_preset, u8::MAX, [0xff, 0x01, 0x00, 0x03, 0x00, 0xff, 0x03]; "set highest")]
    #[test_case(PelcoCamera::go_to_preset, 1, [0xff, 0x01, 0x00, 0x07, 0x00, 0x01, 0x09]; "recall lowest")]
    #[test_case(PelcoCamera::go_to_preset, u8::MAX, [0xff, 0x01, 0x00, 0x07, 0x00, 0xff, 0x07]; "recall highest")]
    fn preset_sends_packet(
        command: fn(&mut PelcoCamera, u8) -> Result<()>,
        preset: u8,
        expected: [u8; 7],
    ) {
        assert_packet(|camera| command(camera, preset), expected);
    }

    #[test_case(PelcoCamera::set_preset; "set")]
    #[test_case(PelcoCamera::go_to_preset; "recall")]
    fn preset_zero_is_rejected_before_writing(command: fn(&mut PelcoCamera, u8) -> Result<()>) {
        let port = RecordingPort::default();
        let bytes = Arc::clone(&port.bytes);
        let mut camera = PelcoCamera {
            port: Box::new(port),
        };

        assert!(matches!(command(&mut camera, 0), Err(Error::PelcoD(_))));
        assert!(bytes.lock().unwrap().is_empty());
    }

    #[test]
    fn command_propagates_port_write_error() {
        let mut camera = PelcoCamera {
            port: Box::new(RecordingPort {
                write_error: Some(io::ErrorKind::BrokenPipe),
                ..RecordingPort::default()
            }),
        };

        let error = camera.stop().unwrap_err();

        assert!(matches!(error, Error::Io(error) if error.kind() == io::ErrorKind::BrokenPipe));
    }
}
