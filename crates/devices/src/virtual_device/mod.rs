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
mod script;
mod source;

use generator::RandomGenerator;
use script::{FailureScript, search_trigger};
pub use source::{RandomSource, SignalSource, SignalSourceFactory};

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
    /// Fills `out` with the samples for the generator positions `start ..`.
    fn fill(&mut self, start: u64, out: &mut [u32]) {
        match self {
            Self::Random(generator) => generator.fill(out),
            Self::Source(source) => source.fill(start, out),
        }
    }

    /// Advances over `count` generator positions without keeping the samples.
    fn skip(&mut self, start: u64, count: u64) {
        match self {
            Self::Random(generator) => generator.skip(count),
            Self::Source(source) => {
                // A source only knows how to fill, so produce and discard in chunks.
                let mut scratch = vec![0_u32; 65_536];
                let mut done = 0_u64;
                while done < count {
                    let len = usize::try_from(count.saturating_sub(done))
                        .map_or(scratch.len(), |l| l.min(scratch.len()));
                    if let Some(part) = scratch.get_mut(..len) {
                        source.fill(start.saturating_add(done), part);
                    }
                    done = done.saturating_add(u64::try_from(len).unwrap_or(1));
                }
            }
        }
    }
}

/// The virtual device.
pub struct VirtualDevice {
    descriptor: DeviceDescriptor,
    producer: Option<Producer>,
    remaining: Option<u64>,
    pacing: Pacing,
    /// The capture position of the next sample: samples delivered plus positions lost.
    position: u64,
    /// How many samples the generator or source has produced (including samples that were
    /// skipped while waiting for a trigger or lost).
    generated: u64,
    /// Samples generated while searching for the trigger that belong to the capture.
    carry: Vec<u32>,
    carry_used: usize,
    script: FailureScript,
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
            generated: 0,
            carry: Vec::new(),
            carry_used: 0,
            script: FailureScript::default(),
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

    /// Samples left over from the trigger search.
    fn carry_left(&self) -> usize {
        self.carry.len().saturating_sub(self.carry_used)
    }

    /// Discards `count` capture positions: first from the trigger carry, then from the
    /// generator.
    fn skip_positions(&mut self, count: u64) {
        let from_carry = usize::try_from(count)
            .unwrap_or(usize::MAX)
            .min(self.carry_left());
        self.carry_used = self.carry_used.saturating_add(from_carry);
        let rest = count.saturating_sub(u64::try_from(from_carry).unwrap_or(0));
        if rest > 0 {
            if let Some(producer) = self.producer.as_mut() {
                producer.skip(self.generated, rest);
            }
            self.generated = self.generated.saturating_add(rest);
        }
        self.position = self.position.saturating_add(count);
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
        let mut producer = match &options.source {
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
        };
        self.generated = 0;
        self.carry = Vec::new();
        self.carry_used = 0;
        let mut skipped = 0;
        if let Some(trigger) = config.trigger() {
            // Generate and discard samples until the trigger condition holds; capturing starts
            // at the trigger sample.
            let found = search_trigger(
                &mut |start, out| producer.fill(start, out),
                trigger,
                options.trigger_search_limit,
            )?;
            skipped = found.skipped;
            self.generated = found.generated;
            self.carry = found.carry;
        }
        self.producer = Some(producer);
        self.script = FailureScript::new(options.failures);
        self.remaining = config.capture_length();
        self.pacing = options.pacing;
        self.sample_rate_hz = config.sample_rate_hz();
        self.position = 0;
        self.started = Some(Instant::now());
        Ok(AppliedConfig {
            sample_rate_hz: config.sample_rate_hz(),
            samples_before_trigger: skipped,
        })
    }

    fn read(&mut self, max_samples: usize, out: &mut Vec<u32>) -> Result<ReadOutcome> {
        let wanted = match self.remaining {
            Some(0) => return Ok(ReadOutcome::Finished),
            Some(left) => usize::try_from(left).unwrap_or(usize::MAX).min(max_samples),
            None => max_samples,
        };

        // A scripted failure whose position has been reached happens now.
        if let Some(failure) = self.script.take_due(self.position) {
            let position = self.position;
            match failure.kind {
                FailureKind::Loss(0) | FailureKind::Overrun(0) => return Ok(ReadOutcome::Idle),
                FailureKind::Loss(lost) | FailureKind::Overrun(lost) => {
                    tracing::warn!(
                        target: "la_devices",
                        position,
                        lost,
                        "the virtual device simulates lost samples"
                    );
                    self.skip_positions(lost);
                    return Ok(ReadOutcome::Lost(lost));
                }
                FailureKind::TransportError => {
                    return Err(error::transport(format!(
                        "simulated transport error at position {position}; the virtual device was told to fail here"
                    )));
                }
                FailureKind::Disconnect => {
                    return Err(error::disconnected(format!(
                        "simulated disconnect at position {position}; the virtual device was told to fail here"
                    )));
                }
            }
        }
        // Never read past the next scripted failure: it must happen at its exact position.
        let wanted = match self.script.next_position() {
            Some(next) => wanted
                .min(usize::try_from(next.saturating_sub(self.position)).unwrap_or(usize::MAX)),
            None => wanted,
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
        // Samples left over from the trigger search come first.
        let from_carry = wanted.min(self.carry.len().saturating_sub(self.carry_used));
        if let Some(part) = self
            .carry
            .get(self.carry_used..self.carry_used.saturating_add(from_carry))
        {
            out.extend_from_slice(part);
        }
        self.carry_used = self.carry_used.saturating_add(from_carry);
        let rest = wanted.saturating_sub(from_carry);
        if rest > 0 {
            out.resize(wanted, 0);
            if let Some(part) = out.get_mut(from_carry..) {
                producer.fill(self.generated, part);
            }
            self.generated = self
                .generated
                .saturating_add(u64::try_from(rest).unwrap_or(u64::MAX));
        }
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
