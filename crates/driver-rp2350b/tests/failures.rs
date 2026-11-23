//! What the driver does when the board does not behave (part of T056): a refused request, a board that does not
//! answer, an unplug in the middle of a capture, a stream that stops, and replies that do not follow the
//! contract. Each is a typed error and nothing panics.
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
use la_driver_rp2350b::error::{PROTOCOL, REQUEST_REFUSED, STREAM_STOPPED, TIMEOUT};
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

/// Opens the board whose capture session is `recording`, starts a capture of 16 samples.
fn start(recording: &str) -> (Box<dyn la_devices::Device>, Result<(), Error>) {
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
        capture_length: Some(16),
        pre_trigger_samples: 0,
        bursts: 0,
        options: DeviceOptions::None,
        accept_budget_risk: false,
    };
    let resolved = validate(&descriptor.capabilities, &config, &Limits::default()).unwrap();
    let mut device = provider.open(&descriptor.id).unwrap();
    let result = device.start(&resolved).map(|_| ());
    (device, result)
}

/// Reads until an error or the end.
fn read_all(device: &mut dyn la_devices::Device) -> (usize, Result<(), Error>) {
    let mut samples = 0;
    for _ in 0..100_000 {
        let mut piece = Vec::new();
        match device.read(1000, &mut piece) {
            Ok(ReadOutcome::Samples) => samples += piece.len(),
            Ok(ReadOutcome::Finished) => return (samples, Ok(())),
            Ok(_) => {}
            Err(error) => return (samples, Err(error)),
        }
    }
    panic!("the capture neither ended nor failed");
}

#[test]
fn a_refused_request_names_the_field() {
    let (_, result) = start("refused.rec");
    let error = result.unwrap_err();
    assert_eq!(error.code(), REQUEST_REFUSED);
    assert!(error.detail().contains("frequency"), "{}", error.detail());
}

#[test]
fn an_err_line_is_a_refusal_too() {
    let error = start("err_busy.rec").1.unwrap_err();
    assert_eq!(error.code(), REQUEST_REFUSED);
    assert!(error.detail().contains("ERR_BUSY"));
}

#[test]
fn a_board_that_does_not_answer_the_request_times_out() {
    assert_eq!(start("no_answer.rec").1.unwrap_err().code(), TIMEOUT);
}

#[test]
fn an_unplug_in_the_middle_of_a_capture_keeps_what_arrived_and_says_so() {
    let (mut device, started) = start("unplugged.rec");
    started.unwrap();
    let (samples, result) = read_all(&mut *device);
    assert_eq!(samples, 4, "the four samples that came before the unplug");
    assert_eq!(result.unwrap_err().code(), la_devices::error::DISCONNECTED);
}

#[test]
fn a_stream_that_stops_is_reported_with_the_samples_so_far() {
    let (mut device, started) = start("stopped.rec");
    started.unwrap();
    let (samples, result) = read_all(&mut *device);
    assert_eq!(samples, 4);
    let error = result.unwrap_err();
    assert_eq!(error.code(), STREAM_STOPPED);
    assert!(error.detail().contains("4 of 16"), "{}", error.detail());
}

#[test]
fn replies_that_do_not_follow_the_contract_are_protocol_errors() {
    for recording in ["compact_unasked.rec", "too_many.rec"] {
        let (mut device, started) = start(recording);
        started.unwrap();
        let (_, result) = read_all(&mut *device);
        assert_eq!(result.unwrap_err().code(), PROTOCOL, "{recording}");
    }
}

#[test]
fn reading_before_starting_is_an_invalid_state_not_a_panic() {
    let ports = Arc::new(ScriptedPorts::new());
    ports.add_port(
        PortInfo {
            name: "COM7".to_owned(),
            vid: USB_VID,
            pid: USB_PID,
            serial: Some("S1".to_owned()),
        },
        vec![fixture("info_32ch.rec"), Vec::new()],
    );
    let provider = Rp2350bProvider::with_backend(ports);
    let descriptor = provider.scan().unwrap().remove(0);
    let mut device = provider.open(&descriptor.id).unwrap();
    let error = device.read(10, &mut Vec::new()).unwrap_err();
    assert_eq!(error.code(), la_devices::error::INVALID_STATE);
    device.stop().unwrap();
}
