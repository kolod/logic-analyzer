//! One open board: starts a capture with a request, reads the samples the board returns and aborts.

use std::time::Duration;

use la_core::Result;
use la_devices::{
    AppliedConfig, Device, DeviceDescriptor, READ_TIMEOUT, ReadOutcome, ResolvedConfig,
    START_STOP_TIMEOUT,
};
use la_proto::{
    CaptureRequest2, DataHeader, Encoding, HEADER_LEN, LossMark, MARK_LEN, check_marks, command,
    encode_frame, unpack_raw,
};

use crate::capabilities::build_request;
use crate::error;
use crate::link::SerialLink;

/// How long the stream may be silent, once it has begun, before the capture is called stopped.
const STALL: Duration = Duration::from_secs(2);
/// The most bytes read from the port in one call.
const CHUNK: usize = 16 * 1024;
/// The longest reply line the driver accepts.
const MAX_LINE: usize = 256;
/// The byte that aborts a capture (any byte does).
const ABORT: u8 = 0x00;

/// Where the data of a capture is.
enum Phase {
    /// Waiting for the head: the encoding, the sample count and the number of loss marks.
    Header { bytes: [u8; HEADER_LEN], got: usize },
    /// Reading the loss marks.
    Marks { bytes: Vec<u8>, left: usize },
    /// Reading samples.
    Samples,
    /// Reading the burst times: the count, then four bytes each.
    Times { count: Option<u8>, left: usize },
    /// Everything was read.
    Done,
}

/// A capture in progress.
struct Running {
    phase: Phase,
    channels: Vec<u8>,
    per: usize,
    /// The samples the board announced.
    promised: u64,
    /// The most samples the request allows.
    most: u64,
    received: u64,
    /// The losses still to be reported, each before the stored sample at its position.
    marks: std::collections::VecDeque<LossMark>,
    /// Bytes of a sample that has not arrived whole.
    pending: Vec<u8>,
    silent: Duration,
}

/// The driver of one board.
pub struct Rp2350bDevice {
    descriptor: DeviceDescriptor,
    port: String,
    link: Box<dyn SerialLink>,
    running: Option<Running>,
}

impl Rp2350bDevice {
    pub(crate) fn new(
        descriptor: DeviceDescriptor,
        port: String,
        link: Box<dyn SerialLink>,
    ) -> Self {
        Self {
            descriptor,
            port,
            link,
            running: None,
        }
    }

    fn send(&mut self, request: &CaptureRequest2) -> Result<()> {
        let mut body = [0_u8; 1 + la_proto::REQUEST2_LEN];
        let mut frame = [0_u8; 4 + 2 * (1 + la_proto::REQUEST2_LEN)];
        if let Some((first, rest)) = body.split_first_mut() {
            *first = command::CAPTURE2;
            rest.copy_from_slice(&request.to_bytes());
        }
        let n = encode_frame(&body, &mut frame)
            .map_err(|e| error::protocol(&format!("the request could not be framed: {e}")))?;
        self.link.write_all(frame.get(..n).unwrap_or(&[]))
    }

    /// Reads one reply line (without its end), waiting at most [`START_STOP_TIMEOUT`].
    fn line(&mut self) -> Result<String> {
        read_line(
            &mut *self.link,
            "waiting for the answer to a capture request",
        )
    }

    fn fail_on_idle(running: &mut Running, step: Duration, what: Option<(u64, u64)>) -> Result<()> {
        running.silent = running.silent.saturating_add(step);
        if running.silent < STALL {
            return Ok(());
        }
        Err(match what {
            Some((received, promised)) => error::stream_stopped(received, promised),
            None => error::timeout(
                "reading the burst times",
                u64::try_from(STALL.as_millis()).unwrap_or(u64::MAX),
            ),
        })
    }
    /// Sends `request` and waits for the board to accept it; the capture then runs on the board.
    pub(crate) fn start_request(
        &mut self,
        request: &CaptureRequest2,
        rate_hz: u64,
    ) -> Result<AppliedConfig> {
        self.send(request)?;
        let answer = self.line()?;
        match answer.as_str() {
            "CAPTURE_STARTED" => {}
            "CAPTURE_ERROR" => {
                let field = self.line().unwrap_or_default();
                return Err(error::request_refused(&format!("CAPTURE_ERROR {field}")));
            }
            other if other.starts_with("ERR_") => return Err(error::request_refused(other)),
            _ => {
                return Err(error::protocol(
                    "the answer to a capture request is not CAPTURE_STARTED",
                ));
            }
        }
        self.running = Some(Running {
            phase: Phase::Header {
                bytes: [0; HEADER_LEN],
                got: 0,
            },
            channels: request.used_channels().to_vec(),
            per: request.bytes_per_sample(),
            promised: 0,
            most: u64::from(request.samples_before)
                .saturating_add(u64::from(request.samples_after)),
            received: 0,
            marks: std::collections::VecDeque::new(),
            pending: Vec::new(),
            silent: Duration::ZERO,
        });
        Ok(AppliedConfig::new(rate_hz))
    }

    /// The reads of [`Device::read`] with the time a read waits given: a chain of boards reads each board for a
    /// share of [`READ_TIMEOUT`], so that all of them together stay within it.
    pub(crate) fn read_within(
        &mut self,
        max_samples: usize,
        out: &mut Vec<u32>,
        timeout: Duration,
    ) -> Result<ReadOutcome> {
        let Some(running) = self.running.as_mut() else {
            return Err(la_devices::error::invalid_state(
                "the board was read before a capture was started".to_owned(),
            ));
        };
        loop {
            match &mut running.phase {
                Phase::Header { bytes, got } => {
                    let wanted = bytes.get_mut(*got..).unwrap_or(&mut []);
                    let n = self.link.read(wanted, timeout)?;
                    if n == 0 {
                        // Waiting for the trigger: the board is alive, the capture has not begun.
                        return Ok(ReadOutcome::Idle);
                    }
                    *got = got.saturating_add(n);
                    if *got < HEADER_LEN {
                        continue;
                    }
                    let head = DataHeader::from_bytes(*bytes).map_err(|e| {
                        error::protocol(&format!("the data head is not valid: {e}"))
                    })?;
                    if head.encoding != Encoding::Raw {
                        return Err(error::protocol(
                            "compact samples arrived but were not requested",
                        ));
                    }
                    let promised = u64::from(head.samples);
                    if promised > running.most {
                        return Err(error::protocol(
                            "the board announced more samples than were requested",
                        ));
                    }
                    running.promised = promised;
                    running.silent = Duration::ZERO;
                    let left = usize::from(head.marks).saturating_mul(MARK_LEN);
                    running.phase = if left > 0 {
                        Phase::Marks {
                            bytes: Vec::with_capacity(left),
                            left,
                        }
                    } else {
                        after_marks(running.promised, false)
                    };
                }
                Phase::Marks { bytes, left } => {
                    let mut buffer = [0_u8; 256];
                    let wanted = (*left).min(buffer.len());
                    let n = self
                        .link
                        .read(buffer.get_mut(..wanted).unwrap_or(&mut []), timeout)?;
                    if n == 0 {
                        Self::fail_on_idle(
                            running,
                            timeout,
                            Some((running.received, running.promised)),
                        )?;
                        return Ok(ReadOutcome::Idle);
                    }
                    running.silent = Duration::ZERO;
                    bytes.extend_from_slice(buffer.get(..n).unwrap_or(&[]));
                    *left = left.saturating_sub(n);
                    if *left > 0 {
                        continue;
                    }
                    let marks: Vec<LossMark> = bytes
                        .chunks_exact(MARK_LEN)
                        .filter_map(|chunk| <[u8; MARK_LEN]>::try_from(chunk).ok())
                        .map(LossMark::from_bytes)
                        .collect();
                    let stored = u32::try_from(running.promised).unwrap_or(u32::MAX);
                    check_marks(&marks, stored, running.most).map_err(|e| {
                        error::protocol(&format!("the loss marks are not valid: {e}"))
                    })?;
                    running.marks = marks.into();
                    running.phase = after_marks(running.promised, !running.marks.is_empty());
                }
                Phase::Samples => {
                    // A loss is reported where it happened: before the stored sample at its position.
                    if let Some(mark) = running.marks.front().copied() {
                        if u64::from(mark.position) == running.received {
                            running.marks.pop_front();
                            return Ok(ReadOutcome::Lost(u64::from(mark.lost)));
                        }
                    }
                    let remaining = running.promised.saturating_sub(running.received);
                    if remaining == 0 && running.pending.is_empty() {
                        running.phase = Phase::Times {
                            count: None,
                            left: 1,
                        };
                        continue;
                    }
                    // Not past the next loss: the samples before it come first.
                    let until_loss = running.marks.front().map_or(remaining, |mark| {
                        u64::from(mark.position).saturating_sub(running.received)
                    });
                    let samples = usize::try_from(remaining.min(until_loss))
                        .unwrap_or(usize::MAX)
                        .min(max_samples.max(1));
                    let wanted = samples
                        .saturating_mul(running.per)
                        .saturating_sub(running.pending.len())
                        .min(CHUNK);
                    let mut buffer = vec![0_u8; wanted.max(1)];
                    let n = self.link.read(&mut buffer, timeout)?;
                    if n == 0 {
                        Self::fail_on_idle(
                            running,
                            timeout,
                            Some((running.received, running.promised)),
                        )?;
                        return Ok(ReadOutcome::Idle);
                    }
                    running.silent = Duration::ZERO;
                    running
                        .pending
                        .extend_from_slice(buffer.get(..n).unwrap_or(&[]));
                    let whole = running.pending.len().checked_div(running.per).unwrap_or(0);
                    let take = whole.saturating_mul(running.per);
                    let raw: Vec<u8> = running.pending.drain(..take).collect();
                    let start = out.len();
                    out.resize(start.saturating_add(whole), 0);
                    let target = out.get_mut(start..).unwrap_or(&mut []);
                    unpack_raw(&raw, &running.channels, target).map_err(|e| {
                        error::protocol(&format!("samples could not be unpacked: {e}"))
                    })?;
                    running.received = running
                        .received
                        .saturating_add(u64::try_from(whole).unwrap_or(0));
                    if whole > 0 {
                        return Ok(ReadOutcome::Samples);
                    }
                }
                Phase::Times { count, left } => {
                    let mut buffer = [0_u8; 64];
                    let wanted = (*left).min(buffer.len());
                    let n = self
                        .link
                        .read(buffer.get_mut(..wanted).unwrap_or(&mut []), timeout)?;
                    if n == 0 {
                        Self::fail_on_idle(running, timeout, None)?;
                        return Ok(ReadOutcome::Idle);
                    }
                    running.silent = Duration::ZERO;
                    *left = left.saturating_sub(n);
                    if count.is_none() {
                        let times = buffer.first().copied().unwrap_or(0);
                        *count = Some(times);
                        *left = usize::from(times).saturating_mul(4);
                    }
                    if *left == 0 {
                        running.phase = Phase::Done;
                    }
                }
                Phase::Done => return Ok(ReadOutcome::Finished),
            }
        }
    }
}

impl Device for Rp2350bDevice {
    fn descriptor(&self) -> &DeviceDescriptor {
        &self.descriptor
    }

    fn start(&mut self, config: &ResolvedConfig) -> Result<AppliedConfig> {
        let request = build_request(config)?;
        self.start_request(&request, config.sample_rate_hz())
    }

    fn read(&mut self, max_samples: usize, out: &mut Vec<u32>) -> Result<ReadOutcome> {
        self.read_within(max_samples, out, READ_TIMEOUT)
    }

    fn stop(&mut self) -> Result<()> {
        let Some(running) = self.running.take() else {
            return Ok(());
        };
        if matches!(running.phase, Phase::Done) {
            return Ok(());
        }
        // Any byte aborts; then what the board still sends is read and dropped, so that the next
        // capture starts on a quiet port.
        self.link.write_all(&[ABORT])?;
        let mut buffer = [0_u8; CHUNK];
        let (mut quiet, mut waited) = (0_u32, Duration::ZERO);
        while quiet < 2 && waited < START_STOP_TIMEOUT {
            if self.link.read(&mut buffer, READ_TIMEOUT)? == 0 {
                quiet = quiet.saturating_add(1);
                waited = waited.saturating_add(READ_TIMEOUT);
            } else {
                quiet = 0;
            }
        }
        tracing::debug!(
            target: "la_driver_rp2350b",
            port = %self.port,
            received = running.received,
            "capture stopped"
        );
        Ok(())
    }
}

/// Where the data goes after the head and the marks: the samples (and the losses between them), or the burst
/// times if there is neither.
fn after_marks(promised: u64, has_losses: bool) -> Phase {
    if promised == 0 && !has_losses {
        Phase::Times {
            count: None,
            left: 1,
        }
    } else {
        Phase::Samples
    }
}

/// Reads one reply line from `link` (without its end), waiting at most [`START_STOP_TIMEOUT`]; `doing` says
/// what was waited for if the time runs out.
pub(crate) fn read_line(link: &mut dyn SerialLink, doing: &str) -> Result<String> {
    let mut line = Vec::new();
    let mut waited = Duration::ZERO;
    let mut byte = [0_u8; 1];
    loop {
        if link.read(&mut byte, READ_TIMEOUT)? == 0 {
            waited = waited.saturating_add(READ_TIMEOUT);
            if waited >= START_STOP_TIMEOUT {
                return Err(error::timeout(
                    doing,
                    u64::try_from(waited.as_millis()).unwrap_or(u64::MAX),
                ));
            }
            continue;
        }
        match byte.first() {
            Some(b'\n') => break,
            Some(b'\r') => {}
            Some(&b) => {
                if line.len() >= MAX_LINE {
                    return Err(error::protocol("a reply line is too long"));
                }
                line.push(b);
            }
            None => {}
        }
    }
    String::from_utf8(line).map_err(|_| error::protocol("a reply line is not text"))
}
