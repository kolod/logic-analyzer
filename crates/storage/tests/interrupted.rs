//! A capture ended by a failure is flagged incomplete (spec of item 004, research R6).
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::panic
)]

mod common;

use common::ReferenceCapture;
use la_storage::{Capture, CaptureConfig, Limits, error};

fn capture() -> Capture {
    Capture::new(CaptureConfig {
        channels: 2,
        sample_rate_hz: 1_000_000,
        limits: Limits::default(),
    })
    .unwrap()
}

#[test]
fn closing_as_interrupted_flags_the_capture_incomplete_without_any_lost_position() {
    let mut c = capture();
    let words = common::random(5_000, 2, 100, &mut common::Rng::new(3));
    c.append(0, &words).unwrap();
    let status = c.close_interrupted().unwrap();
    assert!(status.closed);
    assert!(status.interrupted);
    assert!(!status.complete, "a failure must never read as complete");
    assert_eq!(status.lost_positions, 0);
    assert!(!status.stopped_by_limit);
    assert_eq!(c.reader().snapshot().status(), status);
}

#[test]
fn data_before_the_interruption_stays_readable_and_exact() {
    let mut c = capture();
    let words = common::random(3_000, 2, 200, &mut common::Rng::new(4));
    c.append(0, &words).unwrap();
    c.close_interrupted().unwrap();
    let mut reference = ReferenceCapture::new(2);
    reference.push_chunk(&words);
    let snap = c.reader().snapshot();
    assert_eq!(snap.length(), reference.len());
    for ch in 0..2 {
        for p in 0..reference.len() {
            assert_eq!(snap.level_at(ch, p).unwrap(), reference.level(ch, p));
        }
        let got: Vec<_> = snap.transitions(ch, 0, reference.len()).unwrap().collect();
        assert_eq!(got, reference.transitions(ch, 0, reference.len()));
    }
}

#[test]
fn nothing_can_be_added_after_an_interruption() {
    let mut c = capture();
    c.append(0, &[0, 1]).unwrap();
    c.close_interrupted().unwrap();
    assert_eq!(c.append(2, &[1]).unwrap_err().code(), error::CLOSED);
    assert_eq!(c.record_gap(3).unwrap_err().code(), error::CLOSED);
}

#[test]
fn closing_again_returns_the_same_interrupted_status() {
    let mut c = capture();
    c.append(0, &[0, 1]).unwrap();
    let first = c.close_interrupted().unwrap();
    assert_eq!(c.close().unwrap(), first);
    assert_eq!(c.close_interrupted().unwrap(), first);
}

#[test]
fn a_capture_that_was_closed_normally_cannot_be_interrupted_afterwards() {
    let mut c = capture();
    c.append(0, &[0, 1]).unwrap();
    let normal = c.close().unwrap();
    assert!(normal.complete && !normal.interrupted);
    let again = c.close_interrupted().unwrap();
    assert_eq!(again, normal, "closing twice changes nothing");
    assert!(c.reader().snapshot().status().complete);
}

#[test]
fn a_normal_close_never_sets_the_flag() {
    let mut c = capture();
    c.append(0, &[0, 1, 0]).unwrap();
    assert!(!c.reader().snapshot().status().interrupted);
    assert!(!c.close().unwrap().interrupted);
}

#[test]
fn interruption_combines_with_the_other_reasons() {
    // Interrupted after a gap.
    let mut c = capture();
    c.append(0, &[0, 1]).unwrap();
    c.record_gap(10).unwrap();
    let status = c.close_interrupted().unwrap();
    assert!(status.interrupted);
    assert_eq!(status.lost_positions, 10);
    assert!(!status.complete);

    // Interrupted after the memory budget stopped the capture.
    let mut c = Capture::new(CaptureConfig {
        channels: 1,
        sample_rate_hz: 1_000_000,
        limits: Limits {
            memory_budget_bytes: 1024 * 1024,
            ..Limits::default()
        },
    })
    .unwrap();
    let mut start = 0_u64;
    loop {
        let chunk: Vec<u32> = (0..4096_u64).map(|i| ((start + i) & 1) as u32).collect();
        if c.append(start, &chunk).is_err() {
            break;
        }
        start = c.length();
    }
    let status = c.close_interrupted().unwrap();
    assert!(status.interrupted && status.stopped_by_limit && !status.complete);
}
