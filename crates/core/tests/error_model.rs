//! Tests for the shared typed error model (spec US3, FR-008).
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::panic
)]

use std::error::Error as StdError;

use la_core::{Error, ErrorKind, Result};

/// A sample parser over external bytes: 2-byte big-endian length, then that many bytes.
fn parse_frame(input: &[u8]) -> Result<&[u8]> {
    let header = input.get(..2).ok_or_else(|| {
        Error::new(
            ErrorKind::InvalidInput,
            "frame.header",
            "need 2 header bytes",
        )
    })?;
    let len = usize::from(u16::from_be_bytes([header[0], header[1]]));
    let end = len
        .checked_add(2)
        .ok_or_else(|| Error::new(ErrorKind::InvalidInput, "frame.len", "length overflow"))?;
    input.get(2..end).ok_or_else(|| {
        Error::new(
            ErrorKind::Corrupt,
            "frame.truncated",
            "payload shorter than header says",
        )
    })
}

#[test]
fn every_kind_can_be_constructed_with_code_and_detail() {
    let kinds = [
        ErrorKind::Io,
        ErrorKind::InvalidInput,
        ErrorKind::Corrupt,
        ErrorKind::Device,
        ErrorKind::Cancelled,
        ErrorKind::Resource,
        ErrorKind::Internal,
    ];
    for kind in kinds {
        let e = Error::new(kind, "test.code", "something failed");
        assert_eq!(e.kind(), kind);
        assert_eq!(e.code(), "test.code");
        assert_eq!(e.detail(), "something failed");
    }
}

#[test]
fn display_contains_code_and_detail() {
    let text = Error::new(ErrorKind::Io, "io.read", "disk gone").to_string();
    assert!(text.contains("io.read"), "{text}");
    assert!(text.contains("disk gone"), "{text}");
}

#[test]
fn source_cause_is_preserved() {
    let cause = std::io::Error::new(std::io::ErrorKind::NotFound, "missing");
    let e = Error::new(ErrorKind::Io, "io.open", "cannot open").with_source(cause);
    let source = e.source().expect("source present");
    assert!(source.to_string().contains("missing"));
}

#[test]
fn error_without_source_has_none() {
    assert!(Error::new(ErrorKind::Internal, "x", "y").source().is_none());
}

#[test]
fn error_is_send_sync_static() {
    fn assert_bounds<T: Send + Sync + 'static>() {}
    assert_bounds::<Error>();
}

#[test]
fn malformed_input_returns_typed_error_never_panics() {
    let long = vec![0xff_u8; 100_000];
    let inputs: Vec<&[u8]> = vec![
        &[],
        &[0],
        &[0, 5],
        &[0, 5, 1, 2],
        &[0xff, 0xff],
        &[0xff, 0xff, 1],
        &long,
    ];
    for input in inputs {
        // Must not panic; the result is either Ok or a typed Err.
        let result = std::panic::catch_unwind(|| parse_frame(input).map(<[u8]>::len));
        let result = result.expect("parser must not panic");
        if let Err(e) = result {
            assert!(matches!(
                e.kind(),
                ErrorKind::InvalidInput | ErrorKind::Corrupt
            ));
            assert!(!e.code().is_empty());
        }
    }
}

#[test]
fn well_formed_input_parses() {
    assert_eq!(parse_frame(&[0, 2, 9, 8, 7]).unwrap(), &[9, 8]);
}

#[test]
fn pseudo_random_inputs_never_panic() {
    // Deterministic xorshift generator: no extra dependency.
    let mut state: u64 = 0x9e37_79b9_7f4a_7c15;
    for _ in 0..2_000 {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        let len = usize::try_from(state % 64).unwrap();
        let bytes: Vec<u8> = (0..len)
            .map(|i| u8::try_from((state >> (i % 56)) & 0xff).unwrap())
            .collect();
        let _ = parse_frame(&bytes);
    }
}
