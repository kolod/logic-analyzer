//! The virtual device on request: scripted failures and triggers (spec US6, FR-028, FR-029).
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::panic
)]

mod common;

use common::{registry_fast, wait_outcome};
use la_devices::error::{DISCONNECTED, TRANSPORT, TRIGGER_TIMEOUT};
use la_devices::{
    Acquisition, AcquisitionOptions, AcquisitionOutcome, DeviceConfig, DeviceId, DeviceOptions,
    DeviceRegistry, EndReason, FailureKind, ScriptedFailure, Trigger, TriggerKind, VirtualOptions,
};
use la_storage::{Gap, LevelState, Snapshot};

fn virtual_id() -> DeviceId {
    DeviceId::new("virtual", "0")
}

fn options(seed: u64) -> VirtualOptions {
    VirtualOptions {
        seed,
        ..VirtualOptions::default()
    }
}

fn config(mask: u32, length: u64, options: VirtualOptions) -> DeviceConfig {
    DeviceConfig {
        enabled_channels: mask,
        sample_rate_hz: 1_000_000,
        trigger: None,
        capture_length: Some(length),
        options: DeviceOptions::Virtual(options),
        accept_budget_risk: false,
    }
}

fn scripted(mut options: VirtualOptions, failures: &[(u64, FailureKind)]) -> VirtualOptions {
    options.failures = failures
        .iter()
        .map(|&(position, kind)| ScriptedFailure { position, kind })
        .collect();
    options
}

/// Runs the virtual device and returns the acquisition and its outcome, whatever it was.
fn run(
    registry: &DeviceRegistry,
    config: DeviceConfig,
    chunk_samples: usize,
) -> (Acquisition, AcquisitionOutcome) {
    let acq = registry
        .start(
            &virtual_id(),
            config,
            AcquisitionOptions {
                chunk_samples,
                ..AcquisitionOptions::default()
            },
        )
        .unwrap();
    let outcome = wait_outcome(&acq);
    (acq, outcome)
}

/// A clean, untriggered capture used as the reference for the same seed.
fn clean(registry: &DeviceRegistry, mask: u32, length: u64, options: VirtualOptions) -> Snapshot {
    let (acq, outcome) = run(registry, config(mask, length, options), 16_384);
    assert_eq!(outcome.end, EndReason::Completed);
    acq.capture().snapshot()
}

#[test]
fn a_scripted_loss_is_an_exact_gap_at_the_exact_position_for_any_chunking() {
    let registry = registry_fast();
    for chunk in [7_usize, 4_096, 65_536] {
        for kind in [FailureKind::Loss(1_000), FailureKind::Overrun(1_000)] {
            let script = scripted(options(3), &[(50_000, kind)]);
            let (acq, outcome) = run(&registry, config(0xFFFF, 100_000, script), chunk);
            assert_eq!(outcome.end, EndReason::Completed, "chunk {chunk} {kind:?}");
            assert_eq!(outcome.delivered_samples, 100_000);
            assert_eq!(outcome.lost_positions, 1_000);
            assert_eq!(outcome.positions, 101_000);
            assert!(
                !outcome.capture_status.complete,
                "lost samples make it incomplete"
            );
            assert_eq!(
                acq.capture().snapshot().gaps(),
                &[Gap {
                    start: 50_000,
                    length: 1_000
                }],
                "chunk {chunk} {kind:?}"
            );
        }
    }
}

#[test]
fn a_transport_error_or_disconnect_ends_the_acquisition_after_exactly_the_scripted_samples() {
    let registry = registry_fast();
    let reference = clean(&registry, 0xFFFF, 40_000, options(5));
    for (kind, code, at) in [
        (FailureKind::TransportError, TRANSPORT, 20_000_u64),
        (FailureKind::Disconnect, DISCONNECTED, 30_000),
    ] {
        for chunk in [7_usize, 65_536] {
            let script = scripted(options(5), &[(at, kind)]);
            let (acq, outcome) = run(&registry, config(0xFFFF, 100_000, script), chunk);
            assert_eq!(outcome.end, EndReason::Failed, "{kind:?} chunk {chunk}");
            assert_eq!(outcome.error.as_ref().unwrap().code(), code);
            assert_eq!(
                outcome.positions, at,
                "exactly the scripted number of samples"
            );
            assert_eq!(outcome.delivered_samples, at);
            assert!(outcome.capture_status.interrupted && !outcome.capture_status.complete);
            // The data before the failure is the data a clean run produces.
            let snap = acq.capture().snapshot();
            for channel in [0_u8, 7, 15] {
                for p in (0..at).step_by(61) {
                    assert_eq!(
                        snap.level_at(channel, p).unwrap(),
                        reference.level_at(channel, p).unwrap(),
                        "{kind:?} ch {channel} pos {p}"
                    );
                }
            }
        }
    }
}

#[test]
fn several_failures_apply_in_position_order_even_if_listed_out_of_order() {
    let registry = registry_fast();
    let script = scripted(
        options(8),
        &[
            (5_000, FailureKind::Disconnect),
            (1_000, FailureKind::Loss(10)),
        ],
    );
    let (acq, outcome) = run(&registry, config(0xFF, 100_000, script), 16_384);
    assert_eq!(outcome.end, EndReason::Failed);
    assert_eq!(outcome.error.as_ref().unwrap().code(), DISCONNECTED);
    // Positions count capture positions (samples plus lost ones): the loss comes first, then
    // the disconnect at position 5,000, so 4,990 samples were delivered.
    assert_eq!(outcome.positions, 5_000);
    assert_eq!(outcome.lost_positions, 10);
    assert_eq!(outcome.delivered_samples, 4_990);
    assert_eq!(
        acq.capture().snapshot().gaps(),
        &[Gap {
            start: 1_000,
            length: 10
        }]
    );
}

#[test]
fn without_a_script_the_device_never_fails_in_100_runs() {
    let registry = registry_fast();
    for seed in 0..100 {
        let (_, outcome) = run(&registry, config(u32::MAX, 3_000, options(seed)), 16_384);
        assert_eq!(outcome.end, EndReason::Completed, "seed {seed}");
        assert_eq!(outcome.lost_positions, 0);
        assert!(outcome.capture_status.complete);
    }
}

// ---- triggers ----------------------------------------------------------------------------

fn triggered(channel: u8, kind: TriggerKind, seed: u64, length: u64) -> DeviceConfig {
    DeviceConfig {
        trigger: Some(Trigger { channel, kind }),
        ..config(0xFF, length, options(seed))
    }
}

/// The position at which the trigger condition is first met in an untriggered stream.
fn expected_trigger(reference: &Snapshot, channel: u8, kind: TriggerKind) -> u64 {
    let length = reference.length();
    let level = |p: u64| reference.level_at(channel, p).unwrap() == LevelState::High;
    match kind {
        TriggerKind::Level(high) => (0..length).find(|&p| level(p) == high).unwrap(),
        TriggerKind::RisingEdge => (1..length).find(|&p| !level(p - 1) && level(p)).unwrap(),
        TriggerKind::FallingEdge => (1..length).find(|&p| level(p - 1) && !level(p)).unwrap(),
        TriggerKind::AnyEdge => (1..length).find(|&p| level(p - 1) != level(p)).unwrap(),
    }
}

#[test]
fn every_trigger_kind_starts_capturing_at_the_trigger_sample_and_reports_the_skipped_samples() {
    let registry = registry_fast();
    for seed in [2_u64, 3, 4] {
        let reference = clean(&registry, 0xFF, 20_000, options(seed));
        for kind in [
            TriggerKind::RisingEdge,
            TriggerKind::FallingEdge,
            TriggerKind::AnyEdge,
            TriggerKind::Level(true),
            TriggerKind::Level(false),
        ] {
            let channel = 3;
            let skipped = expected_trigger(&reference, channel, kind);
            let (acq, outcome) = run(&registry, triggered(channel, kind, seed, 1_000), 16_384);
            assert_eq!(outcome.end, EndReason::Completed, "seed {seed} {kind:?}");
            assert_eq!(outcome.positions, 1_000);
            assert_eq!(
                acq.description().samples_before_trigger(),
                skipped,
                "seed {seed} {kind:?}"
            );
            assert_eq!(acq.description().trigger().map(|t| t.kind), Some(kind));
            // The capture is exactly the untriggered stream from the trigger sample on.
            let snap = acq.capture().snapshot();
            for ch in [0_u8, 3, 7] {
                for p in 0..1_000 {
                    assert_eq!(
                        snap.level_at(ch, p).unwrap(),
                        reference.level_at(ch, skipped + p).unwrap(),
                        "seed {seed} {kind:?} ch {ch} pos {p}"
                    );
                }
            }
            // The first capture sample satisfies the trigger.
            let first_high = snap.level_at(channel, 0).unwrap() == LevelState::High;
            match kind {
                TriggerKind::RisingEdge | TriggerKind::Level(true) => assert!(first_high),
                TriggerKind::FallingEdge | TriggerKind::Level(false) => assert!(!first_high),
                TriggerKind::AnyEdge => {}
            }
        }
    }
}

#[test]
fn a_trigger_that_never_occurs_fails_with_a_timeout() {
    let registry = registry_fast();
    let mut tested = 0;
    for seed in 1..=10 {
        let sparse = VirtualOptions {
            mean_gap: 1_000_000,
            trigger_search_limit: 1_000,
            ..options(seed)
        };
        // Only seeds whose first 1,000 samples really contain no edge on channel 0.
        let reference = clean(&registry, 0b1, 1_000, sparse.clone());
        if reference.transitions(0, 0, 1_000).unwrap().count() > 0 {
            continue;
        }
        let config = DeviceConfig {
            trigger: Some(Trigger {
                channel: 0,
                kind: TriggerKind::RisingEdge,
            }),
            ..config(0b1, 1_000, sparse)
        };
        let (_, outcome) = run(&registry, config, 16_384);
        assert_eq!(outcome.end, EndReason::Failed, "seed {seed}");
        assert_eq!(outcome.error.as_ref().unwrap().code(), TRIGGER_TIMEOUT);
        assert_eq!(outcome.positions, 0);
        assert!(outcome.error.as_ref().unwrap().detail().contains("1000"));
        tested += 1;
    }
    assert!(tested >= 3, "enough seeds without an early edge ({tested})");
}
