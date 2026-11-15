//! The format can grow without breaking files (spec US5, FR-023..027, SC-011).
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::panic
)]

mod common;

use std::path::PathBuf;

use common::{
    End, RawSection, TempDir, build_file, data, load_bytes, load_error_code, real_file, record,
    repair_checksums, split_sections,
};
use la_session::error::{DAMAGED, INVALID_INPUT, UNKNOWN_CRITICAL_SECTION, UNSUPPORTED_VERSION};
use la_session::{
    Control, LoadLimits, Section, SectionTag, SessionData, load_blocking, save_blocking,
};
use la_storage::{LevelState, Limits};

fn section(tag: &str, critical: bool, payload: Vec<u8>) -> Section {
    Section {
        tag: SectionTag::new(tag).unwrap(),
        critical,
        payload,
    }
}

fn save_bytes(session: &SessionData) -> Vec<u8> {
    let dir = TempDir::new("compat");
    let path = dir.join("s");
    save_blocking(&path, session, &Control::new()).unwrap();
    std::fs::read(path).unwrap()
}

fn base() -> SessionData {
    let capture = record(3, 60, &[(None, 800)], End::Closed, 5, Limits::default());
    data(&capture, false, None)
}

// ---- unknown sections ---------------------------------------------------------------------

#[test]
fn unknown_optional_sections_survive_a_load_and_save_cycle_byte_for_byte() {
    let big: Vec<u8> = (0..1_000_000_u32).map(|i| (i * 7) as u8).collect();
    let with_extras = base()
        .with_section(section("ZZZZ", false, big.clone()))
        .unwrap()
        .with_section(section("TEST", false, vec![]))
        .unwrap();
    let original = save_bytes(&with_extras);
    let loaded = load_bytes(&original, &LoadLimits::default()).unwrap();
    let tags: Vec<&str> = loaded.extras().iter().map(|s| s.tag.as_str()).collect();
    assert_eq!(tags, ["TEST", "ZZZZ"], "ascending tag order");
    assert!(loaded.extras().iter().all(|s| !s.critical));
    assert_eq!(loaded.extras()[0].payload, Vec::<u8>::new());
    assert_eq!(loaded.extras()[1].payload, big);
    let again = save_bytes(&SessionData::from_session(&loaded));
    assert_eq!(again, original, "the extras come back unchanged");
}

#[test]
fn an_unknown_critical_section_names_itself_and_asks_for_a_newer_program() {
    let critical = base()
        .with_section(section("FUTR", true, vec![1, 2, 3]))
        .unwrap();
    let bytes = save_bytes(&critical);
    let error = load_bytes(&bytes, &LoadLimits::default()).unwrap_err();
    assert_eq!(error.code(), UNKNOWN_CRITICAL_SECTION);
    assert!(error.detail().contains("FUTR"), "{}", error.detail());
    assert!(error.detail().contains("newer"), "{}", error.detail());
}

#[test]
fn reserved_and_duplicate_tags_cannot_be_added_and_bad_files_are_damage() {
    for reserved in ["META", "CAPT"] {
        let error = base()
            .with_section(section(reserved, false, vec![]))
            .unwrap_err();
        assert_eq!(error.code(), INVALID_INPUT);
    }
    let duplicate = base()
        .with_section(section("DUPL", false, vec![1]))
        .unwrap()
        .with_section(section("DUPL", false, vec![2]))
        .unwrap_err();
    assert_eq!(duplicate.code(), INVALID_INPUT);
    for bad in ["", "ABC", "ABCDE", "AB D", "AB-D", "ÄBCD"] {
        assert_eq!(
            SectionTag::new(bad).unwrap_err().code(),
            INVALID_INPUT,
            "{bad:?}"
        );
    }

    // Files that break the section rules even though every checksum is right.
    let sections = split_sections(&real_file());
    let extra = |tag: &[u8; 4]| RawSection {
        tag: *tag,
        critical: false,
        payload: vec![1],
    };
    let with = |extras: Vec<RawSection>| {
        let mut all = sections.clone();
        all.extend(extras);
        build_file(&all)
    };
    assert_eq!(
        load_error_code(&with(vec![extra(b"AAAA"), extra(b"AAAA")])),
        DAMAGED,
        "duplicate"
    );
    assert_eq!(
        load_error_code(&with(vec![extra(b"BBBB"), extra(b"AAAA")])),
        DAMAGED,
        "descending"
    );
    assert_eq!(
        load_error_code(&with(vec![extra(b"AB D")])),
        DAMAGED,
        "space in a tag"
    );
    assert_eq!(
        load_error_code(&with(vec![extra(b"META")])),
        DAMAGED,
        "META twice"
    );
    let mut swapped = sections.clone();
    swapped.swap(0, 1);
    assert_eq!(
        load_error_code(&build_file(&swapped)),
        DAMAGED,
        "CAPT before META"
    );
    assert!(
        load_bytes(
            &with(vec![extra(b"AAAA"), extra(b"BBBB")]),
            &LoadLimits::default()
        )
        .is_ok()
    );
}

// ---- versions -----------------------------------------------------------------------------

#[test]
fn a_newer_major_version_is_refused_and_a_newer_minor_is_read() {
    let mut bytes = real_file();
    bytes[8] = 2;
    repair_checksums(&mut bytes);
    let error = load_bytes(&bytes, &LoadLimits::default()).unwrap_err();
    assert_eq!(error.code(), UNSUPPORTED_VERSION);
    assert!(
        error.detail().contains("2.x") && error.detail().contains("1.x"),
        "{}",
        error.detail()
    );

    let mut bytes = real_file();
    bytes[10] = 5;
    repair_checksums(&mut bytes);
    assert!(
        load_bytes(&bytes, &LoadLimits::default()).is_ok(),
        "minor 5 with known sections"
    );

    // Version 0 is not a version this program ever wrote.
    let mut bytes = real_file();
    bytes[8] = 0;
    repair_checksums(&mut bytes);
    assert_eq!(load_error_code(&bytes), UNSUPPORTED_VERSION);
}

// ---- fixtures -----------------------------------------------------------------------------

fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/v1_0")
}

/// The files the writer produced at format 1.1 (the current writer); the 1.0 files above stay as
/// they are and must keep loading.
fn writer_fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/v1_1")
}

/// The sessions behind the committed version 1.0 fixtures (fixed seeds, integer-only generators).
fn fixture_sessions() -> Vec<(&'static str, SessionData)> {
    let plain = record(4, 40, &[(None, 5_000)], End::Closed, 101, Limits::default());
    let gaps = record(
        6,
        70,
        &[(None, 3_000), (Some(120), 2_000), (Some(3), 400)],
        End::Interrupted,
        102,
        Limits::default(),
    );
    let simulated = record(8, 25, &[(None, 6_000)], End::Closed, 103, Limits::default());
    let extras = record(2, 50, &[(None, 1_500)], End::Closed, 104, Limits::default());
    vec![
        ("plain.lasession", data(&plain, false, None)),
        ("gaps.lasession", data(&gaps, false, None)),
        (
            "simulated.lasession",
            data(&simulated, true, Some(0xDEAD_BEEF_0000_0001)),
        ),
        (
            "extras.lasession",
            data(&extras, false, None)
                .with_section(section(
                    "NOTE",
                    false,
                    b"a note from a later feature".to_vec(),
                ))
                .unwrap()
                .with_section(section("VIEW", false, (0..64).collect()))
                .unwrap(),
        ),
    ]
}

#[test]
#[ignore = "writes the committed fixtures; run only to create them"]
fn regenerate_fixtures() {
    std::fs::create_dir_all(writer_fixtures_dir()).unwrap();
    for (name, session) in fixture_sessions() {
        save_blocking(&writer_fixtures_dir().join(name), &session, &Control::new()).unwrap();
    }
}

#[test]
fn the_writer_still_produces_the_committed_version_1_1_files() {
    let dir = TempDir::new("regen");
    for (name, session) in fixture_sessions() {
        let produced = dir.join(name);
        save_blocking(&produced, &session, &Control::new()).unwrap();
        assert_eq!(
            std::fs::read(&produced).unwrap(),
            std::fs::read(writer_fixtures_dir().join(name)).unwrap(),
            "{name}: the writer's output changed. A format change needs a new format version and a \
             new fixture directory (tests/fixtures/v<major>_<minor>); older fixtures must stay as they are."
        );
    }
}

/// What a fixture must load to, written down once as plain values.
struct Facts {
    name: &'static str,
    length: u64,
    channels: u8,
    rate: u64,
    gaps: &'static [(u64, u64)],
    transitions: &'static [u64],
    complete: bool,
    interrupted: bool,
    lost: u64,
    simulated: bool,
    seed: Option<u64>,
    device: &'static str,
    extras: &'static [(&'static str, usize)],
    levels: &'static [(u8, u64, char)],
}

include!("fixtures/v1_0/facts.rs");

fn level_char(level: LevelState) -> char {
    match level {
        LevelState::High => 'H',
        LevelState::Low => 'L',
        LevelState::Missing => 'M',
    }
}

#[test]
fn every_fixture_loads_to_the_content_recorded_for_it() {
    assert_eq!(FACTS.len(), fixture_sessions().len());
    for facts in FACTS {
        let path = fixtures_dir().join(facts.name);
        let session = load_blocking(&path, &LoadLimits::default(), &Control::new()).unwrap();
        let s = session.snapshot();
        let what = facts.name;
        assert_eq!(s.length(), facts.length, "{what}");
        assert_eq!(s.channels(), facts.channels, "{what}");
        assert_eq!(s.sample_rate_hz(), facts.rate, "{what}");
        let gaps: Vec<(u64, u64)> = s.gaps().iter().map(|g| (g.start, g.length)).collect();
        assert_eq!(gaps, facts.gaps, "{what}");
        let transitions: Vec<u64> = (0..s.channels())
            .map(|c| s.transition_count(c).unwrap())
            .collect();
        assert_eq!(transitions, facts.transitions, "{what}");
        let status = s.status();
        assert_eq!(status.complete, facts.complete, "{what}");
        assert_eq!(status.interrupted, facts.interrupted, "{what}");
        assert_eq!(status.lost_positions, facts.lost, "{what}");
        assert!(status.closed, "{what}");
        let d = session.description();
        assert_eq!(d.simulated(), facts.simulated, "{what}");
        assert_eq!(d.seed(), facts.seed, "{what}");
        assert_eq!(d.device_name(), facts.device, "{what}");
        let extras: Vec<(&str, usize)> = session
            .extras()
            .iter()
            .map(|e| (e.tag.as_str(), e.payload.len()))
            .collect();
        assert_eq!(extras, facts.extras, "{what}");
        for (channel, position, expected) in facts.levels {
            assert_eq!(
                level_char(s.level_at(*channel, *position).unwrap()),
                *expected,
                "{what}: channel {channel} position {position}"
            );
        }
    }
}

/// Prints the facts of the current fixtures as Rust source (run once, review, commit).
#[test]
#[ignore = "prints the facts file; run only to create it"]
fn print_fixture_facts() {
    let mut out = String::from(
        "// Generated once from the version 1.0 fixtures and reviewed by hand.\nconst FACTS: &[Facts] = &[\n",
    );
    for (name, _) in fixture_sessions() {
        let session = load_blocking(
            &fixtures_dir().join(name),
            &LoadLimits::default(),
            &Control::new(),
        )
        .unwrap();
        let s = session.snapshot();
        let d = session.description();
        let status = s.status();
        let gaps: Vec<String> = s
            .gaps()
            .iter()
            .map(|g| format!("({}, {})", g.start, g.length))
            .collect();
        let transitions: Vec<String> = (0..s.channels())
            .map(|c| s.transition_count(c).unwrap().to_string())
            .collect();
        let extras: Vec<String> = session
            .extras()
            .iter()
            .map(|e| format!("(\"{}\", {})", e.tag.as_str(), e.payload.len()))
            .collect();
        let mut levels = Vec::new();
        for channel in [0, s.channels() - 1] {
            for position in [0, 1, s.length() / 3, s.length() / 2, s.length() - 1] {
                levels.push(format!(
                    "({channel}, {position}, '{}')",
                    level_char(s.level_at(channel, position).unwrap())
                ));
            }
        }
        out.push_str(&format!(
            "    Facts {{\n        name: \"{name}\",\n        length: {},\n        channels: {},\n        rate: {},\n        gaps: &[{}],\n        transitions: &[{}],\n        complete: {},\n        interrupted: {},\n        lost: {},\n        simulated: {},\n        seed: {:?},\n        device: \"{}\",\n        extras: &[{}],\n        levels: &[{}],\n    }},\n",
            s.length(), s.channels(), s.sample_rate_hz(), gaps.join(", "), transitions.join(", "),
            status.complete, status.interrupted, status.lost_positions, d.simulated(), d.seed(), d.device_name(),
            extras.join(", "), levels.join(", ")
        ));
    }
    out.push_str("];\n");
    std::fs::write(fixtures_dir().join("facts.rs"), out).unwrap();
}
