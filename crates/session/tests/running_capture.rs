//! Saving a capture that is still being recorded (spec US6, FR-030, SC-009).
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::panic
)]

mod common;

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{Duration, Instant};

use common::TempDir;
use la_devices::{
    AcquisitionOptions, DeviceConfig, DeviceId, DeviceOptions, DeviceRegistry, EndReason,
    RegistryConfig, VirtualOptions,
};
use la_session::{Control, LoadLimits, SessionData, load_blocking, start_save};

/// The longest the recording may stand still while saves run.
#[cfg(debug_assertions)]
const STALL_LIMIT: Duration = Duration::from_millis(100);
#[cfg(not(debug_assertions))]
const STALL_LIMIT: Duration = Duration::from_millis(10);

fn config(length: Option<u64>) -> DeviceConfig {
    DeviceConfig {
        enabled_channels: la_devices::ChannelSet::from_u32(0xFFFF),
        sample_rate_hz: 10_000_000,
        trigger: None,
        capture_length: length,
        options: DeviceOptions::Virtual(VirtualOptions {
            seed: 0x5EED,
            mean_gap: 40,
            ..VirtualOptions::default()
        }),
        accept_budget_risk: true,
        pattern: None,
        pre_trigger_samples: 0,
        bursts: 0,
    }
}

#[test]
fn saving_while_recording_gives_a_valid_incomplete_session_and_does_not_disturb_the_recording() {
    let registry = DeviceRegistry::new(RegistryConfig::default());
    let id = DeviceId::new("virtual", "0");
    let acquisition = Arc::new(
        registry
            .start(&id, config(None), AcquisitionOptions::default())
            .unwrap(),
    );
    while acquisition.progress().samples_captured < 200_000 {
        std::thread::sleep(Duration::from_millis(1));
    }

    // A watcher records the longest time the recording stood still.
    let stop = Arc::new(AtomicBool::new(false));
    let longest_stall_micros = Arc::new(AtomicU64::new(0));
    let watcher = {
        let (acquisition, stop, longest) = (
            Arc::clone(&acquisition),
            Arc::clone(&stop),
            Arc::clone(&longest_stall_micros),
        );
        std::thread::spawn(move || {
            let mut last = acquisition.progress().positions;
            let mut since = Instant::now();
            while !stop.load(Ordering::Acquire) {
                let now = acquisition.progress().positions;
                if now != last {
                    last = now;
                    since = Instant::now();
                } else {
                    let stalled = u64::try_from(since.elapsed().as_micros()).unwrap();
                    longest.fetch_max(stalled, Ordering::AcqRel);
                }
                std::thread::sleep(Duration::from_micros(300));
            }
        })
    };

    // Save at several moments while it records.
    let dir = TempDir::new("running");
    let mut files = Vec::new();
    for i in 0..6 {
        let session = SessionData::new(acquisition.description(), acquisition.capture().snapshot());
        let path = dir.join(&format!("s{i}"));
        let saved_length = session.snapshot().length();
        assert!(!session.snapshot().status().closed, "still recording");
        let save = start_save(path.clone(), session);
        save.wait_timeout(Duration::from_secs(30)).unwrap().unwrap();
        files.push((path, saved_length));
        std::thread::sleep(Duration::from_millis(20));
    }
    stop.store(true, Ordering::Release);
    watcher.join().unwrap();

    assert!(
        acquisition.outcome().is_none(),
        "the recording went on during the saves"
    );
    acquisition.stop();
    let outcome = acquisition.wait_timeout(Duration::from_secs(30)).unwrap();
    assert_eq!(outcome.end, EndReason::Stopped);
    let final_snapshot = acquisition.capture().snapshot();
    let stalled = Duration::from_micros(longest_stall_micros.load(Ordering::Acquire));
    println!("longest stall of the recording during the saves: {stalled:?}");
    assert!(
        stalled < STALL_LIMIT,
        "the recording stood still for {stalled:?}"
    );

    // Every file is a valid, closed, incomplete session holding exactly a prefix of the final
    // capture.
    for (path, saved_length) in &files {
        let loaded = load_blocking(path, &LoadLimits::default(), &Control::new()).unwrap();
        let snapshot = loaded.snapshot();
        let status = snapshot.status();
        assert!(status.closed && status.interrupted && !status.complete);
        assert_eq!(snapshot.length(), *saved_length);
        assert!(snapshot.length() <= final_snapshot.length());
        assert!(loaded.description().simulated());
        for channel in 0..snapshot.channels() {
            let saved: Vec<u64> = if snapshot.transition_count(channel).unwrap() == 0 {
                vec![]
            } else {
                snapshot
                    .transitions(channel, 0, snapshot.length())
                    .unwrap()
                    .map(|e| e.position)
                    .collect()
            };
            let prefix: Vec<u64> = final_snapshot
                .transitions(channel, 0, final_snapshot.length())
                .unwrap()
                .map(|e| e.position)
                .take_while(|p| *p < snapshot.length())
                .collect();
            assert_eq!(saved, prefix, "channel {channel} of {}", path.display());
        }
    }

    // The recording itself is exactly what an unsaved run with the same seed produces.
    let control = registry
        .start(
            &id,
            config(Some(final_snapshot.length())),
            AcquisitionOptions::default(),
        )
        .unwrap();
    control.wait_timeout(Duration::from_secs(60)).unwrap();
    let control_snapshot = control.capture().snapshot();
    assert_eq!(control_snapshot.length(), final_snapshot.length());
    for channel in 0..16 {
        let a: Vec<_> = final_snapshot
            .transitions(channel, 0, final_snapshot.length())
            .unwrap()
            .collect();
        let b: Vec<_> = control_snapshot
            .transitions(channel, 0, control_snapshot.length())
            .unwrap()
            .collect();
        assert_eq!(a, b, "channel {channel}: saving changed the recording");
    }
}
