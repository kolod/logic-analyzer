//! The UI thread is never blocked by a device (spec SC-003, SC-004; constitution Principle VI).
//!
//! Every call the UI makes returns quickly even while a provider's `scan` hangs, a driver's
//! `open` hangs and another driver hangs inside `read`.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::panic
)]

mod common;

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use common::{descriptor, registry_fast, std_config, virtual_config, wait_until};
use la_devices::harness::{ScriptedDevice, Step, TestProvider};
use la_devices::{AcquisitionOptions, Device, DeviceId, EndReason, validate};

/// The longest a UI-facing call may take.
#[cfg(debug_assertions)]
const UI_CALL_LIMIT: Duration = Duration::from_millis(50);
/// The longest a UI-facing call may take.
#[cfg(not(debug_assertions))]
const UI_CALL_LIMIT: Duration = Duration::from_millis(10);

/// The longest a stop request may take to close the capture of the virtual device.
#[cfg(debug_assertions)]
const STOP_LIMIT: Duration = Duration::from_millis(500);
/// The longest a stop request may take to close the capture of the virtual device.
#[cfg(not(debug_assertions))]
const STOP_LIMIT: Duration = Duration::from_millis(100);

/// Runs `call` 25 times and returns the slowest run.
fn slowest<T>(mut call: impl FnMut() -> T) -> Duration {
    (0..25)
        .map(|_| {
            let started = Instant::now();
            std::hint::black_box(call());
            started.elapsed()
        })
        .max()
        .unwrap()
}

fn assert_fast(what: &str, observed: Duration) {
    println!("latency {what}: {observed:?} (limit {UI_CALL_LIMIT:?})");
    assert!(
        observed < UI_CALL_LIMIT,
        "{what} took {observed:?}, the limit is {UI_CALL_LIMIT:?}"
    );
}

#[test]
fn ui_calls_stay_fast_while_scan_open_and_read_all_hang() {
    let registry = registry_fast();
    let provider = TestProvider::new("usb");
    registry.add_provider(Arc::new(provider.clone()));

    // A device that blocks inside `read`.
    let stuck = descriptor("usb", "stuck-read", 8);
    let stuck_device = ScriptedDevice::new(
        stuck.clone(),
        vec![
            Step::Samples(vec![1, 2, 3]),
            Step::Hang(Duration::from_secs(3600)),
        ],
    );
    let release = stuck_device.release_handle();
    provider.plug_with(stuck.clone(), common::once_factory(stuck_device));

    // A device whose `open` blocks.
    let slow_open = descriptor("usb", "slow-open", 8);
    let open_release = Arc::new(AtomicBool::new(false));
    let flag = Arc::clone(&open_release);
    let for_factory = slow_open.clone();
    provider.plug_with(
        slow_open.clone(),
        Arc::new(move || {
            let started = Instant::now();
            while !flag.load(Ordering::Acquire) && started.elapsed() < Duration::from_secs(3600) {
                std::thread::sleep(Duration::from_millis(5));
            }
            Box::new(ScriptedDevice::new(for_factory.clone(), vec![])) as Box<dyn Device>
        }),
    );
    assert!(wait_until(Duration::from_secs(5), || {
        registry.describe(&stuck.id).is_some() && registry.describe(&slow_open.id).is_some()
    }));

    let stuck_acquisition = registry
        .start(
            &stuck.id,
            std_config(0b1111, 1_000_000),
            AcquisitionOptions::default(),
        )
        .unwrap();
    // Wait for the hung read to be reached, then hang the scan too.
    assert!(wait_until(Duration::from_secs(5), || stuck_acquisition
        .progress()
        .samples_captured
        == 3));
    provider.set_scan_hang(true);

    assert_fast("list", slowest(|| registry.list()));
    assert_fast("describe", slowest(|| registry.describe(&stuck.id)));
    let caps = stuck.capabilities.clone();
    assert_fast(
        "validate",
        slowest(|| {
            validate(
                &caps,
                &std_config(0b1111, 1_000_000),
                &AcquisitionOptions::default().limits,
            )
        }),
    );
    assert_fast("progress", slowest(|| stuck_acquisition.progress()));
    assert_fast("outcome", slowest(|| stuck_acquisition.outcome()));
    assert_fast("drain_events", slowest(|| stuck_acquisition.drain_events()));

    // `start` returns at once although `open` of this device blocks.
    let started = Instant::now();
    let slow = registry
        .start(
            &slow_open.id,
            std_config(0b1111, 1_000_000),
            AcquisitionOptions::default(),
        )
        .unwrap();
    assert_fast("start (open hangs)", started.elapsed());
    assert_fast("progress (open hangs)", slowest(|| slow.progress()));

    // `stop` is a request, not a wait.
    assert_fast("stop (read hangs)", slowest(|| stuck_acquisition.stop()));
    assert_fast("stop (open hangs)", slowest(|| slow.stop()));

    // Let the abandoned threads finish.
    release.store(true, Ordering::Release);
    open_release.store(true, Ordering::Release);
    provider.release();
    let _ = stuck_acquisition.wait_timeout(Duration::from_secs(10));
    let _ = slow.wait_timeout(Duration::from_secs(10));
}

#[test]
fn stopping_the_virtual_device_closes_the_capture_within_the_limit() {
    let registry = registry_fast();
    let id = DeviceId::new("virtual", "0");
    let mut worst = Duration::ZERO;
    for round in 0..5_u64 {
        let acquisition = registry
            .start(
                &id,
                virtual_config(0xFFFF_FFFF, 100_000_000, None, round),
                AcquisitionOptions::default(),
            )
            .unwrap();
        assert!(wait_until(Duration::from_secs(10), || acquisition
            .progress()
            .samples_captured
            > 100_000));
        let started = Instant::now();
        acquisition.stop();
        let outcome = acquisition
            .wait_timeout(Duration::from_secs(10))
            .expect("the acquisition must end");
        let took = started.elapsed();
        assert_eq!(outcome.end, EndReason::Stopped);
        assert!(outcome.capture_status.closed);
        worst = worst.max(took);
    }
    println!("latency stop to closed capture: {worst:?} (limit {STOP_LIMIT:?})");
    assert!(
        worst < STOP_LIMIT,
        "stop took {worst:?} to close the capture, the limit is {STOP_LIMIT:?}"
    );
}
