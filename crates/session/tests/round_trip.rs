//! Saving a session and loading it back (spec US1, FR-001..006, SC-001, SC-002).
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::panic
)]

mod common;

use std::path::PathBuf;

use common::{End, Rng, TempDir, assert_same_capture, data, record};
use la_session::{Control, LoadLimits, SessionData, load_blocking, save_blocking};
use la_storage::Limits;

fn save(path: &std::path::Path, data: &SessionData) -> u64 {
    save_blocking(path, data, &Control::new())
        .unwrap()
        .bytes_written
}

fn load(path: &std::path::Path) -> la_session::Session {
    load_blocking(path, &LoadLimits::default(), &Control::new()).unwrap()
}

fn round_trip(capture: &la_storage::Capture, simulated: bool, seed: Option<u64>) {
    let dir = TempDir::new("rt");
    let path = dir.join("s.lasession");
    let original = data(capture, simulated, seed);
    let written = save(&path, &original);
    assert_eq!(std::fs::metadata(&path).unwrap().len(), written);
    let loaded = load(&path);
    assert_same_capture(&original.snapshot(), &loaded.snapshot());
    assert_eq!(loaded.description(), original.description());
    assert!(loaded.snapshot().status().closed);
    assert!(loaded.extras().is_empty());
}

#[test]
fn a_thousand_random_captures_round_trip_exactly() {
    let mut rng = Rng::new(2026);
    for case in 0..1_000_u64 {
        let channels = u8::try_from(1 + rng.below(32)).unwrap();
        let per_mille = [1, 5, 20, 80, 300][usize::try_from(rng.below(5)).unwrap()];
        let len = usize::try_from(rng.below(if case % 50 == 0 { 120_000 } else { 3_000 })).unwrap();
        let mut parts = vec![(None, len)];
        if rng.below(3) == 0 {
            parts.push((
                Some(1 + rng.below(500)),
                usize::try_from(rng.below(2_000)).unwrap(),
            ));
        }
        if rng.below(5) == 0 {
            parts.insert(0, (Some(1 + rng.below(50)), 0));
        }
        let end = match rng.below(4) {
            0 => End::Interrupted,
            1 => End::Open,
            _ => End::Closed,
        };
        let capture = record(channels, per_mille, &parts, end, case, Limits::default());
        round_trip(
            &capture,
            rng.below(2) == 0,
            (rng.below(2) == 0).then(|| rng.next_u64()),
        );
    }
}

#[test]
fn the_edges_of_the_supported_range_round_trip() {
    // No samples, one sample, the maximum number of channels, a capture that starts with a gap
    // and a capture that is one big gap.
    for parts in [
        vec![(None, 0)],
        vec![(None, 1)],
        vec![(None, 40_000)],
        vec![(Some(77), 5_000)],
        vec![(Some(10_000), 0)],
    ] {
        for channels in [1_u8, 7, 32] {
            let capture = record(channels, 60, &parts, End::Closed, 9, Limits::default());
            round_trip(&capture, false, None);
        }
    }
    // A stopped-by-limit capture keeps its flag.
    let tiny = Limits {
        memory_budget_bytes: 1024 * 1024,
        ..Limits::default()
    };
    let mut capture = record(4, 500, &[], End::Open, 1, tiny);
    let mut rng = Rng::new(3);
    loop {
        let words = common::random_words(30_000, 4, 500, &mut rng);
        if capture.append(capture.length(), &words).is_err() {
            break;
        }
    }
    capture.close().unwrap();
    assert!(capture.reader().snapshot().status().stopped_by_limit);
    round_trip(&capture, false, None);
}

#[test]
fn the_description_survives_in_every_field() {
    let capture = record(8, 40, &[(None, 5_000)], End::Closed, 4, Limits::default());
    let dir = TempDir::new("desc");
    let path = dir.join("d.lasession");
    let original = data(&capture, true, Some(u64::MAX));
    save(&path, &original);
    let d = load(&path);
    let d = d.description();
    assert_eq!(d.device_name(), "Virtual device");
    assert!(d.simulated());
    assert_eq!(d.seed(), Some(u64::MAX));
    assert_eq!(d.sample_rate_hz(), 12_500_000);
    assert_eq!(d.enabled_mask(), 0xFF);
    assert_eq!(d.samples_before_trigger(), 3);
    assert_eq!(d.trigger().unwrap().channel, 0);
    assert_eq!(*d, *original.description());
    // A hardware device with a name in another script.
    let hw = data(&capture, false, None);
    let path = dir.join("hw.lasession");
    save(&path, &hw);
    let loaded = load(&path);
    assert_eq!(loaded.description().device_name(), "Logic16 Клон");
    assert!(!loaded.description().simulated());
}

#[test]
fn saving_the_same_session_many_times_gives_identical_bytes() {
    let capture = record(
        16,
        30,
        &[(None, 30_000), (Some(40), 10_000)],
        End::Interrupted,
        8,
        Limits::default(),
    );
    let original = data(&capture, true, Some(5));
    let dir = TempDir::new("det");
    let first_path = dir.join("0");
    save(&first_path, &original);
    let first = std::fs::read(&first_path).unwrap();
    for i in 1..100 {
        let path = dir.join(&i.to_string());
        save(&path, &original);
        assert_eq!(std::fs::read(&path).unwrap(), first, "save {i}");
    }
    // A loaded and re-saved session is byte-identical too.
    let again = dir.join("again");
    let loaded = load(&first_path);
    save(&again, &SessionData::from_session(&loaded));
    assert_eq!(std::fs::read(&again).unwrap(), first);
}

fn reference_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/reference")
}

/// The reference sessions: built from fixed seeds with integer-only generators, so the bytes are
/// the same on every platform.
fn reference_sessions() -> Vec<(&'static str, SessionData)> {
    let plain = record(4, 25, &[(None, 20_000)], End::Closed, 11, Limits::default());
    let rich = record(
        32,
        15,
        &[(None, 12_000), (Some(300), 9_000), (Some(1), 500)],
        End::Interrupted,
        12,
        Limits::default(),
    );
    vec![
        ("plain.lasession", data(&plain, false, None)),
        ("rich.lasession", data(&rich, true, Some(0xFEED_BEEF))),
    ]
}

#[test]
fn the_committed_reference_files_are_byte_identical_to_what_the_writer_produces() {
    let dir = TempDir::new("ref");
    for (name, session) in reference_sessions() {
        let produced_path = dir.join(name);
        save(&produced_path, &session);
        let produced = std::fs::read(&produced_path).unwrap();
        let committed = std::fs::read(reference_dir().join(name)).unwrap_or_else(|_| {
            panic!("the reference file {name} is missing: run `cargo test -p la-session --test round_trip -- --ignored regenerate`")
        });
        assert_eq!(
            produced, committed,
            "{name}: the file format changed; a format change needs a new version and a new fixture directory"
        );
        // And the committed file loads to the same content.
        let loaded = load(&reference_dir().join(name));
        assert_same_capture(&session.snapshot(), &loaded.snapshot());
    }
}

#[test]
#[ignore = "writes the committed reference files; run only to create them"]
fn regenerate_reference_files() {
    std::fs::create_dir_all(reference_dir()).unwrap();
    for (name, session) in reference_sessions() {
        save(&reference_dir().join(name), &session);
    }
}

#[test]
fn the_file_grows_with_level_changes_not_with_samples_or_idle_channels() {
    let dir = TempDir::new("size");
    let size = |channels: u8, per_mille: u64, len: usize| {
        let capture = record(
            channels,
            per_mille,
            &[(None, len)],
            End::Closed,
            6,
            Limits::default(),
        );
        let session = data(&capture, false, None);
        let path = dir.join("s");
        let written = save(&path, &session);
        let transitions: u64 = (0..channels)
            .map(|c| session.snapshot().transition_count(c).unwrap())
            .sum();
        (written, transitions)
    };
    // A million samples of a quiet signal: the file is small.
    let (quiet, quiet_transitions) = size(2, 0, 60_000);
    assert_eq!(quiet_transitions, 0);
    assert!(quiet < 400, "an idle capture takes {quiet} bytes");
    // The size follows the transitions: about 1 to 2 bytes each, never 4 or 8.
    let (busy, transitions) = size(8, 100, 50_000);
    assert!(transitions > 10_000);
    let per_transition = busy as f64 / transitions as f64;
    assert!(
        (1.0..2.5).contains(&per_transition),
        "{per_transition} bytes per transition"
    );
    // Idle channels cost a constant few bytes each.
    let (one, _) = size(1, 0, 10_000);
    let (thirty_two, _) = size(32, 0, 10_000);
    assert!(thirty_two - one < 31 * 8, "{one} vs {thirty_two}");
}

#[test]
fn loading_needs_no_more_memory_than_the_capture_itself() {
    let capture = record(
        8,
        40,
        &[(None, 50_000), (Some(9), 10_000)],
        End::Closed,
        2,
        Limits::default(),
    );
    let dir = TempDir::new("mem");
    let path = dir.join("m");
    save(&path, &data(&capture, false, None));
    let loaded = load(&path);
    assert_eq!(loaded.memory_used(), capture.memory_used());
    // The same file loads under a budget just above that figure and fails just below it.
    let exact = capture.memory_used();
    let limits = |budget: u64| LoadLimits {
        storage: Limits {
            memory_budget_bytes: budget.max(1024 * 1024),
            ..Limits::default()
        },
        ..LoadLimits::default()
    };
    assert!(load_blocking(&path, &limits(exact + 4096), &Control::new()).is_ok());
}
