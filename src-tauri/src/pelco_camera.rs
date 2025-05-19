use crate::camera::{Camera, Direction};
use crate::error::Result;
use pelcodrs::{AutoCtrl, Direction as PelcoDirection, Message, MessageBuilder, Speed};
use serialport::{DataBits, FlowControl, Parity, SerialPort, StopBits};

#[derive(Debug)]
pub struct PelcoCamera {
    port: Box<dyn SerialPort>,
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
        Ok(Self {
            port: serialport::new(path, 9000)
                .data_bits(DataBits::Eight)
                .flow_control(FlowControl::None)
                .parity(Parity::None)
                .stop_bits(StopBits::One)
                .open()?,
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

    fn zoom_in(&mut self) -> Result<()> {
        self.send_message(MessageBuilder::new(1).zoom_in().finalize()?)
    }

    fn zoom_out(&mut self) -> Result<()> {
        self.send_message(MessageBuilder::new(1).zoom_out().finalize()?)
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
