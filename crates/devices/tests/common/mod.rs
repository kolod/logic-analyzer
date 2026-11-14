//! Helpers shared by the integration tests (not a test target itself).
#![allow(
    dead_code,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::panic
)]

use std::time::{Duration, Instant};

use la_devices::harness::test_descriptor;
use la_devices::{DeviceConfig, DeviceDescriptor, DeviceOptions, DeviceRegistry, RegistryConfig};

/// A registry that scans quickly, so tests do not wait long.
pub fn registry_fast() -> DeviceRegistry {
    DeviceRegistry::new(RegistryConfig {
        scan_interval: Duration::from_millis(10),
        scan_timeout: Duration::from_millis(100),
    })
}

/// Polls `condition` every millisecond until it holds or `timeout` passes.
pub fn wait_until(timeout: Duration, mut condition: impl FnMut() -> bool) -> bool {
    let started = Instant::now();
    while started.elapsed() < timeout {
        if condition() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(1));
    }
    condition()
}

/// A valid descriptor (see `harness::test_descriptor`).
pub fn descriptor(provider: &str, unique: &str, channels: u8) -> DeviceDescriptor {
    test_descriptor(provider, unique, channels)
}

/// A valid configuration for the devices of `descriptor`: the given channel mask and rate,
/// no trigger, no length limit.
pub fn std_config(enabled_channels: u32, sample_rate_hz: u64) -> DeviceConfig {
    DeviceConfig {
        enabled_channels,
        sample_rate_hz,
        trigger: None,
        capture_length: None,
        options: DeviceOptions::None,
        accept_budget_risk: false,
    }
}

use la_devices::{Acquisition, AcquisitionOutcome, VirtualOptions};

/// Waits up to 10 seconds for the outcome of an acquisition and fails the test on timeout.
pub fn wait_outcome(acquisition: &Acquisition) -> AcquisitionOutcome {
    acquisition
        .wait_timeout(Duration::from_secs(10))
        .expect("the acquisition must end within 10 seconds")
}

/// A configuration for the virtual device with the given mask, rate, length and seed.
pub fn virtual_config(
    enabled_channels: u32,
    sample_rate_hz: u64,
    capture_length: Option<u64>,
    seed: u64,
) -> DeviceConfig {
    DeviceConfig {
        enabled_channels,
        sample_rate_hz,
        trigger: None,
        capture_length,
        options: DeviceOptions::Virtual(VirtualOptions {
            seed,
            ..VirtualOptions::default()
        }),
        accept_budget_risk: false,
    }
}
