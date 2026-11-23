//! One open board: starts a capture with a request, reads the samples the board returns and aborts.

use std::time::Duration;

use la_core::Result;
use la_devices::{
    AppliedConfig, Device, DeviceDescriptor, READ_TIMEOUT, ReadOutcome, ResolvedConfig,
    START_STOP_TIMEOUT,
};
use la_proto::{CaptureRequest2, command, encode_frame, unpack_raw};

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
    /// Waiting for the encoding byte and the sample count.
    Header { bytes: [u8; 5], got: usize },
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
        let mut line = Vec::new();
        let mut waited = Duration::ZERO;
        let mut byte = [0_u8; 1];
        loop {
            if self.link.read(&mut byte, READ_TIMEOUT)? == 0 {
                waited = waited.saturating_add(READ_TIMEOUT);
                if waited >= START_STOP_TIMEOUT {
                    return Err(error::timeout(
                        "waiting for the answer to a capture request",
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

    fn fail_on_idle(running: &mut Running, what: Option<(u64, u64)>) -> Result<()> {
        running.silent = running.silent.saturating_add(READ_TIMEOUT);
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
}

impl Device for Rp2350bDevice {
    fn descriptor(&self) -> &DeviceDescriptor {
        &self.descriptor
    }

    fn start(&mut self, config: &ResolvedConfig) -> Result<AppliedConfig> {
        let request = build_request(config)?;
        self.send(&request)?;
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
                bytes: [0; 5],
                got: 0,
            },
            channels: request.used_channels().to_vec(),
            per: request.bytes_per_sample(),
            promised: 0,
            most: u64::from(request.samples_before)
                .saturating_add(u64::from(request.samples_after)),
            received: 0,
            pending: Vec::new(),
            silent: Duration::ZERO,
        });
        Ok(AppliedConfig::new(config.sample_rate_hz()))
    }

    fn read(&mut self, max_samples: usize, out: &mut Vec<u32>) -> Result<ReadOutcome> {
        let Some(running) = self.running.as_mut() else {
            return Err(la_devices::error::invalid_state(
                "the board was read before a capture was started".to_owned(),
            ));
        };
        loop {
            match &mut running.phase {
                Phase::Header { bytes, got } => {
                    let wanted = bytes.get_mut(*got..).unwrap_or(&mut []);
                    let n = self.link.read(wanted, READ_TIMEOUT)?;
                    if n == 0 {
                        // Waiting for the trigger: the board is alive, the capture has not begun.
                        return Ok(ReadOutcome::Idle);
                    }
                    *got = got.saturating_add(n);
                    if *got < 5 {
                        continue;
                    }
                    let [encoding, a, b, c, d] = *bytes;
                    if encoding != 0 {
                        return Err(error::protocol(
                            "compact samples arrived but were not requested",
                        ));
                    }
                    let promised = u64::from(u32::from_le_bytes([a, b, c, d]));
                    if promised > running.most {
                        return Err(error::protocol(
                            "the board announced more samples than were requested",
                        ));
                    }
                    running.promised = promised;
                    running.silent = Duration::ZERO;
                    running.phase = if promised == 0 {
                        Phase::Times {
                            count: None,
                            left: 1,
                        }
                    } else {
                        Phase::Samples
                    };
                }
                Phase::Samples => {
                    let remaining = running.promised.saturating_sub(running.received);
                    if remaining == 0 && running.pending.is_empty() {
                        running.phase = Phase::Times {
                            count: None,
                            left: 1,
                        };
                        continue;
                    }
                    let samples = usize::try_from(remaining)
                        .unwrap_or(usize::MAX)
                        .min(max_samples.max(1));
                    let wanted = samples
                        .saturating_mul(running.per)
                        .saturating_sub(running.pending.len())
                        .min(CHUNK);
                    let mut buffer = vec![0_u8; wanted.max(1)];
                    let n = self.link.read(&mut buffer, READ_TIMEOUT)?;
                    if n == 0 {
                        Self::fail_on_idle(running, Some((running.received, running.promised)))?;
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
                        .read(buffer.get_mut(..wanted).unwrap_or(&mut []), READ_TIMEOUT)?;
                    if n == 0 {
                        Self::fail_on_idle(running, None)?;
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
