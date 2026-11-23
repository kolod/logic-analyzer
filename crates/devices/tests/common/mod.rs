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
        enabled_channels: la_devices::ChannelSet::from_u32(enabled_channels),
        sample_rate_hz,
        trigger: None,
        capture_length: None,
        options: DeviceOptions::None,
        accept_budget_risk: false,
        pattern: None,
        pre_trigger_samples: 0,
        bursts: 0,
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
        enabled_channels: la_devices::ChannelSet::from_u32(enabled_channels),
        sample_rate_hz,
        trigger: None,
        capture_length,
        options: DeviceOptions::Virtual(VirtualOptions {
            seed,
            ..VirtualOptions::default()
        }),
        accept_budget_risk: false,
        pattern: None,
        pre_trigger_samples: 0,
        bursts: 0,
    }
}

use std::sync::{Arc, Mutex};

use la_devices::harness::{DeviceFactory, ScriptedDevice, TestProvider};
use la_devices::{AcquisitionOptions, Device};

/// A factory that hands out the given device once (a second `open` of the same device would
/// panic, which the tests never do).
pub fn once_factory(device: ScriptedDevice) -> DeviceFactory {
    let slot = Arc::new(Mutex::new(Some(device)));
    Arc::new(move || {
        let device = slot
            .lock()
            .unwrap()
            .take()
            .expect("the scripted device is opened once");
        Box::new(device) as Box<dyn Device>
    })
}

/// Plugs `device` into `provider` (registering the provider on first use), waits until the
/// registry lists it and starts an acquisition on it with an 8-channel standard configuration.
pub fn start_scripted(
    registry: &DeviceRegistry,
    provider: &TestProvider,
    device: ScriptedDevice,
    options: AcquisitionOptions,
) -> Acquisition {
    let descriptor = device.descriptor().clone();
    provider.plug_with(descriptor.clone(), once_factory(device));
    assert!(wait_until(Duration::from_secs(5), || registry
        .describe(&descriptor.id)
        .is_some()));
    registry
        .start(&descriptor.id, std_config(0b1111, 1_000_000), options)
        .unwrap()
}

/// A deterministic sample pattern of `n` words using the lowest four channels.
pub fn pattern(n: usize, salt: u32) -> Vec<u32> {
    (0..n as u32)
        .map(|i| (i.wrapping_mul(37).wrapping_add(i / 3).wrapping_add(salt)) & 0xF)
        .collect()
}

/// Asserts that the first `expected.len()` positions of the capture equal `expected`.
pub fn assert_prefix(acquisition: &Acquisition, expected: &[u32]) {
    let snap = acquisition.capture().snapshot();
    assert!(
        snap.length() >= expected.len() as u64,
        "capture is shorter than the data delivered"
    );
    for (p, word) in expected.iter().enumerate() {
        for ch in 0..4_u8 {
            let want = if (word >> ch) & 1 == 1 {
                la_storage::LevelState::High
            } else {
                la_storage::LevelState::Low
            };
            assert_eq!(
                snap.level_at(ch, p as u64).unwrap(),
                want,
                "channel {ch} position {p}"
            );
        }
    }
}
