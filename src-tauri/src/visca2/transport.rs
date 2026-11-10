use std::{
    io::{self, BufRead, BufReader, Read, Write},
    time::Duration,
};

use serialport::SerialPort;
use thiserror::Error;

use super::codec::{CodecError, Command, MAX_PACKET_LEN, Reply, TERMINATOR};

#[derive(Debug, Error)]
pub enum TransportError {
    #[error(transparent)]
    Codec(#[from] CodecError),
    #[error(transparent)]
    Io(#[from] io::Error),
}

/// Blocking I/O for a single port owned by the camera worker.
pub struct Transport<P> {
    port: BufReader<P>,
    frame: [u8; MAX_PACKET_LEN],
    frame_len: usize,
}

impl Transport<Box<dyn SerialPort>> {
    /// Take ownership of an open port and set its I/O timeout.
    pub fn from_serial_port(
        mut port: Box<dyn SerialPort>,
        timeout: Duration,
    ) -> serialport::Result<Self> {
        port.set_timeout(timeout)?;
        Ok(Self::new(port))
    }
}

impl<P: Read + Write> Transport<P> {
    pub(super) fn new(port: P) -> Self {
        Self {
            port: BufReader::with_capacity(MAX_PACKET_LEN * 4, port),
            frame: [0; MAX_PACKET_LEN],
            frame_len: 0,
        }
    }

    /// Write a command without waiting for a camera reply.
    pub fn send(&mut self, address: u8, command: Command) -> Result<(), TransportError> {
        self.port.get_mut().write_all(&command.encode(address)?)?;
        Ok(())
    }

    /// Return at most one reply, performing at most one underlying read.
    ///
    /// A partial read, timeout, or transient I/O error returns `Ok(None)` and
    /// preserves any buffered bytes. The worker can check its control queue
    /// between calls. Oversized frames are discarded through their terminator.
    pub fn poll_reply(&mut self) -> Result<Option<Reply<'_>>, TransportError> {
        let bytes = match self.port.fill_buf() {
            Ok([]) => return Err(io::Error::from(io::ErrorKind::UnexpectedEof).into()),
            Ok(bytes) => bytes,
            Err(error)
                if matches!(
                    error.kind(),
                    io::ErrorKind::TimedOut
                        | io::ErrorKind::WouldBlock
                        | io::ErrorKind::Interrupted
                ) =>
            {
                return Ok(None);
            }
            Err(error) => return Err(error.into()),
        };

        let terminator = bytes.iter().position(|&byte| byte == TERMINATOR);
        let consumed = terminator.map_or(bytes.len(), |index| index + 1);
        let end = self.frame_len + consumed;
        if end <= MAX_PACKET_LEN {
            self.frame[self.frame_len..end].copy_from_slice(&bytes[..consumed]);
        }
        // Keep one overflow value while discarding the rest of a long frame.
        self.frame_len = end.min(MAX_PACKET_LEN + 1);
        self.port.consume(consumed);

        if terminator.is_none() {
            return Ok(None);
        }

        let frame_len = std::mem::take(&mut self.frame_len);
        if frame_len > MAX_PACKET_LEN {
            return Err(CodecError::InvalidPacketLength.into());
        }

        Ok(Some(Reply::decode(&self.frame[..frame_len])?))
    }
}

#[cfg(test)]
mod tests {
    use std::collections::VecDeque;

    use super::*;
    use crate::visca2::codec::{ReplyKind, Zoom};

    #[derive(Default)]
    struct ScriptedPort {
        reads: VecDeque<io::Result<&'static [u8]>>,
        written: Vec<u8>,
        write_error: Option<io::ErrorKind>,
    }

    impl Read for ScriptedPort {
        fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
            let bytes = self.reads.pop_front().expect("unexpected serial read")?;
            buffer[..bytes.len()].copy_from_slice(bytes);
            Ok(bytes.len())
        }
    }

    impl Write for ScriptedPort {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            if let Some(error) = self.write_error {
                return Err(error.into());
            }

            let count = bytes.len().min(2);
            self.written.extend_from_slice(&bytes[..count]);
            Ok(count)
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn sends_complete_commands_without_reading() {
        let mut transport = Transport::new(ScriptedPort::default());
        transport.send(1, Command::Zoom(Zoom::Tele)).unwrap();
        assert_eq!(
            transport.port.get_ref().written,
            [0x81, 0x01, 0x04, 0x07, 0x02, 0xff]
        );
    }

    #[test]
    fn preserves_partial_reply_across_timeout_and_stop_write() {
        let mut transport = Transport::new(ScriptedPort {
            reads: [
                Ok(&[0x90, 0x41][..]),
                Err(io::ErrorKind::TimedOut.into()),
                Ok(&[0xff, 0x90, 0x51, 0xff][..]),
            ]
            .into(),
            ..Default::default()
        });

        assert_eq!(transport.poll_reply().unwrap(), None);
        assert_eq!(transport.poll_reply().unwrap(), None);
        transport.send(1, Command::Zoom(Zoom::Stop)).unwrap();
        assert_eq!(
            transport.poll_reply().unwrap().unwrap().kind,
            ReplyKind::Ack
        );
        assert_eq!(
            transport.poll_reply().unwrap().unwrap().kind,
            ReplyKind::Completion(&[])
        );
        assert!(transport.port.get_ref().reads.is_empty());
    }

    #[test]
    fn discards_oversized_frame_and_keeps_next_reply() {
        let mut transport = Transport::new(ScriptedPort {
            reads: [
                Ok(&[0x90; MAX_PACKET_LEN * 4][..]),
                Ok(&[0x90; MAX_PACKET_LEN * 4][..]),
                Ok(&[0xff, 0x90, 0x42, 0xff][..]),
            ]
            .into(),
            ..Default::default()
        });

        assert_eq!(transport.poll_reply().unwrap(), None);
        assert_eq!(transport.poll_reply().unwrap(), None);
        assert!(matches!(
            transport.poll_reply(),
            Err(TransportError::Codec(CodecError::InvalidPacketLength))
        ));
        assert_eq!(transport.poll_reply().unwrap().unwrap().socket, 2);
    }

    #[test]
    fn keeps_next_reply_after_malformed_frame() {
        let mut transport = Transport::new(ScriptedPort {
            reads: [Ok(&[0x90, 0x71, 0xff, 0x90, 0x42, 0xff][..])].into(),
            ..Default::default()
        });

        assert!(matches!(
            transport.poll_reply(),
            Err(TransportError::Codec(CodecError::InvalidReplyBody))
        ));
        assert_eq!(transport.poll_reply().unwrap().unwrap().socket, 2);
    }

    #[test]
    fn propagates_io_errors_and_rejects_invalid_commands_before_writing() {
        let mut transport = Transport::new(ScriptedPort {
            reads: [Err(io::ErrorKind::BrokenPipe.into()), Ok(&[][..])].into(),
            write_error: Some(io::ErrorKind::BrokenPipe),
            ..Default::default()
        });

        assert!(matches!(
            transport.send(0, Command::Zoom(Zoom::Stop)),
            Err(TransportError::Codec(CodecError::InvalidAddress(0)))
        ));
        assert!(matches!(
            transport.send(1, Command::Zoom(Zoom::Stop)),
            Err(TransportError::Io(error)) if error.kind() == io::ErrorKind::BrokenPipe
        ));
        for kind in [io::ErrorKind::BrokenPipe, io::ErrorKind::UnexpectedEof] {
            assert!(matches!(
                transport.poll_reply(),
                Err(TransportError::Io(error)) if error.kind() == kind
            ));
        }
    }
}
