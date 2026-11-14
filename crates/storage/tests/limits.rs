//! Bounded, predictable memory (spec US4, FR-015..FR-018, SC-005).
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::panic
)]

mod common;

use common::ReferenceCapture;
use la_core::ErrorKind;
use la_storage::{Capture, CaptureConfig, LevelState, Limits, error};

const BUDGET: u64 = 1024 * 1024;
const CHUNK: usize = 4096;

fn small_budget_capture(channels: u8) -> Capture {
    Capture::new(CaptureConfig {
        channels,
        sample_rate_hz: 1_000_000,
        limits: Limits {
            memory_budget_bytes: BUDGET,
            ..Limits::default()
        },
    })
    .unwrap()
}

/// A continuing alternating stream: every sample differs from the previous one on all
/// channels, so every chunk starts with a transition.
fn alternating_from(start: u64, len: usize, channels: u8) -> Vec<u32> {
    let all = common::mask(channels);
    (0..len as u64)
        .map(|i| if (start + i) & 1 == 1 { all } else { 0 })
        .collect()
}

fn append_alternating(capture: &mut Capture, len: usize, channels: u8) -> la_core::Result<()> {
    let start = capture.length();
    capture.append(start, &alternating_from(start, len, channels))
}

/// A fresh capture holding `chunks` full alternating chunks.
fn replay(chunks: usize, channels: u8) -> Capture {
    let mut capture = small_budget_capture(channels);
    for _ in 0..chunks {
        append_alternating(&mut capture, CHUNK, channels).unwrap();
    }
    capture
}

#[test]
fn appending_until_the_budget_is_reached_is_atomic_and_keeps_data_exact() {
    let mut capture = small_budget_capture(1);
    let mut reference = ReferenceCapture::new(1);
    let mut accepted = 0_usize;
    let failure = loop {
        let start = capture.length();
        let chunk = alternating_from(start, CHUNK, 1);
        match capture.append(start, &chunk) {
            Ok(()) => {
                reference.push_chunk(&chunk);
                accepted += 1;
                assert!(
                    capture.memory_used() <= BUDGET,
                    "budget exceeded after {accepted} chunks"
                );
            }
            Err(e) => break e,
        }
        assert!(accepted < 1000, "the budget must be reached");
    };
    assert_eq!(failure.code(), error::MEMORY_LIMIT);
    assert_eq!(failure.kind(), ErrorKind::Resource);
    assert!(accepted > 10, "a 1 MiB budget holds many chunks");

    // The rejected chunk left no partial data.
    assert_eq!(capture.length(), reference.len());
    assert!(capture.memory_used() <= BUDGET);
    let snap = capture.reader().snapshot();
    assert_eq!(snap.length(), reference.len());
    assert!(snap.status().stopped_by_limit);
    for p in (0..reference.len()).step_by(997) {
        assert_eq!(
            snap.level_at(0, p).unwrap(),
            reference.level(0, p),
            "position {p}"
        );
    }
    let trans = reference.all_transitions(0);
    assert_eq!(
        snap.transitions(0, 0, reference.len()).unwrap().count(),
        trans.len()
    );
    // Queries keep working after the limit.
    let view = snap.viewport(0, 0, reference.len(), 500).unwrap();
    assert_eq!(view.pixels, reference.viewport(0, 0, reference.len(), 500));

    // Once stopped, further appends keep failing with the limit error, not `closed`.
    for _ in 0..3 {
        let e = append_alternating(&mut capture, 8, 1).unwrap_err();
        assert_eq!(e.code(), error::MEMORY_LIMIT);
    }
    // Closing reports why the capture is incomplete; later appends say `closed`.
    let status = capture.close().unwrap();
    assert!(status.closed && status.stopped_by_limit && !status.complete);
    let e = append_alternating(&mut capture, 8, 1).unwrap_err();
    assert_eq!(e.code(), error::CLOSED);
}

#[test]
fn a_chunk_that_exactly_fills_the_budget_is_accepted_and_one_more_transition_is_not() {
    // Find the largest accepted size of the next chunk after a number of full chunks.
    let full_chunks = replay(0, 1);
    drop(full_chunks);
    let mut chunks = 0;
    while append_alternating(&mut replay(chunks, 1), CHUNK, 1).is_ok() {
        chunks += 1;
        assert!(chunks < 1000);
    }
    // `replay(chunks)` is the last state in which a full chunk no longer fits.
    let accepts = |n: usize| append_alternating(&mut replay(chunks, 1), n, 1).is_ok();
    let (mut lo, mut hi) = (0_usize, CHUNK); // lo accepted (0 trivially), hi rejected
    while hi - lo > 1 {
        let mid = lo + (hi - lo) / 2;
        if accepts(mid) {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    let largest = lo;
    assert!(largest >= 1, "some samples still fit");
    assert!(accepts(largest));
    assert!(!accepts(largest + 1), "one more sample must be rejected");

    // After accepting the largest chunk the budget is almost exactly used: one more
    // transition (4 bytes, or a new 32-byte block header) would not have fitted.
    let mut capture = replay(chunks, 1);
    append_alternating(&mut capture, largest, 1).unwrap();
    let used = capture.memory_used();
    assert!(used <= BUDGET);
    assert!(BUDGET - used < 4 + 32, "{used} of {BUDGET} bytes used");
}

#[test]
fn rejected_chunks_never_change_stored_data() {
    let mut capture = replay(1, 1);
    let before_len = capture.length();
    let before_used = capture.memory_used();
    // A chunk bigger than the whole budget could ever hold.
    let huge = alternating_from(before_len, 65_536, 1);
    let mut grown = 0;
    while capture.append(capture.length(), &huge).is_ok() {
        grown += 1;
        assert!(grown < 100);
    }
    let len_at_stop = capture.length();
    let used_at_stop = capture.memory_used();
    assert!(len_at_stop >= before_len && used_at_stop >= before_used);
    for _ in 0..5 {
        assert!(capture.append(capture.length(), &huge).is_err());
        assert_eq!(capture.length(), len_at_stop);
        assert_eq!(capture.memory_used(), used_at_stop);
    }
}

#[test]
fn worst_case_memory_per_sample_matches_the_documentation() {
    // A level change on all 32 channels at every sample, in maximum-size chunks.
    let channels = 32;
    let mut capture = Capture::new(CaptureConfig {
        channels,
        sample_rate_hz: 100_000_000,
        limits: Limits {
            memory_budget_bytes: 1024 * 1024 * 1024,
            ..Limits::default()
        },
    })
    .unwrap();
    for _ in 0..8 {
        append_alternating(&mut capture, 65_536, channels).unwrap();
    }
    let per_sample = capture.memory_used() as f64 / capture.length() as f64;
    println!("worst case: {per_sample:.2} bytes per sample for 32 channels");
    assert!(
        per_sample <= 130.0,
        "documented bound is 32 x 4.03 = 129 bytes: {per_sample}"
    );
    assert!(
        per_sample >= 128.0,
        "sanity: 4 bytes per channel-sample: {per_sample}"
    );
}

#[test]
fn sparse_signals_use_far_less_memory() {
    let mut capture = small_budget_capture(32);
    let chunk = vec![0_u32; 4096];
    for _ in 0..200 {
        capture.append(capture.length(), &chunk).unwrap();
    }
    assert!(capture.memory_used() < 1024, "{}", capture.memory_used());
}

#[test]
fn a_chunk_whose_conversion_cannot_fit_the_budget_is_rejected_without_stopping_the_capture() {
    // Worst-case temporary memory of 65,536 samples x 32 channels is 8 MiB > 1 MiB.
    let mut capture = small_budget_capture(32);
    let big = vec![0_u32; 65_536];
    let e = capture.append(0, &big).unwrap_err();
    assert_eq!(e.code(), error::MEMORY_LIMIT);
    assert_eq!(capture.length(), 0);
    assert!(!capture.reader().snapshot().status().stopped_by_limit);
    // A smaller chunk is still accepted: the capture was not stopped.
    capture.append(0, &vec![0_u32; 1024]).unwrap();
    assert_eq!(capture.length(), 1024);
    assert_eq!(
        capture.reader().snapshot().level_at(5, 10).unwrap(),
        LevelState::Low
    );
}
