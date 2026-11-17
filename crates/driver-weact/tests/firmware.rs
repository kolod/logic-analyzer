//! The firmware image that ships with the application (spec 013, task T017).
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::panic
)]

use la_driver_weact::firmware::{EMBEDDED, Image, MAX_IMAGE};
use sha2::{Digest, Sha256};

#[test]
fn the_embedded_image_is_the_one_recorded_in_the_source_notes() {
    let notes = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/firmware/SOURCE.md"))
        .unwrap();
    let line = notes
        .lines()
        .find(|line| line.contains("SHA-256 of the file"))
        .expect("SOURCE.md records the checksum of the image");
    let recorded = line
        .split('`')
        .nth(1)
        .expect("the checksum is in backticks");
    let actual = format!("{:x}", Sha256::digest(EMBEDDED));
    assert_eq!(
        actual, recorded,
        "the image and SOURCE.md have drifted apart"
    );
}

#[test]
fn the_embedded_image_fits_the_chip_and_passes_its_own_check() {
    assert!(!EMBEDDED.is_empty());
    assert!(EMBEDDED.len() <= MAX_IMAGE, "{} bytes", EMBEDDED.len());
    let image = Image::embedded();
    assert_eq!(image.bytes(), EMBEDDED);
    image.checked().unwrap();
}
