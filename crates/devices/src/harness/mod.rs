//! Hardware-free test harness (feature `test-support`; not part of the stable API).
//!
//! * [`ScriptedDevice`] replays a script of samples, losses, errors, panics and hangs;
//! * [`CountingDevice`] counts every call made to the device it wraps;
//! * [`TestProvider`] is a provider with scripted appear/disappear sequences;
//! * [`RecordedTransport`] replays recorded USB or serial traffic to protocol code;
//! * [`conformance`] is the shared suite every driver must pass.
//!
//! Deliberate panics and hangs exist only here, as fault injection for tests; production
//! code never panics on external input.

pub mod conformance;
mod provider;
mod scripted;
mod transport;

pub use provider::{DeviceFactory, TestProvider};
pub use scripted::{CallCounter, CountingDevice, ScriptedDevice, Step, StopBehavior};
pub use transport::{RecordedTransport, TransportLog, TransportStep};

use crate::capabilities::{Capabilities, DeviceDescriptor, DeviceId, SampleRates, TriggerKind};

/// A valid descriptor for tests: `"<provider>:<unique>"`, the given channel count, sample rates
/// 1 kHz, 1 MHz and 100 MHz, rising and falling edge triggers, streaming, not simulated.
#[must_use]
pub fn test_descriptor(provider: &str, unique: &str, channels: u8) -> DeviceDescriptor {
    DeviceDescriptor {
        id: DeviceId::new(provider, unique),
        name: format!("Test device {unique}"),
        simulated: false,
        capabilities: Capabilities {
            channels,
            sample_rates: SampleRates::List(vec![1_000, 1_000_000, 100_000_000]),
            triggers: vec![TriggerKind::RisingEdge, TriggerKind::FallingEdge],
            buffer_depth: None,
            trigger_limits: crate::TriggerLimits::NONE,
            pre_post: Vec::new(),
            buffer_depths: Vec::new(),
            bursts: None,
            max_rates: Vec::new(),
            actions: Vec::new(),
        },
    }
}
