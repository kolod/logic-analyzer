//! Exact storage and readback (spec US1, FR-002..FR-005, SC-001).
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::panic
)]

mod common;

use common::{ReferenceCapture, Rng};
use la_storage::{Capture, CaptureConfig, Edge, LevelState, Limits};

const CHUNK_PLANS: [&[usize]; 6] = [
    &[1],
    &[7],
    &[1024],
    &[4096],
    &[65_536],
    &[1, 3, 100, 5000, 65_536, 17],
];

fn new_capture(channels: u8) -> Capture {
    Capture::new(CaptureConfig {
        channels,
        sample_rate_hz: 1_000_000,
        limits: Limits::default(),
    })
    .unwrap()
}

/// Exact expected transitions with `start < p <= end` from a precomputed list.
fn expected(
    trans: &[u64],
    reference: &ReferenceCapture,
    ch: u8,
    start: u64,
    end: u64,
) -> Vec<Edge> {
    let lo = trans.partition_point(|&p| p <= start);
    let hi = trans.partition_point(|&p| p <= end);
    trans[lo..hi]
        .iter()
        .map(|&p| Edge {
            position: p,
            rising: reference.level(ch, p) == LevelState::High,
        })
        .collect()
}

/// Stores `samples` with the given chunking and compares everything with the reference.
fn check(channels: u8, samples: &[u32], sizes: &[usize], seed: u64) {
    let mut capture = new_capture(channels);
    common::feed(&mut capture, samples, sizes);
    let status = capture.close().unwrap();
    assert!(status.complete && status.closed);
    let snap = capture.reader().snapshot();

    let mut reference = ReferenceCapture::new(channels);
    reference.push_chunk(samples);
    let len = reference.len();
    assert_eq!(snap.length(), len);

    let mut rng = Rng::new(seed);
    for ch in 0..channels {
        let trans = reference.all_transitions(ch);

        // Levels: every position for short captures, a dense sample otherwise.
        if len <= 20_000 {
            for p in 0..len {
                assert_eq!(
                    snap.level_at(ch, p).unwrap(),
                    reference.level(ch, p),
                    "ch {ch} pos {p}"
                );
            }
        } else {
            let mut positions: Vec<u64> = (0..2000).map(|_| rng.below(len)).collect();
            for &t in trans.iter().take(500) {
                positions.extend([t.saturating_sub(1), t, t + 1]);
            }
            for p in positions {
                assert_eq!(
                    snap.level_at(ch, p).unwrap(),
                    reference.level(ch, p),
                    "ch {ch} pos {p}"
                );
            }
        }

        // The whole capture, then random ranges.
        if len > 0 {
            let got: Vec<Edge> = snap.transitions(ch, 0, len).unwrap().collect();
            assert_eq!(
                got,
                expected(&trans, &reference, ch, 0, len),
                "ch {ch} full range"
            );
            for _ in 0..30 {
                let a = rng.below(len);
                let b = a + 1 + rng.below(len - a);
                let got: Vec<Edge> = snap.transitions(ch, a, b).unwrap().collect();
                assert_eq!(
                    got,
                    expected(&trans, &reference, ch, a, b),
                    "ch {ch} range ({a}, {b}]"
                );
            }
            // Ranges that start and end exactly on transitions.
            for w in trans.windows(2).take(50) {
                let got: Vec<Edge> = snap.transitions(ch, w[0], w[1]).unwrap().collect();
                assert_eq!(got.len(), 1, "exactly the transition at {}", w[1]);
                assert_eq!(got[0].position, w[1]);
            }
        }
    }
}

#[test]
fn constant_signals() {
    for word in [0_u32, u32::MAX, 0xAAAA_AAAA] {
        for sizes in CHUNK_PLANS {
            check(32, &common::constant(10_000, word), sizes, 1);
        }
    }
}

#[test]
fn alternating_signals_on_all_channel_counts() {
    for channels in [1_u8, 3, 16, 32] {
        for sizes in CHUNK_PLANS {
            check(channels, &common::alternating(5_000, channels), sizes, 2);
        }
    }
}

#[test]
fn random_signals_of_different_density() {
    let mut rng = Rng::new(0xC0FFEE);
    for per_mille in [10_u64, 100, 500] {
        for channels in [8_u8, 32] {
            let samples = common::random(20_000, channels, per_mille, &mut rng);
            for sizes in CHUNK_PLANS {
                check(channels, &samples, sizes, per_mille);
            }
        }
    }
}

#[test]
fn runs_of_random_words() {
    let mut rng = Rng::new(77);
    let samples = common::runs(50_000, 16, 200, &mut rng);
    for sizes in CHUNK_PLANS {
        check(16, &samples, sizes, 3);
    }
}

#[test]
fn very_long_runs_between_changes() {
    let mut rng = Rng::new(5);
    let samples = common::runs(5_000_000, 2, 1_000_000, &mut rng);
    check(2, &samples, &[65_536], 4);
    check(2, &samples, &[1, 3, 100, 5000, 65_536, 17], 4);
}

#[test]
fn block_boundaries_of_1024_transitions() {
    // Channel 0 flips at every sample, so n samples give n - 1 transitions.
    for transitions in [1023_usize, 1024, 1025, 2047, 2048, 2049, 5000] {
        let samples = common::alternating(transitions + 1, 1);
        for sizes in CHUNK_PLANS {
            check(1, &samples, sizes, transitions as u64);
        }
    }
}

#[test]
fn transitions_on_chunk_boundaries_appear_exactly_once() {
    // Alternating signal with chunk size 1 puts every transition on a chunk boundary.
    let samples = common::alternating(3000, 4);
    check(4, &samples, &[1], 9);
    let mut capture = new_capture(4);
    common::feed(&mut capture, &samples, &[1]);
    let snap = capture.reader().snapshot();
    for ch in 0..4 {
        let all: Vec<Edge> = snap.transitions(ch, 0, 3000).unwrap().collect();
        assert_eq!(all.len(), 2999);
        let positions: Vec<u64> = all.iter().map(|e| e.position).collect();
        let mut sorted = positions.clone();
        sorted.dedup();
        assert_eq!(positions, sorted, "no duplicates");
    }
}

#[test]
fn channels_are_independent() {
    // Channel 0 flips every sample, channel 1 every 3rd, channel 2 never.
    let samples: Vec<u32> = (0..9_u32).map(|i| (i & 1) | (((i / 3) & 1) << 1)).collect();
    check(3, &samples, &[2], 11);
}

#[test]
fn empty_capture_gives_defined_results() {
    let mut capture = new_capture(4);
    let snap = capture.reader().snapshot();
    assert_eq!(snap.length(), 0);
    assert_eq!(snap.level_at(0, 0).unwrap(), LevelState::Missing);
    assert_eq!(snap.transitions(0, 0, 10).unwrap().count(), 0);
    let status = capture.close().unwrap();
    assert!(status.closed);
}

#[test]
fn single_sample_capture() {
    let mut capture = new_capture(2);
    capture.append(0, &[0b10]).unwrap();
    let snap = capture.reader().snapshot();
    assert_eq!(snap.length(), 1);
    assert_eq!(snap.level_at(0, 0).unwrap(), LevelState::Low);
    assert_eq!(snap.level_at(1, 0).unwrap(), LevelState::High);
    assert_eq!(snap.level_at(1, 1).unwrap(), LevelState::Missing);
    assert_eq!(snap.transitions(1, 0, 5).unwrap().count(), 0);
}

#[test]
fn invalid_channel_and_window_are_typed_errors() {
    let mut capture = new_capture(2);
    capture.append(0, &[0, 1, 0]).unwrap();
    let snap = capture.reader().snapshot();
    assert_eq!(
        snap.level_at(2, 0).unwrap_err().code(),
        la_storage::error::CHANNEL_OUT_OF_RANGE
    );
    assert_eq!(
        snap.transitions(0, 5, 5).err().unwrap().code(),
        la_storage::error::INVALID_WINDOW
    );
    assert_eq!(
        snap.transitions(0, 6, 5).err().unwrap().code(),
        la_storage::error::INVALID_WINDOW
    );
}

#[test]
fn storage_cost_depends_on_transitions_not_on_duration() {
    // 100 million constant samples followed by a single transition (spec FR-005).
    let channels = 8;
    let mut capture = new_capture(channels);
    let chunk = vec![0_u32; 65_536];
    let mut written = 0_u64;
    while written < 100_000_000 {
        capture.append(capture.length(), &chunk).unwrap();
        written += chunk.len() as u64;
    }
    capture.append(capture.length(), &[1]).unwrap();
    assert_eq!(capture.length(), written + 1);
    assert!(
        capture.memory_used() < 4096 * u64::from(channels),
        "memory used {} for {} samples",
        capture.memory_used(),
        capture.length()
    );
    let snap = capture.reader().snapshot();
    assert_eq!(snap.level_at(0, written - 1).unwrap(), LevelState::Low);
    assert_eq!(snap.level_at(0, written).unwrap(), LevelState::High);
}
