//! Every truncation and every single-bit flip of a valid file is rejected (spec US3, FR-014,
//! SC-004). Nothing here may panic or load a damaged file as valid.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::panic
)]

mod common;

use common::{End, TempDir, data, load_bytes, load_error_code, record};
use la_session::error::{DAMAGED, NOT_A_SESSION, TRUNCATED, UNSUPPORTED_VERSION};
use la_session::{Control, LoadLimits, Section, SectionTag, save_blocking};
use la_storage::Limits;

/// The representative files: small with gaps and flags, busier with several channels, and one
/// with an unknown optional section.
fn representative_files() -> Vec<(&'static str, Vec<u8>)> {
    let dir = TempDir::new("flips");
    let save = |name: &str, session: &la_session::SessionData| {
        let path = dir.join(name);
        save_blocking(&path, session, &Control::new()).unwrap();
        std::fs::read(path).unwrap()
    };
    let small = record(
        3,
        80,
        &[(None, 600), (Some(40), 300)],
        End::Interrupted,
        1,
        Limits::default(),
    );
    let busy = record(6, 60, &[(None, 4_000)], End::Closed, 2, Limits::default());
    let with_extra = data(&small, true, Some(9))
        .with_section(Section {
            tag: SectionTag::new("TEST").unwrap(),
            critical: false,
            payload: (0..40).collect(),
        })
        .unwrap();
    vec![
        ("small", save("a", &data(&small, true, Some(7)))),
        ("busy", save("b", &data(&busy, false, None))),
        ("extra", save("c", &with_extra)),
    ]
}

#[test]
fn a_truncation_at_every_length_is_rejected() {
    for (name, bytes) in representative_files() {
        for len in 0..bytes.len() {
            let code = load_error_code(&bytes[..len]);
            let expected = if len == 0 { NOT_A_SESSION } else { TRUNCATED };
            assert_eq!(code, expected, "{name} cut to {len} of {}", bytes.len());
        }
        load_bytes(&bytes, &LoadLimits::default()).unwrap();
    }
}

#[test]
fn appended_bytes_are_rejected() {
    for (name, bytes) in representative_files() {
        for extra in [1_usize, 2, 3, 4, 15, 16, 4_096] {
            let mut longer = bytes.clone();
            longer.extend(std::iter::repeat_n(0xA5, extra));
            assert_eq!(load_error_code(&longer), DAMAGED, "{name} + {extra} bytes");
        }
    }
}

#[test]
fn a_flip_of_any_single_bit_is_detected() {
    for (name, bytes) in representative_files() {
        for byte in 0..bytes.len() {
            for bit in 0..8 {
                let mut flipped = bytes.clone();
                flipped[byte] ^= 1 << bit;
                let code = load_error_code(&flipped);
                let expected: &[&str] = match byte {
                    0..=7 => &[NOT_A_SESSION],
                    8 | 9 => &[UNSUPPORTED_VERSION],
                    _ => &[DAMAGED, TRUNCATED],
                };
                assert!(
                    expected.contains(&code),
                    "{name}: bit {bit} of byte {byte} gave {code}"
                );
            }
        }
    }
}
