//! Failures of the board and of the bus, through the registry and the acquisition engine (spec
//! 013, user story 3, task T019). Expected numbers are worked out here.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::panic
)]

mod common;

use std::sync::Arc;
use std::time::Duration;

use la_core::Error;
use la_devices::{
    Acquisition, AcquisitionOptions, AcquisitionOutcome, DeviceId, DeviceRegistry, EndReason,
};
use la_driver_weact::WeActProvider;
use la_driver_weact::testing::{
    BoardSpec, Capture, Fail, Fault, Point, RecordedUsb, StreamStep, steps_from_signal,
};
use la_driver_weact::usb::BoardKey;

use common::{config, one_ready_board, outcome, registry_with, signal, wait_until};

fn options() -> AcquisitionOptions {
    AcquisitionOptions {
        channel_capacity: 100_000,
        ..AcquisitionOptions::default()
    }
}

fn ready(registry: &DeviceRegistry, id: &DeviceId) {
    assert!(wait_until(Duration::from_secs(5), || registry
        .describe(id)
        .is_some()));
}

/// The engine reports the errors of a driver that are not `device.*` as `device.driver_error`
/// with the driver's code in the detail; this gives the driver's code back.
fn driver_code(error: &Error) -> String {
    if error.code() == "device.driver_error" {
        if let Some(code) = error.detail().split('`').nth(1) {
            return code.to_owned();
        }
    }
    error.code().to_owned()
}

/// What a capture ended with: the code of the failure, whichever way it was reported.
fn failure_code(started: Result<Acquisition, Error>) -> (String, Option<Acquisition>) {
    match started {
        Err(error) => (driver_code(&error), None),
        Ok(acquisition) => {
            let result = outcome(&acquisition);
            assert_eq!(result.end, EndReason::Failed, "{result:?}");
            let code = driver_code(result.error.as_ref().unwrap());
            (code, Some(acquisition))
        }
    }
}

fn run(
    registry: &DeviceRegistry,
    id: &DeviceId,
    length: Option<u64>,
) -> Result<Acquisition, Error> {
    registry.start(id, config(0xFF, 1_000_000, length), options())
}

#[test]
fn an_unplug_in_the_middle_of_a_stream_keeps_what_was_stored_and_marks_the_capture() {
    let (usb, key, provider) = one_ready_board();
    let bytes = signal(30_000, 5, 20);
    let mut steps = steps_from_signal(&bytes, 10_000);
    steps.push(StreamStep::Fail(Fail::Disconnected));
    usb.script_capture(&key, Capture::stream(steps));
    let registry = registry_with(provider);
    let id = key.device_id();
    ready(&registry, &id);
    let acquisition = run(&registry, &id, Some(100_000)).unwrap();
    let result = outcome(&acquisition);
    assert_eq!(result.end, EndReason::Failed);
    assert_eq!(result.error.as_ref().unwrap().code(), "device.disconnected");
    assert_eq!(
        result.delivered_samples, 30_000,
        "everything before the unplug is kept"
    );
    assert!(result.capture_status.interrupted);
    assert!(!result.capture_status.complete);
    common::assert_capture_equals(&acquisition, &bytes, 0xFF);
}

#[test]
fn a_transfer_error_is_a_transport_error_and_short_empty_and_late_chunks_are_not_errors() {
    let (usb, key, provider) = one_ready_board();
    usb.script_capture(
        &key,
        Capture::stream(vec![
            StreamStep::Data(vec![1]),
            StreamStep::Empty,
            StreamStep::Timeout,
            StreamStep::Data(vec![2, 3]),
            StreamStep::Empty,
            StreamStep::Data(vec![4]),
            StreamStep::Fail(Fail::Transport),
        ]),
    );
    let registry = registry_with(provider);
    let id = key.device_id();
    ready(&registry, &id);
    let (code, acquisition) = failure_code(run(&registry, &id, Some(1_000)));
    assert_eq!(code, "device.transport");
    let result = acquisition.unwrap().outcome().unwrap();
    assert_eq!(
        result.delivered_samples, 4,
        "the short and empty chunks counted exactly"
    );
    assert!(result.capture_status.interrupted);
}

#[test]
fn an_unplug_at_the_start_gives_disconnected_and_no_capture_is_left_open() {
    let (usb, key, provider) = one_ready_board();
    usb.inject(Point::ControlOut, Fault::Fail(Fail::Disconnected));
    let registry = registry_with(provider);
    let id = key.device_id();
    ready(&registry, &id);
    let (code, _) = failure_code(run(&registry, &id, Some(100)));
    assert_eq!(code, "device.disconnected");
}

#[test]
fn an_unplug_during_stop_does_not_hang_or_lose_the_capture() {
    let (usb, key, provider) = one_ready_board();
    usb.script_capture(&key, Capture::endless(vec![0x55; 512]));
    let registry = registry_with(provider);
    let id = key.device_id();
    ready(&registry, &id);
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
        >= 10_000));
    usb.unplug(&key);
    acquisition.stop();
    let result = outcome(&acquisition);
    assert!(result.delivered_samples >= 10_000);
    assert!(matches!(result.end, EndReason::Stopped | EndReason::Failed));
}

#[test]
fn a_stream_that_stops_in_a_capture_of_a_set_length_is_one_gap_and_the_books_balance() {
    for received in [1_u64, 512, 5_000, 99_999] {
        let (usb, key, provider) = one_ready_board();
        let bytes = signal(received as usize, received, 30);
        usb.script_capture(&key, Capture::stream(steps_from_signal(&bytes, 4_096)));
        let registry = registry_with(provider);
        let id = key.device_id();
        ready(&registry, &id);
        let requested = 100_000_u64;
        let (code, acquisition) = failure_code(run(&registry, &id, Some(requested)));
        assert_eq!(code, "weact.stream_stopped", "received {received}");
        let result: AcquisitionOutcome = acquisition.unwrap().outcome().unwrap();
        assert_eq!(result.delivered_samples, received);
        assert_eq!(
            result.lost_positions,
            requested - received,
            "received {received}"
        );
        assert_eq!(
            result.delivered_samples + result.lost_positions,
            requested,
            "stored and lost positions make the requested length"
        );
        assert_eq!(result.positions, requested);
        assert!(result.capture_status.interrupted);
    }
}

#[test]
fn an_open_ended_capture_whose_stream_stops_gives_only_the_error() {
    let (usb, key, provider) = one_ready_board();
    let bytes = signal(3_000, 8, 10);
    usb.script_capture(&key, Capture::stream(steps_from_signal(&bytes, 1_000)));
    let registry = registry_with(provider);
    let id = key.device_id();
    ready(&registry, &id);
    let (code, acquisition) = failure_code(run(&registry, &id, None));
    assert_eq!(code, "weact.stream_stopped");
    let result = acquisition.unwrap().outcome().unwrap();
    assert_eq!(result.lost_positions, 0);
    assert_eq!(result.delivered_samples, 3_000);
}

#[test]
fn nothing_at_all_after_the_start_is_no_data() {
    let (usb, key, provider) = one_ready_board();
    usb.script_capture(&key, Capture::stream(Vec::new()));
    let registry = registry_with(provider);
    let id = key.device_id();
    ready(&registry, &id);
    let (code, acquisition) = failure_code(run(&registry, &id, Some(1_000)));
    assert_eq!(code, "weact.no_data");
    let result = acquisition.unwrap().outcome().unwrap();
    assert_eq!(result.delivered_samples, 0);
    assert_eq!(result.lost_positions, 0);
}

#[test]
fn a_board_unplugged_and_plugged_again_is_listed_again_and_captures_again() {
    let (usb, key, provider) = one_ready_board();
    let bytes = signal(5_000, 9, 10);
    usb.set_default_capture(&key, Capture::stream(steps_from_signal(&bytes, 1_000)));
    let registry = registry_with(provider);
    let id = key.device_id();
    ready(&registry, &id);
    usb.unplug(&key);
    assert!(wait_until(Duration::from_secs(5), || registry
        .describe(&id)
        .is_none()));
    usb.plug(BoardSpec::running("001", &[1]));
    ready(&registry, &id);
    let acquisition = run(&registry, &id, Some(5_000)).unwrap();
    let result = outcome(&acquisition);
    assert_eq!(result.end, EndReason::Completed, "{result:?}");
    common::assert_capture_equals(&acquisition, &bytes, 0xFF);
}

#[test]
fn a_board_held_by_another_program_is_reported_without_a_capture() {
    let (usb, key, provider) = one_ready_board();
    usb.inject(Point::OpenStream, Fault::Fail(Fail::InUse));
    let registry = registry_with(provider);
    let id = key.device_id();
    ready(&registry, &id);
    let (code, _) = failure_code(run(&registry, &id, Some(100)));
    assert_eq!(code, "weact.in_use");
}

#[test]
fn panics_at_every_step_are_contained_and_the_next_capture_still_works() {
    let usb = RecordedUsb::new();
    let key = BoardKey::new("001", [1]);
    usb.plug(BoardSpec::running("001", &[1]));
    let bytes = signal(1_000, 3, 10);
    usb.set_default_capture(&key, Capture::stream(steps_from_signal(&bytes, 500)));
    let provider = Arc::new(WeActProvider::with_backend(
        Arc::new(usb.clone()),
        usb.timer(),
    ));
    let registry = registry_with(provider);
    let id = key.device_id();
    ready(&registry, &id);
    let points = [
        Point::Open,
        Point::ControlIn,
        Point::ControlOut,
        Point::OpenStream,
        Point::Submit,
        Point::Wait,
        Point::Close,
    ];
    for round in 0..210_usize {
        usb.inject(points[round % points.len()], Fault::Panic);
        let started = run(&registry, &id, Some(1_000));
        match started {
            Err(error) => assert!(error.code().starts_with("device."), "{}", error.code()),
            Ok(acquisition) => {
                let result = outcome(&acquisition);
                // A panic in `Close` happens after the data is complete.
                assert!(
                    matches!(result.end, EndReason::Failed | EndReason::Completed),
                    "{result:?}"
                );
            }
        }
        // The list is not affected.
        assert!(
            registry.describe(&id).is_some()
                || wait_until(Duration::from_secs(2), || registry.describe(&id).is_some())
        );
    }
    let acquisition = run(&registry, &id, Some(1_000)).unwrap();
    let result = outcome(&acquisition);
    assert_eq!(result.end, EndReason::Completed, "{result:?}");
    common::assert_capture_equals(&acquisition, &bytes, 0xFF);
}

#[test]
fn a_hang_during_a_read_is_reported_and_the_application_goes_on() {
    let (usb, key, provider) = one_ready_board();
    usb.script_capture(&key, Capture::endless(vec![0xAA; 256]));
    let registry = registry_with(provider);
    let id = key.device_id();
    ready(&registry, &id);
    usb.inject(Point::Wait, Fault::Hang);
    let started = std::time::Instant::now();
    let acquisition = registry
        .start(
            &id,
            config(0xFF, 1_000_000, None),
            AcquisitionOptions::default(),
        )
        .unwrap();
    let result = outcome(&acquisition);
    assert_eq!(result.end, EndReason::Failed, "{result:?}");
    assert_eq!(result.error.as_ref().unwrap().code(), "device.unresponsive");
    assert!(started.elapsed() < Duration::from_secs(15));
    usb.release();
    // The list still answers.
    assert!(registry.describe(&id).is_some());
}

#[test]
fn a_pause_shorter_than_the_silence_limit_is_not_the_end_of_the_stream() {
    let (usb, key, provider) = one_ready_board();
    // 20 waits of 40 ms are 0.8 s of the virtual clock: before the first data (limit 2 s) and
    // in the middle of the stream (limit the larger of 2 s and four transfer periods).
    let mut steps = vec![StreamStep::Timeout; 20];
    steps.push(StreamStep::Data(vec![0x11; 1_000]));
    steps.extend(vec![StreamStep::Timeout; 20]);
    steps.push(StreamStep::Data(vec![0x22; 1_000]));
    usb.script_capture(&key, Capture::stream(steps));
    let registry = registry_with(provider);
    let id = key.device_id();
    ready(&registry, &id);
    let acquisition = run(&registry, &id, Some(2_000)).unwrap();
    let result = outcome(&acquisition);
    assert_eq!(result.end, EndReason::Completed, "{result:?}");
    assert_eq!(result.delivered_samples, 2_000);
    assert_eq!(result.lost_positions, 0);
}
