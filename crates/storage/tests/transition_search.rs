//! Finding the next and the previous transition of a channel (spec 014, user story 4, task T031).
//!
//! The answers are compared with a scan of the list of all transitions.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::panic
)]

mod common;

use common::{Rng, feed, runs};
use la_storage::{Capture, CaptureConfig, Edge, Limits};

fn capture(channels: u8) -> Capture {
    Capture::new(CaptureConfig {
        channels,
        sample_rate_hz: 1_000_000,
        limits: Limits::default(),
    })
    .unwrap()
}

/// The first transition after `from` and the last one before `before`, by a scan.
fn scan_next(all: &[Edge], from: u64) -> Option<Edge> {
    all.iter().copied().find(|e| e.position > from)
}

fn scan_previous(all: &[Edge], before: u64) -> Option<Edge> {
    all.iter().copied().rev().find(|e| e.position < before)
}

#[test]
fn the_answers_equal_a_scan_on_random_captures() {
    let mut rng = Rng::new(0x014_u64);
    for case in 0..300_u64 {
        let channels = 1 + (case % 3) as u8;
        let length = rng.range(0, 3_000) as usize;
        let max_run = [1, 3, 20, 400][(case % 4) as usize];
        let mut c = capture(channels);
        let samples = runs(length, channels, max_run, &mut rng);
        // Some captures have a gap in the middle.
        let split = if case % 5 == 0 && length > 10 {
            rng.range(1, length as u64 - 1) as usize
        } else {
            length
        };
        feed(&mut c, &samples[..split], &[7, 200, 1, 64]);
        if split < length {
            c.record_gap(rng.range(1, 50)).unwrap();
            feed(&mut c, &samples[split..], &[13, 300]);
        }
        let snap = c.reader().snapshot();
        let total = snap.length();
        for channel in 0..channels {
            let all: Vec<Edge> = if total == 0 {
                Vec::new()
            } else {
                snap.transitions(channel, 0, total).unwrap().collect()
            };
            let mut positions = vec![0, 1, total.saturating_sub(1), total, total + 5];
            for e in &all {
                positions.extend([e.position.saturating_sub(1), e.position, e.position + 1]);
            }
            for p in positions {
                assert_eq!(
                    snap.next_transition(channel, p).unwrap(),
                    scan_next(&all, p),
                    "case {case}, channel {channel}, next after {p}"
                );
                assert_eq!(
                    snap.previous_transition(channel, p).unwrap(),
                    scan_previous(&all, p),
                    "case {case}, channel {channel}, previous before {p}"
                );
            }
        }
    }
}

#[test]
fn a_channel_that_never_changes_has_no_transition_either_way() {
    let mut c = capture(2);
    c.append(0, &[0; 500]).unwrap();
    let snap = c.reader().snapshot();
    for p in [0, 1, 250, 499, 500, 10_000] {
        assert_eq!(snap.next_transition(0, p).unwrap(), None);
        assert_eq!(snap.previous_transition(0, p).unwrap(), None);
    }
    let empty = capture(1).reader().snapshot();
    assert_eq!(empty.next_transition(0, 0).unwrap(), None);
    assert_eq!(empty.previous_transition(0, 0).unwrap(), None);
}

#[test]
fn the_first_and_the_last_possible_transitions_are_found() {
    // Low at 0, high from 1 (a transition at position 1), low again at the last sample.
    let mut c = capture(1);
    let mut words = vec![1_u32; 100];
    words[0] = 0;
    words[99] = 0;
    c.append(0, &words).unwrap();
    let snap = c.reader().snapshot();
    assert_eq!(
        snap.next_transition(0, 0).unwrap(),
        Some(Edge {
            position: 1,
            rising: true
        })
    );
    assert_eq!(
        snap.next_transition(0, 1).unwrap(),
        Some(Edge {
            position: 99,
            rising: false
        })
    );
    assert_eq!(snap.next_transition(0, 99).unwrap(), None);
    assert_eq!(
        snap.previous_transition(0, 100).unwrap(),
        Some(Edge {
            position: 99,
            rising: false
        })
    );
    assert_eq!(
        snap.previous_transition(0, 99).unwrap(),
        Some(Edge {
            position: 1,
            rising: true
        })
    );
    assert_eq!(snap.previous_transition(0, 1).unwrap(), None);
    assert_eq!(snap.previous_transition(0, 0).unwrap(), None);
}

#[test]
fn a_channel_that_does_not_exist_is_an_error() {
    let mut c = capture(2);
    c.append(0, &[0, 1, 0]).unwrap();
    let snap = c.reader().snapshot();
    let next = snap.next_transition(2, 0).unwrap_err();
    assert_eq!(next.code(), "storage.channel_out_of_range");
    let previous = snap.previous_transition(200, 5).unwrap_err();
    assert_eq!(previous.code(), "storage.channel_out_of_range");
}

/// The cost does not depend on the length: a capture of 100,000,000 samples answers a thousand
/// queries in far less than 50 ms each (a generous limit; run in release builds).
#[test]
#[ignore = "builds a capture of 100,000,000 samples; run with --release -- --ignored"]
fn a_huge_capture_answers_in_logarithmic_time() {
    const LENGTH: u64 = 100_000_000;
    let mut c = capture(1);
    let chunk = 50_000_u64;
    let mut level = 0_u32;
    for index in 0..LENGTH / chunk {
        // A few edges spread over the capture: the level flips every 700th chunk.
        if index % 700 == 0 {
            level ^= 1;
        }
        let words = vec![level; usize::try_from(chunk).unwrap()];
        c.append(c.length(), &words).unwrap();
    }
    let snap = c.reader().snapshot();
    let mut rng = Rng::new(9);
    let started = std::time::Instant::now();
    for _ in 0..1_000 {
        let p = rng.below(LENGTH);
        let next = snap.next_transition(0, p).unwrap();
        let previous = snap.previous_transition(0, p).unwrap();
        assert!(next.is_none_or(|e| e.position > p));
        assert!(previous.is_none_or(|e| e.position < p));
    }
    assert!(
        started.elapsed() < std::time::Duration::from_millis(50),
        "{:?}",
        started.elapsed()
    );
}
