//! Recordings: the text format of an exchange with a board, a parser and writer, a way to turn
//! one into a script of the scripted bus, and a wrapper that records what passes through a real
//! link (feature `test-support`).
//!
//! A recording is a UTF-8 text file. The first line is `la-recording 1 <origin>`; every other line
//! is one step (blank lines are skipped, `# text` is a note):
//!
//! | Line | Meaning |
//! |------|---------|
//! | `appear blank\|running [driver]` | the board is plugged in (without or with the firmware) |
//! | `disappear` | the board is unplugged |
//! | `renumerate <ms>\|never` | how long after the download the board returns |
//! | `version <hex>` / `revid <hex>` | the answer to the version / revision request |
//! | `fail-open <failure>` | opening the board is refused |
//! | `stale <hex>` | bytes left in the board's buffer by an earlier capture |
//! | `data <hex>` / `short <hex>` | a completed transfer with these bytes |
//! | `empty` / `timeout` | a transfer without bytes / nothing completes |
//! | `disconnect` / `error <failure>` | a transfer fails |
//! | `expect <words>` | what the replay must find (read by the test that replays) |
//!
//! A failure is `disconnected`, `transport`, `access_denied`, `driver_missing [driver]` or
//! `in_use`. The parser never panics: every defect is a typed error with the line number.

use std::fmt::Write as _;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use la_core::Error;
use la_devices::error as device_error;
use la_diagnostics::lock_recover;

use crate::protocol::{BufferPlan, REQUEST_GET_FW_VERSION, REQUEST_GET_REVID};
use crate::testing::{BoardSpec, Capture, Fail, RecordedUsb, StreamStep};
use crate::usb::{BoardKey, BulkStream, StreamEvent, UsbBackend, UsbInfo, UsbLink};

/// The version of the format this code reads and writes.
pub const VERSION: u32 = 1;
/// The longest line accepted, in bytes.
pub const MAX_LINE: usize = 65_536;
/// The most steps accepted in one recording.
pub const MAX_STEPS: usize = 50_000;

/// One line of a recording.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Step {
    /// A note for the reader.
    Note(String),
    /// What the replay must find, in words the replaying test reads.
    Expect(String),
    /// The board is plugged in.
    Appear {
        /// Whether it runs the firmware.
        running: bool,
        /// The bound driver name, if given.
        driver: Option<String>,
    },
    /// The board is unplugged.
    Disappear,
    /// The delay of the return after the download (`None`: never).
    Renumerate(Option<u64>),
    /// The answer to the version request.
    Version(Vec<u8>),
    /// The answer to the revision request.
    Revid(Vec<u8>),
    /// Opening the board is refused.
    FailOpen(Fail),
    /// Stale bytes in the board's buffer.
    Stale(Vec<u8>),
    /// A transfer with bytes.
    Data(Vec<u8>),
    /// A transfer with fewer bytes than its buffer (replayed like `Data`).
    Short(Vec<u8>),
    /// A transfer without bytes.
    Empty,
    /// Nothing completes.
    Timeout,
    /// The board is unplugged during the stream.
    Disconnect,
    /// A transfer fails.
    Error(Fail),
}

/// A recording: where it came from and the steps.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Recording {
    /// Where it came from, in words.
    pub origin: String,
    /// The steps in order.
    pub steps: Vec<Step>,
}

fn bad(line: usize, what: impl std::fmt::Display) -> Error {
    device_error::driver_error(format!("recording, line {line}: {what}"))
}

fn digit(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte.wrapping_sub(b'0')),
        b'a'..=b'f' => Some(byte.wrapping_sub(b'a').wrapping_add(10)),
        b'A'..=b'F' => Some(byte.wrapping_sub(b'A').wrapping_add(10)),
        _ => None,
    }
}

fn hex_to_bytes(line: usize, text: &str) -> Result<Vec<u8>, Error> {
    let digits = text.as_bytes();
    if digits.len() % 2 != 0 {
        return Err(bad(line, "an odd number of hex digits"));
    }
    digits
        .chunks_exact(2)
        .map(|pair| {
            let mut it = pair.iter().copied().map(digit);
            match (it.next().flatten(), it.next().flatten()) {
                (Some(high), Some(low)) => Ok(high.wrapping_shl(4) | low),
                _ => Err(bad(line, "a character that is not a hex digit")),
            }
        })
        .collect()
}

fn bytes_to_hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len().saturating_mul(2));
    for byte in bytes {
        let _ = write!(out, "{byte:02x}");
    }
    out
}

fn fail_from_text(line: usize, text: &str) -> Result<Fail, Error> {
    let mut words = text.split_whitespace();
    let fail = match words.next() {
        Some("disconnected") => Fail::Disconnected,
        Some("transport") => Fail::Transport,
        Some("access_denied") => Fail::AccessDenied,
        Some("in_use") => Fail::InUse,
        Some("driver_missing") => Fail::DriverMissing(words.next().map(str::to_owned)),
        _ => return Err(bad(line, format!("an unknown failure `{text}`"))),
    };
    if words.next().is_some() {
        return Err(bad(line, "text after the failure"));
    }
    Ok(fail)
}

fn fail_to_text(fail: &Fail) -> String {
    match fail {
        Fail::Disconnected => "disconnected".to_owned(),
        Fail::Transport => "transport".to_owned(),
        Fail::AccessDenied => "access_denied".to_owned(),
        Fail::InUse => "in_use".to_owned(),
        Fail::DriverMissing(None) => "driver_missing".to_owned(),
        Fail::DriverMissing(Some(driver)) => format!("driver_missing {driver}"),
    }
}

fn parse_step(number: usize, line: &str) -> Result<Step, Error> {
    let line = line.trim_end();
    if let Some(note) = line.strip_prefix('#') {
        return Ok(Step::Note(
            note.strip_prefix(' ').unwrap_or(note).to_owned(),
        ));
    }
    let (word, arg) = line
        .split_once(' ')
        .map_or((line, ""), |(word, arg)| (word, arg.trim()));
    let no_argument = |step: Step| {
        if arg.is_empty() {
            Ok(step)
        } else {
            Err(bad(number, format!("`{word}` takes no argument")))
        }
    };
    let hex = |make: fn(Vec<u8>) -> Step| {
        let bytes = hex_to_bytes(number, arg)?;
        if bytes.is_empty() {
            return Err(bad(number, format!("`{word}` needs at least one byte")));
        }
        Ok(make(bytes))
    };
    match word {
        "expect" => Ok(Step::Expect(arg.to_owned())),
        "appear" => {
            let mut words = arg.split_whitespace();
            let running = match words.next() {
                Some("blank") => false,
                Some("running") => true,
                _ => return Err(bad(number, "`appear` needs `blank` or `running`")),
            };
            let driver = words.next().map(str::to_owned);
            if words.next().is_some() {
                return Err(bad(number, "text after the driver"));
            }
            Ok(Step::Appear { running, driver })
        }
        "disappear" => no_argument(Step::Disappear),
        "renumerate" => {
            if arg == "never" {
                return Ok(Step::Renumerate(None));
            }
            arg.parse::<u64>()
                .map(|ms| Step::Renumerate(Some(ms)))
                .map_err(|_| bad(number, "`renumerate` needs milliseconds or `never`"))
        }
        "version" => hex(Step::Version),
        "revid" => hex(Step::Revid),
        "stale" => hex(Step::Stale),
        "data" => hex(Step::Data),
        "short" => hex(Step::Short),
        "empty" => no_argument(Step::Empty),
        "timeout" => no_argument(Step::Timeout),
        "disconnect" => no_argument(Step::Disconnect),
        "error" => fail_from_text(number, arg).map(Step::Error),
        "fail-open" => fail_from_text(number, arg).map(Step::FailOpen),
        other => Err(bad(number, format!("an unknown step `{other}`"))),
    }
}

fn step_to_line(step: &Step) -> String {
    match step {
        Step::Note(text) => format!("# {text}"),
        Step::Expect(text) => format!("expect {text}"),
        Step::Appear { running, driver } => {
            let kind = if *running { "running" } else { "blank" };
            driver.as_ref().map_or_else(
                || format!("appear {kind}"),
                |d| format!("appear {kind} {d}"),
            )
        }
        Step::Disappear => "disappear".to_owned(),
        Step::Renumerate(None) => "renumerate never".to_owned(),
        Step::Renumerate(Some(ms)) => format!("renumerate {ms}"),
        Step::Version(bytes) => format!("version {}", bytes_to_hex(bytes)),
        Step::Revid(bytes) => format!("revid {}", bytes_to_hex(bytes)),
        Step::FailOpen(fail) => format!("fail-open {}", fail_to_text(fail)),
        Step::Stale(bytes) => format!("stale {}", bytes_to_hex(bytes)),
        Step::Data(bytes) => format!("data {}", bytes_to_hex(bytes)),
        Step::Short(bytes) => format!("short {}", bytes_to_hex(bytes)),
        Step::Empty => "empty".to_owned(),
        Step::Timeout => "timeout".to_owned(),
        Step::Disconnect => "disconnect".to_owned(),
        Step::Error(fail) => format!("error {}", fail_to_text(fail)),
    }
}

impl Recording {
    /// Reads a recording.
    ///
    /// # Errors
    ///
    /// A typed error naming the line for text that is not UTF-8, a missing header, a version this
    /// code does not know, a line longer than [`MAX_LINE`], more than [`MAX_STEPS`] steps, an
    /// unknown step or a bad argument.
    pub fn parse(bytes: &[u8]) -> Result<Self, Error> {
        let text = std::str::from_utf8(bytes)
            .map_err(|error| bad(0, format!("the file is not UTF-8 ({error})")))?;
        let mut lines = text.lines().enumerate();
        let Some((_, first)) = lines.next() else {
            return Err(bad(1, "the header line is missing"));
        };
        if first.len() > MAX_LINE {
            return Err(bad(1, "the line is too long"));
        }
        let Some(rest) = first.strip_prefix("la-recording ") else {
            return Err(bad(
                1,
                "the header `la-recording <version> <origin>` is missing",
            ));
        };
        let (version, origin) = rest.split_once(' ').unwrap_or((rest, ""));
        match version.parse::<u32>() {
            Ok(VERSION) => {}
            Ok(other) => {
                return Err(bad(
                    1,
                    format!("version {other} is not known (this reads {VERSION})"),
                ));
            }
            Err(_) => return Err(bad(1, "the version is not a number")),
        }
        let mut steps = Vec::new();
        for (index, line) in lines {
            let number = index.saturating_add(1);
            if line.len() > MAX_LINE {
                return Err(bad(number, "the line is too long"));
            }
            if line.trim().is_empty() {
                continue;
            }
            if steps.len() >= MAX_STEPS {
                return Err(bad(number, format!("more than {MAX_STEPS} steps")));
            }
            steps.push(parse_step(number, line)?);
        }
        Ok(Self {
            origin: origin.trim().to_owned(),
            steps,
        })
    }

    /// The text of the recording; [`parse`](Self::parse) of it gives the same recording.
    #[must_use]
    pub fn to_text(&self) -> String {
        let mut out = format!("la-recording {VERSION} {}\n", self.origin);
        for step in &self.steps {
            out.push_str(&step_to_line(step));
            out.push('\n');
        }
        out
    }

    /// What the replay must find: the text after every `expect`.
    #[must_use]
    pub fn expectations(&self) -> Vec<&str> {
        self.steps
            .iter()
            .filter_map(|step| match step {
                Step::Expect(text) => Some(text.as_str()),
                _ => None,
            })
            .collect()
    }

    /// Sets the scripted bus up as the recording says, for the board at `key`: the board, the
    /// answers, and one capture made of the stale bytes and the stream.
    pub fn apply(&self, usb: &RecordedUsb, key: &BoardKey) {
        let mut capture = Capture::default();
        for step in &self.steps {
            match step {
                Step::Note(_) | Step::Expect(_) | Step::Revid(_) => {}
                Step::Appear { running, driver } => {
                    let mut spec = if *running {
                        BoardSpec::running(key.bus(), key.ports())
                    } else {
                        BoardSpec::blank(key.bus(), key.ports())
                    };
                    if driver.is_some() {
                        spec.driver.clone_from(driver);
                    }
                    usb.plug(spec);
                }
                Step::Disappear => usb.unplug(key),
                Step::Renumerate(ms) => {
                    usb.renumerate_after(key, ms.map(Duration::from_millis));
                }
                Step::Version(bytes) => usb.set_version(key, bytes),
                Step::FailOpen(fail) => usb.fail_open(key, Some(fail.clone())),
                Step::Stale(bytes) => capture.stale.push(StreamStep::Data(bytes.clone())),
                Step::Data(bytes) | Step::Short(bytes) => {
                    capture.stream.push(StreamStep::Data(bytes.clone()));
                }
                Step::Empty => capture.stream.push(StreamStep::Empty),
                Step::Timeout => capture.stream.push(StreamStep::Timeout),
                Step::Disconnect => capture.stream.push(StreamStep::Fail(Fail::Disconnected)),
                Step::Error(fail) => capture.stream.push(StreamStep::Fail(fail.clone())),
            }
        }
        usb.script_capture(key, capture);
    }
}

/// The steps recorded so far, shared between the wrappers and whoever writes the file.
pub type Sink = Arc<Mutex<Vec<Step>>>;

fn fail_of(error: &Error) -> Fail {
    if error.code() == device_error::DISCONNECTED {
        Fail::Disconnected
    } else {
        Fail::Transport
    }
}

/// A backend that records what passes through the links it opens.
pub struct RecordingBackend {
    inner: Arc<dyn UsbBackend>,
    sink: Sink,
}

impl RecordingBackend {
    /// Wraps `inner`; the steps go to `sink`.
    #[must_use]
    pub fn new(inner: Arc<dyn UsbBackend>, sink: Sink) -> Self {
        Self { inner, sink }
    }
}

impl UsbBackend for RecordingBackend {
    fn list(&self) -> Result<Vec<UsbInfo>, Error> {
        self.inner.list()
    }

    fn open(&self, key: &BoardKey) -> Result<Box<dyn UsbLink>, Error> {
        let link = self.inner.open(key)?;
        Ok(Box::new(RecordingLink::new(link, Arc::clone(&self.sink))))
    }
}

/// A link that records the answers of the board and the stream, and passes everything on.
pub struct RecordingLink {
    inner: Box<dyn UsbLink>,
    sink: Sink,
}

impl RecordingLink {
    /// Wraps `inner`; the steps go to `sink`.
    #[must_use]
    pub fn new(inner: Box<dyn UsbLink>, sink: Sink) -> Self {
        Self { inner, sink }
    }
}

impl UsbLink for RecordingLink {
    fn control_in(
        &mut self,
        request: u8,
        value: u16,
        index: u16,
        length: u16,
        timeout: Duration,
    ) -> Result<Vec<u8>, Error> {
        let answer = self
            .inner
            .control_in(request, value, index, length, timeout)?;
        let step = match request {
            REQUEST_GET_FW_VERSION if !answer.is_empty() => Some(Step::Version(answer.clone())),
            REQUEST_GET_REVID if !answer.is_empty() => Some(Step::Revid(answer.clone())),
            _ => None,
        };
        if let Some(step) = step {
            lock_recover(&self.sink).push(step);
        }
        Ok(answer)
    }

    fn control_out(
        &mut self,
        request: u8,
        value: u16,
        index: u16,
        data: &[u8],
        timeout: Duration,
    ) -> Result<(), Error> {
        self.inner.control_out(request, value, index, data, timeout)
    }

    fn open_stream(&mut self, plan: BufferPlan) -> Result<Box<dyn BulkStream>, Error> {
        let stream = self.inner.open_stream(plan)?;
        Ok(Box::new(RecordingStream {
            inner: stream,
            sink: Arc::clone(&self.sink),
        }))
    }
}

struct RecordingStream {
    inner: Box<dyn BulkStream>,
    sink: Sink,
}

impl BulkStream for RecordingStream {
    fn submit_all(&mut self) -> Result<(), Error> {
        self.inner.submit_all()
    }

    fn wait(&mut self, timeout: Duration) -> StreamEvent {
        let event = self.inner.wait(timeout);
        let step = match &event {
            StreamEvent::Data(bytes) => Some(Step::Data(bytes.clone())),
            StreamEvent::Empty => Some(Step::Empty),
            StreamEvent::Timeout => Some(Step::Timeout),
            StreamEvent::Error(error) => Some(Step::Error(fail_of(error))),
        };
        if let Some(step) = step {
            let mut steps = lock_recover(&self.sink);
            // A quiet board would fill the file with timeouts: one stands for a run of them.
            if !(step == Step::Timeout && steps.last() == Some(&Step::Timeout)) {
                steps.push(step);
            }
        }
        event
    }

    fn close(&mut self, within: Duration) {
        self.inner.close(within);
    }
}

/// The recording of what a sink holds, with the origin and the expectation given.
#[must_use]
pub fn recording_of(sink: &Sink, origin: &str, expect: &str) -> Recording {
    let mut steps = vec![Step::Expect(expect.to_owned())];
    steps.extend(lock_recover(sink).iter().cloned());
    Recording {
        origin: origin.to_owned(),
        steps,
    }
}
