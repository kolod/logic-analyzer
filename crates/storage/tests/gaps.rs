//! Incomplete captures are flagged (spec US6, FR-022..FR-024, SC-009), plus the gap cases
//! of the hostile-input story.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::panic
)]

mod common;

use common::{ReferenceCapture, Rng};
use la_storage::{Activity, Capture, CaptureConfig, Edge, Gap, LevelState, Limits, error};

fn capture(channels: u8) -> Capture {
    Capture::new(CaptureConfig {
        channels,
        sample_rate_hz: 1_000_000,
        limits: Limits::default(),
    })
    .unwrap()
}

fn code<T>(result: la_core::Result<T>) -> &'static str {
    match result {
        Ok(_) => panic!("expected an error"),
        Err(e) => e.code(),
    }
}

#[test]
fn recording_a_gap_advances_the_length_and_flags_the_capture() {
    let mut c = capture(1);
    c.append(0, &[0, 1, 0]).unwrap();
    c.record_gap(10).unwrap();
    assert_eq!(c.length(), 13);
    let snap = c.reader().snapshot();
    assert_eq!(snap.length(), 13);
    assert_eq!(
        snap.gaps(),
        &[Gap {
            start: 3,
            length: 10
        }]
    );
    assert_eq!(snap.status().lost_positions, 10);
    assert!(!snap.status().complete);
    c.append(13, &[1, 1]).unwrap();
    assert_eq!(c.length(), 15);
}

#[test]
fn adjacent_gaps_are_merged() {
    let mut c = capture(1);
    c.append(0, &[0]).unwrap();
    c.record_gap(3).unwrap();
    c.record_gap(4).unwrap();
    c.append(8, &[0]).unwrap();
    c.record_gap(2).unwrap();
    let snap = c.reader().snapshot();
    assert_eq!(
        snap.gaps(),
        &[
            Gap {
                start: 1,
                length: 7
            },
            Gap {
                start: 9,
                length: 2
            }
        ]
    );
    assert_eq!(snap.status().lost_positions, 9);
}

#[test]
fn a_gap_at_the_very_start_is_handled() {
    let mut c = capture(2);
    c.record_gap(5).unwrap();
    c.append(5, &[0b10, 0b10, 0b00]).unwrap();
    let snap = c.reader().snapshot();
    assert_eq!(snap.length(), 8);
    for p in 0..5 {
        assert_eq!(snap.level_at(1, p).unwrap(), LevelState::Missing);
    }
    // The initial level is the level of the first sample that exists.
    assert_eq!(snap.level_at(1, 5).unwrap(), LevelState::High);
    assert_eq!(snap.level_at(0, 5).unwrap(), LevelState::Low);
    assert_eq!(snap.transitions(1, 0, 8).unwrap().count(), 1);
    let view = snap.viewport(1, 0, 8, 8).unwrap();
    assert!(view.pixels[..5].iter().all(|p| p.missing_data));
}

#[test]
fn a_gap_at_the_very_end_is_handled() {
    let mut c = capture(1);
    c.append(0, &[1, 1]).unwrap();
    c.record_gap(4).unwrap();
    let snap = c.reader().snapshot();
    assert_eq!(snap.length(), 6);
    assert_eq!(snap.level_at(0, 1).unwrap(), LevelState::High);
    for p in 2..6 {
        assert_eq!(snap.level_at(0, p).unwrap(), LevelState::Missing);
    }
    let view = snap.viewport(0, 0, 6, 6).unwrap();
    assert!(!view.pixels[0].missing_data);
    assert!(view.pixels[3].missing_data);
}

#[test]
fn the_first_sample_after_a_gap_is_not_compared_with_the_one_before_it() {
    // Levels differ across the gap: a resync transition is stored at the gap's end.
    let mut c = capture(1);
    c.append(0, &[0, 0, 0]).unwrap();
    c.record_gap(5).unwrap();
    c.append(8, &[1, 1]).unwrap();
    let snap = c.reader().snapshot();
    let edges: Vec<Edge> = snap.transitions(0, 0, 10).unwrap().collect();
    assert_eq!(
        edges,
        vec![Edge {
            position: 8,
            rising: true
        }]
    );
    assert_eq!(snap.level_at(0, 2).unwrap(), LevelState::Low);
    assert_eq!(snap.level_at(0, 4).unwrap(), LevelState::Missing);
    assert_eq!(snap.level_at(0, 8).unwrap(), LevelState::High);
    // The column holding the resync transition is flagged as touching lost data.
    let view = snap.viewport(0, 6, 10, 2).unwrap();
    assert!(view.pixels.iter().any(|p| p.missing_data));
    assert!(view.pixels.iter().any(|p| p.activity
        == Activity::One(Edge {
            position: 8,
            rising: true
        })));

    // Equal levels across the gap: no transition at all.
    let mut c = capture(1);
    c.append(0, &[1, 1]).unwrap();
    c.record_gap(3).unwrap();
    c.append(5, &[1]).unwrap();
    assert_eq!(
        c.reader().snapshot().transitions(0, 0, 6).unwrap().count(),
        0
    );
}

#[test]
fn closing_decides_completeness() {
    // Not closed: never complete, even without gaps.
    let mut c = capture(1);
    c.append(0, &[0, 1]).unwrap();
    assert!(!c.reader().snapshot().status().complete);
    // Closed without gaps: complete.
    let status = c.close().unwrap();
    assert!(status.closed && status.complete && status.lost_positions == 0);
    assert!(c.reader().snapshot().status().complete);

    // Closed with a gap: incomplete, with the number of lost positions.
    let mut c = capture(1);
    c.append(0, &[0]).unwrap();
    c.record_gap(2).unwrap();
    c.record_gap(5).unwrap();
    let status = c.close().unwrap();
    assert!(status.closed && !status.complete);
    assert_eq!(status.lost_positions, 7);
    assert!(!c.reader().snapshot().status().complete);
}

#[test]
fn nothing_can_be_added_after_closing() {
    let mut c = capture(1);
    c.append(0, &[0]).unwrap();
    c.close().unwrap();
    assert_eq!(code(c.record_gap(1)), error::CLOSED);
    assert_eq!(code(c.append(1, &[0])), error::CLOSED);
}

#[test]
fn invalid_gaps_are_rejected_and_change_nothing() {
    let mut c = capture(1);
    c.append(0, &[0, 1, 0]).unwrap();
    assert_eq!(code(c.record_gap(0)), error::INVALID_GAP);
    assert_eq!(code(c.record_gap(u64::MAX)), error::POSITION_OVERFLOW);
    assert_eq!(code(c.record_gap(u64::MAX - 2)), error::POSITION_OVERFLOW);
    assert_eq!(c.length(), 3);
    assert!(c.reader().snapshot().gaps().is_empty());
    assert!(!c.reader().snapshot().status().closed);
    assert_eq!(c.reader().snapshot().status().lost_positions, 0);
}

#[test]
fn positions_near_the_largest_value_do_not_overflow() {
    let mut c = capture(1);
    c.append(0, &[0, 1, 0, 1, 0]).unwrap();
    // The largest gap that still fits leaves room for exactly three more positions.
    c.record_gap(u64::MAX - 5 - 3).unwrap();
    assert_eq!(c.length(), u64::MAX - 3);
    c.append(u64::MAX - 3, &[1, 0, 1]).unwrap();
    assert_eq!(c.length(), u64::MAX);
    // No more room: appending and recording gaps fail cleanly.
    assert_eq!(code(c.append(u64::MAX, &[1])), error::POSITION_OVERFLOW);
    assert_eq!(code(c.record_gap(1)), error::POSITION_OVERFLOW);
    let snap = c.reader().snapshot();
    assert_eq!(snap.length(), u64::MAX);
    assert_eq!(snap.level_at(0, u64::MAX - 1).unwrap(), LevelState::High);
    assert_eq!(snap.level_at(0, u64::MAX).unwrap(), LevelState::Missing);
    assert_eq!(
        snap.level_at(0, 100).unwrap(),
        LevelState::Missing,
        "inside the gap"
    );
    assert!(snap.viewport(0, u64::MAX - 10, u64::MAX, 10).is_ok());
    assert!(snap.viewport(0, 0, u64::MAX, 100).is_ok());
}

#[test]
fn a_capture_can_be_incomplete_for_both_reasons() {
    let mut c = Capture::new(CaptureConfig {
        channels: 1,
        sample_rate_hz: 1_000_000,
        limits: Limits {
            memory_budget_bytes: 1024 * 1024,
            ..Limits::default()
        },
    })
    .unwrap();
    c.append(0, &[0, 1]).unwrap();
    c.record_gap(100).unwrap();
    let mut start = c.length();
    loop {
        let chunk: Vec<u32> = (0..4096_u64).map(|i| ((start + i) & 1) as u32).collect();
        if c.append(start, &chunk).is_err() {
            break;
        }
        start = c.length();
    }
    let status = c.close().unwrap();
    assert!(status.stopped_by_limit, "{status:?}");
    assert_eq!(status.lost_positions, 100);
    assert!(status.closed && !status.complete);
}

#[test]
fn random_interleavings_of_chunks_and_gaps_match_the_reference() {
    let mut rng = Rng::new(0x6A9);
    for round in 0..30 {
        let channels = [1_u8, 3, 8][round % 3];
        let mut c = capture(channels);
        let mut reference = ReferenceCapture::new(channels);
        for _ in 0..rng.range(2, 12) {
            if rng.below(3) == 0 {
                let n = rng.range(1, 60);
                c.record_gap(n).unwrap();
                reference.push_gap(n);
            } else {
                let len = rng.range(1, 400) as usize;
                let words = common::random(len, channels, [20, 150, 600][round % 3], &mut rng);
                c.append(c.length(), &words).unwrap();
                reference.push_chunk(&words);
            }
        }
        let snap = c.reader().snapshot();
        assert_eq!(snap.length(), reference.len(), "round {round}");
        assert_eq!(snap.gaps(), reference.gaps().as_slice(), "round {round}");
        let lost: u64 = reference.gaps().iter().map(|g| g.length).sum();
        assert_eq!(snap.status().lost_positions, lost);
        assert!(!snap.status().complete);

        for ch in 0..channels {
            for p in 0..reference.len() + 3 {
                assert_eq!(
                    snap.level_at(ch, p).unwrap(),
                    reference.level(ch, p),
                    "round {round} ch {ch} pos {p}"
                );
            }
            let len = reference.len();
            for _ in 0..10 {
                let a = rng.below(len + 2);
                let b = a + 1 + rng.below(len + 5);
                let got: Vec<Edge> = snap.transitions(ch, a, b).unwrap().collect();
                assert_eq!(
                    got,
                    reference.transitions(ch, a, b),
                    "round {round} ch {ch} ({a}, {b}]"
                );
                let pixels = [1_u32, 3, 7, 50, 300][rng.below(5) as usize];
                let got = snap.viewport(ch, a, b, pixels).unwrap();
                assert_eq!(
                    got.pixels,
                    reference.viewport(ch, a, b, pixels),
                    "round {round} ch {ch} window [{a}, {b}) pixels {pixels}"
                );
            }
        }
    }
}
