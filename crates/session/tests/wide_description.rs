//! Sessions of more than 32 channels (feature 017, task T010): saved and opened identically,
//! and a session of 32 channels or fewer is written as it always was.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::panic
)]

mod common;

use common::{End, TempDir, assert_same_capture, data, record};
use la_devices::ChannelSet;
use la_session::{Control, LoadLimits, load_blocking, save_blocking};
use la_storage::Limits;

#[test]
fn wide_sessions_round_trip_with_their_enabled_channels() {
    for channels in [33_u8, 64, 65, 100, 160] {
        let capture = record(
            channels,
            20,
            &[(None, 3_000), (Some(40), 1_000)],
            End::Closed,
            u64::from(channels),
            Limits::default(),
        );
        let dir = TempDir::new("wide-rt");
        let path = dir.join("w.lasession");
        let original = data(&capture, false, None);
        assert_eq!(
            original.description().enabled_mask(),
            ChannelSet::all(channels)
        );
        save_blocking(&path, &original, &Control::new()).unwrap();
        let loaded = load_blocking(&path, &LoadLimits::default(), &Control::new()).unwrap();
        assert_same_capture(&original.snapshot(), &loaded.snapshot());
        assert_eq!(loaded.description(), original.description());
        assert_eq!(loaded.description().channels(), channels);
        assert!(loaded.snapshot().transition_count(channels - 1).unwrap() > 0);
    }
}

#[test]
fn a_partial_wide_set_of_enabled_channels_survives() {
    let capture = record(100, 30, &[(None, 2_000)], End::Closed, 5, Limits::default());
    let mut set = ChannelSet::empty();
    for c in [0_u8, 31, 32, 63, 64, 99] {
        set.insert(c);
    }
    let base = data(&capture, false, None);
    let description = la_devices::CaptureDescription::restore(la_devices::SavedDescription {
        enabled_mask: set,
        ..saved_of(base.description())
    })
    .unwrap();
    let session = la_session::SessionData::new(description, capture.reader().snapshot());
    let dir = TempDir::new("wide-partial");
    let path = dir.join("p.lasession");
    save_blocking(&path, &session, &Control::new()).unwrap();
    let loaded = load_blocking(&path, &LoadLimits::default(), &Control::new()).unwrap();
    assert_eq!(loaded.description().enabled_mask(), set);
}

fn saved_of(d: &la_devices::CaptureDescription) -> la_devices::SavedDescription {
    la_devices::SavedDescription {
        device_id: d.device_id().as_str().to_owned(),
        device_name: d.device_name().to_owned(),
        simulated: d.simulated(),
        seed: d.seed(),
        requested_sample_rate_hz: d.requested_sample_rate_hz(),
        sample_rate_hz: d.sample_rate_hz(),
        channels: d.channels(),
        enabled_mask: d.enabled_mask(),
        trigger: d.trigger(),
        samples_before_trigger: d.samples_before_trigger(),
        started_at: d.started_at(),
    }
}
