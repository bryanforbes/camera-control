use std::{
    env,
    error::Error,
    fs::File,
    io::{self, ErrorKind, Read, Write},
    path::PathBuf,
    process::ExitCode,
    time::{Duration, Instant},
};

const USAGE: &str = "\
Usage: visca-probe --port DEVICE [--baud 9600] [--address 1] [--timeout-ms 500]
                   [--log-file PATH] <baseline|overlap> <pan|zoom>

pan moves left at low speed. zoom moves in.
baseline sends stop after start completion.
overlap sends stop immediately after start ACK.
If the start trigger fails or times out, the probe attempts a fallback stop.
Each reply window is bounded by --timeout-ms.
--log-file writes the trace to a new file. By default it goes to stdout.
Close other camera controllers before running. Watch the camera to verify it stops.
";

#[derive(Clone, Copy)]
enum Mode {
    Baseline,
    Overlap,
}

#[derive(Clone, Copy)]
enum Motion {
    Pan,
    Zoom,
}

impl Motion {
    fn packets(self, address: u8) -> (Vec<u8>, Vec<u8>) {
        let header = 0x80 | address;
        match self {
            Self::Pan => (
                vec![header, 1, 6, 1, 1, 1, 1, 3, 0xff],
                vec![header, 1, 6, 1, 1, 1, 3, 3, 0xff],
            ),
            Self::Zoom => (
                vec![header, 1, 4, 7, 2, 0xff],
                vec![header, 1, 4, 7, 0, 0xff],
            ),
        }
    }
}

struct Options {
    port: String,
    log_file: Option<PathBuf>,
    baud: u32,
    address: u8,
    timeout: Duration,
    mode: Mode,
    motion: Motion,
}

impl Options {
    fn parse(args: impl IntoIterator<Item = String>) -> Result<Option<Self>, String> {
        let mut args = args.into_iter();
        let mut port = None;
        let mut log_file = None;
        let mut baud = 9600;
        let mut address = 1;
        let mut timeout_ms = 500;
        let mut positional = Vec::new();

        while let Some(arg) = args.next() {
            let mut value = || args.next().ok_or_else(|| format!("{arg} needs a value"));
            match arg.as_str() {
                "-h" | "--help" => return Ok(None),
                "--port" => port = Some(value()?),
                "--log-file" => log_file = Some(PathBuf::from(value()?)),
                "--baud" => baud = value()?.parse().map_err(|_| "invalid baud rate")?,
                "--address" => address = value()?.parse().map_err(|_| "invalid camera address")?,
                "--timeout-ms" => {
                    timeout_ms = value()?.parse().map_err(|_| "invalid timeout")?;
                }
                _ if arg.starts_with('-') => return Err(format!("unknown option {arg}")),
                _ => positional.push(arg),
            }
        }

        let [mode, motion] = positional.as_slice() else {
            return Err("expected a mode and a motion".into());
        };
        let mode = match mode.as_str() {
            "baseline" => Mode::Baseline,
            "overlap" => Mode::Overlap,
            _ => return Err(format!("unknown mode {mode}")),
        };
        let motion = match motion.as_str() {
            "pan" => Motion::Pan,
            "zoom" => Motion::Zoom,
            _ => return Err(format!("unknown motion {motion}")),
        };
        if !(1..=7).contains(&address) {
            return Err("camera address must be between 1 and 7".into());
        }
        if baud == 0 || !(1..=60_000).contains(&timeout_ms) {
            return Err("baud must be positive and timeout must be 1–60000 ms".into());
        }
        Ok(Some(Self {
            port: port
                .filter(|path| !path.is_empty())
                .ok_or("--port is required")?,
            log_file,
            baud,
            address,
            timeout: Duration::from_millis(timeout_ms),
            mode,
            motion,
        }))
    }
}

struct Trace<'a, T, W> {
    port: &'a mut T,
    output: &'a mut W,
    origin: Instant,
    frame: Vec<u8>,
}

impl<T: Read + Write, W: Write> Trace<'_, T, W> {
    fn note(&mut self, message: impl std::fmt::Display) {
        // A closed output pipe must not prevent the stop write.
        let _ = writeln!(
            self.output,
            "{:9.3} ms  {message}",
            self.origin.elapsed().as_secs_f64() * 1000.0
        );
    }

    fn send(&mut self, label: &str, packet: &[u8]) -> io::Result<()> {
        self.note(format_args!("TX attempt {label} {packet:02X?}"));
        self.port.write_all(packet)
    }

    fn receive(&mut self, deadline: Instant) -> io::Result<Option<Vec<u8>>> {
        while Instant::now() < deadline {
            let mut byte = [0];
            match self.port.read(&mut byte) {
                Ok(0) => return Err(io::Error::from(ErrorKind::UnexpectedEof)),
                Ok(_) => {
                    self.frame.push(byte[0]);
                    if byte[0] == 0xff {
                        let frame = std::mem::take(&mut self.frame);
                        self.note(format_args!("RX {frame:02X?}"));
                        return Ok(Some(frame));
                    }
                    if self.frame.len() >= 16 {
                        let frame = std::mem::take(&mut self.frame);
                        self.note(format_args!("RX overlong frame {frame:02X?}"));
                        return Err(io::Error::new(
                            ErrorKind::InvalidData,
                            "VISCA frame exceeded 16 bytes",
                        ));
                    }
                }
                Err(error)
                    if matches!(
                        error.kind(),
                        ErrorKind::TimedOut | ErrorKind::WouldBlock | ErrorKind::Interrupted
                    ) => {}
                Err(error) => return Err(error),
            }
        }

        Ok(None)
    }

    fn wait_for_start(
        &mut self,
        mode: Mode,
        address: u8,
        timeout: Duration,
    ) -> io::Result<&'static str> {
        let deadline = Instant::now() + timeout;
        let reply_header = (address + 8) << 4;
        let mut ack_socket = None;

        while let Some(frame) = self.receive(deadline)? {
            if frame.first() != Some(&reply_header) {
                continue;
            }

            match frame.as_slice() {
                [_, reply, 0xff] if reply & 0xf0 == 0x40 => {
                    if matches!(mode, Mode::Overlap) {
                        return Ok("start ACK");
                    }
                    ack_socket = Some(reply & 0x0f);
                }
                [_, reply, 0xff] if reply & 0xf0 == 0x50 => {
                    if ack_socket == Some(reply & 0x0f) {
                        return Ok("start completion");
                    }
                }
                [_, reply, _, 0xff] if reply & 0xf0 == 0x60 => {
                    return Err(io::Error::other(format!(
                        "camera rejected start: {frame:02X?}"
                    )));
                }
                _ => {}
            }
        }

        Err(io::Error::new(
            ErrorKind::TimedOut,
            "start trigger not observed",
        ))
    }
}

fn probe(
    port: &mut (impl Read + Write),
    output: &mut impl Write,
    options: &Options,
) -> io::Result<()> {
    let mut trace = Trace {
        port,
        output,
        origin: Instant::now(),
        frame: Vec::new(),
    };
    let (start, stop) = options.motion.packets(options.address);
    let start_result = trace
        .send("start", &start)
        .and_then(|()| trace.wait_for_start(options.mode, options.address, options.timeout));
    match &start_result {
        Ok(trigger) => trace.note(format_args!("Stop trigger: {trigger}")),
        Err(error) => trace.note(format_args!("Stop trigger: fallback ({error})")),
    }

    trace.send("stop", &stop)?;
    let deadline = Instant::now() + options.timeout;
    let mut received = false;
    while trace.receive(deadline)?.is_some() {
        received = true;
    }
    if !trace.frame.is_empty() {
        let frame = std::mem::take(&mut trace.frame);
        trace.note(format_args!("RX partial at deadline {frame:02X?}"));
    }
    start_result?;
    if !received {
        return Err(io::Error::new(
            ErrorKind::TimedOut,
            "no frames received after the stop write",
        ));
    }
    trace.note("Capture complete. Check the reply order and confirm the camera stopped.");
    Ok(())
}

fn run(options: &Options) -> Result<(), Box<dyn Error>> {
    let mut output: Box<dyn Write> = match &options.log_file {
        Some(path) => Box::new(File::create_new(path).map_err(|error| {
            io::Error::new(
                error.kind(),
                format!("cannot create log {}: {error}", path.display()),
            )
        })?),
        None => Box::new(io::stdout().lock()),
    };
    let result = capture(options, &mut output);
    if options.log_file.is_some()
        && let Err(error) = &result
    {
        let _ = writeln!(output, "visca-probe: {error}");
    }
    output.flush()?;
    result
}

fn capture(options: &Options, output: &mut impl Write) -> Result<(), Box<dyn Error>> {
    writeln!(
        output,
        "Port: {} | baud: {} | address: {} | reply window: {} ms",
        options.port,
        options.baud,
        options.address,
        options.timeout.as_millis(),
    )?;
    let mut port = serialport::new(&options.port, options.baud)
        .data_bits(serialport::DataBits::Eight)
        .parity(serialport::Parity::None)
        .stop_bits(serialport::StopBits::One)
        .flow_control(serialport::FlowControl::None)
        .timeout(Duration::from_millis(10).min(options.timeout))
        .open()?;

    probe(&mut port, output, options)?;
    Ok(())
}

fn main() -> ExitCode {
    match Options::parse(env::args().skip(1)) {
        Ok(Some(options)) => match run(&options) {
            Ok(()) => ExitCode::SUCCESS,
            Err(error) => {
                eprintln!("visca-probe: {error}");
                ExitCode::FAILURE
            }
        },
        Ok(None) => {
            print!("{USAGE}");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("visca-probe: {error}\n\n{USAGE}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct ScriptedPort<'a> {
        replies: &'a [u8],
        bytes_read: usize,
        writes: Vec<(usize, Vec<u8>)>,
    }

    impl Read for ScriptedPort<'_> {
        fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
            let Some((&byte, rest)) = self.replies.split_first() else {
                return Err(ErrorKind::TimedOut.into());
            };
            self.replies = rest;
            buffer[0] = byte;
            self.bytes_read += 1;
            Ok(1)
        }
    }

    impl Write for ScriptedPort<'_> {
        fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
            self.writes.push((self.bytes_read, buffer.to_vec()));
            Ok(buffer.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    fn options(mode: Mode) -> Options {
        Options {
            port: String::new(),
            log_file: None,
            baud: 9600,
            address: 1,
            timeout: Duration::from_millis(100),
            mode,
            motion: Motion::Pan,
        }
    }

    #[test]
    fn modes_send_stop_at_different_reply_boundaries() {
        for (mode, stop_after, trigger) in [
            (Mode::Baseline, 6, "start completion"),
            (Mode::Overlap, 3, "start ACK"),
        ] {
            let mut port = ScriptedPort {
                replies: &[
                    0x90, 0x41, 0xff, 0x90, 0x51, 0xff, 0x90, 0x42, 0xff, 0x90, 0x52, 0xff,
                ],
                bytes_read: 0,
                writes: Vec::new(),
            };
            let mut trace = Vec::new();
            probe(&mut port, &mut trace, &options(mode)).unwrap();
            assert_eq!(
                port.writes,
                [
                    (0, vec![0x81, 1, 6, 1, 1, 1, 1, 3, 0xff]),
                    (stop_after, vec![0x81, 1, 6, 1, 1, 1, 3, 3, 0xff]),
                ]
            );
            let trace = String::from_utf8(trace).unwrap();
            assert!(trace.contains(&format!("Stop trigger: {trigger}")));
            assert!(trace.contains("RX [90, 52, FF]"));
        }
    }

    #[test]
    fn missing_start_reply_still_sends_zoom_stop() {
        let mut port = ScriptedPort {
            replies: &[],
            bytes_read: 0,
            writes: Vec::new(),
        };
        let options = Options {
            address: 2,
            timeout: Duration::ZERO,
            motion: Motion::Zoom,
            ..options(Mode::Overlap)
        };
        let mut trace = Vec::new();
        let error = probe(&mut port, &mut trace, &options).unwrap_err();

        assert_eq!(error.kind(), ErrorKind::TimedOut);
        assert_eq!(
            port.writes,
            [
                (0, vec![0x82, 1, 4, 7, 2, 0xff]),
                (0, vec![0x82, 1, 4, 7, 0, 0xff]),
            ]
        );
        assert!(
            String::from_utf8(trace)
                .unwrap()
                .contains("Stop trigger: fallback")
        );
    }
}
