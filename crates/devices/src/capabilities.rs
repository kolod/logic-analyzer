//! Device identity, description and declared capabilities.

use std::fmt;

use la_core::Result;
use la_storage::MAX_CHANNELS;

use crate::error;

/// A stable device identity: `"<provider>:<unique>"`.
///
/// It is equal across listings and across a reconnect of the same physical device (for a USB
/// device the provider builds `unique` from vendor, product and serial number).
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct DeviceId(String);

impl DeviceId {
    /// Builds an id from the provider name and a provider-defined unique string.
    #[must_use]
    pub fn new(provider: &str, unique: &str) -> Self {
        Self(format!("{provider}:{unique}"))
    }

    /// Wraps an id read from outside (a saved session). The text must be `provider:unique`
    /// with a non-empty provider.
    pub(crate) fn from_text(text: &str) -> Option<Self> {
        let (provider, _) = text.split_once(':')?;
        (!provider.is_empty()).then(|| Self(text.to_owned()))
    }

    /// The full id text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// The provider part (everything before the first `:`).
    #[must_use]
    pub fn provider(&self) -> &str {
        self.0.split(':').next().unwrap_or("")
    }
}

impl fmt::Display for DeviceId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// The sample rates a device supports.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SampleRates {
    /// An explicit, strictly increasing list of rates in hertz.
    List(Vec<u64>),
    /// Every rate from `min` to `max` hertz (inclusive).
    Range {
        /// Lowest supported rate.
        min: u64,
        /// Highest supported rate.
        max: u64,
    },
}

impl SampleRates {
    /// Whether `rate` is supported.
    #[must_use]
    pub fn contains(&self, rate: u64) -> bool {
        match self {
            Self::List(list) => list.contains(&rate),
            Self::Range { min, max } => (*min..=*max).contains(&rate),
        }
    }

    /// The supported rate closest to `rate` (the lower one on a tie).
    #[must_use]
    pub fn nearest(&self, rate: u64) -> Option<u64> {
        match self {
            Self::List(list) => list.iter().copied().min_by_key(|r| (r.abs_diff(rate), *r)),
            Self::Range { min, max } => Some(rate.clamp(*min, *max)),
        }
    }

    /// A human-readable description for error messages.
    #[must_use]
    pub fn describe(&self) -> String {
        match self {
            Self::List(list) => {
                let rates: Vec<String> = list.iter().map(u64::to_string).collect();
                format!("{} Hz", rates.join(", "))
            }
            Self::Range { min, max } => format!("{min} Hz to {max} Hz"),
        }
    }

    fn validate(&self) -> Result<()> {
        match self {
            Self::List(list) => {
                if list.is_empty() {
                    return Err(error::invalid_capabilities(
                        "the list of sample rates is empty",
                    ));
                }
                if list.first().copied() == Some(0) {
                    return Err(error::invalid_capabilities(
                        "a sample rate of zero is not valid",
                    ));
                }
                if list.windows(2).any(|w| w.first() >= w.get(1)) {
                    return Err(error::invalid_capabilities(
                        "the sample rates must be strictly increasing",
                    ));
                }
                Ok(())
            }
            Self::Range { min, max } => {
                if *min == 0 || min > max {
                    return Err(error::invalid_capabilities(
                        "the sample rate range must satisfy 1 <= min <= max",
                    ));
                }
                Ok(())
            }
        }
    }
}

/// A kind of trigger a device may support.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TriggerKind {
    /// Start when the channel is at the given level (`true` = high).
    Level(bool),
    /// Start on a low-to-high transition.
    RisingEdge,
    /// Start on a high-to-low transition.
    FallingEdge,
    /// Start on any transition.
    AnyEdge,
    /// Start when a group of adjacent channels shows a given pattern (see [`Pattern`](crate::Pattern)).
    Pattern,
    /// Like [`Pattern`](Self::Pattern), but evaluated in the device's fast trigger path; the group is narrower.
    FastPattern,
}

/// The widest patterns a device can trigger on, and whether a trigger works in a chain of devices.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TriggerLimits {
    /// The most channels of a [`TriggerKind::Pattern`]; `0` when the device has none.
    pub pattern_width: u8,
    /// The most channels of a [`TriggerKind::FastPattern`]; `0` when the device has none.
    pub fast_pattern_width: u8,
    /// This device is a chain of devices.
    pub chained: bool,
    /// A pattern trigger is possible although the device is a chain.
    pub allowed_in_chain: bool,
}

impl TriggerLimits {
    /// No pattern triggers.
    pub const NONE: Self = Self {
        pattern_width: 0,
        fast_pattern_width: 0,
        chained: false,
        allowed_in_chain: false,
    };
}

/// The samples around the trigger that one capture mode allows. A mode is chosen by the number of enabled
/// channels: the first [`ModeLimit`] whose `up_to_channels` is at least that number.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ModeLimit {
    /// This mode serves captures of up to this many enabled channels.
    pub up_to_channels: u8,
    /// The most samples kept before the trigger.
    pub max_before: u64,
    /// The most samples captured after the trigger.
    pub max_after: u64,
}

/// The buffer depth in a range of sample rates.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DepthBand {
    /// The lowest rate of the band in hertz.
    pub from_rate_hz: u64,
    /// The highest rate of the band in hertz (inclusive).
    pub to_rate_hz: u64,
    /// The buffer depth in samples at these rates.
    pub depth: u64,
}

/// What a device declares it can do. The core enforces it before a configuration reaches
/// the device.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Capabilities {
    /// Number of channels, `1..=160`.
    pub channels: u8,
    /// Supported sample rates.
    pub sample_rates: SampleRates,
    /// Supported trigger kinds (empty if the device has no trigger).
    pub triggers: Vec<TriggerKind>,
    /// Buffer depth in samples for devices that capture into an on-board buffer; `None` for
    /// devices that stream.
    pub buffer_depth: Option<u64>,
    /// The widest pattern triggers; [`TriggerLimits::NONE`] for a device without them.
    pub trigger_limits: TriggerLimits,
    /// Samples before and after the trigger by capture mode, ascending in `up_to_channels`; empty when the
    /// device cannot keep samples from before the trigger.
    pub pre_post: Vec<ModeLimit>,
    /// The buffer depth by rate band, ascending and not overlapping; empty when `buffer_depth` (or nothing)
    /// applies at every rate. A device with bands captures into its buffer, so a length is required.
    pub buffer_depths: Vec<DepthBand>,
    /// The most bursts of one capture; `None` when the device captures once.
    pub bursts: Option<u32>,
    /// The highest rate by enabled channel count: `(up to this many channels, rate in hertz)`, ascending in
    /// the channel count and not rising; empty when the rates of `sample_rates` apply to every count.
    pub max_rates: Vec<(u8, u64)>,
}

impl Capabilities {
    /// The mode that serves `enabled` channels.
    #[must_use]
    pub fn mode_for(&self, enabled: usize) -> Option<&ModeLimit> {
        self.pre_post
            .iter()
            .find(|mode| usize::from(mode.up_to_channels) >= enabled)
    }

    /// The buffer depth at `rate_hz`: the band's depth, else the device's single depth.
    #[must_use]
    pub fn depth_at(&self, rate_hz: u64) -> Option<u64> {
        self.buffer_depths
            .iter()
            .find(|band| (band.from_rate_hz..=band.to_rate_hz).contains(&rate_hz))
            .map(|band| band.depth)
            .or(self.buffer_depth)
    }

    /// The highest rate for `enabled` channels, if the device limits it by channel count.
    #[must_use]
    pub fn max_rate_for(&self, enabled: usize) -> Option<u64> {
        self.max_rates
            .iter()
            .find(|(up_to, _)| usize::from(*up_to) >= enabled)
            .map(|(_, rate)| *rate)
    }

    fn validate_rich(&self) -> Result<()> {
        let limits = &self.trigger_limits;
        if limits.pattern_width > self.channels || limits.fast_pattern_width > self.channels {
            return Err(error::invalid_capabilities(
                "a pattern is wider than the device",
            ));
        }
        if limits.allowed_in_chain && !limits.chained {
            return Err(error::invalid_capabilities(
                "a trigger is allowed in a chain, but the device is not a chain",
            ));
        }
        let mut last = 0_u8;
        for mode in &self.pre_post {
            if mode.up_to_channels <= last || mode.up_to_channels > self.channels {
                return Err(error::invalid_capabilities(
                    "the capture modes must ascend in channels and stay within the device",
                ));
            }
            if mode.max_after == 0 {
                return Err(error::invalid_capabilities(
                    "a capture mode must allow at least 1 sample after the trigger",
                ));
            }
            last = mode.up_to_channels;
        }
        let mut next_rate = 1_u64;
        for band in &self.buffer_depths {
            if band.from_rate_hz < next_rate || band.from_rate_hz > band.to_rate_hz {
                return Err(error::invalid_capabilities(
                    "the buffer depth bands must ascend, not overlap and have from <= to",
                ));
            }
            if band.depth == 0 {
                return Err(error::invalid_capabilities(
                    "a buffer depth band must hold at least 1 sample",
                ));
            }
            next_rate = band.to_rate_hz.saturating_add(1);
        }
        if self.bursts == Some(0) {
            return Err(error::invalid_capabilities(
                "support for 0 bursts is declared by `None`",
            ));
        }
        let (mut last_channels, mut last_rate) = (0_u8, u64::MAX);
        for &(up_to, rate) in &self.max_rates {
            if up_to <= last_channels || up_to > self.channels || rate == 0 || rate > last_rate {
                return Err(error::invalid_capabilities(
                    "the highest rates must ascend in channels, never rise and stay within the device",
                ));
            }
            (last_channels, last_rate) = (up_to, rate);
        }
        Ok(())
    }
}

/// A listed device: identity, name and capabilities.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceDescriptor {
    /// Stable identity.
    pub id: DeviceId,
    /// Human-readable name (not empty).
    pub name: String,
    /// `true` only for devices that generate data instead of measuring it.
    pub simulated: bool,
    /// Declared capabilities.
    pub capabilities: Capabilities,
}

impl DeviceDescriptor {
    /// Checks that the description is consistent. A device that fails this check is not
    /// listed.
    ///
    /// # Errors
    ///
    /// `device.invalid_capabilities` naming the problem.
    pub fn validate(&self) -> Result<()> {
        if self.name.trim().is_empty() {
            return Err(error::invalid_capabilities("the device name is empty"));
        }
        let caps = &self.capabilities;
        if caps.channels == 0 || caps.channels > MAX_CHANNELS {
            return Err(error::invalid_capabilities(&format!(
                "the channel count {} is outside 1..={MAX_CHANNELS}",
                caps.channels
            )));
        }
        caps.sample_rates.validate()?;
        if caps.buffer_depth == Some(0) {
            return Err(error::invalid_capabilities(
                "the buffer depth must be at least 1",
            ));
        }
        caps.validate_rich()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn descriptor() -> DeviceDescriptor {
        DeviceDescriptor {
            id: DeviceId::new("test", "1"),
            name: "Test device".to_owned(),
            simulated: false,
            capabilities: Capabilities {
                channels: 8,
                sample_rates: SampleRates::List(vec![1_000, 1_000_000]),
                triggers: vec![TriggerKind::RisingEdge],
                buffer_depth: None,
                trigger_limits: TriggerLimits::NONE,
                pre_post: Vec::new(),
                buffer_depths: Vec::new(),
                bursts: None,
                max_rates: Vec::new(),
            },
        }
    }

    fn code(d: &DeviceDescriptor) -> &'static str {
        d.validate().unwrap_err().code()
    }

    #[test]
    fn a_valid_descriptor_passes() {
        assert!(descriptor().validate().is_ok());
        for channels in [1, 32, 160] {
            let mut d = descriptor();
            d.capabilities.channels = channels;
            assert!(d.validate().is_ok());
        }
        let mut d = descriptor();
        d.capabilities.sample_rates = SampleRates::Range {
            min: 1,
            max: 500_000_000,
        };
        d.capabilities.triggers = Vec::new();
        d.capabilities.buffer_depth = Some(1);
        assert!(d.validate().is_ok());
    }

    #[test]
    fn invalid_descriptors_are_rejected_with_the_documented_code() {
        type Mutation = Box<dyn Fn(&mut DeviceDescriptor)>;
        let cases: Vec<Mutation> = vec![
            Box::new(|d| d.capabilities.channels = 0),
            Box::new(|d| d.capabilities.channels = 161),
            Box::new(|d| d.capabilities.sample_rates = SampleRates::List(Vec::new())),
            Box::new(|d| d.capabilities.sample_rates = SampleRates::List(vec![2_000, 1_000])),
            Box::new(|d| d.capabilities.sample_rates = SampleRates::List(vec![1_000, 1_000])),
            Box::new(|d| d.capabilities.sample_rates = SampleRates::List(vec![0, 1_000])),
            Box::new(|d| d.capabilities.sample_rates = SampleRates::Range { min: 0, max: 10 }),
            Box::new(|d| d.capabilities.sample_rates = SampleRates::Range { min: 11, max: 10 }),
            Box::new(|d| d.name = "  ".to_owned()),
            Box::new(|d| d.capabilities.buffer_depth = Some(0)),
        ];
        for (i, mutate) in cases.iter().enumerate() {
            let mut d = descriptor();
            mutate(&mut d);
            assert_eq!(code(&d), error::INVALID_CAPABILITIES, "case {i}");
        }
    }

    #[test]
    fn sample_rate_helpers() {
        let list = SampleRates::List(vec![1_000, 1_000_000, 100_000_000]);
        assert!(list.contains(1_000_000) && !list.contains(2_000));
        assert_eq!(list.nearest(2_000), Some(1_000));
        assert_eq!(list.nearest(500_000_000), Some(100_000_000));
        assert_eq!(list.nearest(500_500), Some(1_000));
        assert_eq!(list.nearest(500_501), Some(1_000_000));
        assert!(list.describe().contains("1000, 1000000, 100000000"));
        let range = SampleRates::Range { min: 10, max: 20 };
        assert!(
            range.contains(10) && range.contains(20) && !range.contains(9) && !range.contains(21)
        );
        assert_eq!(range.nearest(5), Some(10));
        assert_eq!(range.nearest(99), Some(20));
        assert_eq!(range.nearest(15), Some(15));
    }

    #[test]
    fn device_ids_are_stable_and_structured() {
        let a = DeviceId::new("usb", "0925:3881:SN1");
        assert_eq!(a, DeviceId::new("usb", "0925:3881:SN1"));
        assert_eq!(a.provider(), "usb");
        assert_eq!(a.to_string(), "usb:0925:3881:SN1");
        assert_ne!(a, DeviceId::new("usb", "0925:3881:SN2"));
    }
}
