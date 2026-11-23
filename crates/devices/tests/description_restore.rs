//! Restoring a capture description from a saved session (spec of item 003, data model).
//!
//! `restore` is the loading path: it accepts exactly what a real acquisition can produce and
//! nothing else. The guarantees of item 004 stay: no setter, no struct literal.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::panic
)]

mod common;

use std::time::{Duration, SystemTime};

use common::{virtual_config, wait_outcome};
use la_devices::{
    AcquisitionOptions, CaptureDescription, DeviceId, DeviceRegistry, RegistryConfig,
    SavedDescription, Trigger, TriggerKind, error,
};

fn saved() -> SavedDescription {
    SavedDescription {
        device_id: "usb:logic16-0042".to_owned(),
        device_name: "Logic16 clone".to_owned(),
        simulated: false,
        seed: None,
        requested_sample_rate_hz: 24_000_000,
        sample_rate_hz: 24_000_000,
        channels: 16,
        enabled_mask: la_devices::ChannelSet::from_u32(0x00FF),
        trigger: Some(Trigger {
            channel: 3,
            kind: TriggerKind::RisingEdge,
        }),
        samples_before_trigger: 7,
        pattern: None,
        pre_trigger_samples: 0,
        started_at: SystemTime::UNIX_EPOCH + Duration::new(1_700_000_000, 123_456_789),
    }
}

fn fails(mutate: impl FnOnce(&mut SavedDescription)) -> &'static str {
    let mut s = saved();
    mutate(&mut s);
    CaptureDescription::restore(s).unwrap_err().code()
}

#[test]
fn restore_gives_a_description_equal_in_every_accessor() {
    let d = CaptureDescription::restore(saved()).unwrap();
    assert_eq!(d.device_id(), &DeviceId::new("usb", "logic16-0042"));
    assert_eq!(d.device_name(), "Logic16 clone");
    assert!(!d.simulated());
    assert_eq!(d.seed(), None);
    assert_eq!(d.requested_sample_rate_hz(), 24_000_000);
    assert_eq!(d.sample_rate_hz(), 24_000_000);
    assert_eq!(d.channels(), 16);
    assert_eq!(d.enabled_mask(), la_devices::ChannelSet::from_u32(0x00FF));
    assert_eq!(d.trigger().unwrap().channel, 3);
    assert_eq!(d.samples_before_trigger(), 7);
    assert_eq!(
        d.started_at(),
        SystemTime::UNIX_EPOCH + Duration::new(1_700_000_000, 123_456_789)
    );
    // And back.
    assert_eq!(CaptureDescription::restore(d.to_saved()).unwrap(), d);
}

#[test]
fn a_real_acquisition_round_trips_through_the_saved_form() {
    let registry = DeviceRegistry::new(RegistryConfig::default());
    let acquisition = registry
        .start(
            &DeviceId::new("virtual", "0"),
            virtual_config(0b1011, 1_000_000, Some(2_000), 77),
            AcquisitionOptions::default(),
        )
        .unwrap();
    wait_outcome(&acquisition);
    let original = acquisition.description();
    assert!(original.simulated());
    assert_eq!(original.seed(), Some(77));
    let restored = CaptureDescription::restore(original.to_saved()).unwrap();
    assert_eq!(
        restored, original,
        "every field survives, including the simulated mark"
    );
}

#[test]
fn simulated_seed_and_times_around_the_epoch_are_kept() {
    for (simulated, seed) in [
        (true, Some(0)),
        (true, Some(u64::MAX)),
        (true, None),
        (false, None),
    ] {
        let mut s = saved();
        s.simulated = simulated;
        s.seed = seed;
        let d = CaptureDescription::restore(s).unwrap();
        assert_eq!((d.simulated(), d.seed()), (simulated, seed));
    }
    for time in [
        SystemTime::UNIX_EPOCH,
        SystemTime::UNIX_EPOCH - Duration::new(86_400 * 365, 5),
        SystemTime::UNIX_EPOCH + Duration::new(4_000_000_000, 999_999_999),
    ] {
        let mut s = saved();
        s.started_at = time;
        assert_eq!(CaptureDescription::restore(s).unwrap().started_at(), time);
    }
}

#[test]
fn restore_rejects_everything_a_real_acquisition_cannot_produce() {
    use error::{INVALID_CAPABILITIES, INVALID_CHANNEL};
    assert_eq!(fails(|s| s.channels = 0), INVALID_CAPABILITIES);
    assert_eq!(fails(|s| s.channels = 161), INVALID_CAPABILITIES);
    assert_eq!(
        fails(|s| s.enabled_mask = la_devices::ChannelSet::from_u32(0)),
        INVALID_CHANNEL
    );
    assert_eq!(
        fails(|s| s.enabled_mask = la_devices::ChannelSet::from_u32(1 << 16)),
        INVALID_CHANNEL
    );
    assert_eq!(
        fails(|s| s.enabled_mask = la_devices::ChannelSet::from_u32(u32::MAX)),
        INVALID_CHANNEL
    );
    assert_eq!(
        fails(|s| s.requested_sample_rate_hz = 0),
        INVALID_CAPABILITIES
    );
    assert_eq!(fails(|s| s.sample_rate_hz = 0), INVALID_CAPABILITIES);
    // A trigger on a disabled channel, or beyond the channel count.
    assert_eq!(
        fails(|s| s.trigger = Some(Trigger {
            channel: 12,
            kind: TriggerKind::AnyEdge
        })),
        INVALID_CHANNEL
    );
    assert_eq!(
        fails(|s| s.trigger = Some(Trigger {
            channel: 40,
            kind: TriggerKind::AnyEdge
        })),
        INVALID_CHANNEL
    );
    // Identity and text.
    assert_eq!(fails(|s| s.device_id = String::new()), INVALID_CAPABILITIES);
    assert_eq!(
        fails(|s| s.device_id = "nocolon".to_owned()),
        INVALID_CAPABILITIES
    );
    assert_eq!(
        fails(|s| s.device_id = ":unique".to_owned()),
        INVALID_CAPABILITIES
    );
    assert_eq!(
        fails(|s| s.device_name = String::new()),
        INVALID_CAPABILITIES
    );
    assert_eq!(
        fails(|s| s.device_name = "  ".to_owned()),
        INVALID_CAPABILITIES
    );
    assert_eq!(
        fails(|s| s.device_name = "x".repeat(4_097)),
        INVALID_CAPABILITIES
    );
    assert_eq!(
        fails(|s| s.device_id = format!("usb:{}", "x".repeat(4_100))),
        INVALID_CAPABILITIES
    );
    // The longest legal text is accepted.
    let mut s = saved();
    s.device_name = "ж".repeat(2_048);
    assert_eq!(s.device_name.len(), 4_096);
    assert!(CaptureDescription::restore(s).is_ok());
    // A time that cannot be represented on this platform is a typed error, not a panic.
    let mut s = saved();
    s.started_at = SystemTime::UNIX_EPOCH;
    assert!(CaptureDescription::restore(s).is_ok());
}

#[test]
fn the_guarantees_of_item_004_still_hold() {
    // The compile_fail doctests on `CaptureDescription` (no struct literal, no setter) run with
    // `cargo test --doc`; this test documents that `restore` is the only way to build one from
    // arbitrary values and that it validates.
    assert!(CaptureDescription::restore(saved()).is_ok());
    assert_eq!(fails(|s| s.channels = 0), error::INVALID_CAPABILITIES);
}
