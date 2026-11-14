//! The virtual (simulated) device.
//!
//! It is always available, needs nothing external, is selectable like any hardware device and
//! generates its own signal data. With the same seed and configuration it produces exactly
//! the same samples. Everything it produces is marked as simulated by the acquisition, so it
//! can never be mistaken for a real measurement.

use std::fmt;
use std::sync::Arc;

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

/// The virtual device.
#[derive(Debug)]
pub struct VirtualDevice {
    descriptor: DeviceDescriptor,
    generator: Option<RandomGenerator>,
    remaining: Option<u64>,
    pacing: Pacing,
}

impl VirtualDevice {
    /// Creates the virtual device.
    #[must_use]
    pub fn new() -> Self {
        Self {
            descriptor: virtual_descriptor(),
            generator: None,
            remaining: None,
            pacing: Pacing::Fast,
        }
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
        // As fast as the store accepts: the device waits for the store and never loses data.
        // In real time it cannot wait, like real hardware.
        match self.pacing {
            Pacing::Fast => Flow::Throttleable,
            Pacing::Realtime => Flow::Realtime,
        }
    }

    fn start(&mut self, config: &ResolvedConfig) -> Result<AppliedConfig> {
        let options = match config.options() {
            DeviceOptions::Virtual(options) => options.clone(),
            _ => VirtualOptions::default(),
        };
        options.validate()?;
        self.generator = Some(RandomGenerator::new(
            config.enabled_channels(),
            options.seed,
            options.mean_gap,
        ));
        self.remaining = config.capture_length();
        self.pacing = options.pacing;
        Ok(AppliedConfig::new(config.sample_rate_hz()))
    }

    fn read(&mut self, max_samples: usize, out: &mut Vec<u32>) -> Result<ReadOutcome> {
        let Some(generator) = self.generator.as_mut() else {
            return Err(error::invalid_state(
                "the virtual device was read before it was started".to_owned(),
            ));
        };
        let wanted = match self.remaining {
            Some(0) => return Ok(ReadOutcome::Finished),
            Some(left) => usize::try_from(left).unwrap_or(usize::MAX).min(max_samples),
            None => max_samples,
        };
        if wanted == 0 {
            return Ok(ReadOutcome::Idle);
        }
        out.clear();
        out.resize(wanted, 0);
        generator.fill(out);
        if let Some(left) = self.remaining.as_mut() {
            *left = left.saturating_sub(u64::try_from(wanted).unwrap_or(u64::MAX));
        }
        Ok(ReadOutcome::Samples)
    }

    fn stop(&mut self) -> Result<()> {
        Ok(())
    }
}
