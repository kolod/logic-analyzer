//! The virtual device with 33 to 160 channels (feature 017, task T009).
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

use common::{registry_fast, wait_outcome, wait_until};
use la_devices::{
    AcquisitionOptions, ChannelSet, DeviceConfig, DeviceId, DeviceOptions, DeviceRegistry,
    EndReason, SignalSource, SignalSourceFactory, Trigger, TriggerKind, VirtualOptions,
    WideVirtualProvider, error, validate, virtual_descriptor_with_channels,
};
use la_storage::{LevelState, Limits, Snapshot, words_per_sample};

fn id(channels: u8) -> DeviceId {
    virtual_descriptor_with_channels(channels).id
}

fn registry(channels: u8) -> DeviceRegistry {
    let registry = registry_fast();
    if channels != 32 {
        registry.add_provider(Arc::new(WideVirtualProvider::new(channels)));
    }
    assert!(wait_until(Duration::from_secs(5), || registry
        .describe(&id(channels))
        .is_some()));
    registry
}

fn config(enabled: ChannelSet, length: u64, options: VirtualOptions) -> DeviceConfig {
    DeviceConfig {
        enabled_channels: enabled,
        sample_rate_hz: 1_000_000,
        trigger: None,
        capture_length: Some(length),
        options: DeviceOptions::Virtual(options),
        accept_budget_risk: false,
        pattern: None,
        pre_trigger_samples: 0,
        bursts: 0,
    }
}

fn run(registry: &DeviceRegistry, channels: u8, config: DeviceConfig, chunk: usize) -> Snapshot {
    let acq = registry
        .start(
            &id(channels),
            config,
            AcquisitionOptions {
                chunk_samples: chunk,
                ..AcquisitionOptions::default()
            },
        )
        .unwrap();
    let outcome = wait_outcome(&acq);
    assert_eq!(outcome.end, EndReason::Completed, "{:?}", outcome.error);
    assert_eq!(outcome.lost_positions, 0);
    acq.capture().snapshot()
}

/// A source that drives every channel from a formula written here, by hand.
fn level(channel: u8, sample: u64) -> bool {
    let half = 3 + u64::from(channel) % 5;
    (sample / half + u64::from(channel)) % 2 == 1
}

struct Wave {
    stride: usize,
}

impl SignalSource for Wave {
    fn fill(&mut self, _start: u64, _out: &mut [u32]) {
        panic!("a wide capture must use fill_wide");
    }

    fn fill_wide(&mut self, start: u64, out: &mut [u32], stride: usize) {
        assert_eq!(stride, self.stride);
        for (i, sample) in out.chunks_exact_mut(stride).enumerate() {
            for (k, word) in sample.iter_mut().enumerate() {
                *word = 0;
                for bit in 0..32 {
                    let channel = (k * 32 + bit) as u8;
                    if k * 32 + bit < 160 && level(channel, start + i as u64) {
                        *word |= 1 << bit;
                    }
                }
            }
        }
    }
}

struct WaveFactory {
    channels: u8,
}

impl SignalSourceFactory for WaveFactory {
    fn create(&self, _: ChannelSet, _: u64, _: u64) -> Box<dyn SignalSource> {
        Box::new(Wave {
            stride: words_per_sample(self.channels),
        })
    }
}

#[test]
fn a_wide_capture_equals_the_signal_channel_by_channel() {
    for channels in [33_u8, 64, 100, 160] {
        let registry = registry(channels);
        let options = VirtualOptions {
            source: Some(Arc::new(WaveFactory { channels })),
            ..VirtualOptions::default()
        };
        for chunk in [1, 7, 1000] {
            let snap = run(
                &registry,
                channels,
                config(ChannelSet::all(channels), 600, options.clone()),
                chunk,
            );
            assert_eq!(snap.channels(), channels);
            assert_eq!(snap.length(), 600);
            for c in 0..channels {
                let flips = (1..600).filter(|&p| level(c, p) != level(c, p - 1)).count();
                assert_eq!(snap.transition_count(c).unwrap(), flips as u64, "ch {c}");
                for p in [0_u64, 1, 299, 599] {
                    let want = if level(c, p) {
                        LevelState::High
                    } else {
                        LevelState::Low
                    };
                    assert_eq!(snap.level_at(c, p).unwrap(), want, "ch {c} at {p}");
                }
            }
        }
    }
}

#[test]
fn random_signals_of_a_channel_do_not_depend_on_the_width() {
    let narrow = run(
        &registry(32),
        32,
        config(ChannelSet::all(32), 3000, VirtualOptions::default()),
        512,
    );
    let wide = run(
        &registry(160),
        160,
        config(ChannelSet::all(160), 3000, VirtualOptions::default()),
        512,
    );
    for c in [0_u8, 5, 31] {
        let a: Vec<_> = narrow.transitions(c, 0, 3000).unwrap().collect();
        let b: Vec<_> = wide.transitions(c, 0, 3000).unwrap().collect();
        assert_eq!(a, b, "channel {c}");
    }
    assert!(wide.transition_count(150).unwrap() > 0);
}

#[test]
fn a_trigger_on_a_channel_above_31_finds_its_edge() {
    let registry = registry(160);
    let mut c = config(ChannelSet::all(160), 100, VirtualOptions::default());
    c.trigger = Some(Trigger {
        channel: 140,
        kind: TriggerKind::RisingEdge,
    });
    let snap = run(&registry, 160, c, 64);
    // The capture starts at the trigger sample: channel 140 has just risen.
    assert_eq!(snap.level_at(140, 0).unwrap(), LevelState::High);
    assert_eq!(snap.length(), 100);
}

#[test]
fn validation_refuses_a_channel_the_device_does_not_have_and_too_many_channels() {
    let caps = virtual_descriptor_with_channels(100).capabilities;
    let mut set = ChannelSet::all(100);
    set.insert(100);
    let bad = config(set, 10, VirtualOptions::default());
    let err = validate(&caps, &bad, &Limits::default()).unwrap_err();
    assert_eq!(err.code(), error::INVALID_CHANNEL);
    let mut wide = virtual_descriptor_with_channels(255).capabilities;
    assert_eq!(wide.channels, 160, "the virtual device is clamped to 160");
    wide.channels = 161;
    let descriptor = la_devices::DeviceDescriptor {
        id: id(160),
        name: "x".to_owned(),
        simulated: true,
        capabilities: wide,
    };
    assert_eq!(
        descriptor.validate().unwrap_err().code(),
        error::INVALID_CAPABILITIES
    );
}
