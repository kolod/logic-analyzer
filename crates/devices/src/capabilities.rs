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
