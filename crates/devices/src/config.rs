//! Device configuration: what the user chooses, and the validated form a device receives.
//!
//! [`DeviceConfig`] is checked against a device's capabilities by [`validate`](crate::validate)
//! before anything reaches the device; the result is a [`ResolvedConfig`], the only
//! configuration type a driver ever sees.

use la_core::Result;
use la_storage::Limits;

use crate::capabilities::{Capabilities, TriggerKind};
use crate::channel_set::ChannelSet;
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

/// The levels a pattern trigger waits for on a group of adjacent channels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Pattern {
    /// The first channel of the group; bit `i` of `value` is the level of channel `base_channel + i`.
    pub base_channel: u8,
    /// The number of channels in the group, at least 1.
    pub length: u8,
    /// The levels (`1` = high); no bit at or above `length` may be set (a pattern is up to 32 channels).
    pub value: u32,
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
    /// The enabled channels. At least one must be enabled and every one must be below the
    /// device's channel count.
    pub enabled_channels: ChannelSet,
    /// The sample rate in hertz; it must be one the device supports (never rounded).
    pub sample_rate_hz: u64,
    /// Optional trigger.
    pub trigger: Option<Trigger>,
    /// The levels of a [`TriggerKind::Pattern`] or [`TriggerKind::FastPattern`] trigger, and only then.
    pub pattern: Option<Pattern>,
    /// Samples to capture, in all (before and after the trigger); `None` captures until stopped (not
    /// allowed for devices with an on-board buffer).
    pub capture_length: Option<u64>,
    /// Samples to keep from before the trigger; `0` keeps none. Part of `capture_length`.
    pub pre_trigger_samples: u64,
    /// The number of bursts of one capture; `0` is a single capture.
    pub bursts: u32,
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
    enabled_channels: ChannelSet,
    sample_rate_hz: u64,
    trigger: Option<Trigger>,
    pattern: Option<Pattern>,
    capture_length: Option<u64>,
    pre_trigger_samples: u64,
    bursts: u32,
    options: DeviceOptions,
    channels: u8,
}

impl ResolvedConfig {
    pub(crate) fn new(config: &DeviceConfig, channels: u8) -> Self {
        Self {
            enabled_channels: config.enabled_channels,
            sample_rate_hz: config.sample_rate_hz,
            trigger: config.trigger,
            pattern: config.pattern,
            capture_length: config.capture_length,
            pre_trigger_samples: config.pre_trigger_samples,
            bursts: config.bursts,
            options: config.options.clone(),
            channels,
        }
    }

    /// The enabled channels.
    #[must_use]
    pub fn enabled_channels(&self) -> ChannelSet {
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

    /// The pattern of a pattern trigger.
    #[must_use]
    pub fn pattern(&self) -> Option<Pattern> {
        self.pattern
    }

    /// Samples to capture, or `None` until stopped.
    #[must_use]
    pub fn capture_length(&self) -> Option<u64> {
        self.capture_length
    }

    /// Samples kept from before the trigger.
    #[must_use]
    pub fn pre_trigger_samples(&self) -> u64 {
        self.pre_trigger_samples
    }

    /// The number of bursts, `0` for a single capture.
    #[must_use]
    pub fn bursts(&self) -> u32 {
        self.bursts
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

/// Worst-case bytes the store needs for `samples` samples on `channels` channels: 4 bytes per
/// transition plus 32 bytes per 1,024 transitions (about 4.03 bytes per channel and sample),
/// assuming a level change on every channel at every sample. `None` on overflow.
fn worst_case_bytes(samples: u64, channels: usize) -> Option<u64> {
    let transitions = samples.checked_mul(u64::try_from(channels).ok()?)?;
    transitions.checked_mul(129)?.checked_div(32)
}

/// The checks of a pattern trigger: it is complete, within the widest pattern and on enabled channels.
fn validate_pattern(caps: &Capabilities, config: &DeviceConfig) -> Result<()> {
    let kind = config.trigger.map(|trigger| trigger.kind);
    let width = match kind {
        Some(TriggerKind::Pattern) => caps.trigger_limits.pattern_width,
        Some(TriggerKind::FastPattern) => caps.trigger_limits.fast_pattern_width,
        _ => {
            return if config.pattern.is_some() {
                Err(error::unsupported_trigger(
                    "a pattern is given, but the trigger is not a pattern trigger; remove the pattern"
                        .to_owned(),
                ))
            } else {
                Ok(())
            };
        }
    };
    if !caps
        .triggers
        .contains(&kind.unwrap_or(TriggerKind::Pattern))
        || width == 0
    {
        return Err(error::unsupported_trigger(format!(
            "the device does not support the trigger kind {kind:?}"
        )));
    }
    if caps.trigger_limits.chained && !caps.trigger_limits.allowed_in_chain {
        return Err(error::unsupported_trigger(
            "a pattern trigger is not possible in a chain of devices; use an edge trigger"
                .to_owned(),
        ));
    }
    let Some(pattern) = config.pattern else {
        return Err(error::unsupported_trigger(
            "a pattern trigger needs a pattern (the first channel, the number of channels and their levels)"
                .to_owned(),
        ));
    };
    if pattern.length == 0 {
        return Err(error::unsupported_trigger(
            "the pattern has no channels; give it at least 1".to_owned(),
        ));
    }
    if pattern.length > width {
        return Err(error::unsupported_trigger(format!(
            "the pattern has {} channels; the device supports at most {width} for {kind:?}",
            pattern.length
        )));
    }
    if pattern.length < 32 && pattern.value >> pattern.length != 0 {
        return Err(error::unsupported_trigger(format!(
            "the pattern value {:#b} has levels above its {} channels",
            pattern.value, pattern.length
        )));
    }
    if config.trigger.map(|trigger| trigger.channel) != Some(pattern.base_channel) {
        return Err(error::unsupported_trigger(format!(
            "the trigger channel must be the first channel of the pattern, {}",
            pattern.base_channel
        )));
    }
    let last = u16::from(pattern.base_channel)
        .saturating_add(u16::from(pattern.length))
        .saturating_sub(1);
    if last >= u16::from(caps.channels) {
        return Err(error::unsupported_trigger(format!(
            "the pattern reaches channel {last}, but the device has {} channels (0..={})",
            caps.channels,
            caps.channels.saturating_sub(1)
        )));
    }
    if let Some(missing) = (pattern.base_channel..=u8::try_from(last).unwrap_or(u8::MAX))
        .find(|&channel| !config.enabled_channels.contains(channel))
    {
        return Err(error::unsupported_trigger(format!(
            "pattern channel {missing} is not enabled; enable it or shorten the pattern"
        )));
    }
    Ok(())
}

/// The samples before and after the trigger against the capture mode of the enabled channels.
fn validate_around_trigger(
    caps: &Capabilities,
    config: &DeviceConfig,
    enabled: usize,
) -> Result<()> {
    let before = config.pre_trigger_samples;
    if before > 0 && config.trigger.is_none() {
        return Err(error::invalid_capture_length(format!(
            "{before} samples before the trigger were requested without a trigger; set a trigger or use 0"
        )));
    }
    if let Some(length) = config.capture_length.filter(|&length| before > length) {
        return Err(error::invalid_capture_length(format!(
            "{before} samples before the trigger do not fit the capture length of {length}"
        )));
    }
    if caps.pre_post.is_empty() {
        if before > 0 {
            return Err(error::invalid_capture_length(format!(
                "this device cannot keep samples from before the trigger; use 0 instead of {before}"
            )));
        }
        return Ok(());
    }
    let Some(mode) = caps.mode_for(enabled) else {
        return Err(error::invalid_capture_length(format!(
            "no capture mode of the device serves {enabled} enabled channels"
        )));
    };
    if before > mode.max_before {
        return Err(error::invalid_capture_length(format!(
            "{before} samples before the trigger exceed the limit of {} for {enabled} enabled channels",
            mode.max_before
        )));
    }
    if let Some(length) = config.capture_length {
        let after = length.saturating_sub(before);
        if after > mode.max_after {
            return Err(error::invalid_capture_length(format!(
                "{after} samples after the trigger exceed the limit of {} for {enabled} enabled channels",
                mode.max_after
            )));
        }
    }
    Ok(())
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
    if config.enabled_channels.is_empty() {
        return Err(error::invalid_channel(format!(
            "no channel is enabled; enable at least one of the {} channels (0..={})",
            caps.channels,
            caps.channels.saturating_sub(1)
        )));
    }
    if let Some(highest) = config
        .enabled_channels
        .highest()
        .filter(|&highest| highest >= caps.channels)
    {
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

    // The highest rate for this many channels.
    let enabled = config.enabled_channels.count();
    if let Some(highest) = caps
        .max_rate_for(enabled)
        .filter(|&highest| config.sample_rate_hz > highest)
    {
        return Err(error::unsupported_sample_rate(
            config.sample_rate_hz,
            &format!(
                "up to {highest} Hz with {enabled} enabled channels (fewer channels allow a higher rate)"
            ),
            Some(highest),
        ));
    }

    // Trigger.
    validate_pattern(caps, config)?;
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
        if !config.enabled_channels.contains(trigger.channel) {
            return Err(error::unsupported_trigger(format!(
                "trigger channel {} is not enabled; enable it or trigger on an enabled channel",
                trigger.channel
            )));
        }
    }

    // Capture length, buffer depth and the samples around the trigger.
    let depth = caps.depth_at(config.sample_rate_hz);
    match (config.capture_length, depth) {
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
                "the capture length {length} exceeds the device's buffer depth of {depth} samples at {} Hz",
                config.sample_rate_hz
            )));
        }
        _ => {}
    }
    validate_around_trigger(caps, config, enabled)?;
    if config.bursts > 0 {
        match caps.bursts {
            Some(most) if config.bursts <= most => {}
            Some(most) => {
                return Err(error::unsupported_bursts(format!(
                    "{} bursts were requested; the device supports at most {most}",
                    config.bursts
                )));
            }
            None => {
                return Err(error::unsupported_bursts(format!(
                    "{} bursts were requested; the device captures once, so use 0 bursts",
                    config.bursts
                )));
            }
        }
    }

    // Memory budget (worst case), unless the caller accepts the risk.
    if let (Some(length), false) = (config.capture_length, config.accept_budget_risk) {
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
