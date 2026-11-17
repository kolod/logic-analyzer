//! Each file of `tests/recordings/` replays against the driver and gives the outcome its
//! `expect` line names (spec 013, user story 4, task T023).
//!
//! The files are written by hand from the protocol facts (their headers say so). The `expect`
//! line is a few words: `completed N`, `prepared completed N`, `failed CODE received R requested L`
//! or `open CODE [driver]`, with `rate R` and `signal counter` as options.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::panic
)]

mod common;

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use la_devices::{AcquisitionOptions, DeviceProvider, EndReason};
use la_driver_weact::WeActProvider;
use la_driver_weact::recording::Recording;
use la_driver_weact::testing::RecordedUsb;
use la_driver_weact::usb::BoardKey;

use common::{assert_capture_equals, config, outcome, registry_with, wait_until};

fn files() -> Vec<PathBuf> {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/recordings");
    let mut files: Vec<PathBuf> = std::fs::read_dir(dir)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|e| e == "rec"))
        .collect();
    files.sort();
    files
}

/// The word after `name` in the expectation, as a number.
fn number_after(words: &[&str], name: &str) -> Option<u64> {
    let at = words.iter().position(|w| *w == name)?;
    words.get(at + 1)?.parse().ok()
}

fn replay(path: &PathBuf) {
    let label = path.file_name().unwrap().to_string_lossy().into_owned();
    let recording = Recording::parse(&std::fs::read(path).unwrap()).unwrap();
    assert!(
        recording.origin.contains("hand-written") || recording.origin.contains("recorded"),
        "{label}: the header must say where the file came from"
    );
    let expectations = recording.expectations();
    assert_eq!(expectations.len(), 1, "{label}: one expect line");
    let words: Vec<&str> = expectations[0].split_whitespace().collect();
    let usb = RecordedUsb::new();
    let key = BoardKey::new("001", [1]);
    recording.apply(&usb, &key);
    let provider = Arc::new(WeActProvider::with_backend(
        Arc::new(usb.clone()),
        usb.timer(),
    ));
    let id = key.device_id();
    if words[0] == "open" {
        provider.scan().unwrap();
        let error = provider.open(&id).err().expect(&label);
        assert_eq!(error.code(), words[1], "{label}");
        if let Some(driver) = words.get(2) {
            assert!(
                error.detail().contains(driver),
                "{label}: {}",
                error.detail()
            );
        }
        return;
    }
    let registry = registry_with(provider);
    let rate = number_after(&words, "rate").unwrap_or(1_000_000);
    let options = AcquisitionOptions {
        channel_capacity: 100_000,
        ..AcquisitionOptions::default()
    };
    // A blank board is prepared by the provider; it is usable once the registry lists it as
    // ready, which `start` shows by not answering `weact.preparing`.
    let prepared = words[0] == "prepared";
    let start = |length: u64| {
        let started = std::time::Instant::now();
        loop {
            match registry.start(&id, config(0xFF, rate, Some(length)), options) {
                Err(error)
                    if started.elapsed() < Duration::from_secs(10)
                        && (error.detail().contains("weact.preparing")
                            || error.code() == "device.not_found") =>
                {
                    std::thread::sleep(Duration::from_millis(2));
                }
                other => return other.unwrap(),
            }
        }
    };
    let words: Vec<&str> = if prepared { words[1..].to_vec() } else { words };
    match words[0] {
        "completed" => {
            let length: u64 = words[1].parse().unwrap();
            assert!(wait_until(Duration::from_secs(10), || registry
                .describe(&id)
                .is_some()));
            let acquisition = start(length);
            let result = outcome(&acquisition);
            assert_eq!(result.end, EndReason::Completed, "{label}: {result:?}");
            assert_eq!(result.delivered_samples, length, "{label}");
            assert_eq!(result.lost_positions, 0, "{label}");
            if words.contains(&"counter") {
                let signal: Vec<u8> = (0..length).map(|i| (i % 256) as u8).collect();
                assert_capture_equals(&acquisition, &signal, 0xFF);
            }
            if prepared {
                assert!(
                    usb.log().iter().any(|r| matches!(
                        r,
                        la_driver_weact::testing::Request::ControlOut(0xA0, ..)
                    )),
                    "{label}: the board must have been downloaded to"
                );
            }
        }
        "failed" => {
            let received = number_after(&words, "received").unwrap();
            let requested = number_after(&words, "requested").unwrap();
            assert!(wait_until(Duration::from_secs(10), || registry
                .describe(&id)
                .is_some()));
            let acquisition = start(requested);
            let result = outcome(&acquisition);
            assert_eq!(result.end, EndReason::Failed, "{label}: {result:?}");
            let error = result.error.as_ref().unwrap();
            let detail_or_code = format!("{} {}", error.code(), error.detail());
            assert!(
                detail_or_code.contains(words[1]),
                "{label}: {detail_or_code}"
            );
            assert_eq!(result.delivered_samples, received, "{label}");
            if words[1] == "weact.stream_stopped" {
                assert_eq!(result.lost_positions, requested - received, "{label}");
                assert_eq!(result.positions, requested, "{label}: the books balance");
            } else {
                assert_eq!(result.lost_positions, 0, "{label}");
            }
            assert!(result.capture_status.interrupted, "{label}");
        }
        other => panic!("{label}: unknown expectation `{other}`"),
    }
}

#[test]
fn the_six_scenarios_are_present() {
    let names: Vec<String> = files()
        .iter()
        .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
        .collect();
    for wanted in [
        "ready-1mhz.rec",
        "blank-prepared.rec",
        "stream-stops-50khz.rec",
        "unplug-mid-stream.rec",
        "no-winusb.rec",
        "held-board.rec",
    ] {
        assert!(
            names.iter().any(|n| n == wanted),
            "{wanted} is missing: {names:?}"
        );
    }
}

#[test]
fn every_recording_replays_to_the_outcome_its_header_names() {
    for path in files() {
        replay(&path);
    }
}
