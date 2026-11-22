//! Hostile or malformed input never crashes the store (spec US7, FR-018, FR-025, FR-026,
//! SC-008).
//!
//! Cases that need `record_gap` (gap overflow, appending near `u64::MAX`) live in
//! `gaps.rs`.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::panic
)]

mod common;

use std::collections::hash_map::DefaultHasher;
use std::hash::Hasher;
use std::panic::{AssertUnwindSafe, catch_unwind};

use common::Rng;
use la_storage::{Capture, CaptureConfig, CaptureStatus, LevelState, Limits, Snapshot, error};

fn config(channels: u8) -> CaptureConfig {
    CaptureConfig {
        channels,
        sample_rate_hz: 1_000_000,
        limits: Limits::default(),
    }
}

fn capture_with_data() -> Capture {
    let mut capture = Capture::new(config(2)).unwrap();
    capture.append(0, &[0, 1, 2, 3, 0]).unwrap();
    capture
}

fn code<T>(result: la_core::Result<T>) -> &'static str {
    match result {
        Ok(_) => panic!("expected an error"),
        Err(e) => e.code(),
    }
}

// ---- named cases -----------------------------------------------------------------

#[test]
fn chunks_that_do_not_continue_the_capture_are_rejected() {
    let mut capture = capture_with_data();
    for bad_start in [0_u64, 1, 4, 6, 10, u64::MAX] {
        assert_eq!(
            code(capture.append(bad_start, &[1, 2, 3])),
            error::CHUNK_OUT_OF_ORDER,
            "start {bad_start}"
        );
    }
    assert_eq!(capture.length(), 5);
    // The correct start still works afterwards.
    capture.append(5, &[1]).unwrap();
}

#[test]
fn empty_and_oversized_chunks_are_rejected_before_any_work() {
    let mut capture = capture_with_data();
    assert_eq!(code(capture.append(5, &[])), error::CHUNK_EMPTY);
    let max = Limits::default().max_chunk_samples as usize;
    let too_big = vec![0_u32; max + 1];
    assert_eq!(code(capture.append(5, &too_big)), error::CHUNK_TOO_LARGE);
    let exactly_max = vec![1_u32; max];
    capture.append(5, &exactly_max).unwrap();
}

#[test]
fn bits_above_the_channel_count_are_ignored_not_an_error() {
    let mut capture = Capture::new(config(2)).unwrap();
    capture.append(0, &[u32::MAX, 0, u32::MAX]).unwrap();
    let snap = capture.reader().snapshot();
    assert_eq!(snap.level_at(0, 0).unwrap(), LevelState::High);
    assert_eq!(snap.level_at(1, 0).unwrap(), LevelState::High);
    assert_eq!(snap.channels(), 2);
    assert_eq!(code(snap.level_at(2, 0)), error::CHANNEL_OUT_OF_RANGE);
    assert_eq!(snap.transitions(0, 0, 3).unwrap().count(), 2);
}

#[test]
fn channels_outside_the_range_are_rejected_by_every_query() {
    let snap = capture_with_data().reader().snapshot();
    for ch in [2_u8, 3, 32, 255] {
        assert_eq!(code(snap.level_at(ch, 0)), error::CHANNEL_OUT_OF_RANGE);
        assert_eq!(
            code(snap.transitions(ch, 0, 5)),
            error::CHANNEL_OUT_OF_RANGE
        );
        assert_eq!(
            code(snap.viewport(ch, 0, 5, 10)),
            error::CHANNEL_OUT_OF_RANGE
        );
    }
}

#[test]
fn empty_or_reversed_windows_are_rejected() {
    let snap = capture_with_data().reader().snapshot();
    for (s, e) in [
        (0_u64, 0_u64),
        (5, 5),
        (6, 5),
        (u64::MAX, 0),
        (u64::MAX, u64::MAX),
    ] {
        assert_eq!(code(snap.transitions(0, s, e)), error::INVALID_WINDOW);
        assert_eq!(code(snap.viewport(0, s, e, 10)), error::INVALID_WINDOW);
    }
}

#[test]
fn invalid_pixel_counts_are_rejected_without_allocating() {
    let snap = capture_with_data().reader().snapshot();
    for pixels in [0_u32, 16_385, 65_536, 1 << 24, u32::MAX] {
        assert_eq!(code(snap.viewport(0, 0, 5, pixels)), error::INVALID_PIXELS);
    }
}

#[test]
fn extreme_positions_are_defined_and_do_not_overflow() {
    let snap = capture_with_data().reader().snapshot();
    for p in [5_u64, 6, u64::MAX - 1, u64::MAX] {
        assert_eq!(snap.level_at(0, p).unwrap(), LevelState::Missing);
    }
    assert_eq!(
        snap.transitions(0, u64::MAX - 1, u64::MAX).unwrap().count(),
        0
    );
    assert!(snap.transitions(0, 0, u64::MAX).is_ok());
    assert!(snap.viewport(0, 0, u64::MAX, 100).is_ok());
    assert!(snap.viewport(0, u64::MAX - 1, u64::MAX, 100).is_ok());
    assert!(snap.viewport(0, u64::MAX / 2, u64::MAX, 16_384).is_ok());
}

#[test]
fn everything_fails_with_closed_after_close() {
    let mut capture = capture_with_data();
    capture.close().unwrap();
    assert_eq!(code(capture.append(5, &[1])), error::CLOSED);
    // Closing again is harmless.
    assert!(capture.close().unwrap().closed);
}

#[test]
fn creation_with_invalid_configurations_fails_cleanly() {
    for channels in [0_u8, 161, 255] {
        assert_eq!(
            code(Capture::new(config(channels))),
            error::INVALID_CHANNEL_COUNT
        );
    }
    let zero_rate = CaptureConfig {
        sample_rate_hz: 0,
        ..config(1)
    };
    assert_eq!(code(Capture::new(zero_rate)), error::INVALID_SAMPLE_RATE);
    let bad_limits = CaptureConfig {
        limits: Limits {
            max_pixels: 0,
            ..Limits::default()
        },
        ..config(1)
    };
    assert_eq!(code(Capture::new(bad_limits)), error::INVALID_LIMITS);
}

// ---- generated corpus ------------------------------------------------------------

/// Everything observable about a capture, hashed.
fn fingerprint(snap: &Snapshot, memory_used: u64) -> (u64, CaptureStatus, u64, u64) {
    let mut h = DefaultHasher::new();
    let len = snap.length();
    for ch in 0..snap.channels() {
        for p in [
            0,
            1,
            len / 2,
            len.saturating_sub(1),
            len,
            len.saturating_add(1),
        ] {
            h.write(format!("{:?}", snap.level_at(ch, p).unwrap()).as_bytes());
        }
        let edges: Vec<_> = snap
            .transitions(ch, 0, len.saturating_add(1))
            .unwrap()
            .take(2000)
            .collect();
        h.write(format!("{edges:?}").as_bytes());
        let view = snap.viewport(ch, 0, len.saturating_add(1), 50).unwrap();
        h.write(format!("{:?}", view.pixels).as_bytes());
    }
    (len, snap.status(), memory_used, h.finish())
}

/// One random call; returns whether it failed and a description for diagnostics.
fn random_call(
    capture: &mut Capture,
    snap: &Snapshot,
    rng: &mut Rng,
    max_chunk: usize,
) -> (bool, String, Result<(), la_core::Error>) {
    let extremes = [
        0_u64,
        1,
        2,
        u64::MAX,
        u64::MAX - 1,
        u64::MAX / 2,
        1 << 32,
        (1 << 32) - 1,
    ];
    let pick = |rng: &mut Rng, len: u64| -> u64 {
        if rng.below(4) == 0 {
            extremes[rng.below(extremes.len() as u64) as usize]
        } else {
            rng.below(len + 5)
        }
    };
    let op = rng.below(1000); // close() is one call in a thousand
    let len = capture.length();
    match op {
        0..=449 => {
            // append: mostly the right start, sometimes a wrong or extreme one
            let start = if rng.below(10) < 6 {
                len
            } else {
                pick(rng, len)
            };
            let count = match rng.below(20) {
                0 => 0,
                1 => max_chunk + 1 + rng.below(3) as usize,
                2 => max_chunk,
                _ => rng.below(64) as usize,
            };
            let words: Vec<u32> = (0..count).map(|_| rng.next_u64() as u32).collect();
            let r = capture.append(start, &words);
            (
                r.is_err(),
                format!("append(start {start}, {count} words)"),
                r,
            )
        }
        450..=599 => {
            let (ch, p) = (rng.below(40) as u8, pick(rng, len));
            let r = snap.level_at(ch, p).map(|_| ());
            (r.is_err(), format!("level_at({ch}, {p})"), r)
        }
        600..=749 => {
            let (ch, s, e) = (rng.below(40) as u8, pick(rng, len), pick(rng, len));
            let r = snap
                .transitions(ch, s, e)
                .map(|it| it.take(100).for_each(drop));
            (r.is_err(), format!("transitions({ch}, {s}, {e})"), r)
        }
        750..=998 => {
            let (ch, s, e) = (rng.below(40) as u8, pick(rng, len), pick(rng, len));
            let px = match rng.below(6) {
                0 => 0,
                1 => u32::MAX,
                2 => 16_385,
                _ => rng.range(1, 3000) as u32,
            };
            let r = snap.viewport(ch, s, e, px).map(|_| ());
            (r.is_err(), format!("viewport({ch}, {s}, {e}, {px})"), r)
        }
        _ => {
            let r = capture.close().map(|_| ());
            (false, "close()".to_owned(), r)
        }
    }
}

#[test]
fn ten_thousand_generated_calls_never_panic_and_failures_change_nothing() {
    const SEED: u64 = 0x5EED_0042;
    const CALLS: usize = 15_000;
    let mut rng = Rng::new(SEED);
    let max_chunk = Limits::default().max_chunk_samples as usize;
    let mut failures = 0_usize;
    let mut successes = 0_usize;
    let mut capture = Capture::new(config(3)).unwrap();
    let mut snap = capture.reader().snapshot();
    let mut known = fingerprint(&snap, capture.memory_used());

    // After a close(), keep calling for a while (everything that mutates must now fail
    // with `storage.closed`), then start a fresh capture so valid calls continue.
    let mut calls_since_close = 0_usize;
    for i in 0..CALLS {
        if calls_since_close > 0 {
            calls_since_close += 1;
            if calls_since_close > 40 {
                calls_since_close = 0;
                capture = Capture::new(config(3)).unwrap();
                snap = capture.reader().snapshot();
                known = fingerprint(&snap, capture.memory_used());
            }
        }
        let outcome = catch_unwind(AssertUnwindSafe(|| {
            random_call(&mut capture, &snap, &mut rng, max_chunk)
        }));
        let (failed, what, result) = match outcome {
            Ok(v) => v,
            Err(_) => panic!("call {i} panicked (seed {SEED:#x})"),
        };
        match result {
            Err(e) => {
                failures += 1;
                assert!(
                    e.code().starts_with("storage."),
                    "call {i} {what}: unexpected error code {} (seed {SEED:#x})",
                    e.code()
                );
                let now = fingerprint(&capture.reader().snapshot(), capture.memory_used());
                // A rejected call may only set the "stopped by limit" flag.
                assert_eq!(
                    (now.0, now.2, now.3),
                    (known.0, known.2, known.3),
                    "call {i} {what}: a failed call changed stored data (seed {SEED:#x})"
                );
            }
            Ok(()) => {
                successes += 1;
                debug_assert!(!failed);
                if what == "close()" {
                    calls_since_close = 1;
                }
                snap = capture.reader().snapshot();
                known = fingerprint(&snap, capture.memory_used());
            }
        }
    }
    println!("{CALLS} generated calls: {successes} succeeded, {failures} failed (seed {SEED:#x})");
    assert!(
        failures >= 10_000,
        "SC-008 needs at least 10,000 invalid or extreme inputs"
    );
    assert!(
        successes > 2_000,
        "the corpus must also exercise valid inputs"
    );
}
