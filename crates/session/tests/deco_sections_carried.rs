//! The `DECO` and `DANN` sections of feature 007 travel through the session library untouched, and
//! the writer emits format minor 2 (spec 007, task T066).
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects
)]

mod common;

use common::{End, TempDir, data, record};
use la_session::{Control, LoadLimits, Section, SectionTag, load_blocking, save_blocking};
use la_storage::Limits;

fn section(tag: &str, payload: Vec<u8>) -> Section {
    Section {
        tag: SectionTag::new(tag).unwrap(),
        critical: false,
        payload,
    }
}

#[test]
fn deco_and_dann_round_trip_unchanged_and_the_minor_is_two() {
    let dir = TempDir::new("deco-carried");
    let capture = record(3, 20, &[(None, 5_000)], End::Closed, 9, Limits::default());
    let deco: Vec<u8> = (0..=200).collect();
    let dann: Vec<u8> = (50..=255).rev().collect();
    let session = data(&capture, false, Some(1))
        .with_section(section("DECO", deco.clone()))
        .unwrap()
        .with_section(section("DANN", dann.clone()))
        .unwrap();
    let path = dir.join("a.lasession");
    save_blocking(&path, &session, &Control::new()).unwrap();
    let bytes = std::fs::read(&path).unwrap();
    assert_eq!(u16::from_le_bytes([bytes[8], bytes[9]]), 1, "major");
    assert_eq!(u16::from_le_bytes([bytes[10], bytes[11]]), 2, "minor");
    let loaded = load_blocking(&path, &LoadLimits::default(), &Control::new()).unwrap();
    let extras = loaded.extras();
    let tags: Vec<&str> = extras.iter().map(|e| e.tag.as_str()).collect();
    assert_eq!(tags, ["DANN", "DECO"], "optional sections sort by tag");
    assert!(extras.iter().all(|e| !e.critical));
    assert_eq!(extras[0].payload, dann);
    assert_eq!(extras[1].payload, deco);
    // Saved again, the bytes of the whole file are the same.
    let again = dir.join("b.lasession");
    save_blocking(
        &again,
        &la_session::SessionData::from_session(&loaded),
        &Control::new(),
    )
    .unwrap();
    assert_eq!(std::fs::read(&again).unwrap(), bytes);
}
