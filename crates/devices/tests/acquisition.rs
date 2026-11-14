//! Running a capture into the signal store, in the background (spec US3, FR-009..FR-016,
//! SC-001).
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

use common::{descriptor, registry_fast, std_config, virtual_config, wait_outcome, wait_until};
use la_devices::error::{BUSY, NOT_FOUND};
use la_devices::harness::{ScriptedDevice, Step, TestProvider};
use la_devices::{
    AcquisitionEvent, AcquisitionOptions, AcquisitionState, AppliedConfig, Device,
    DeviceDescriptor, DeviceId, EndReason, ReadOutcome, ResolvedConfig,
};
use la_storage::{LevelState, Limits};

const WAIT: Duration = Duration::from_secs(10);

fn virtual_id() -> DeviceId {
    DeviceId::new("virtual", "0")
}

fn start_virtual(
    registry: &la_devices::DeviceRegistry,
    length: Option<u64>,
    seed: u64,
) -> la_devices::Acquisition {
    registry
        .start(
            &virtual_id(),
            virtual_config(0xFFFF_FFFF, 1_000_000, length, seed),
            AcquisitionOptions::default(),
        )
        .unwrap()
}

#[test]
fn start_returns_immediately_and_samples_arrive_in_the_capture() {
    let registry = registry_fast();
    let started = Instant::now();
    let acq = start_virtual(&registry, None, 7);
    assert!(
        started.elapsed() < Duration::from_millis(50),
        "start took {:?}",
        started.elapsed()
    );
    assert!(wait_until(WAIT, || acq.progress().state == AcquisitionState::Running));
    assert!(wait_until(WAIT, || acq.progress().samples_captured > 100_000));
    assert!(acq.capture().snapshot().length() > 0);
    acq.stop();
    wait_outcome(&acq);
}

#[test]
fn readers_see_consistent_data_and_progress_while_it_runs() {
    let registry = registry_fast();
    let acq = start_virtual(&registry, None, 11);
    let reader = acq.capture();
    assert!(wait_until(WAIT, || reader.snapshot().length() > 50_000));

    // A snapshot taken mid-run never changes afterwards, and the final capture agrees with it.
    let mid = reader.snapshot();
    let mid_len = mid.length();
    let levels = |s: &la_storage::Snapshot, n: u64| -> Vec<LevelState> {
        (0..n)
            .step_by(97)
            .map(|p| s.level_at(5, p).unwrap())
            .collect()
    };
    let before = levels(&mid, mid_len);

    let mut last = 0;
    for _ in 0..50 {
        let progress = acq.progress();
        assert!(
            progress.samples_captured >= last,
            "progress never goes backwards"
        );
        assert!(progress.positions >= progress.samples_captured);
        last = progress.samples_captured;
        std::thread::sleep(Duration::from_millis(1));
    }
    acq.stop();
    let outcome = wait_outcome(&acq);
    let fin = reader.snapshot();
    assert!(fin.length() >= mid_len);
    assert_eq!(
        levels(&mid, mid_len),
        before,
        "a held snapshot never changes"
    );
    assert_eq!(
        levels(&fin, mid_len),
        before,
        "the final capture agrees with the mid-run one"
    );
    assert_eq!(fin.length(), outcome.positions);
}

#[test]
fn stop_returns_at_once_and_closes_the_capture_keeping_everything() {
    let registry = registry_fast();
    let acq = start_virtual(&registry, None, 3);
    assert!(wait_until(WAIT, || acq.progress().samples_captured > 10_000));
    let started = Instant::now();
    acq.stop();
    assert!(
        started.elapsed() < Duration::from_millis(10),
        "stop must not block"
    );
    let outcome = acq
        .wait_timeout(Duration::from_millis(500))
        .expect("the capture is closed shortly after stop");
    assert_eq!(outcome.end, EndReason::Stopped);
    assert!(outcome.error.is_none());
    assert!(outcome.capture_status.closed);
    assert!(
        outcome.capture_status.complete,
        "a user stop with no loss is complete"
    );
    assert_eq!(outcome.lost_positions, 0);
    assert_eq!(outcome.delivered_samples, outcome.positions);
    assert_eq!(acq.capture().snapshot().length(), outcome.positions);
    assert_eq!(acq.progress().state, AcquisitionState::Finished);
}

#[test]
fn a_finite_length_finishes_by_itself_with_a_complete_capture() {
    let registry = registry_fast();
    let length = 123_457; // not a multiple of any chunk size
    let acq = start_virtual(&registry, Some(length), 5);
    let outcome = wait_outcome(&acq);
    assert_eq!(outcome.end, EndReason::Completed);
    assert!(outcome.error.is_none());
    assert_eq!(outcome.positions, length);
    assert_eq!(outcome.delivered_samples, length);
    assert_eq!(outcome.lost_positions, 0);
    assert!(outcome.capture_status.complete && outcome.capture_status.closed);
    assert_eq!(acq.capture().snapshot().length(), length);
    assert_eq!(acq.progress().samples_captured, length);
}

#[test]
fn the_capture_description_reports_where_the_data_came_from() {
    let registry = registry_fast();
    let config = virtual_config(0x0000_00F0, 2_000_000, Some(1_000), 9);
    let acq = registry
        .start(&virtual_id(), config, AcquisitionOptions::default())
        .unwrap();
    let d = acq.description();
    assert_eq!(*d.device_id(), virtual_id());
    assert_eq!(d.device_name(), "Virtual device");
    assert_eq!(d.requested_sample_rate_hz(), 2_000_000);
    assert_eq!(d.sample_rate_hz(), 2_000_000);
    assert_eq!(d.channels(), 32);
    assert_eq!(d.enabled_mask(), 0x0000_00F0);
    assert!(d.trigger().is_none());
    wait_outcome(&acq);
    let snap = acq.capture().snapshot();
    assert_eq!(snap.sample_rate_hz(), 2_000_000);
    assert_eq!(snap.channels(), 32);
}

#[test]
fn a_second_start_on_a_busy_device_is_rejected_and_the_device_is_reusable_afterwards() {
    let registry = registry_fast();
    let first = start_virtual(&registry, None, 1);
    let e = registry
        .start(
            &virtual_id(),
            virtual_config(1, 1_000_000, None, 2),
            AcquisitionOptions::default(),
        )
        .unwrap_err();
    assert_eq!(e.code(), BUSY);
    first.stop();
    wait_outcome(&first);
    // The device is released once the outcome is set.
    let again = start_virtual(&registry, Some(1_000), 3);
    assert_eq!(wait_outcome(&again).end, EndReason::Completed);
}

#[test]
fn an_unknown_device_is_rejected() {
    let registry = registry_fast();
    let e = registry
        .start(
            &DeviceId::new("usb", "nope"),
            std_config(1, 1_000_000),
            AcquisitionOptions::default(),
        )
        .unwrap_err();
    assert_eq!(e.code(), NOT_FOUND);
}

#[test]
fn two_devices_run_independently() {
    let registry = registry_fast();
    let provider = TestProvider::new("usb");
    let d = descriptor("usb", "scripted", 8);
    let d_for_factory = d.clone();
    provider.plug_with(
        d.clone(),
        Arc::new(move || {
            let steps: Vec<Step> = (0..20)
                .map(|_| Step::Samples(vec![0b1, 0b0, 0b1, 0b0]))
                .collect();
            Box::new(
                ScriptedDevice::new(d_for_factory.clone(), steps)
                    .with_flow(la_devices::Flow::Throttleable),
            )
        }),
    );
    registry.add_provider(Arc::new(provider.clone()));
    assert!(wait_until(WAIT, || registry.describe(&d.id).is_some()));

    let virtual_run = start_virtual(&registry, None, 4);
    let scripted_run = registry
        .start(
            &d.id,
            std_config(0b1, 1_000_000),
            AcquisitionOptions::default(),
        )
        .unwrap();
    let scripted = wait_outcome(&scripted_run);
    assert_eq!(scripted.end, EndReason::Completed);
    assert_eq!(scripted.positions, 80);
    assert!(scripted.capture_status.complete);
    // The virtual acquisition keeps running independently until it is stopped.
    assert_eq!(virtual_run.progress().state, AcquisitionState::Running);
    virtual_run.stop();
    assert_eq!(wait_outcome(&virtual_run).end, EndReason::Stopped);
}

/// A driver that returns far more samples per read than the store accepts in one chunk.
struct OversizedDevice {
    descriptor: DeviceDescriptor,
    done: bool,
}

impl Device for OversizedDevice {
    fn descriptor(&self) -> &DeviceDescriptor {
        &self.descriptor
    }
    fn start(&mut self, config: &ResolvedConfig) -> la_core::Result<AppliedConfig> {
        Ok(AppliedConfig::new(config.sample_rate_hz()))
    }
    fn read(&mut self, _max: usize, out: &mut Vec<u32>) -> la_core::Result<ReadOutcome> {
        if self.done {
            return Ok(ReadOutcome::Finished);
        }
        self.done = true;
        out.extend((0..200_000_u32).map(|i| i & 1));
        Ok(ReadOutcome::Samples)
    }
    fn stop(&mut self) -> la_core::Result<()> {
        Ok(())
    }
}

#[test]
fn chunks_larger_than_the_store_accepts_are_split() {
    let registry = registry_fast();
    let provider = TestProvider::new("usb");
    let d = descriptor("usb", "oversized", 8);
    let for_factory = d.clone();
    provider.plug_with(
        d.clone(),
        Arc::new(move || {
            Box::new(OversizedDevice {
                descriptor: for_factory.clone(),
                done: false,
            })
        }),
    );
    registry.add_provider(Arc::new(provider.clone()));
    assert!(wait_until(WAIT, || registry.describe(&d.id).is_some()));
    let acq = registry
        .start(
            &d.id,
            std_config(0b1, 1_000_000),
            AcquisitionOptions::default(),
        )
        .unwrap();
    let outcome = wait_outcome(&acq);
    assert_eq!(outcome.end, EndReason::Completed);
    assert_eq!(outcome.positions, 200_000);
    assert_eq!(outcome.delivered_samples, 200_000);
    let snap = acq.capture().snapshot();
    // The pattern alternates at every sample, so the data survived the splitting intact.
    for p in [0_u64, 1, 65_535, 65_536, 131_071, 131_072, 199_999] {
        let expected = if p & 1 == 1 {
            LevelState::High
        } else {
            LevelState::Low
        };
        assert_eq!(snap.level_at(0, p).unwrap(), expected, "position {p}");
    }
}

#[test]
fn the_storage_budget_ends_an_acquisition_cleanly_with_intact_data() {
    let registry = registry_fast();
    let options = AcquisitionOptions {
        limits: Limits {
            memory_budget_bytes: 1024 * 1024,
            ..Limits::default()
        },
        chunk_samples: 4096,
        ..AcquisitionOptions::default()
    };
    // Every channel flips at every sample: the storage fills up quickly.
    let mut config = virtual_config(0xFF, 1_000_000, None, 21);
    if let la_devices::DeviceOptions::Virtual(v) = &mut config.options {
        v.mean_gap = 1;
    }
    let acq = registry.start(&virtual_id(), config, options).unwrap();
    let outcome = wait_outcome(&acq);
    assert_eq!(outcome.end, EndReason::Failed);
    assert_eq!(
        outcome.error.as_ref().unwrap().code(),
        "storage.memory_limit"
    );
    assert!(outcome.capture_status.closed);
    assert!(outcome.capture_status.stopped_by_limit);
    assert!(!outcome.capture_status.complete);

    // Everything stored before the limit is intact: it equals the same seed captured briefly.
    let mut reference_config = virtual_config(0xFF, 1_000_000, Some(1_000), 21);
    if let la_devices::DeviceOptions::Virtual(v) = &mut reference_config.options {
        v.mean_gap = 1;
    }
    let reference = registry
        .start(
            &virtual_id(),
            reference_config,
            AcquisitionOptions::default(),
        )
        .unwrap();
    wait_outcome(&reference);
    let (stored, expected) = (acq.capture().snapshot(), reference.capture().snapshot());
    assert!(
        stored.length() >= 1_000,
        "the budget holds more than a thousand positions"
    );
    for ch in [0_u8, 3, 7] {
        for p in 0..1_000 {
            assert_eq!(
                stored.level_at(ch, p).unwrap(),
                expected.level_at(ch, p).unwrap(),
                "channel {ch} position {p}"
            );
        }
    }
}

#[test]
fn stopping_twice_or_after_the_end_is_harmless() {
    let registry = registry_fast();
    let acq = start_virtual(&registry, Some(500), 8);
    wait_outcome(&acq);
    acq.stop();
    acq.stop();
    let outcome = acq.outcome().unwrap();
    assert_eq!(
        outcome.end,
        EndReason::Completed,
        "the outcome never changes once set"
    );
    assert_eq!(acq.outcome().unwrap().positions, outcome.positions);
}

#[test]
fn started_and_finished_events_arrive() {
    let registry = registry_fast();
    let acq = start_virtual(&registry, Some(10_000), 6);
    wait_outcome(&acq);
    let events: Vec<AcquisitionEvent> = acq.drain_events();
    assert!(
        matches!(events.first(), Some(AcquisitionEvent::Started { applied }) if applied.sample_rate_hz == 1_000_000)
    );
    assert!(matches!(events.last(), Some(AcquisitionEvent::Finished)));
}

#[test]
fn a_consumer_that_never_reads_events_cannot_slow_the_acquisition() {
    let registry = registry_fast();
    let started = Instant::now();
    for seed in 0..50 {
        let acq = start_virtual(&registry, Some(2_000), seed);
        // Nobody drains `acq.events()`; the acquisition must finish all the same.
        assert_eq!(wait_outcome(&acq).end, EndReason::Completed);
    }
    assert!(started.elapsed() < Duration::from_secs(30));
}

// ---- flow control: a device that can wait is never made to lose data --------------------

#[test]
fn a_device_that_can_wait_is_never_made_to_lose_samples_by_a_slow_store() {
    use la_devices::Flow;

    let registry = registry_fast();
    let provider = TestProvider::new("usb");
    registry.add_provider(Arc::new(provider.clone()));
    // Dense data keeps the store busy; the device hands over chunks instantly.
    let dense: Vec<u32> = (0..4_096_u32)
        .map(|i| if i & 1 == 1 { 0xFF } else { 0 })
        .collect();
    let steps: Vec<Step> = (0..500).map(|_| Step::Samples(dense.clone())).collect();
    let device =
        ScriptedDevice::new(descriptor("usb", "patient", 8), steps).with_flow(Flow::Throttleable);
    let acq = common::start_scripted(
        &registry,
        &provider,
        device,
        AcquisitionOptions {
            channel_capacity: 1,
            chunk_samples: 4_096,
            ..AcquisitionOptions::default()
        },
    );
    let outcome = wait_outcome(&acq);
    assert_eq!(outcome.end, EndReason::Completed);
    assert_eq!(
        outcome.lost_positions, 0,
        "a device that can wait loses nothing"
    );
    assert_eq!(outcome.delivered_samples, 500 * 4_096);
    assert!(outcome.capture_status.complete);
}

#[test]
fn the_virtual_device_in_fast_mode_never_loses_samples_even_with_a_tiny_channel() {
    let registry = registry_fast();
    let mut config = virtual_config(0xFF, 100_000_000, Some(300_000), 17);
    if let la_devices::DeviceOptions::Virtual(v) = &mut config.options {
        v.mean_gap = 2; // very dense: the store is the bottleneck
    }
    let acq = registry
        .start(
            &virtual_id(),
            config,
            AcquisitionOptions {
                channel_capacity: 1,
                limits: Limits {
                    memory_budget_bytes: 4 * 1024 * 1024 * 1024,
                    ..Limits::default()
                },
                ..AcquisitionOptions::default()
            },
        )
        .unwrap();
    let outcome = wait_outcome(&acq);
    assert_eq!(outcome.end, EndReason::Completed);
    assert_eq!(
        outcome.lost_positions, 0,
        "the virtual device never fails on its own"
    );
    assert_eq!(outcome.positions, 300_000);
    assert!(outcome.capture_status.complete);
}
