//! The virtual (simulated) device.
//!
//! It is always available, needs nothing external, is selectable like any hardware device and
//! generates its own signal data. With the same seed and configuration it produces exactly
//! the same samples. Everything it produces is marked as simulated by the acquisition, so it
//! can never be mistaken for a real measurement.

use std::fmt;
use std::sync::Arc;
use std::time::{Duration, Instant};

use la_core::Result;

use crate::capabilities::{Capabilities, DeviceDescriptor, DeviceId, SampleRates, TriggerKind};
use crate::config::{AppliedConfig, DeviceOptions, ResolvedConfig};
use crate::driver::{Device, DeviceProvider, Flow, ReadOutcome};
use crate::error;

pub(crate) mod generator;
mod source;

use generator::RandomGenerator;
pub use source::{SignalSource, SignalSourceFactory};

/// The name of the virtual provider and the prefix of its device id.
pub const PROVIDER_NAME: &str = "virtual";

/// How the virtual device paces its output.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Pacing {
    /// As fast as the store accepts (for tests and fast demos).
    #[default]
    Fast,
    /// Released at the configured sample rate in real time.
    Realtime,
}

/// A failure the virtual device is asked to simulate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FailureKind {
    /// This many samples are lost.
    Loss(u64),
    /// This many samples are lost, reported as a buffer overrun.
    Overrun(u64),
    /// The transport fails.
    TransportError,
    /// The device is disconnected.
    Disconnect,
}

/// A failure at a position of the capture (samples counted from the start of the capture).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScriptedFailure {
    /// Capture position at which the failure happens.
    pub position: u64,
    /// What happens.
    pub kind: FailureKind,
}

/// Options of the virtual device.
#[derive(Clone)]
pub struct VirtualOptions {
    /// Seed of the generator; every value including 0 and `u64::MAX` is valid.
    pub seed: u64,
    /// Supplies the samples instead of the random generator (used by decoder generators).
    pub source: Option<Arc<dyn SignalSourceFactory>>,
    /// Mean number of samples between two flips of a channel (`1..=1_000_000`).
    pub mean_gap: u32,
    /// Failures to simulate; nothing fails unless listed.
    pub failures: Vec<ScriptedFailure>,
    /// Pacing of the output.
    pub pacing: Pacing,
    /// Samples to search for the trigger condition before giving up.
    pub trigger_search_limit: u64,
}

impl Default for VirtualOptions {
    fn default() -> Self {
        Self {
            seed: 1,
            source: None,
            mean_gap: 50,
            failures: Vec::new(),
            pacing: Pacing::Fast,
            trigger_search_limit: 10_000_000,
        }
    }
}

impl fmt::Debug for VirtualOptions {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("VirtualOptions")
            .field("seed", &self.seed)
            .field("source", &self.source.as_ref().map(|_| "<custom source>"))
            .field("mean_gap", &self.mean_gap)
            .field("failures", &self.failures)
            .field("pacing", &self.pacing)
            .field("trigger_search_limit", &self.trigger_search_limit)
            .finish()
    }
}

impl VirtualOptions {
    /// Checks the options.
    ///
    /// # Errors
    ///
    /// `device.invalid_state`-class input error if `mean_gap` is outside `1..=1_000_000`.
    pub fn validate(&self) -> Result<()> {
        if !(1..=1_000_000).contains(&self.mean_gap) {
            return Err(error::invalid_state(format!(
                "mean_gap {} is outside 1..=1000000",
                self.mean_gap
            )));
        }
        Ok(())
    }
}

/// The description of the one virtual device: `virtual:0`, 32 channels, rates from 1 Hz to
/// 500 MHz, every trigger kind, streaming.
#[must_use]
pub fn virtual_descriptor() -> DeviceDescriptor {
    DeviceDescriptor {
        id: DeviceId::new(PROVIDER_NAME, "0"),
        name: "Virtual device".to_owned(),
        simulated: true,
        capabilities: Capabilities {
            channels: 32,
            sample_rates: SampleRates::Range {
                min: 1,
                max: 500_000_000,
            },
            triggers: vec![
                TriggerKind::Level(true),
                TriggerKind::Level(false),
                TriggerKind::RisingEdge,
                TriggerKind::FallingEdge,
                TriggerKind::AnyEdge,
            ],
            buffer_depth: None,
        },
    }
}

/// The provider of the virtual device. Always reports it and never reports it disappearing.
#[derive(Debug, Default, Clone, Copy)]
pub struct VirtualProvider;

impl DeviceProvider for VirtualProvider {
    fn name(&self) -> &str {
        PROVIDER_NAME
    }

    fn scan(&self) -> Result<Vec<DeviceDescriptor>> {
        Ok(vec![virtual_descriptor()])
    }

    fn open(&self, id: &DeviceId) -> Result<Box<dyn Device>> {
        if *id != virtual_descriptor().id {
            return Err(error::not_found(id.as_str()));
        }
        Ok(Box::new(VirtualDevice::new()))
    }
}

/// Produces the samples of one acquisition: the random generator or a custom source.
enum Producer {
    Random(RandomGenerator),
    Source(Box<dyn SignalSource>),
}

impl Producer {
    /// Fills `out` with the samples for the capture positions `start ..`.
    fn fill(&mut self, start: u64, out: &mut [u32]) {
        match self {
            Self::Random(generator) => generator.fill(out),
            Self::Source(source) => source.fill(start, out),
        }
    }
}

/// The virtual device.
pub struct VirtualDevice {
    descriptor: DeviceDescriptor,
    producer: Option<Producer>,
    remaining: Option<u64>,
    pacing: Pacing,
    /// Samples produced so far (the capture position of the next sample).
    position: u64,
    sample_rate_hz: u64,
    started: Option<Instant>,
}

impl fmt::Debug for VirtualDevice {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("VirtualDevice")
            .field("pacing", &self.pacing)
            .field("position", &self.position)
            .field("remaining", &self.remaining)
            .finish_non_exhaustive()
    }
}

impl VirtualDevice {
    /// Creates the virtual device.
    #[must_use]
    pub fn new() -> Self {
        Self {
            descriptor: virtual_descriptor(),
            producer: None,
            remaining: None,
            pacing: Pacing::Fast,
            position: 0,
            sample_rate_hz: 1,
            started: None,
        }
    }

    /// How many samples may be released now in real time: those whose time has come.
    fn due(&self) -> u64 {
        let Some(started) = self.started else {
            return 0;
        };
        let due = started
            .elapsed()
            .as_nanos()
            .saturating_mul(u128::from(self.sample_rate_hz))
            / 1_000_000_000;
        u64::try_from(due)
            .unwrap_or(u64::MAX)
            .saturating_sub(self.position)
    }
}

impl Default for VirtualDevice {
    fn default() -> Self {
        Self::new()
    }
}

impl Device for VirtualDevice {
    fn descriptor(&self) -> &DeviceDescriptor {
        &self.descriptor
    }

    fn flow(&self) -> Flow {
        // The virtual device can always wait for the store, also when paced in real time:
        // pacing only decides when samples are released, and the device never loses samples
        // on its own (losses are simulated only when a failure script asks for them).
        Flow::Throttleable
    }

    fn start(&mut self, config: &ResolvedConfig) -> Result<AppliedConfig> {
        let options = match config.options() {
            DeviceOptions::Virtual(options) => options.clone(),
            _ => VirtualOptions::default(),
        };
        options.validate()?;
        self.producer = Some(match &options.source {
            Some(factory) => Producer::Source(factory.create(
                config.enabled_channels(),
                config.sample_rate_hz(),
                options.seed,
            )),
            None => Producer::Random(RandomGenerator::new(
                config.enabled_channels(),
                options.seed,
                options.mean_gap,
            )),
        });
        self.remaining = config.capture_length();
        self.pacing = options.pacing;
        self.sample_rate_hz = config.sample_rate_hz();
        self.position = 0;
        self.started = Some(Instant::now());
        Ok(AppliedConfig::new(config.sample_rate_hz()))
    }

    fn read(&mut self, max_samples: usize, out: &mut Vec<u32>) -> Result<ReadOutcome> {
        let wanted = match self.remaining {
            Some(0) => return Ok(ReadOutcome::Finished),
            Some(left) => usize::try_from(left).unwrap_or(usize::MAX).min(max_samples),
            None => max_samples,
        };
        let wanted = if self.pacing == Pacing::Realtime {
            let due = usize::try_from(self.due()).unwrap_or(usize::MAX);
            if due == 0 {
                // Nothing is due yet: wait a little (short slices keep stop responsive).
                std::thread::sleep(Duration::from_millis(1));
                return Ok(ReadOutcome::Idle);
            }
            wanted.min(due)
        } else {
            wanted
        };
        if wanted == 0 {
            return Ok(ReadOutcome::Idle);
        }
        let Some(producer) = self.producer.as_mut() else {
            return Err(error::invalid_state(
                "the virtual device was read before it was started".to_owned(),
            ));
        };
        out.clear();
        out.resize(wanted, 0);
        producer.fill(self.position, out);
        let produced = u64::try_from(wanted).unwrap_or(u64::MAX);
        self.position = self.position.saturating_add(produced);
        if let Some(left) = self.remaining.as_mut() {
            *left = left.saturating_sub(produced);
        }
        Ok(ReadOutcome::Samples)
    }

    fn stop(&mut self) -> Result<()> {
        Ok(())
    }
}
