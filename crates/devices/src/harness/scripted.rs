//! A device that replays a script, and a wrapper that counts calls.

use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use la_core::{Error, Result};

use crate::capabilities::DeviceDescriptor;
use crate::config::{AppliedConfig, ResolvedConfig};
use crate::driver::{Device, Flow, ReadOutcome};

/// One step of a [`ScriptedDevice`] script, consumed by one `read` call.
#[derive(Debug)]
pub enum Step {
    /// Deliver these samples (split over several reads if they exceed `max_samples`).
    Samples(Vec<u32>),
    /// Report this many lost samples.
    Lost(u64),
    /// Report that nothing is available.
    Idle,
    /// Fail with this error.
    Error(Error),
    /// Panic (deliberate fault injection).
    Panic,
    /// Stop responding for this long (or until the release flag is set), then report idle.
    Hang(Duration),
    /// A finite capture is complete.
    Finish,
}

/// What `stop` does in a [`ScriptedDevice`].
#[derive(Debug)]
pub enum StopBehavior {
    /// Succeed.
    Ok,
    /// Fail with this error.
    Error(Error),
    /// Panic (deliberate fault injection).
    Panic,
    /// Stop responding for this long (or until the release flag is set), then succeed.
    Hang(Duration),
}

/// A device that replays a script. When the script is exhausted it reports `Finished`.
#[derive(Debug)]
pub struct ScriptedDevice {
    descriptor: DeviceDescriptor,
    steps: VecDeque<Step>,
    start_error: Option<Error>,
    stop_behavior: Option<StopBehavior>,
    release: Arc<AtomicBool>,
    flow: Flow,
}

impl ScriptedDevice {
    /// Creates a device with the given script.
    #[must_use]
    pub fn new(descriptor: DeviceDescriptor, steps: Vec<Step>) -> Self {
        Self {
            descriptor,
            steps: steps.into(),
            start_error: None,
            stop_behavior: None,
            release: Arc::new(AtomicBool::new(false)),
            flow: Flow::Realtime,
        }
    }

    /// Makes `start` fail with this error.
    #[must_use]
    pub fn with_start_error(mut self, error: Error) -> Self {
        self.start_error = Some(error);
        self
    }

    /// Sets what `stop` does.
    #[must_use]
    pub fn with_stop_behavior(mut self, behavior: StopBehavior) -> Self {
        self.stop_behavior = Some(behavior);
        self
    }

    /// Declares whether the device can wait for the store (default: it cannot, like real
    /// hardware).
    #[must_use]
    pub fn with_flow(mut self, flow: Flow) -> Self {
        self.flow = flow;
        self
    }

    /// A flag that, once set, ends every hang so abandoned threads can finish.
    #[must_use]
    pub fn release_handle(&self) -> Arc<AtomicBool> {
        Arc::clone(&self.release)
    }

    fn hang(&self, duration: Duration) {
        let started = Instant::now();
        while started.elapsed() < duration && !self.release.load(Ordering::Acquire) {
            std::thread::sleep(Duration::from_millis(10));
        }
    }
}

impl Device for ScriptedDevice {
    fn flow(&self) -> Flow {
        self.flow
    }

    fn descriptor(&self) -> &DeviceDescriptor {
        &self.descriptor
    }

    fn start(&mut self, config: &ResolvedConfig) -> Result<AppliedConfig> {
        match self.start_error.take() {
            Some(error) => Err(error),
            None => Ok(AppliedConfig::new(config.sample_rate_hz())),
        }
    }

    #[allow(clippy::panic, reason = "deliberate test-only fault injection")]
    fn read(&mut self, max_samples: usize, out: &mut Vec<u32>) -> Result<ReadOutcome> {
        let Some(step) = self.steps.pop_front() else {
            return Ok(ReadOutcome::Finished);
        };
        match step {
            Step::Samples(mut samples) => {
                let take = samples.len().min(max_samples.max(1));
                let rest = samples.split_off(take);
                out.extend_from_slice(&samples);
                if !rest.is_empty() {
                    self.steps.push_front(Step::Samples(rest));
                }
                Ok(ReadOutcome::Samples)
            }
            Step::Lost(n) => Ok(ReadOutcome::Lost(n)),
            Step::Idle => {
                std::thread::sleep(Duration::from_millis(1));
                Ok(ReadOutcome::Idle)
            }
            Step::Error(error) => Err(error),
            // Deliberate, test-only fault injection: this is the harness's contract.
            Step::Panic => std::panic::panic_any("scripted driver panic"),
            Step::Hang(duration) => {
                self.hang(duration);
                Ok(ReadOutcome::Idle)
            }
            Step::Finish => Ok(ReadOutcome::Finished),
        }
    }

    #[allow(clippy::panic, reason = "deliberate test-only fault injection")]
    fn stop(&mut self) -> Result<()> {
        match self.stop_behavior.take() {
            None | Some(StopBehavior::Ok) => Ok(()),
            Some(StopBehavior::Error(error)) => Err(error),
            // Deliberate, test-only fault injection.
            Some(StopBehavior::Panic) => std::panic::panic_any("scripted driver panic in stop"),
            Some(StopBehavior::Hang(duration)) => {
                self.hang(duration);
                Ok(())
            }
        }
    }
}

/// Shared counters of the calls made to a [`CountingDevice`]. Clone it before the device is
/// moved into an acquisition.
#[derive(Debug, Clone, Default)]
pub struct CallCounter {
    starts: Arc<AtomicUsize>,
    reads: Arc<AtomicUsize>,
    stops: Arc<AtomicUsize>,
}

impl CallCounter {
    /// Number of `start` calls.
    #[must_use]
    pub fn starts(&self) -> usize {
        self.starts.load(Ordering::Acquire)
    }

    /// Number of `read` calls.
    #[must_use]
    pub fn reads(&self) -> usize {
        self.reads.load(Ordering::Acquire)
    }

    /// Number of `stop` calls.
    #[must_use]
    pub fn stops(&self) -> usize {
        self.stops.load(Ordering::Acquire)
    }

    /// Total number of calls.
    #[must_use]
    pub fn total(&self) -> usize {
        self.starts()
            .saturating_add(self.reads())
            .saturating_add(self.stops())
    }
}

/// Wraps a device and counts every call made to it.
#[derive(Debug)]
pub struct CountingDevice<D: Device> {
    inner: D,
    counter: CallCounter,
}

impl<D: Device> CountingDevice<D> {
    /// Wraps `inner`.
    #[must_use]
    pub fn new(inner: D) -> Self {
        Self {
            inner,
            counter: CallCounter::default(),
        }
    }

    /// The shared counters.
    #[must_use]
    pub fn counter(&self) -> CallCounter {
        self.counter.clone()
    }
}

impl<D: Device> Device for CountingDevice<D> {
    fn descriptor(&self) -> &DeviceDescriptor {
        self.inner.descriptor()
    }

    fn start(&mut self, config: &ResolvedConfig) -> Result<AppliedConfig> {
        self.counter.starts.fetch_add(1, Ordering::AcqRel);
        self.inner.start(config)
    }

    fn read(&mut self, max_samples: usize, out: &mut Vec<u32>) -> Result<ReadOutcome> {
        self.counter.reads.fetch_add(1, Ordering::AcqRel);
        self.inner.read(max_samples, out)
    }

    fn stop(&mut self) -> Result<()> {
        self.counter.stops.fetch_add(1, Ordering::AcqRel);
        self.inner.stop()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::DeviceConfig;
    use crate::harness::test_descriptor;

    fn resolved() -> ResolvedConfig {
        ResolvedConfig::new(
            &DeviceConfig {
                enabled_channels: crate::ChannelSet::from_u32(0b11),
                sample_rate_hz: 1_000,
                trigger: None,
                capture_length: None,
                options: crate::config::DeviceOptions::None,
                accept_budget_risk: false,
            },
            2,
        )
    }

    #[test]
    fn samples_are_split_to_the_requested_size_and_the_script_ends_with_finished() {
        let mut d = ScriptedDevice::new(
            test_descriptor("t", "1", 2),
            vec![
                Step::Samples(vec![1, 2, 3, 4, 5]),
                Step::Lost(7),
                Step::Idle,
            ],
        );
        d.start(&resolved()).unwrap();
        let mut out = Vec::new();
        assert_eq!(d.read(2, &mut out).unwrap(), ReadOutcome::Samples);
        assert_eq!(out, vec![1, 2]);
        out.clear();
        assert_eq!(d.read(10, &mut out).unwrap(), ReadOutcome::Samples);
        assert_eq!(out, vec![3, 4, 5]);
        assert_eq!(d.read(10, &mut Vec::new()).unwrap(), ReadOutcome::Lost(7));
        assert_eq!(d.read(10, &mut Vec::new()).unwrap(), ReadOutcome::Idle);
        assert_eq!(d.read(10, &mut Vec::new()).unwrap(), ReadOutcome::Finished);
    }

    #[test]
    fn errors_start_failures_and_stop_behaviour_are_scripted() {
        let mut d = ScriptedDevice::new(
            test_descriptor("t", "1", 2),
            vec![Step::Error(crate::error::disconnected("unplugged"))],
        )
        .with_start_error(crate::error::driver_error("cannot start"))
        .with_stop_behavior(StopBehavior::Error(crate::error::transport("stop failed")));
        assert_eq!(
            d.start(&resolved()).unwrap_err().code(),
            crate::error::DRIVER_ERROR
        );
        assert!(
            d.start(&resolved()).is_ok(),
            "the start error is reported once"
        );
        assert_eq!(
            d.read(1, &mut Vec::new()).unwrap_err().code(),
            crate::error::DISCONNECTED
        );
        assert_eq!(d.stop().unwrap_err().code(), crate::error::TRANSPORT);
        assert!(d.stop().is_ok());
    }

    #[test]
    fn a_hang_ends_when_released() {
        let mut d = ScriptedDevice::new(
            test_descriptor("t", "1", 2),
            vec![Step::Hang(Duration::from_secs(3600))],
        );
        let release = d.release_handle();
        let handle = std::thread::spawn(move || d.read(1, &mut Vec::new()));
        std::thread::sleep(Duration::from_millis(30));
        release.store(true, Ordering::Release);
        assert_eq!(handle.join().unwrap().unwrap(), ReadOutcome::Idle);
    }

    #[test]
    fn the_counting_wrapper_counts_every_call() {
        let inner = ScriptedDevice::new(test_descriptor("t", "1", 2), vec![Step::Finish]);
        let mut d = CountingDevice::new(inner);
        let counter = d.counter();
        assert_eq!(counter.total(), 0);
        d.start(&resolved()).unwrap();
        d.read(1, &mut Vec::new()).unwrap();
        d.read(1, &mut Vec::new()).unwrap();
        d.stop().unwrap();
        assert_eq!(
            (counter.starts(), counter.reads(), counter.stops()),
            (1, 2, 1)
        );
        assert_eq!(counter.total(), 4);
    }
}
