//! 1,000 injected driver panics (spec SC-007).
//!
//! One test per file: the panic hook and the log are process-wide.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::panic
)]

mod common;

use std::sync::Arc;
use std::time::{Duration, Instant};

use common::{
    assert_prefix, descriptor, once_factory, pattern, registry_fast, std_config, wait_outcome,
    wait_until,
};
use la_devices::error::DRIVER_PANIC;
use la_devices::harness::{ScriptedDevice, Step, StopBehavior, TestProvider};
use la_devices::{AcquisitionOptions, AcquisitionState, DeviceId, EndReason, VirtualOptions};
use la_diagnostics::{LogConfig, init_logging, install_panic_hook};

const RUNS: usize = 1_000;
/// The whole loop must finish within this budget (it takes a few seconds in practice).
const TIME_BUDGET: Duration = Duration::from_secs(120);

fn log_dir() -> std::path::PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("la-devices-panic-{}-{nanos}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// Counts the log entries that report a panic of a driver worker.
fn panic_entries(dir: &std::path::Path) -> usize {
    std::fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|e| e == "log"))
        .map(|p| {
            std::fs::read_to_string(p)
                .unwrap()
                .lines()
                .filter(|l| {
                    l.contains("scripted driver panic") && l.contains("\"worker\":\"driver-")
                })
                .count()
        })
        .sum()
}

#[test]
fn a_thousand_injected_panics_never_crash_anything_or_lose_delivered_data() {
    let dir = log_dir();
    let _guard = init_logging(LogConfig {
        directory: dir.clone(),
        max_total_bytes: 64 * 1024 * 1024,
        ..LogConfig::default()
    });
    install_panic_hook();

    let registry = registry_fast();
    let provider = TestProvider::new("usb");
    registry.add_provider(Arc::new(provider.clone()));
    let id = DeviceId::new("usb", "victim");

    // An unrelated acquisition runs the whole time and must be unaffected by every panic.
    let bystander = registry
        .start(
            &DeviceId::new("virtual", "0"),
            common::virtual_config(0xFF, 1_000_000, None, 99),
            AcquisitionOptions::default(),
        )
        .unwrap();
    assert!(wait_until(Duration::from_secs(5), || bystander
        .progress()
        .samples_captured
        > 0));

    let started = Instant::now();
    for run in 0..RUNS {
        let kind = run % 4;
        let blocks = run % 6; // 0..=5 sample steps before the panic
        let expected: Vec<u32> = (0..blocks)
            .flat_map(|b| pattern(200 + b * 37, run as u32))
            .collect();
        let mut steps: Vec<Step> = (0..blocks)
            .map(|b| Step::Samples(pattern(200 + b * 37, run as u32)))
            .collect();
        let d = descriptor("usb", "victim", 8);
        let device = match kind {
            // Panics while reading, after `blocks` chunks.
            0..=2 => {
                steps.push(Step::Panic);
                ScriptedDevice::new(d.clone(), steps)
            }
            // Panics inside `stop` after a user stop (the device idles until it is stopped).
            _ => {
                steps.extend(std::iter::repeat_with(|| Step::Idle).take(2_000));
                ScriptedDevice::new(d.clone(), steps).with_stop_behavior(StopBehavior::Panic)
            }
        };
        provider.plug_with(d.clone(), once_factory(device));
        assert!(wait_until(Duration::from_secs(5), || registry
            .describe(&id)
            .is_some()));
        let acq = registry
            .start(
                &id,
                std_config(0b1111, 1_000_000),
                AcquisitionOptions::default(),
            )
            .unwrap();
        if kind == 3 {
            assert!(wait_until(Duration::from_secs(5), || {
                acq.progress().samples_captured == expected.len() as u64
            }));
            acq.stop();
        }
        let outcome = wait_outcome(&acq);
        assert_eq!(outcome.end, EndReason::Failed, "run {run}");
        assert_eq!(
            outcome.error.as_ref().unwrap().code(),
            DRIVER_PANIC,
            "run {run}"
        );
        assert!(
            outcome.capture_status.closed && outcome.capture_status.interrupted,
            "run {run}"
        );
        assert!(!outcome.capture_status.complete, "run {run}");
        assert_eq!(
            outcome.delivered_samples,
            expected.len() as u64,
            "run {run}"
        );
        assert_eq!(outcome.positions, expected.len() as u64, "run {run}");
        assert_prefix(&acq, &expected);
        assert!(
            started.elapsed() < TIME_BUDGET,
            "run {run} is past the time budget"
        );
    }

    // The bystander never noticed.
    assert_eq!(bystander.progress().state, AcquisitionState::Running);
    bystander.stop();
    let outcome = wait_outcome(&bystander);
    assert_eq!(outcome.end, EndReason::Stopped);
    assert!(outcome.capture_status.complete);
    drop(VirtualOptions::default());

    // Every panic was logged exactly once, by the worker that contained it.
    assert_eq!(
        panic_entries(&dir),
        RUNS,
        "one log entry per injected panic"
    );
}
