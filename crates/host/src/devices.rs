//! The device list and the configuration the user is editing.
//!
//! The shell lists devices through the registry only. The configuration is a *draft*: every edit
//! is validated against the capabilities of the selected device (and the memory limit) before
//! anything can reach the device, and a problem names the setting that is wrong.

use std::time::{SystemTime, UNIX_EPOCH};

use la_devices::{
    Capabilities, DeviceConfig, DeviceDescriptor, DeviceOptions, Pacing, ScriptedFailure, Trigger,
    VIRTUAL_PROVIDER_NAME, VirtualOptions, error as device_error, validate,
};
use la_storage::Limits;

use crate::command::ConfigEdit;

/// How busy the virtual device's generated signals are.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SignalProfile {
    /// Long quiet stretches with rare transitions.
    Sparse,
    /// Moderate activity.
    Normal,
    /// A transition every few samples.
    Dense,
}

impl SignalProfile {
    /// The mean number of samples between transitions of a channel.
    #[must_use]
    pub fn mean_gap(self) -> u32 {
        match self {
            SignalProfile::Sparse => 5000,
            SignalProfile::Normal => 50,
            SignalProfile::Dense => 4,
        }
    }
}

/// Which setting a problem is about.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Setting {
    /// The enabled channels.
    Channels,
    /// The sample rate.
    SampleRate,
    /// The capture length.
    Length,
    /// The trigger.
    Trigger,
    /// Something else about the configuration.
    Other,
}

/// A configuration the device would refuse.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConfigProblem {
    /// The setting to correct.
    pub setting: Setting,
    /// The stable error code (the message names what is allowed).
    pub code: &'static str,
}

/// What the user is configuring for the next capture.
#[derive(Debug, Clone, PartialEq)]
pub struct ConfigDraft {
    /// Enabled channels as a mask (bit `i` is channel `i`).
    pub enabled_channels: u32,
    /// The sample rate in hertz.
    pub sample_rate_hz: u64,
    /// The capture length in samples.
    pub length: u64,
    /// The trigger, if any.
    pub trigger: Option<Trigger>,
    /// The seed of the virtual device; `None` takes a fresh one at each start.
    pub seed: Option<u64>,
    /// The signal profile of the virtual device.
    pub profile: SignalProfile,
    /// Whether the virtual device runs at the real sample rate.
    pub realtime: bool,
    /// Start although the worst case exceeds the memory limit.
    pub accept_budget_risk: bool,
}

/// The default capture length in samples.
const DEFAULT_LENGTH: u64 = 1_000_000;
/// The default sample rate in hertz (the nearest supported one is used).
const DEFAULT_RATE_HZ: u64 = 1_000_000;
/// The number of channels enabled by default.
const DEFAULT_CHANNELS: u8 = 8;

impl ConfigDraft {
    /// The defaults for a device: the first eight channels, about 1 MHz, a million samples.
    #[must_use]
    pub fn for_device(capabilities: &Capabilities) -> Self {
        let channels = capabilities.channels.min(DEFAULT_CHANNELS);
        let enabled_channels = 1_u32
            .checked_shl(u32::from(channels))
            .map_or(u32::MAX, |bit| bit.wrapping_sub(1));
        Self {
            enabled_channels,
            sample_rate_hz: capabilities
                .sample_rates
                .nearest(DEFAULT_RATE_HZ)
                .unwrap_or(DEFAULT_RATE_HZ),
            length: capabilities
                .buffer_depth
                .map_or(DEFAULT_LENGTH, |depth| depth.min(DEFAULT_LENGTH)),
            trigger: None,
            seed: None,
            profile: SignalProfile::Normal,
            realtime: true,
            accept_budget_risk: false,
        }
    }

    /// Applies one edit.
    pub fn apply(&mut self, edit: ConfigEdit) {
        match edit {
            ConfigEdit::Channels(mask) => self.enabled_channels = mask,
            ConfigEdit::SampleRate(hz) => self.sample_rate_hz = hz,
            ConfigEdit::Length(samples) => self.length = samples,
            ConfigEdit::Trigger(trigger) => self.trigger = trigger,
            ConfigEdit::Seed(seed) => self.seed = seed,
            ConfigEdit::Profile(profile) => self.profile = profile,
            ConfigEdit::Realtime(realtime) => self.realtime = realtime,
            ConfigEdit::AcceptBudgetRisk(accept) => self.accept_budget_risk = accept,
        }
    }

    /// The configuration the device receives (still to be validated), with the resolved seed
    /// and the failures a test scripted.
    #[must_use]
    pub fn to_device_config(
        &self,
        device: &DeviceDescriptor,
        seed: u64,
        failures: &[ScriptedFailure],
        source: Option<std::sync::Arc<dyn la_devices::SignalSourceFactory>>,
    ) -> DeviceConfig {
        let options = if device.id.provider() == VIRTUAL_PROVIDER_NAME {
            DeviceOptions::Virtual(VirtualOptions {
                seed,
                mean_gap: self.profile.mean_gap(),
                pacing: if self.realtime {
                    Pacing::Realtime
                } else {
                    Pacing::Fast
                },
                failures: failures.to_vec(),
                source,
                ..VirtualOptions::default()
            })
        } else {
            DeviceOptions::None
        };
        DeviceConfig {
            enabled_channels: self.enabled_channels,
            sample_rate_hz: self.sample_rate_hz,
            trigger: self.trigger,
            capture_length: Some(self.length),
            options,
            accept_budget_risk: self.accept_budget_risk,
        }
    }
}

/// Checks the draft against the device and the memory limit. The result holds at most one
/// problem (the first one the device reports); fixing it may reveal the next.
#[must_use]
pub fn problems(
    device: &DeviceDescriptor,
    draft: &ConfigDraft,
    limits: &Limits,
) -> Vec<ConfigProblem> {
    let config = draft.to_device_config(device, 1, &[], None);
    match validate(&device.capabilities, &config, limits) {
        Ok(_) => Vec::new(),
        Err(error) => {
            let code = error.code();
            let setting = match code {
                device_error::INVALID_CHANNEL => Setting::Channels,
                device_error::UNSUPPORTED_SAMPLE_RATE => Setting::SampleRate,
                device_error::UNSUPPORTED_TRIGGER => Setting::Trigger,
                device_error::INVALID_CAPTURE_LENGTH | device_error::MEMORY_BUDGET => {
                    Setting::Length
                }
                _ => Setting::Other,
            };
            vec![ConfigProblem { setting, code }]
        }
    }
}

/// A seed for a capture whose user did not choose one.
#[must_use]
pub fn fresh_seed() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(1, |d| d.as_nanos() as u64 ^ u64::from(std::process::id()))
}
