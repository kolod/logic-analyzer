//! Samples the board could not keep (feature 017, FR-013): the driver reports each loss where it happened, the
//! samples stored plus the samples lost are the positions the capture covers, and marks that break the rules
//! are a protocol error. The recordings are synthetic (`tests/fixtures/`).
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::panic
)]

use std::sync::Arc;

use la_core::Error;
use la_devices::{ChannelSet, DeviceConfig, DeviceOptions, DeviceProvider, ReadOutcome, validate};
use la_driver_rp2350b::error::PROTOCOL;
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

/// What `read` reported, in order: a run of stored samples or a loss.
#[derive(Debug, PartialEq, Eq)]
enum Seen {
    Stored(usize),
    Lost(u64),
}

/// Reads the capture of `recording` (a request of `length` samples) to its end.
fn read(
    recording: &str,
    length: u64,
    max_samples: usize,
) -> (Vec<Seen>, Vec<u32>, Result<(), Error>) {
    let ports = Arc::new(ScriptedPorts::new());
    ports.add_port(
        PortInfo {
            name: "COM7".to_owned(),
            vid: USB_VID,
            pid: USB_PID,
            serial: Some("S1".to_owned()),
        },
        vec![fixture("info_32ch.rec"), fixture(recording)],
    );
    let provider = Rp2350bProvider::with_backend(ports);
    let descriptor = provider.scan().unwrap().remove(0);
    let config = DeviceConfig {
        enabled_channels: ChannelSet::all(32),
        sample_rate_hz: 1_000_000,
        trigger: None,
        pattern: None,
        capture_length: Some(length),
        pre_trigger_samples: 0,
        bursts: 0,
        options: DeviceOptions::None,
        accept_budget_risk: false,
    };
    let resolved = validate(&descriptor.capabilities, &config, &Limits::default()).unwrap();
    let mut device = provider.open(&descriptor.id).unwrap();
    device.start(&resolved).unwrap();
    let (mut seen, mut words) = (Vec::new(), Vec::new());
    for _ in 0..10_000 {
        let mut piece = Vec::new();
        match device.read(max_samples, &mut piece) {
            Ok(ReadOutcome::Samples) => {
                match seen.last_mut() {
                    Some(Seen::Stored(n)) => *n += piece.len(),
                    _ => seen.push(Seen::Stored(piece.len())),
                }
                words.extend(piece);
            }
            Ok(ReadOutcome::Lost(n)) => {
                assert!(piece.is_empty(), "a loss comes with no samples");
                seen.push(Seen::Lost(n));
            }
            Ok(ReadOutcome::Finished) => return (seen, words, Ok(())),
            Ok(ReadOutcome::Idle) => {}
            Err(error) => return (seen, words, Err(error)),
        }
    }
    panic!("the capture neither ended nor failed");
}

fn expected_words() -> Vec<u32> {
    (0..16_u32)
        .map(|i| i.wrapping_mul(0x0101_0101) ^ if i % 2 == 1 { 0x8000_0001 } else { 0 })
        .collect()
}

#[test]
fn each_loss_is_reported_where_it_happened() {
    for max_samples in [1000, 3, 1] {
        let (seen, words, result) = read("capture_with_loss.rec", 123, max_samples);
        result.unwrap();
        assert_eq!(
            seen,
            vec![
                Seen::Stored(4),
                Seen::Lost(100),
                Seen::Stored(6),
                Seen::Lost(2),
                Seen::Stored(6),
                Seen::Lost(5),
            ],
            "reads of {max_samples}"
        );
        assert_eq!(
            words,
            expected_words(),
            "the stored samples are exactly the recorded ones"
        );
    }
}

#[test]
fn the_samples_stored_and_lost_are_the_positions_the_capture_covers() {
    let (seen, _, result) = read("capture_with_loss.rec", 123, 1000);
    result.unwrap();
    let covered: u64 = seen
        .iter()
        .map(|s| match s {
            Seen::Stored(n) => *n as u64,
            Seen::Lost(n) => *n,
        })
        .sum();
    assert_eq!(covered, 123);
}

#[test]
fn a_capture_that_lost_everything_reports_the_loss_and_ends() {
    let (seen, words, result) = read("capture_all_lost.rec", 50, 1000);
    result.unwrap();
    assert_eq!(seen, vec![Seen::Lost(50)]);
    assert!(words.is_empty());
}

#[test]
fn marks_that_break_the_rules_are_protocol_errors() {
    // Descending positions; more lost than the request asked for.
    for recording in ["marks_descending.rec", "marks_too_much.rec"] {
        let (seen, _, result) = read(recording, 20, 1000);
        assert_eq!(result.unwrap_err().code(), PROTOCOL, "{recording}");
        assert!(
            seen.is_empty(),
            "no sample is delivered from a capture whose marks are wrong"
        );
    }
}

#[test]
fn a_capture_without_marks_is_unchanged() {
    let (seen, words, result) = read("capture_32ch.rec", 16, 1000);
    result.unwrap();
    assert_eq!(seen, vec![Seen::Stored(16)]);
    assert_eq!(words, expected_words());
}
