//! Helpers shared by the integration tests (not a test target itself).
#![allow(
    dead_code,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::panic
)]

use std::sync::Arc;
use std::time::{Duration, Instant};

use la_devices::{
    Acquisition, AcquisitionOutcome, DeviceConfig, DeviceOptions, DeviceRegistry, RegistryConfig,
};
use la_driver_weact::WeActProvider;
use la_driver_weact::testing::{BoardSpec, RecordedUsb};
use la_driver_weact::usb::BoardKey;

/// A bus with one board that runs the firmware, at `001` port 1, and the provider over it.
pub fn one_ready_board() -> (RecordedUsb, BoardKey, Arc<WeActProvider>) {
    let usb = RecordedUsb::new();
    let spec = BoardSpec::running("001", &[1]);
    let key = spec.key.clone();
    usb.plug(spec);
    let provider = Arc::new(WeActProvider::with_backend(
        Arc::new(usb.clone()),
        usb.timer(),
    ));
    (usb, key, provider)
}

/// A registry that scans quickly, with `provider` added and its first board listed.
pub fn registry_with(provider: Arc<WeActProvider>) -> DeviceRegistry {
    let registry = DeviceRegistry::new(RegistryConfig {
        scan_interval: Duration::from_millis(10),
        scan_timeout: Duration::from_millis(500),
    });
    registry.add_provider(provider);
    registry
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

/// A configuration for the board: the mask, the rate, the length (none: until stopped).
pub fn config(mask: u32, rate: u64, length: Option<u64>) -> DeviceConfig {
    DeviceConfig {
        enabled_channels: la_devices::ChannelSet::from_u32(mask),
        sample_rate_hz: rate,
        trigger: None,
        capture_length: length,
        options: DeviceOptions::None,
        accept_budget_risk: false,
        pattern: None,
        pre_trigger_samples: 0,
        bursts: 0,
    }
}

/// Waits up to 20 seconds for the outcome of an acquisition and fails the test on timeout.
pub fn outcome(acquisition: &Acquisition) -> AcquisitionOutcome {
    acquisition
        .wait_timeout(Duration::from_secs(20))
        .expect("the acquisition must end within 20 seconds")
}

/// A deterministic signal of `length` bytes made of runs of random lengths (1 to `max_run`),
/// from a fixed seed: no input of the board changes more often than the runs allow.
pub fn signal(length: usize, seed: u64, max_run: u64) -> Vec<u8> {
    let mut state = seed;
    let mut next = move || {
        state = state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        state >> 33
    };
    let mut out = Vec::with_capacity(length);
    while out.len() < length {
        let value = (next() & 0xFF) as u8;
        let run = 1 + next() % max_run;
        for _ in 0..run {
            if out.len() == length {
                break;
            }
            out.push(value);
        }
    }
    out
}

/// The positions at which input `channel` changes in `bytes` (the change between `p - 1` and
/// `p` is at `p`), worked out here and not by the code under test.
pub fn expected_transitions(bytes: &[u8], channel: u8) -> Vec<u64> {
    let bit = |byte: u8| (byte >> channel) & 1;
    (1..bytes.len())
        .filter(|p| bit(bytes[*p]) != bit(bytes[*p - 1]))
        .map(|p| p as u64)
        .collect()
}

/// Asserts that the stored capture equals `bytes` restricted to the channels of `mask`, on
/// every one of the eight channels: the first level and every transition.
pub fn assert_capture_equals(acquisition: &Acquisition, bytes: &[u8], mask: u32) {
    let snapshot = acquisition.capture().snapshot();
    assert_eq!(snapshot.length(), bytes.len() as u64, "the length");
    for channel in 0..8_u8 {
        let masked: Vec<u8> = if (mask >> channel) & 1 == 1 {
            bytes.to_vec()
        } else {
            vec![0; bytes.len()]
        };
        let expected = expected_transitions(&masked, channel);
        let found: Vec<u64> = if bytes.is_empty() {
            Vec::new()
        } else {
            snapshot
                .transitions(channel, 0, bytes.len() as u64)
                .unwrap()
                .map(|edge| edge.position)
                .collect()
        };
        assert_eq!(found, expected, "channel {channel}: the transitions");
        let first = masked.first().is_some_and(|b| (b >> channel) & 1 == 1);
        assert_eq!(
            snapshot.initial_high(channel).unwrap(),
            first,
            "channel {channel}: the first level"
        );
    }
}
