use crate::error::Result;
use pelcodrs::{AutoCtrl, Direction, Message, MessageBuilder, Speed};
use serialport::{DataBits, FlowControl, Parity, SerialPort, StopBits};

#[derive(Debug)]
pub struct Camera {
    port: Box<dyn SerialPort>,
}

impl Camera {
    pub fn new(path: &str) -> Result<Self> {
        Ok(Self {
            port: serialport::new(path, 9000)
                .stop_bits(StopBits::One)
                .data_bits(DataBits::Eight)
                .flow_control(FlowControl::None)
                .parity(Parity::None)
                .open()?,
        })
    }

    fn send_message(&mut self, message: Message) -> Result<()> {
        self.port.write_all(message.as_ref())?;
        Ok(())
    }

    pub fn name(&self) -> Option<String> {
        self.port.name()
    }

    pub fn power_on(&mut self) -> Result<()> {
        self.send_message(MessageBuilder::new(1).camera_on().finalize()?)
    }

    pub fn power_off(&mut self) -> Result<()> {
        self.send_message(MessageBuilder::new(1).camera_off().finalize()?)
    }

    pub fn autofocus(&mut self, state: AutoCtrl) -> Result<()> {
        self.send_message(Message::auto_focus(1, state)?)
    }

    pub fn zoom_in(&mut self) -> Result<()> {
        self.send_message(MessageBuilder::new(1).zoom_in().finalize()?)
    }

    pub fn zoom_out(&mut self) -> Result<()> {
        self.send_message(MessageBuilder::new(1).zoom_out().finalize()?)
    }

    pub fn r#move(&mut self, direction: Direction) -> Result<()> {
        self.send_message(
            MessageBuilder::new(1)
                .pan(Speed::Range(0.01))
                .tilt(Speed::Range(0.01))
                .direction(direction)
                .finalize()?,
        )
    }

    pub fn stop(&mut self) -> Result<()> {
        self.send_message(MessageBuilder::new(1).stop().finalize()?)
    }

    pub fn set_preset(&mut self, preset: u8) -> Result<()> {
        self.send_message(Message::set_preset(1, preset)?)
    }

    pub fn go_to_preset(&mut self, preset: u8) -> Result<()> {
        self.send_message(Message::go_to_preset(1, preset)?)
    }
}

impl AsRef<Camera> for Camera {
    fn as_ref(&self) -> &Camera {
        self
    }
}
