//! The virtual device: reproducible, labelled, with a signal-source hook (spec US5, US6;
//! FR-023..FR-030, SC-008, SC-009).
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::panic
)]

mod common;

use std::collections::hash_map::DefaultHasher;
use std::hash::Hasher;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use common::{descriptor, registry_fast, start_scripted, std_config, wait_outcome};
use la_devices::harness::{ScriptedDevice, Step, TestProvider};
use la_devices::{
    AcquisitionOptions, DeviceConfig, DeviceId, DeviceOptions, DeviceRegistry, EndReason, Pacing,
    SignalSource, SignalSourceFactory, VirtualOptions,
};
use la_storage::{LevelState, Snapshot};

fn virtual_id() -> DeviceId {
    DeviceId::new("virtual", "0")
}

fn config(mask: u32, rate: u64, length: u64, options: VirtualOptions) -> DeviceConfig {
    DeviceConfig {
        enabled_channels: la_devices::ChannelSet::from_u32(mask),
        sample_rate_hz: rate,
        trigger: None,
        capture_length: Some(length),
        options: DeviceOptions::Virtual(options),
        accept_budget_risk: false,
    }
}

/// Runs the virtual device to completion and returns the finished capture.
fn capture(
    registry: &DeviceRegistry,
    mask: u32,
    rate: u64,
    length: u64,
    options: VirtualOptions,
    chunk_samples: usize,
) -> (Snapshot, la_devices::Acquisition) {
    let acq = registry
        .start(
            &virtual_id(),
            config(mask, rate, length, options),
            AcquisitionOptions {
                chunk_samples,
                ..AcquisitionOptions::default()
            },
        )
        .unwrap();
    let outcome = wait_outcome(&acq);
    assert_eq!(outcome.end, EndReason::Completed);
    assert_eq!(
        outcome.lost_positions, 0,
        "the virtual device never loses on its own"
    );
    assert!(outcome.capture_status.complete);
    (acq.capture().snapshot(), acq)
}

/// A hash of everything observable in the first `length` positions of every channel.
fn fingerprint(snap: &Snapshot, length: u64) -> u64 {
    let mut hasher = DefaultHasher::new();
    for channel in 0..snap.channels() {
        let edges: Vec<(u64, bool)> = snap
            .transitions(channel, 0, length)
            .unwrap()
            .map(|e| (e.position, e.rising))
            .collect();
        hasher.write(format!("{channel}:{edges:?}").as_bytes());
        for p in (0..length).step_by(251) {
            hasher.write(format!("{:?}", snap.level_at(channel, p).unwrap()).as_bytes());
        }
    }
    hasher.finish()
}

fn options(seed: u64) -> VirtualOptions {
    VirtualOptions {
        seed,
        ..VirtualOptions::default()
    }
}

const LENGTH: u64 = 20_000;

#[test]
fn the_same_seed_and_configuration_give_identical_captures_in_100_runs() {
    let registry = registry_fast();
    let (first, _) = capture(&registry, u32::MAX, 1_000_000, LENGTH, options(42), 16_384);
    let expected = fingerprint(&first, LENGTH);
    for run in 0..100 {
        let (snap, _) = capture(&registry, u32::MAX, 1_000_000, LENGTH, options(42), 16_384);
        assert_eq!(fingerprint(&snap, LENGTH), expected, "run {run}");
    }
}

#[test]
fn different_seeds_give_different_captures() {
    let registry = registry_fast();
    let (base, _) = capture(&registry, u32::MAX, 1_000_000, LENGTH, options(1), 16_384);
    let base = fingerprint(&base, LENGTH);
    for seed in 2..=21 {
        let (snap, _) = capture(
            &registry,
            u32::MAX,
            1_000_000,
            LENGTH,
            options(seed),
            16_384,
        );
        assert_ne!(fingerprint(&snap, LENGTH), base, "seed {seed}");
    }
}

#[test]
fn the_result_does_not_depend_on_the_chunk_size() {
    let registry = registry_fast();
    let (reference, _) = capture(&registry, u32::MAX, 1_000_000, LENGTH, options(7), 65_536);
    let expected = fingerprint(&reference, LENGTH);
    for chunk in [1_usize, 7, 4_096, 16_384] {
        let (snap, _) = capture(&registry, u32::MAX, 1_000_000, LENGTH, options(7), chunk);
        assert_eq!(fingerprint(&snap, LENGTH), expected, "chunk size {chunk}");
    }
}

#[test]
fn real_time_pacing_takes_real_time_and_gives_the_same_samples_as_fast_mode() {
    let registry = registry_fast();
    let (fast, _) = capture(&registry, u32::MAX, 1_000_000, LENGTH, options(9), 16_384);
    let started = Instant::now();
    let paced = VirtualOptions {
        pacing: Pacing::Realtime,
        ..options(9)
    };
    let (slow, _) = capture(&registry, u32::MAX, 1_000_000, LENGTH, paced, 16_384);
    let elapsed = started.elapsed();
    // 20,000 samples at 1 MHz take 20 ms of real time.
    assert!(
        elapsed >= std::time::Duration::from_millis(15),
        "{elapsed:?}"
    );
    assert!(elapsed < std::time::Duration::from_secs(5), "{elapsed:?}");
    assert_eq!(fingerprint(&slow, LENGTH), fingerprint(&fast, LENGTH));
}

#[test]
fn extreme_seeds_and_channel_selections_work() {
    let registry = registry_fast();
    for seed in [0_u64, u64::MAX] {
        let (snap, _) = capture(
            &registry,
            u32::MAX,
            1_000_000,
            LENGTH,
            options(seed),
            16_384,
        );
        assert!(
            snap.transitions(0, 0, LENGTH).unwrap().count() > 0,
            "seed {seed}"
        );
    }
    // One channel and all 32.
    for mask in [1_u32, 1 << 31, u32::MAX] {
        let (snap, _) = capture(&registry, mask, 1_000_000, LENGTH, options(3), 16_384);
        assert_eq!(snap.channels(), 32);
        assert_eq!(snap.length(), LENGTH);
    }
}

#[test]
fn disabled_channels_stay_zero_and_enabled_ones_carry_signals() {
    let registry = registry_fast();
    let mask = 0b0101;
    let (snap, _) = capture(&registry, mask, 1_000_000, LENGTH, options(5), 16_384);
    for channel in 0..32_u8 {
        let edges = snap.transitions(channel, 0, LENGTH).unwrap().count();
        if mask >> channel & 1 == 1 {
            assert!(
                edges > 100,
                "channel {channel} should carry a signal ({edges} edges)"
            );
        } else {
            assert_eq!(edges, 0, "disabled channel {channel} must stay flat");
            for p in (0..LENGTH).step_by(997) {
                assert_eq!(snap.level_at(channel, p).unwrap(), LevelState::Low);
            }
        }
    }
}

// ---- labelling ---------------------------------------------------------------------------

#[test]
fn every_virtual_capture_is_labelled_simulated_with_its_device_and_seed() {
    let registry = registry_fast();
    for seed in [0_u64, 17, u64::MAX] {
        let (_, acq) = capture(&registry, 0xFF, 1_000_000, 1_000, options(seed), 16_384);
        let d = acq.description();
        assert!(d.simulated());
        assert_eq!(*d.device_id(), virtual_id());
        assert_eq!(d.seed(), Some(seed));
        assert_eq!(d.device_name(), "Virtual device");
    }
}

#[test]
fn captures_from_other_devices_are_never_labelled_simulated() {
    let registry = registry_fast();
    let provider = TestProvider::new("usb");
    registry.add_provider(Arc::new(provider.clone()));
    let device = ScriptedDevice::new(
        descriptor("usb", "real", 8),
        vec![Step::Samples(vec![0, 1, 0, 1]), Step::Finish],
    );
    let acq = start_scripted(&registry, &provider, device, AcquisitionOptions::default());
    wait_outcome(&acq);
    let d = acq.description();
    assert!(!d.simulated(), "only the virtual device may carry the mark");
    assert_eq!(d.seed(), None);
    assert_eq!(d.device_id().as_str(), "usb:real");
    let _ = std_config(1, 1_000);
}

// ---- the signal-source hook ----------------------------------------------------------------

/// A source that produces a counter pattern: the low four bits count up with the position.
struct CounterSource {
    mask: u32,
}

impl SignalSource for CounterSource {
    fn fill(&mut self, start: u64, out: &mut [u32]) {
        for (i, word) in out.iter_mut().enumerate() {
            *word = ((start + i as u64) as u32 & 0xF) & self.mask;
        }
    }
}

struct CounterFactory {
    seen: Arc<Mutex<Option<(u32, u64, u64)>>>,
}

impl SignalSourceFactory for CounterFactory {
    fn create(
        &self,
        enabled: la_devices::ChannelSet,
        sample_rate_hz: u64,
        seed: u64,
    ) -> Box<dyn SignalSource> {
        let enabled_mask = enabled.to_u32();
        *self.seen.lock().unwrap() = Some((enabled_mask, sample_rate_hz, seed));
        Box::new(CounterSource { mask: enabled_mask })
    }
}

#[test]
fn a_custom_signal_source_replaces_the_random_generator_for_any_chunking() {
    let registry = registry_fast();
    let length = 5_000_u64;
    for chunk in [1_usize, 100, 65_536] {
        let seen = Arc::new(Mutex::new(None));
        let opts = VirtualOptions {
            seed: 77,
            source: Some(Arc::new(CounterFactory {
                seen: Arc::clone(&seen),
            })),
            ..VirtualOptions::default()
        };
        let (snap, _) = capture(&registry, 0b1111, 2_000_000, length, opts, chunk);
        // The hook was created with the acquisition's mask, rate and seed.
        assert_eq!(*seen.lock().unwrap(), Some((0b1111, 2_000_000, 77)));
        // Positions 0..length carry exactly the counter pattern.
        for p in (0..length).step_by(13) {
            for channel in 0..4_u8 {
                let want = if (p as u32 & 0xF) >> channel & 1 == 1 {
                    LevelState::High
                } else {
                    LevelState::Low
                };
                assert_eq!(
                    snap.level_at(channel, p).unwrap(),
                    want,
                    "chunk {chunk} ch {channel} pos {p}"
                );
            }
        }
        // Channel 0 alternates at every sample, so it has length - 1 transitions.
        assert_eq!(
            snap.transitions(0, 0, length).unwrap().count() as u64,
            length - 1
        );
    }
}

#[test]
fn the_signal_source_is_not_used_when_none_is_configured() {
    let registry = registry_fast();
    let (a, _) = capture(&registry, 0xF, 1_000_000, 2_000, options(1), 16_384);
    // The random generator does not produce a strict counter pattern.
    let edges = a.transitions(0, 0, 2_000).unwrap().count();
    assert!(
        edges < 1_999,
        "random signals are not a counter ({edges} edges)"
    );
}

#[test]
fn the_virtual_options_are_validated() {
    for gap in [0_u32, 1_000_001, u32::MAX] {
        let error = VirtualOptions {
            mean_gap: gap,
            ..VirtualOptions::default()
        }
        .validate()
        .unwrap_err();
        assert_eq!(
            error.kind(),
            la_core::ErrorKind::InvalidInput,
            "mean_gap {gap}"
        );
        assert!(error.detail().contains("mean_gap"));
    }
    for gap in [1_u32, 50, 1_000_000] {
        assert!(
            VirtualOptions {
                mean_gap: gap,
                ..VirtualOptions::default()
            }
            .validate()
            .is_ok()
        );
    }
}
