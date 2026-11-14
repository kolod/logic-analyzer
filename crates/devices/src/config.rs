//! Device configuration: what the user chooses, and the validated form a device receives.
//!
//! [`DeviceConfig`] is checked against a device's capabilities by [`validate`](crate::validate)
//! before anything reaches the device; the result is a [`ResolvedConfig`], the only
//! configuration type a driver ever sees.

use la_core::Result;
use la_storage::Limits;

use crate::capabilities::{Capabilities, TriggerKind};
use crate::error;
use crate::virtual_device::VirtualOptions;

/// A trigger on one channel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Trigger {
    /// The channel the trigger watches; it must be enabled.
    pub channel: u8,
    /// What to wait for.
    pub kind: TriggerKind,
}

/// Device-specific options. New drivers add variants (the enum is `#[non_exhaustive]`).
#[derive(Debug, Clone, Default)]
#[non_exhaustive]
pub enum DeviceOptions {
    /// No device-specific options.
    #[default]
    None,
    /// Options of the virtual device (seed, signal source, scripted failures, pacing).
    Virtual(VirtualOptions),
}

/// What the user chooses for one acquisition.
#[derive(Debug, Clone)]
pub struct DeviceConfig {
    /// Enabled channels as a mask: bit `i` enables channel `i`. At least one bit must be
    /// set and every set bit must be below the device's channel count.
    pub enabled_channels: u32,
    /// The sample rate in hertz; it must be one the device supports (never rounded).
    pub sample_rate_hz: u64,
    /// Optional trigger.
    pub trigger: Option<Trigger>,
    /// Samples to capture; `None` captures until stopped (not allowed for devices with an
    /// on-board buffer).
    pub capture_length: Option<u64>,
    /// Device-specific options.
    pub options: DeviceOptions,
    /// Accept a finite length whose worst-case storage cost exceeds the memory budget.
    /// The storage then ends the acquisition cleanly if the budget is really reached.
    pub accept_budget_risk: bool,
}

/// A configuration that has been checked against the capabilities of its device.
///
/// It can only be created by [`validate`](crate::validate), so a driver never receives an
/// unchecked configuration.
#[derive(Debug, Clone)]
pub struct ResolvedConfig {
    enabled_channels: u32,
    sample_rate_hz: u64,
    trigger: Option<Trigger>,
    capture_length: Option<u64>,
    options: DeviceOptions,
    channels: u8,
}

impl ResolvedConfig {
    pub(crate) fn new(config: &DeviceConfig, channels: u8) -> Self {
        Self {
            enabled_channels: config.enabled_channels,
            sample_rate_hz: config.sample_rate_hz,
            trigger: config.trigger,
            capture_length: config.capture_length,
            options: config.options.clone(),
            channels,
        }
    }

    /// Enabled channels as a mask.
    #[must_use]
    pub fn enabled_channels(&self) -> u32 {
        self.enabled_channels
    }

    /// The sample rate in hertz.
    #[must_use]
    pub fn sample_rate_hz(&self) -> u64 {
        self.sample_rate_hz
    }

    /// The trigger, if any.
    #[must_use]
    pub fn trigger(&self) -> Option<Trigger> {
        self.trigger
    }

    /// Samples to capture, or `None` until stopped.
    #[must_use]
    pub fn capture_length(&self) -> Option<u64> {
        self.capture_length
    }

    /// Device-specific options.
    #[must_use]
    pub fn options(&self) -> &DeviceOptions {
        &self.options
    }

    /// Number of channels in the capture (the device's channel count; disabled channels read
    /// as constant zero).
    #[must_use]
    pub fn channels(&self) -> u8 {
        self.channels
    }
}

/// What a device reports from `start`: the configuration actually in effect.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AppliedConfig {
    /// The sample rate in effect, in hertz.
    pub sample_rate_hz: u64,
    /// Samples the device skipped while waiting for the trigger (zero without a trigger).
    pub samples_before_trigger: u64,
}

impl AppliedConfig {
    /// An applied configuration without a trigger delay.
    #[must_use]
    pub fn new(sample_rate_hz: u64) -> Self {
        Self {
            sample_rate_hz,
            samples_before_trigger: 0,
        }
    }
}

/// Mask with the lowest `channels` bits set.
fn channel_mask(channels: u8) -> u32 {
    1_u32
        .checked_shl(u32::from(channels))
        .map_or(u32::MAX, |bit| bit.wrapping_sub(1))
}

/// Worst-case bytes the store needs for `samples` samples on `channels` channels: 4 bytes per
/// transition plus 32 bytes per 1,024 transitions (about 4.03 bytes per channel and sample),
/// assuming a level change on every channel at every sample. `None` on overflow.
fn worst_case_bytes(samples: u64, channels: u32) -> Option<u64> {
    let transitions = samples.checked_mul(u64::from(channels))?;
    transitions.checked_mul(129)?.checked_div(32)
}

/// Checks `config` against the capabilities of a device and the memory budget and returns
/// the configuration the device will use.
///
/// The check is pure: it never calls a device, so an invalid configuration can never reach
/// hardware. It covers: enabled channels exist and at least one is enabled; the sample rate is
/// supported (never rounded); the trigger kind is supported and its channel exists and is
/// enabled; the capture length is positive, finite for devices with an on-board buffer and
/// within the buffer depth; and the worst-case storage cost of a finite capture fits
/// `limits.memory_budget_bytes` unless the caller accepts the risk.
///
/// # Errors
///
/// A typed `device.*` error whose `detail` names the offending value and what is allowed:
/// `device.invalid_channel`, `device.unsupported_sample_rate`, `device.unsupported_trigger`,
/// `device.invalid_capture_length`, `device.memory_budget`, or an input error for invalid
/// virtual device options.
pub fn validate(
    caps: &Capabilities,
    config: &DeviceConfig,
    limits: &Limits,
) -> Result<ResolvedConfig> {
    // Channels.
    if config.enabled_channels == 0 {
        return Err(error::invalid_channel(format!(
            "no channel is enabled; enable at least one of the {} channels (0..={})",
            caps.channels,
            caps.channels.saturating_sub(1)
        )));
    }
    let invalid = config.enabled_channels & !channel_mask(caps.channels);
    if invalid != 0 {
        let highest = 31_u32.saturating_sub(invalid.leading_zeros());
        return Err(error::invalid_channel(format!(
            "channel {highest} does not exist: the device has {} channels (0..={})",
            caps.channels,
            caps.channels.saturating_sub(1)
        )));
    }

    // Sample rate.
    if !caps.sample_rates.contains(config.sample_rate_hz) {
        return Err(error::unsupported_sample_rate(
            config.sample_rate_hz,
            &caps.sample_rates.describe(),
            caps.sample_rates.nearest(config.sample_rate_hz),
        ));
    }

    // Trigger.
    if let Some(trigger) = config.trigger {
        if !caps.triggers.contains(&trigger.kind) {
            let detail = if caps.triggers.is_empty() {
                "the device has no trigger support; remove the trigger".to_owned()
            } else {
                format!(
                    "trigger kind {:?} is not supported; supported: {:?}",
                    trigger.kind, caps.triggers
                )
            };
            return Err(error::unsupported_trigger(detail));
        }
        if trigger.channel >= caps.channels {
            return Err(error::unsupported_trigger(format!(
                "trigger channel {} does not exist: the device has {} channels (0..={})",
                trigger.channel,
                caps.channels,
                caps.channels.saturating_sub(1)
            )));
        }
        let enabled = 1_u32
            .checked_shl(u32::from(trigger.channel))
            .is_some_and(|bit| config.enabled_channels & bit != 0);
        if !enabled {
            return Err(error::unsupported_trigger(format!(
                "trigger channel {} is not enabled; enable it or trigger on an enabled channel",
                trigger.channel
            )));
        }
    }

    // Capture length and buffer depth.
    match (config.capture_length, caps.buffer_depth) {
        (Some(0), _) => {
            return Err(error::invalid_capture_length(
                "the capture length is zero; use at least 1 sample, or leave the length unlimited"
                    .to_owned(),
            ));
        }
        (None, Some(depth)) => {
            return Err(error::invalid_capture_length(format!(
                "this device captures into a buffer of {depth} samples, so a finite capture length is required"
            )));
        }
        (Some(length), Some(depth)) if length > depth => {
            return Err(error::invalid_capture_length(format!(
                "the capture length {length} exceeds the device's buffer depth of {depth} samples"
            )));
        }
        _ => {}
    }

    // Memory budget (worst case), unless the caller accepts the risk.
    if let (Some(length), false) = (config.capture_length, config.accept_budget_risk) {
        let enabled = config.enabled_channels.count_ones();
        let budget = limits.memory_budget_bytes;
        match worst_case_bytes(length, enabled) {
            Some(cost) if cost <= budget => {}
            cost => {
                let cost =
                    cost.map_or_else(|| "an overflowing number of".to_owned(), |c| c.to_string());
                return Err(error::memory_budget(format!(
                    "the worst-case storage cost of {length} samples on {enabled} channels is about {cost} bytes, above the memory budget of {budget} bytes; shorten the capture, raise the memory budget or accept the risk"
                )));
            }
        }
    }

    // Device-specific options.
    if let DeviceOptions::Virtual(options) = &config.options {
        options.validate()?;
    }

    Ok(ResolvedConfig::new(config, caps.channels))
}
