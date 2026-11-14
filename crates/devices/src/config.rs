//! Device configuration: what the user chooses, and the validated form a device receives.
//!
//! [`DeviceConfig`] is checked against a device's capabilities by [`validate`](crate::validate)
//! before anything reaches the device; the result is a [`ResolvedConfig`], the only
//! configuration type a driver ever sees.

use crate::capabilities::TriggerKind;
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
    #[cfg_attr(not(test), expect(dead_code, reason = "used by validate (phase 4)"))]
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
