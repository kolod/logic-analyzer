//! "Simulated" and "incomplete" survive the file (spec US4, FR-020..022, SC-010).
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::panic
)]

mod common;

use std::time::Duration;

use common::{
    End, TempDir, assert_same_capture, build_file, data, load_bytes, load_error_code, record,
    repair_checksums, split_sections,
};
use la_devices::{
    AcquisitionOptions, DeviceConfig, DeviceId, DeviceOptions, DeviceRegistry, EndReason,
    FailureKind, RegistryConfig, ScriptedFailure, VirtualOptions,
};
use la_session::error::DAMAGED;
use la_session::{Control, LoadLimits, Session, SessionData, load_blocking, save_blocking};
use la_storage::Limits;

fn registry() -> DeviceRegistry {
    DeviceRegistry::new(RegistryConfig::default())
}

fn virtual_run(
    registry: &DeviceRegistry,
    length: Option<u64>,
    seed: u64,
    failures: Vec<ScriptedFailure>,
    limits: Limits,
    mean_gap: u32,
) -> la_devices::Acquisition {
    registry
        .start(
            &DeviceId::new("virtual", "0"),
            DeviceConfig {
                enabled_channels: la_devices::ChannelSet::from_u32(0xFFFF),
                sample_rate_hz: 1_000_000,
                trigger: None,
                capture_length: length,
                options: DeviceOptions::Virtual(VirtualOptions {
                    seed,
                    failures,
                    mean_gap,
                    ..VirtualOptions::default()
                }),
                accept_budget_risk: true,
            },
            AcquisitionOptions {
                limits,
                chunk_samples: 4096,
                ..AcquisitionOptions::default()
            },
        )
        .unwrap()
}

fn finish(acquisition: &la_devices::Acquisition) -> SessionData {
    let outcome = acquisition.wait_timeout(Duration::from_secs(20)).unwrap();
    assert_ne!(outcome.end, EndReason::Stopped);
    SessionData::new(acquisition.description(), acquisition.capture().snapshot())
}

fn cycle(dir: &TempDir, original: &SessionData, times: usize) -> Session {
    let mut current = SessionData::from_session(&round(dir, original, "0"));
    let mut last = None;
    for i in 1..=times {
        let loaded = round(dir, &current, &i.to_string());
        current = SessionData::from_session(&loaded);
        last = Some(loaded);
    }
    last.unwrap()
}

fn round(dir: &TempDir, data: &SessionData, name: &str) -> Session {
    let path = dir.join(name);
    save_blocking(&path, data, &Control::new()).unwrap();
    load_blocking(&path, &LoadLimits::default(), &Control::new()).unwrap()
}

#[test]
fn a_virtual_capture_stays_simulated_with_its_seed_through_repeated_cycles() {
    let registry = registry();
    let acquisition = virtual_run(
        &registry,
        Some(20_000),
        0xABCD_EF01,
        vec![],
        Limits::default(),
        50,
    );
    let original = finish(&acquisition);
    assert!(original.description().simulated());
    let dir = TempDir::new("sim");
    let loaded = cycle(&dir, &original, 5);
    assert!(loaded.description().simulated());
    assert_eq!(loaded.description().seed(), Some(0xABCD_EF01));
    assert_eq!(loaded.description(), original.description());
    assert!(loaded.snapshot().status().complete);
    assert_same_capture(&original.snapshot(), &loaded.snapshot());
}

#[test]
fn a_hardware_capture_is_not_simulated() {
    let capture = record(4, 50, &[(None, 3_000)], End::Closed, 1, Limits::default());
    let original = data(&capture, false, None);
    let dir = TempDir::new("hw");
    let loaded = cycle(&dir, &original, 3);
    assert!(!loaded.description().simulated());
    assert_eq!(loaded.description().seed(), None);
}

#[test]
fn incomplete_captures_stay_incomplete_with_their_gaps_and_reason() {
    let registry = registry();
    let dir = TempDir::new("incomplete");

    // Lost samples: gaps and lost positions.
    let acquisition = virtual_run(
        &registry,
        Some(5_000),
        1,
        vec![
            ScriptedFailure {
                position: 1_000,
                kind: FailureKind::Loss(70),
            },
            ScriptedFailure {
                position: 3_000,
                kind: FailureKind::Overrun(5),
            },
        ],
        Limits::default(),
        50,
    );
    let original = finish(&acquisition);
    let status = original.snapshot().status();
    assert!(status.closed && !status.complete);
    assert_eq!(status.lost_positions, 75);
    let loaded = cycle(&dir, &original, 2);
    let after = loaded.snapshot().status();
    assert!(after.closed && !after.complete);
    assert_eq!(after.lost_positions, 75);
    assert_eq!(loaded.snapshot().gaps(), original.snapshot().gaps());
    assert_same_capture(&original.snapshot(), &loaded.snapshot());

    // A device that disconnects: interrupted.
    let acquisition = virtual_run(
        &registry,
        None,
        2,
        vec![ScriptedFailure {
            position: 2_000,
            kind: FailureKind::Disconnect,
        }],
        Limits::default(),
        50,
    );
    let original = finish(&acquisition);
    assert!(original.snapshot().status().interrupted);
    let loaded = cycle(&dir, &original, 2);
    let status = loaded.snapshot().status();
    assert!(status.interrupted && status.closed && !status.complete);
    assert_eq!(
        status.lost_positions, 0,
        "interrupted without a lost position"
    );

    // The memory limit ended it: stopped by limit.
    let tiny = Limits {
        memory_budget_bytes: 1024 * 1024,
        ..Limits::default()
    };
    let acquisition = virtual_run(&registry, None, 3, vec![], tiny, 1);
    let original = finish(&acquisition);
    assert!(original.snapshot().status().stopped_by_limit);
    let loaded = cycle(&dir, &original, 2);
    let status = loaded.snapshot().status();
    assert!(status.stopped_by_limit && !status.complete);
    assert_same_capture(&original.snapshot(), &loaded.snapshot());
}

#[test]
fn a_capture_saved_while_recording_is_saved_as_interrupted_and_the_recording_goes_on() {
    let registry = registry();
    let acquisition = virtual_run(&registry, None, 4, vec![], Limits::default(), 50);
    while acquisition.progress().samples_captured < 50_000 {
        std::thread::sleep(Duration::from_millis(1));
    }
    let running = SessionData::new(acquisition.description(), acquisition.capture().snapshot());
    assert!(!running.snapshot().status().closed, "still recording");
    let dir = TempDir::new("running");
    let loaded = round(&dir, &running, "r");
    let status = loaded.snapshot().status();
    assert!(status.closed && status.interrupted && !status.complete);
    assert_eq!(loaded.snapshot().length(), running.snapshot().length());
    assert!(loaded.description().simulated());
    assert!(
        acquisition.outcome().is_none(),
        "the recording was not disturbed"
    );
    acquisition.stop();
    acquisition.wait_timeout(Duration::from_secs(20)).unwrap();
}

/// The offsets of the `META` and `CAPT` payloads in a saved file.
fn payload_offsets(bytes: &[u8]) -> (usize, usize) {
    let meta_len = u64::from_le_bytes(bytes[32 + 8..32 + 16].try_into().unwrap()) as usize;
    let meta = 32 + 16;
    let capt = meta + meta_len + 4 + 16;
    (meta, capt)
}

#[test]
fn an_edit_of_a_mark_inside_the_file_is_detected() {
    let registry = registry();
    let acquisition = virtual_run(
        &registry,
        Some(3_000),
        0x0102_0304_0506_0708,
        vec![ScriptedFailure {
            position: 1_000,
            kind: FailureKind::Loss(10),
        }],
        Limits::default(),
        50,
    );
    let original = finish(&acquisition);
    let dir = TempDir::new("edit");
    let path = dir.join("s");
    save_blocking(&path, &original, &Control::new()).unwrap();
    let bytes = std::fs::read(&path).unwrap();
    let (meta, capt) = payload_offsets(&bytes);
    assert_eq!(
        bytes[meta] & 0b011,
        0b011,
        "simulated and seed bits are set"
    );

    // The mark bytes: description flags and seed, and the capture's flag byte.
    let mark_bytes: Vec<usize> = (meta..meta + 9).chain([capt + 1]).collect();
    for &offset in &mark_bytes {
        for bit in 0..8 {
            let mut edited = bytes.clone();
            edited[offset] ^= 1 << bit;
            assert_eq!(
                load_error_code(&edited),
                DAMAGED,
                "bit {bit} of byte {offset} was edited without the checksums being fixed"
            );
        }
    }

    // With the checksums recomputed the file says what it now says, and the loader reports
    // exactly that — nothing is silently kept or cleared.
    let mut cleared = bytes.clone();
    cleared[meta] &= !0b001;
    repair_checksums(&mut cleared);
    let loaded = load_bytes(&cleared, &LoadLimits::default()).unwrap();
    assert!(
        !loaded.description().simulated(),
        "the edit is what the file says now"
    );
    let mut intact = bytes;
    repair_checksums(&mut intact);
    let loaded = load_bytes(&intact, &LoadLimits::default()).unwrap();
    assert!(
        loaded.description().simulated(),
        "repairing an unedited file changes nothing"
    );
}

#[test]
fn a_loaded_description_cannot_be_changed_or_built_with_arbitrary_marks() {
    // The guarantee is structural: `Session::description` returns a shared reference and
    // `CaptureDescription` has no setters (compile-fail doctests of la-devices and la-session).
    let capture = record(2, 50, &[(None, 500)], End::Closed, 1, Limits::default());
    let dir = TempDir::new("ro");
    let loaded = round(&dir, &data(&capture, true, Some(5)), "x");
    assert!(loaded.description().simulated());
    // A crafted file whose description says "not simulated" while claiming the virtual device
    // is still a file with a different, checksummed content: it loads as written.
    let mut sections = split_sections(&std::fs::read(dir.join("x")).unwrap());
    sections[0].payload[0] &= !1;
    let loaded = load_bytes(&build_file(&sections), &LoadLimits::default()).unwrap();
    assert!(!loaded.description().simulated());
}
