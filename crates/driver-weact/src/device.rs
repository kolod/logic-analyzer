//! One open board used by one acquisition: start, read and stop.
//!
//! The device follows the behaviour contract (`specs/013-driver-weact/contracts/driver-behaviour.md`):
//! `start` drains stale data, submits the transfers and only then sends the start command;
//! `read` waits at most 40 ms, turns bytes into words and honours the set length; `stop` cancels
//! the transfers and sends no command (the board has none).

use std::sync::Arc;
use std::time::Duration;

use la_core::Error;
use la_devices::error as device_error;
use la_devices::{AppliedConfig, Device, DeviceDescriptor, ReadOutcome, ResolvedConfig};

use crate::protocol::{
    self, COMMAND_TIMEOUT, DRAIN_LIMIT, READ_WAIT, REQUEST_START, STOP_WAIT, buffer_plan,
    samples_from_bytes, start_payload,
};
use crate::usb::{BulkStream, StreamEvent, Timer, UsbLink};

/// The state of one capture.
struct Run {
    stream: Box<dyn BulkStream>,
    /// The enabled channels as a mask.
    mask: u32,
    /// The set length, if any.
    requested: Option<u64>,
    /// Samples handed to the engine so far.
    received: u64,
    /// Bytes of the last transfer not yet handed out.
    pending: Vec<u8>,
    /// How many bytes of `pending` have been handed out.
    taken: usize,
    /// The set length was reached: the next `read` says `Finished`.
    complete: bool,
}

/// An open board.
pub(crate) struct WeActDevice {
    descriptor: DeviceDescriptor,
    link: Box<dyn UsbLink>,
    timer: Arc<dyn Timer>,
    run: Option<Run>,
}

impl WeActDevice {
    pub(crate) fn new(
        descriptor: DeviceDescriptor,
        link: Box<dyn UsbLink>,
        timer: Arc<dyn Timer>,
    ) -> Self {
        Self {
            descriptor,
            link,
            timer,
            run: None,
        }
    }

    /// Reads and discards whatever an earlier capture left in the board's buffer: the board
    /// holds a few KB until it fills and resets, so old samples could be the first of the new
    /// capture. Done with the transfers already submitted, until the stream has been quiet for
    /// two packet periods or the limit is reached.
    fn drain(&self, stream: &mut dyn BulkStream, rate: u64) -> Result<(), Error> {
        // Two packets of 512 bytes at `rate` bytes a second, between 5 and 100 ms.
        let quiet_ns = 2_u64
            .saturating_mul(512)
            .saturating_mul(1_000_000_000)
            .checked_div(rate.max(1))
            .unwrap_or(100_000_000);
        let quiet = Duration::from_nanos(quiet_ns)
            .clamp(Duration::from_millis(5), Duration::from_millis(100));
        let deadline = self.timer.now().saturating_add(DRAIN_LIMIT);
        while self.timer.now() < deadline {
            match stream.wait(quiet) {
                StreamEvent::Data(_) | StreamEvent::Empty => {}
                StreamEvent::Timeout => return Ok(()),
                StreamEvent::Error(error) => return Err(error),
            }
        }
        Ok(())
    }
}

impl Device for WeActDevice {
    fn descriptor(&self) -> &DeviceDescriptor {
        &self.descriptor
    }

    fn start(&mut self, config: &ResolvedConfig) -> Result<AppliedConfig, Error> {
        if self.run.is_some() {
            return Err(device_error::invalid_state(
                "the board is already capturing".to_owned(),
            ));
        }
        let rate = config.sample_rate_hz();
        let payload = start_payload(rate)?;
        let plan = buffer_plan(rate)?;
        let mut stream = self.link.open_stream(plan)?;
        // Every failure from here on closes the stream: the board is left as it was found.
        if let Err(error) = stream
            .submit_all()
            .and_then(|()| self.drain(&mut *stream, rate))
        {
            stream.close(STOP_WAIT);
            return Err(error);
        }
        if let Err(error) = self
            .link
            .control_out(REQUEST_START, 0, 0, &payload, COMMAND_TIMEOUT)
        {
            stream.close(STOP_WAIT);
            return Err(error);
        }
        self.run = Some(Run {
            stream,
            mask: config.enabled_channels(),
            requested: config.capture_length(),
            received: 0,
            pending: Vec::new(),
            taken: 0,
            complete: false,
        });
        Ok(AppliedConfig::new(rate))
    }

    fn read(&mut self, max_samples: usize, out: &mut Vec<u32>) -> Result<ReadOutcome, Error> {
        let Some(run) = self.run.as_mut() else {
            return Err(device_error::invalid_state(
                "the board is not capturing".to_owned(),
            ));
        };
        if run.complete {
            return Ok(ReadOutcome::Finished);
        }
        if max_samples == 0 {
            return Ok(ReadOutcome::Idle);
        }
        if run.taken >= run.pending.len() {
            match run.stream.wait(READ_WAIT) {
                StreamEvent::Data(bytes) => {
                    run.pending = bytes;
                    run.taken = 0;
                }
                StreamEvent::Empty | StreamEvent::Timeout => return Ok(ReadOutcome::Idle),
                StreamEvent::Error(error) => return Err(error),
            }
        }
        let available = run.pending.len().saturating_sub(run.taken);
        let allowed = run
            .requested
            .map_or(u64::MAX, |length| length.saturating_sub(run.received));
        let take = available
            .min(max_samples)
            .min(usize::try_from(allowed).unwrap_or(usize::MAX));
        let end = run.taken.saturating_add(take);
        if let Some(bytes) = run.pending.get(run.taken..end) {
            samples_from_bytes(bytes, run.mask, out);
        }
        run.taken = end;
        run.received = run
            .received
            .saturating_add(u64::try_from(take).unwrap_or(u64::MAX));
        if run.requested.is_some_and(|length| run.received >= length) {
            run.complete = true;
        }
        if take == 0 {
            return Ok(ReadOutcome::Idle);
        }
        Ok(ReadOutcome::Samples)
    }

    fn stop(&mut self) -> Result<(), Error> {
        if let Some(mut run) = self.run.take() {
            run.stream.close(STOP_WAIT);
        }
        Ok(())
    }
}

impl Drop for WeActDevice {
    fn drop(&mut self) {
        if let Some(mut run) = self.run.take() {
            run.stream.close(STOP_WAIT);
        }
    }
}

/// The version check of a freshly opened board: the firmware must be of the major version the
/// driver works with.
///
/// # Errors
///
/// `weact.firmware_rejected` for another major version, `device.transport` for an answer of
/// the wrong length, or the error of the control request.
pub(crate) fn check_firmware(link: &mut dyn UsbLink) -> Result<protocol::Version, Error> {
    let answer = link.control_in(protocol::REQUEST_GET_FW_VERSION, 0, 0, 2, COMMAND_TIMEOUT)?;
    let version = protocol::parse_version(&answer)?;
    if version.major != protocol::REQUIRED_MAJOR {
        return Err(crate::error::firmware_rejected(format!(
            "the board reports firmware version {}.{}; this driver needs major version {}",
            version.major,
            version.minor,
            protocol::REQUIRED_MAJOR
        )));
    }
    tracing::info!(
        target: "la_driver_weact",
        major = version.major,
        minor = version.minor,
        "the firmware of the board answered"
    );
    Ok(version)
}
