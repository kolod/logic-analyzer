//! The depth of a board with external memory (feature 017, T043): by rate band, the external memory below the rate it
//! can be written at and the internal one above it. The recordings are synthetic (`tests/fixtures/`).
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::panic
)]

use std::sync::Arc;

use la_devices::{
    ChannelSet, DeviceConfig, DeviceOptions, DeviceProvider, error::INVALID_CAPTURE_LENGTH,
    validate,
};
use la_driver_rp2350b::link::PortInfo;
use la_driver_rp2350b::testing::{ScriptedPorts, Step, parse_recording};
use la_driver_rp2350b::{Rp2350bProvider, USB_PID, USB_VID};
use la_storage::Limits;

fn fixture(name: &str) -> Vec<Step> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(name);
    parse_recording(&std::fs::read_to_string(path).unwrap()).unwrap()
}

fn capabilities(recording: &str) -> la_devices::Capabilities {
    let ports = Arc::new(ScriptedPorts::new());
    ports.add_port(
        PortInfo {
            name: "COM7".to_owned(),
            vid: USB_VID,
            pid: USB_PID,
            serial: Some("S1".to_owned()),
        },
        vec![fixture(recording)],
    );
    Rp2350bProvider::with_backend(ports)
        .scan()
        .unwrap()
        .remove(0)
        .capabilities
}

fn config(rate: u64, length: u64) -> DeviceConfig {
    DeviceConfig {
        enabled_channels: ChannelSet::all(32),
        sample_rate_hz: rate,
        trigger: None,
        pattern: None,
        capture_length: Some(length),
        pre_trigger_samples: 0,
        bursts: 0,
        options: DeviceOptions::None,
        accept_budget_risk: false,
    }
}

#[test]
fn a_board_without_external_memory_has_one_depth() {
    let caps = capabilities("info_32ch.rec");
    assert!(caps.buffer_depths.is_empty());
    assert_eq!(caps.buffer_depth, Some(96_000));
}

#[test]
fn a_board_with_external_memory_is_deeper_at_the_rates_it_can_write() {
    let caps = capabilities("info_32ch_psram.rec");
    assert_eq!(caps.buffer_depths.len(), 2);
    // Up to 20 MHz: 8 MiB of 4-byte samples. Above it: the internal memory, 96,000 samples.
    assert_eq!(caps.depth_at(20_000_000), Some(2_097_152));
    assert_eq!(caps.depth_at(20_000_001), Some(96_000));
    assert_eq!(caps.depth_at(100_000_000), Some(96_000));
    assert_eq!(caps.depth_at(3_100), Some(2_097_152));
    let limits = Limits {
        memory_budget_bytes: u64::MAX / 4,
        ..Limits::default()
    };
    // The core refuses what the memory cannot take at that rate, naming the depth.
    assert!(validate(&caps, &config(20_000_000, 2_097_152), &limits).is_ok());
    let error = validate(&caps, &config(20_000_000, 2_097_153), &limits).unwrap_err();
    assert_eq!(error.code(), INVALID_CAPTURE_LENGTH);
    assert!(error.detail().contains("2097152"), "{}", error.detail());
    assert!(validate(&caps, &config(50_000_000, 96_000), &limits).is_ok());
    assert_eq!(
        validate(&caps, &config(50_000_000, 96_001), &limits)
            .unwrap_err()
            .code(),
        INVALID_CAPTURE_LENGTH
    );
}
