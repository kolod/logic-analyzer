//! Failures keep data and are never silent (spec US4, FR-017..FR-022, SC-005, SC-006).
//!
//! Every case uses a `ScriptedDevice`, so the exact stream the device accounted for is known.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::panic
)]

mod common;

use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use common::{
    assert_prefix, descriptor, pattern, registry_fast, start_scripted, wait_outcome, wait_until,
};
use la_core::ErrorKind;
use la_devices::error::{
    DISCONNECTED, DRIVER_ERROR, DRIVER_PANIC, TRANSPORT, UNRESPONSIVE, disconnected, driver_error,
    transport,
};
use la_devices::harness::{ScriptedDevice, Step, StopBehavior, TestProvider};
use la_devices::{
    AcquisitionEvent, AcquisitionOptions, AcquisitionOutcome, AcquisitionState, DeviceRegistry,
    EndReason,
};
use la_storage::Gap;

fn rig() -> (DeviceRegistry, TestProvider) {
    let registry = registry_fast();
    let provider = TestProvider::new("usb");
    registry.add_provider(std::sync::Arc::new(provider.clone()));
    (registry, provider)
}

fn device(unique: &str, steps: Vec<Step>) -> ScriptedDevice {
    ScriptedDevice::new(descriptor("usb", unique, 8), steps)
}

fn quick_stall() -> AcquisitionOptions {
    AcquisitionOptions {
        stall_timeout: Duration::from_millis(200),
        ..AcquisitionOptions::default()
    }
}

/// The accounting invariant of FR-019: stored samples plus lost positions equal what the
/// device accounted for, and the storage agrees.
fn assert_accounting(outcome: &AcquisitionOutcome, delivered: u64, lost: u64) {
    assert_eq!(outcome.delivered_samples, delivered, "delivered samples");
    assert_eq!(outcome.lost_positions, lost, "lost positions");
    assert_eq!(
        outcome.delivered_samples + outcome.lost_positions,
        outcome.positions,
        "delivered plus lost equals the positions of the capture"
    );
    assert_eq!(outcome.capture_status.lost_positions, lost);
}

/// A failure ends the acquisition, keeps the data and flags the capture.
fn assert_failed(outcome: &AcquisitionOutcome, code: &str) {
    assert_eq!(outcome.end, EndReason::Failed);
    let error = outcome.error.as_ref().expect("a failure carries an error");
    assert_eq!(error.code(), code, "{error}");
    assert!(
        !error.detail().is_empty(),
        "the detail must explain what happened"
    );
    assert!(outcome.capture_status.closed);
    assert!(
        outcome.capture_status.interrupted,
        "a failure interrupts the capture"
    );
    assert!(
        !outcome.capture_status.complete,
        "a failed capture is never complete"
    );
}

#[test]
fn a_disconnect_keeps_the_data_before_it_and_flags_the_capture() {
    let (registry, provider) = rig();
    let (a, b) = (pattern(5_000, 1), pattern(3_000, 2));
    let acq = start_scripted(
        &registry,
        &provider,
        device(
            "disconnect",
            vec![
                Step::Samples(a.clone()),
                Step::Samples(b.clone()),
                Step::Error(disconnected(
                    "the USB cable was unplugged; reconnect the device and start again",
                )),
            ],
        ),
        AcquisitionOptions::default(),
    );
    let outcome = wait_outcome(&acq);
    assert_failed(&outcome, DISCONNECTED);
    assert_eq!(outcome.error.as_ref().unwrap().kind(), ErrorKind::Device);
    assert!(
        outcome
            .error
            .as_ref()
            .unwrap()
            .detail()
            .contains("reconnect")
    );
    assert_accounting(&outcome, 8_000, 0);
    assert_prefix(&acq, &[a, b].concat());
    assert_eq!(acq.progress().state, AcquisitionState::Failed);
    assert!(
        acq.drain_events()
            .iter()
            .any(|e| matches!(e, AcquisitionEvent::Failed { code } if *code == DISCONNECTED))
    );
}

#[test]
fn transport_and_driver_errors_are_reported_with_their_own_code() {
    let (registry, provider) = rig();
    for (name, code, error) in [
        (
            "transport",
            TRANSPORT,
            transport("the USB transfer failed; try another port"),
        ),
        (
            "driver",
            DRIVER_ERROR,
            driver_error("the device sent an invalid packet"),
        ),
    ] {
        let words = pattern(2_000, 3);
        let acq = start_scripted(
            &registry,
            &provider,
            device(name, vec![Step::Samples(words.clone()), Step::Error(error)]),
            AcquisitionOptions::default(),
        );
        let outcome = wait_outcome(&acq);
        assert_failed(&outcome, code);
        assert_accounting(&outcome, 2_000, 0);
        assert_prefix(&acq, &words);
    }
}

#[test]
fn an_error_with_a_foreign_code_is_wrapped_as_a_driver_error() {
    let (registry, provider) = rig();
    let foreign = la_core::Error::new(ErrorKind::Internal, "weird.code", "something odd");
    let acq = start_scripted(
        &registry,
        &provider,
        device(
            "foreign",
            vec![Step::Samples(pattern(100, 4)), Step::Error(foreign)],
        ),
        AcquisitionOptions::default(),
    );
    let outcome = wait_outcome(&acq);
    assert_failed(&outcome, DRIVER_ERROR);
    assert!(
        outcome
            .error
            .as_ref()
            .unwrap()
            .detail()
            .contains("weird.code")
    );
}

#[test]
fn an_overrun_is_recorded_as_an_exact_gap_and_the_acquisition_continues() {
    let (registry, provider) = rig();
    let (a, b) = (pattern(4_000, 5), pattern(2_500, 6));
    let acq = start_scripted(
        &registry,
        &provider,
        device(
            "overrun",
            vec![
                Step::Samples(a.clone()),
                Step::Lost(1_000),
                Step::Samples(b.clone()),
                Step::Finish,
            ],
        ),
        AcquisitionOptions::default(),
    );
    let outcome = wait_outcome(&acq);
    assert_eq!(
        outcome.end,
        EndReason::Completed,
        "the device could continue"
    );
    assert!(outcome.error.is_none());
    assert_accounting(&outcome, 6_500, 1_000);
    assert_eq!(outcome.positions, 7_500);
    assert!(outcome.capture_status.closed);
    assert!(
        !outcome.capture_status.complete,
        "lost samples make the capture incomplete"
    );
    assert!(!outcome.capture_status.interrupted);

    let snap = acq.capture().snapshot();
    assert_eq!(
        snap.gaps(),
        &[Gap {
            start: 4_000,
            length: 1_000
        }]
    );
    assert_prefix(&acq, &a);
    // The data after the gap is intact too.
    for (i, word) in b.iter().enumerate() {
        let p = 5_000 + i as u64;
        let want = (word & 1) == 1;
        assert_eq!(
            snap.level_at(0, p).unwrap(),
            if want {
                la_storage::LevelState::High
            } else {
                la_storage::LevelState::Low
            }
        );
    }
    let events = acq.drain_events();
    assert!(events.contains(&AcquisitionEvent::GapRecorded {
        position: 4_000,
        length: 1_000
    }));
}

#[test]
fn a_driver_panic_is_contained_and_reported() {
    let (registry, provider) = rig();
    let words = pattern(6_000, 7);
    let acq = start_scripted(
        &registry,
        &provider,
        device("panic", vec![Step::Samples(words.clone()), Step::Panic]),
        AcquisitionOptions::default(),
    );
    let outcome = wait_outcome(&acq);
    assert_failed(&outcome, DRIVER_PANIC);
    assert_accounting(&outcome, 6_000, 0);
    assert_prefix(&acq, &words);
    // The application, the registry and other devices keep working.
    assert!(registry.list().iter().any(|d| d.id.as_str() == "virtual:0"));
}

#[test]
fn a_hanging_driver_is_abandoned_after_the_stall_timeout_without_blocking_the_caller() {
    let (registry, provider) = rig();
    let words = pattern(3_000, 8);
    let hanging = device(
        "hang",
        vec![
            Step::Samples(words.clone()),
            Step::Hang(Duration::from_secs(3600)),
        ],
    );
    let release = hanging.release_handle();
    let acq = start_scripted(&registry, &provider, hanging, quick_stall());
    assert!(wait_until(Duration::from_secs(5), || acq
        .progress()
        .samples_captured
        == 3_000));

    // The caller is never blocked, not even by a stop request.
    let started = Instant::now();
    acq.stop();
    assert!(started.elapsed() < Duration::from_millis(10));
    let waited = Instant::now();
    let outcome = acq
        .wait_timeout(Duration::from_secs(3))
        .expect("the stall timeout ends the acquisition");
    assert!(
        waited.elapsed() < Duration::from_millis(1_500),
        "{:?}",
        waited.elapsed()
    );
    assert_failed(&outcome, UNRESPONSIVE);
    assert_accounting(&outcome, 3_000, 0);
    assert_prefix(&acq, &words);
    // The device is free again although its abandoned driver thread still hangs.
    assert!(!registry_busy(&registry, &provider, "hang"));
    release.store(true, Ordering::Release);
}

/// Whether starting the device again reports `device.busy` (it must not after an outcome).
fn registry_busy(registry: &DeviceRegistry, provider: &TestProvider, unique: &str) -> bool {
    let d = descriptor("usb", unique, 8);
    provider.plug_with(
        d.clone(),
        common::once_factory(ScriptedDevice::new(d.clone(), vec![Step::Finish])),
    );
    assert!(wait_until(Duration::from_secs(5), || registry
        .describe(&d.id)
        .is_some()));
    match registry.start(
        &d.id,
        common::std_config(0b1, 1_000_000),
        AcquisitionOptions::default(),
    ) {
        Ok(acq) => {
            wait_outcome(&acq);
            false
        }
        Err(e) => e.code() == la_devices::error::BUSY,
    }
}

#[test]
fn a_hang_inside_stop_after_a_user_stop_ends_with_a_timeout_not_a_blocked_caller() {
    let (registry, provider) = rig();
    let device = ScriptedDevice::new(
        descriptor("usb", "stophang", 8),
        std::iter::repeat_with(|| Step::Idle)
            .take(100_000)
            .collect(),
    )
    .with_stop_behavior(StopBehavior::Hang(Duration::from_secs(3600)));
    let release = device.release_handle();
    let acq = start_scripted(&registry, &provider, device, quick_stall());
    assert!(wait_until(Duration::from_secs(5), || acq.progress().state
        == AcquisitionState::Running));
    let started = Instant::now();
    acq.stop();
    assert!(started.elapsed() < Duration::from_millis(10));
    let outcome = acq
        .wait_timeout(Duration::from_secs(3))
        .expect("a timeout outcome");
    assert_failed(&outcome, UNRESPONSIVE);
    assert!(outcome.capture_status.closed);
    release.store(true, Ordering::Release);
}

#[test]
fn a_device_that_cannot_start_fails_cleanly_without_data() {
    let (registry, provider) = rig();
    let acq = start_scripted(
        &registry,
        &provider,
        device("nostart", vec![]).with_start_error(driver_error("the firmware did not answer")),
        AcquisitionOptions::default(),
    );
    let outcome = wait_outcome(&acq);
    assert_failed(&outcome, DRIVER_ERROR);
    assert_accounting(&outcome, 0, 0);
    assert_eq!(outcome.positions, 0);
}

#[test]
fn a_disconnect_during_start_is_reported_as_a_disconnect() {
    let (registry, provider) = rig();
    let acq = start_scripted(
        &registry,
        &provider,
        device("startgone", vec![])
            .with_start_error(disconnected("the device vanished while starting")),
        AcquisitionOptions::default(),
    );
    let outcome = wait_outcome(&acq);
    assert_failed(&outcome, DISCONNECTED);
    assert_eq!(outcome.positions, 0);
}

#[test]
fn a_failure_right_after_a_loss_keeps_both_accounted() {
    let (registry, provider) = rig();
    let words = pattern(1_500, 9);
    let acq = start_scripted(
        &registry,
        &provider,
        device(
            "lossthenfail",
            vec![
                Step::Samples(words.clone()),
                Step::Lost(5),
                Step::Error(disconnected("unplugged right after an overrun")),
            ],
        ),
        AcquisitionOptions::default(),
    );
    let outcome = wait_outcome(&acq);
    assert_failed(&outcome, DISCONNECTED);
    assert_accounting(&outcome, 1_500, 5);
    assert_prefix(&acq, &words);
}

#[test]
fn a_failure_racing_with_the_stop_request_still_ends_in_a_defined_way() {
    let (registry, provider) = rig();
    let words = pattern(500, 10);
    let acq = start_scripted(
        &registry,
        &provider,
        device(
            "race",
            vec![
                Step::Samples(words.clone()),
                Step::Idle,
                Step::Idle,
                Step::Error(disconnected("unplugged while stopping")),
            ],
        ),
        AcquisitionOptions::default(),
    );
    acq.stop();
    let outcome = wait_outcome(&acq);
    assert!(matches!(
        outcome.end,
        EndReason::Stopped | EndReason::Failed
    ));
    assert!(outcome.capture_status.closed);
    assert_eq!(
        outcome.delivered_samples + outcome.lost_positions,
        outcome.positions,
        "accounting holds whichever way the race went"
    );
}

// ---- backpressure and the storage budget ------------------------------------------------

#[test]
fn a_store_that_is_slower_than_the_device_never_blocks_it_and_loses_nothing_unaccounted() {
    let (registry, provider) = rig();
    const CHUNKS: usize = 2_000;
    const CHUNK: usize = 4_096;
    // Dense data (every channel flips at every sample) keeps the ingest thread busy while the
    // scripted device hands over chunks as fast as it can.
    let dense: Vec<u32> = (0..CHUNK as u32)
        .map(|i| if i & 1 == 1 { 0xFF } else { 0 })
        .collect();
    let steps: Vec<Step> = (0..CHUNKS).map(|_| Step::Samples(dense.clone())).collect();
    let options = AcquisitionOptions {
        channel_capacity: 1,
        chunk_samples: CHUNK,
        ..AcquisitionOptions::default()
    };
    let acq = start_scripted(&registry, &provider, device("slowstore", steps), options);
    let outcome = wait_outcome(&acq);
    let accounted = (CHUNKS * CHUNK) as u64;
    assert_eq!(outcome.end, EndReason::Completed);
    assert_eq!(
        outcome.delivered_samples + outcome.lost_positions,
        accounted,
        "every position the device produced is either stored or recorded as lost"
    );
    assert_eq!(outcome.positions, accounted);
    assert!(
        outcome.lost_positions > 0,
        "the store cannot keep up with an instant device"
    );
    assert!(!outcome.capture_status.complete);
    let gap_sum: u64 = acq
        .capture()
        .snapshot()
        .gaps()
        .iter()
        .map(|g| g.length)
        .sum();
    assert_eq!(gap_sum, outcome.lost_positions);
}

#[test]
fn the_storage_budget_keeps_the_accounting_invariant() {
    let (registry, provider) = rig();
    let dense: Vec<u32> = (0..4_096_u32)
        .map(|i| if i & 1 == 1 { 0xFF } else { 0 })
        .collect();
    let steps: Vec<Step> = (0..2_000).map(|_| Step::Samples(dense.clone())).collect();
    let options = AcquisitionOptions {
        limits: la_storage::Limits {
            memory_budget_bytes: 1024 * 1024,
            ..la_storage::Limits::default()
        },
        chunk_samples: 4_096,
        // Room for every chunk: the store, not the channel, must be what runs out.
        channel_capacity: 4_096,
        ..AcquisitionOptions::default()
    };
    let acq = start_scripted(&registry, &provider, device("budget", steps), options);
    let outcome = wait_outcome(&acq);
    assert_eq!(outcome.end, EndReason::Failed);
    assert_eq!(
        outcome.error.as_ref().unwrap().code(),
        "storage.memory_limit"
    );
    assert!(outcome.capture_status.stopped_by_limit && !outcome.capture_status.complete);
    assert_eq!(
        outcome.delivered_samples + outcome.lost_positions,
        outcome.positions
    );
}
