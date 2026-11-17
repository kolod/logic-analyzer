//! Captures from a ready board through the real acquisition engine (spec 013, user story 1,
//! task T011). The signals and the expected transitions are worked out in the test, not by the
//! code under test.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::panic
)]

mod common;

use std::time::{Duration, Instant};

use la_devices::{AcquisitionOptions, EndReason};
use la_driver_weact::protocol::{REQUEST_GET_FW_VERSION, REQUEST_START, REQUEST_STOP};
use la_driver_weact::testing::{Capture, Request, StreamStep, steps_from_signal};

use common::{
    assert_capture_equals, config, one_ready_board, outcome, registry_with, signal, wait_until,
};

/// Options that never let the store lag behind the scripted board (so nothing is lost for a
/// reason that is not the test's).
fn options() -> AcquisitionOptions {
    AcquisitionOptions {
        channel_capacity: 100_000,
        ..AcquisitionOptions::default()
    }
}

#[test]
fn a_capture_of_a_known_signal_is_stored_sample_for_sample() {
    let (usb, key, provider) = one_ready_board();
    let bytes = signal(100_000, 1, 40);
    usb.script_capture(&key, Capture::stream(steps_from_signal(&bytes, 10_240)));
    let registry = registry_with(provider);
    let id = key.device_id();
    assert!(wait_until(Duration::from_secs(5), || registry
        .describe(&id)
        .is_some()));
    let acquisition = registry
        .start(&id, config(0xFF, 1_000_000, Some(100_000)), options())
        .unwrap();
    let result = outcome(&acquisition);
    assert_eq!(result.end, EndReason::Completed, "{result:?}");
    assert_eq!(result.delivered_samples, 100_000);
    assert_eq!(result.lost_positions, 0);
    assert!(result.capture_status.complete);
    assert_capture_equals(&acquisition, &bytes, 0xFF);
}

#[test]
fn channels_that_are_not_enabled_are_zero() {
    let (usb, key, provider) = one_ready_board();
    let bytes = signal(20_000, 2, 30);
    usb.script_capture(&key, Capture::stream(steps_from_signal(&bytes, 4_096)));
    let registry = registry_with(provider);
    let id = key.device_id();
    assert!(wait_until(Duration::from_secs(5), || registry
        .describe(&id)
        .is_some()));
    // Channels 1 and 3 only.
    let acquisition = registry
        .start(&id, config(0b0000_1010, 1_000_000, Some(20_000)), options())
        .unwrap();
    let result = outcome(&acquisition);
    assert_eq!(result.end, EndReason::Completed, "{result:?}");
    assert_capture_equals(&acquisition, &bytes, 0b0000_1010);
}

#[test]
fn a_set_length_shorter_than_the_data_truncates_the_last_chunk_and_finishes() {
    let (usb, key, provider) = one_ready_board();
    let bytes = signal(100_000, 3, 25);
    usb.script_capture(&key, Capture::stream(steps_from_signal(&bytes, 30_000)));
    let registry = registry_with(provider);
    let id = key.device_id();
    assert!(wait_until(Duration::from_secs(5), || registry
        .describe(&id)
        .is_some()));
    // 70,000 is in the middle of the third chunk (60,000 to 90,000).
    let acquisition = registry
        .start(&id, config(0xFF, 1_000_000, Some(70_000)), options())
        .unwrap();
    let result = outcome(&acquisition);
    assert_eq!(result.end, EndReason::Completed, "{result:?}");
    assert_eq!(result.delivered_samples, 70_000);
    assert_capture_equals(&acquisition, &bytes[..70_000], 0xFF);
}

#[test]
fn an_open_ended_capture_runs_until_stopped_and_stops_promptly() {
    let (usb, key, provider) = one_ready_board();
    usb.script_capture(
        &key,
        Capture::endless([0xA5, 0x5A, 0x00, 0xFF].repeat(1_024)),
    );
    let registry = registry_with(provider);
    let id = key.device_id();
    assert!(wait_until(Duration::from_secs(5), || registry
        .describe(&id)
        .is_some()));
    let acquisition = registry
        .start(
            &id,
            config(0xFF, 1_000_000, None),
            AcquisitionOptions::default(),
        )
        .unwrap();
    assert!(wait_until(Duration::from_secs(10), || acquisition
        .progress()
        .samples_captured
        >= 50_000));
    let asked = Instant::now();
    acquisition.stop();
    let result = outcome(&acquisition);
    // The contract gives 100 ms; the margin is for a loaded machine.
    assert!(
        asked.elapsed() < Duration::from_millis(250),
        "stop took {:?}",
        asked.elapsed()
    );
    assert_eq!(result.end, EndReason::Stopped, "{result:?}");
    // The scripted board is instant, so the store may lag and the excess is recorded as lost
    // (the default queue): only the end and the amount are asserted here.
    assert!(result.delivered_samples >= 50_000);
    // The board is ready again: a second capture can start at once.
    usb.script_capture(&key, Capture::endless(vec![0x0F]));
    let second = registry
        .start(&id, config(0xFF, 1_000_000, Some(1_000)), options())
        .unwrap();
    assert_eq!(outcome(&second).end, EndReason::Completed);
}

#[test]
fn two_captures_in_a_row_work_and_old_bytes_in_the_buffer_never_enter_the_second() {
    two_captures_in_a_row(&[1, 5]);
}

#[test]
fn the_same_holds_for_the_stock_firmware_that_keeps_old_bytes_until_they_are_read() {
    two_captures_in_a_row(&[1, 4]);
}

fn two_captures_in_a_row(firmware: &[u8]) {
    let (usb, key, provider) = one_ready_board();
    usb.set_version(&key, firmware);
    let first = signal(30_000, 4, 20);
    // Bytes left in the board's buffer by the first capture; the signal starts with 0x00 and
    // never has 0xFF in its first samples, so a leak would be visible.
    let mut second = vec![0x00_u8; 16];
    second.extend(signal(30_000, 5, 20));
    usb.script_capture(&key, Capture::stream(steps_from_signal(&first, 8_192)));
    usb.script_capture(
        &key,
        Capture {
            stale: vec![
                StreamStep::Data(vec![0xFF; 3_000]),
                StreamStep::Data(vec![0xFF; 512]),
            ],
            stream: steps_from_signal(&second, 8_192),
            endless: None,
        },
    );
    let registry = registry_with(provider);
    let id = key.device_id();
    assert!(wait_until(Duration::from_secs(5), || registry
        .describe(&id)
        .is_some()));
    let one = registry
        .start(&id, config(0xFF, 1_000_000, Some(30_000)), options())
        .unwrap();
    assert_eq!(outcome(&one).end, EndReason::Completed);
    assert_capture_equals(&one, &first, 0xFF);
    let two = registry
        .start(
            &id,
            config(0xFF, 1_000_000, Some(second.len() as u64)),
            options(),
        )
        .unwrap();
    assert_eq!(outcome(&two).end, EndReason::Completed);
    assert_capture_equals(&two, &second, 0xFF);
}

#[test]
fn the_requests_are_sent_in_the_order_of_the_contract() {
    let log = requests_of_one_capture(&[1, 5]);
    let position = |wanted: &dyn Fn(&Request) -> bool| log.iter().position(wanted);
    let open = position(&|r| matches!(r, Request::Open(_))).expect("the board is opened");
    let version = position(&|r| matches!(r, Request::ControlIn(REQUEST_GET_FW_VERSION, 0, 0, 2)))
        .expect("the firmware version is read");
    let stream = position(&|r| matches!(r, Request::OpenStream(_))).expect("the stream is opened");
    let start = position(&|r| {
        matches!(r, Request::ControlOut(REQUEST_START, 0, 0, data) if data == &[0x40, 0x00, 0x2F])
    })
    .expect("CMD_START with the payload of 1 MHz: 48 MHz clock, delay 47");
    let submit = position(&|r| matches!(r, Request::SubmitAll)).expect("the buffers are submitted");
    let close = position(&|r| matches!(r, Request::Close)).expect("the stream is closed");
    let stop = position(
        &|r| matches!(r, Request::ControlOut(REQUEST_STOP, 0, 0, data) if data.is_empty()),
    )
    .expect("CMD_STOP ends the capture of firmware 1.5");
    // START with nothing in flight, then the reads; the stop after the transfers are cancelled.
    assert!(
        open < version
            && version < stream
            && stream < start
            && start < submit
            && submit < close
            && close < stop,
        "{log:?}"
    );
}

#[test]
fn stock_firmware_is_started_with_the_buffers_already_submitted_and_gets_no_stop() {
    let log = requests_of_one_capture(&[1, 4]);
    let position = |wanted: &dyn Fn(&Request) -> bool| log.iter().position(wanted);
    let submit = position(&|r| matches!(r, Request::SubmitAll)).unwrap();
    let start = position(&|r| matches!(r, Request::ControlOut(REQUEST_START, ..))).unwrap();
    let close = position(&|r| matches!(r, Request::Close)).unwrap();
    assert!(submit < start && start < close, "{log:?}");
    assert!(
        !log.iter()
            .any(|r| matches!(r, Request::ControlOut(REQUEST_STOP, ..))),
        "firmware 1.4 has no stop command: {log:?}"
    );
}

/// The requests of one 5,000-sample capture at 1 MHz on a board with this firmware version.
fn requests_of_one_capture(firmware: &[u8]) -> Vec<Request> {
    let (usb, key, provider) = one_ready_board();
    usb.set_version(&key, firmware);
    let bytes = signal(5_000, 6, 10);
    usb.script_capture(&key, Capture::stream(steps_from_signal(&bytes, 2_048)));
    let registry = registry_with(provider);
    let id = key.device_id();
    assert!(wait_until(Duration::from_secs(5), || registry
        .describe(&id)
        .is_some()));
    usb.clear_log();
    let acquisition = registry
        .start(&id, config(0xFF, 1_000_000, Some(5_000)), options())
        .unwrap();
    assert_eq!(outcome(&acquisition).end, EndReason::Completed);
    // The stop command follows the end of the capture: give the worker a moment.
    std::thread::sleep(Duration::from_millis(100));
    usb.log()
        .into_iter()
        .filter(|r| !matches!(r, Request::List))
        .collect()
}

#[test]
fn a_configuration_the_core_rejects_never_reaches_the_board() {
    let (usb, key, provider) = one_ready_board();
    let registry = registry_with(provider);
    let id = key.device_id();
    assert!(wait_until(Duration::from_secs(5), || registry
        .describe(&id)
        .is_some()));
    usb.clear_log();
    for (cfg, code) in [
        (
            config(0xFF, 48_000_000, Some(1_000)),
            "device.unsupported_sample_rate",
        ),
        (
            config(0xFF, 30_000_000, Some(1_000)),
            "device.unsupported_sample_rate",
        ),
        (
            config(0xFF, 123_456, Some(1_000)),
            "device.unsupported_sample_rate",
        ),
        (
            config(0x100, 1_000_000, Some(1_000)),
            "device.invalid_channel",
        ),
        (
            config(0x00, 1_000_000, Some(1_000)),
            "device.invalid_channel",
        ),
    ] {
        let error = registry.start(&id, cfg, options()).err().unwrap();
        assert_eq!(error.code(), code);
    }
    let sent = usb.log().into_iter().any(|r| {
        matches!(
            r,
            Request::Open(_)
                | Request::OpenStream(_)
                | Request::SubmitAll
                | Request::ControlOut(..)
        )
    });
    assert!(!sent, "nothing may be sent to the board: {:?}", usb.log());
}

#[test]
fn a_capture_is_not_simulated_and_records_the_device_and_the_rate() {
    let (usb, key, provider) = one_ready_board();
    let bytes = signal(2_000, 7, 10);
    usb.script_capture(&key, Capture::stream(steps_from_signal(&bytes, 1_024)));
    let registry = registry_with(provider);
    let id = key.device_id();
    assert!(wait_until(Duration::from_secs(5), || registry
        .describe(&id)
        .is_some()));
    let acquisition = registry
        .start(&id, config(0xFF, 1_000_000, Some(2_000)), options())
        .unwrap();
    assert_eq!(outcome(&acquisition).end, EndReason::Completed);
    let description = acquisition.description();
    assert!(!description.simulated());
    assert_eq!(description.device_id(), &id);
    assert_eq!(description.device_name(), "WeAct LogicAnalyzer V1");
    assert_eq!(description.sample_rate_hz(), 1_000_000);
    assert_eq!(description.requested_sample_rate_hz(), 1_000_000);
    assert_eq!(description.enabled_mask(), 0xFF);
    assert_eq!(description.channels(), 8);
}

#[test]
fn every_listed_rate_starts_a_capture_with_its_own_payload() {
    use la_driver_weact::protocol::{SAMPLE_RATES, start_payload};
    let (usb, key, provider) = one_ready_board();
    let registry = registry_with(provider);
    let id = key.device_id();
    assert!(wait_until(Duration::from_secs(5), || registry
        .describe(&id)
        .is_some()));
    for rate in SAMPLE_RATES {
        usb.clear_log();
        usb.script_capture(&key, Capture::stream(vec![StreamStep::Data(vec![1; 64])]));
        let acquisition = registry
            .start(&id, config(0xFF, rate, Some(64)), options())
            .unwrap();
        assert_eq!(
            outcome(&acquisition).end,
            EndReason::Completed,
            "rate {rate}"
        );
        let payload = start_payload(rate).unwrap();
        assert!(
            usb.log().iter().any(
                |r| matches!(r, Request::ControlOut(REQUEST_START, 0, 0, data) if data == &payload)
            ),
            "rate {rate}: {:?}",
            usb.log()
        );
    }
}
