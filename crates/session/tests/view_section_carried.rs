//! The `VIEW` section of feature 006 travels through the session library untouched, and the writer
//! emits the current format minor, 2 since feature 007 (spec 006, task T063).
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

fn view_section(payload: Vec<u8>) -> Section {
    Section {
        tag: SectionTag::new("VIEW").unwrap(),
        critical: false,
        payload,
    }
}

#[test]
fn a_view_section_round_trips_unchanged_and_the_minor_is_two() {
    let dir = TempDir::new("view-carried");
    let capture = record(3, 20, &[(None, 5_000)], End::Closed, 9, Limits::default());
    let payload: Vec<u8> = (0..=255).collect();
    let session = data(&capture, false, Some(1))
        .with_section(view_section(payload.clone()))
        .unwrap();
    let path = dir.join("a.lasession");
    save_blocking(&path, &session, &Control::new()).unwrap();

    // Header: magic (8), major (2), minor (2).
    let bytes = std::fs::read(&path).unwrap();
    assert_eq!(u16::from_le_bytes([bytes[8], bytes[9]]), 1, "major");
    assert_eq!(u16::from_le_bytes([bytes[10], bytes[11]]), 2, "minor");

    let loaded = load_blocking(&path, &LoadLimits::default(), &Control::new()).unwrap();
    let extras = loaded.extras();
    assert_eq!(extras.len(), 1);
    assert_eq!(extras[0].tag.as_str(), "VIEW");
    assert!(!extras[0].critical);
    assert_eq!(extras[0].payload, payload);

    // Saved again, the section is still there byte for byte.
    let again = dir.join("b.lasession");
    save_blocking(
        &again,
        &la_session::SessionData::from_session(&loaded),
        &Control::new(),
    )
    .unwrap();
    let reloaded = load_blocking(&again, &LoadLimits::default(), &Control::new()).unwrap();
    assert_eq!(reloaded.extras()[0].payload, payload);
}
