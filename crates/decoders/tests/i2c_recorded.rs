//! Recorded I2C signals and what decoding them must give (constitution Principle IX; spec 008,
//! task T009). The signals are files under `tests/fixtures/i2c/` in the format described in
//! `tests/common/i2c.rs`; their expectations were derived by hand and are kept next to them.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::panic
)]

mod common;

use common::i2c::{decode, load_fixture, sorted};

fn check(name: &str) {
    let fixture = load_fixture(name);
    let decoded = decode(&fixture.words);
    assert_eq!(
        decoded.all,
        sorted(fixture.expected),
        "the decode of `{name}` differs"
    );
    assert_eq!(decoded.refused, 0, "{name}");
}

#[test]
fn a_write_of_two_bytes() {
    check("write_two_bytes");
}

#[test]
fn a_read_of_two_bytes() {
    check("read_two_bytes");
}

#[test]
fn a_repeated_start() {
    check("repeated_start");
}

#[test]
fn a_capture_that_begins_in_the_middle_of_a_byte() {
    check("begins_mid_byte");
}

#[test]
fn lines_low_and_noise_before_the_interface_is_set_up() {
    check("preinit_low_then_noise");
}

#[test]
fn clock_stretching_changes_nothing() {
    check("clock_stretching");
}
