//! A cropped copy of a capture (`specs/015-markers-rework`, research R6).
//!
//! The copy of the positions `start..end` must answer every query like the source does at the
//! shifted positions; gaps are clipped and shifted; a budget that is too small is refused before
//! anything is copied; a cancel builds nothing. Expected values of the hand-made cases are
//! written out by hand.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::panic
)]

mod common;

use std::cell::Cell;

use common::Rng;
use la_storage::{
    Capture, CaptureConfig, CropControl, Gap, LevelState, Limits, Snapshot, crop, error,
};

fn config(channels: u8) -> CaptureConfig {
    CaptureConfig {
        channels,
        sample_rate_hz: 1_000_000,
        limits: Limits::default(),
    }
}

/// A capture whose channel `c` starts at `initial[c]` and flips at every position of
/// `flips[c]`, `length` positions long, with the given gaps (`(start, length)`, sorted, inside the
/// length). The positions of a gap hold no samples.
fn build(
    initial: &[bool],
    flips: &[&[u64]],
    length: u64,
    gaps: &[(u64, u64)],
    close: u8,
) -> Snapshot {
    let channels = u8::try_from(initial.len()).unwrap();
    let mut capture = Capture::new(config(channels)).unwrap();
    let word_at = |p: u64| -> u32 {
        let mut word = 0_u32;
        for (c, start) in initial.iter().enumerate() {
            let flipped = flips[c].iter().filter(|f| **f <= p).count() % 2 == 1;
            if *start ^ flipped {
                word |= 1 << c;
            }
        }
        word
    };
    let mut at = 0_u64;
    let stored = |capture: &mut Capture, from: u64, to: u64| {
        if to > from {
            let words: Vec<u32> = (from..to).map(word_at).collect();
            capture.append(capture.length(), &words).unwrap();
        }
    };
    for (start, len) in gaps {
        stored(&mut capture, at, *start);
        capture.record_gap(*len).unwrap();
        at = start + len;
    }
    stored(&mut capture, at, length);
    match close {
        0 => {
            capture.close().unwrap();
        }
        1 => {
            capture.close_interrupted().unwrap();
        }
        _ => {}
    }
    capture.reader().snapshot()
}

fn all(snapshot: &Snapshot, channel: u8) -> Vec<(u64, bool)> {
    let len = snapshot.length();
    if len < 2 || snapshot.transition_count(channel).unwrap() == 0 {
        return Vec::new();
    }
    snapshot
        .transitions(channel, 0, len)
        .unwrap()
        .map(|e| (e.position, e.rising))
        .collect()
}

fn run(snapshot: &Snapshot, start: u64, end: u64) -> la_core::Result<Snapshot> {
    crop(snapshot, start, end, Limits::default(), &()).map(|c| c.reader().snapshot())
}

#[test]
fn a_cropped_range_has_the_levels_and_transitions_written_by_hand() {
    // Channel 0 starts low and flips at 10, 11, 29, 30; channel 1 starts low and flips at 5, 20;
    // channel 2 starts high and never flips.
    let source = build(
        &[false, false, true],
        &[&[10, 11, 29, 30], &[5, 20], &[]],
        40,
        &[],
        0,
    );
    let copy = run(&source, 10, 30).unwrap();
    assert_eq!(copy.length(), 20);
    assert_eq!(copy.channels(), 3);
    assert_eq!(copy.sample_rate_hz(), 1_000_000);
    // Channel 0: high at 10 (the flip at 10 is folded into the initial level), low at 11, high
    // at 29; the flip at 30 is outside.
    assert!(copy.initial_high(0).unwrap());
    assert_eq!(all(&copy, 0), vec![(1, false), (19, true)]);
    // Channel 1: low, high from 5, low again from 20: high at 10, falling at 20 (position 10).
    assert!(copy.initial_high(1).unwrap());
    assert_eq!(all(&copy, 1), vec![(10, false)]);
    // Channel 2 never changes.
    assert!(copy.initial_high(2).unwrap());
    assert_eq!(copy.transition_count(2).unwrap(), 0);
    assert!(copy.status().closed);
}

#[test]
fn gaps_are_clipped_and_shifted() {
    // A gap 5..15 inside the range 10..30 becomes 0..5.
    let source = build(&[false], &[&[20]], 40, &[(5, 10)], 0);
    let copy = run(&source, 10, 30).unwrap();
    assert_eq!(
        copy.gaps(),
        &[Gap {
            start: 0,
            length: 5
        }]
    );
    assert_eq!(copy.level_at(0, 2).unwrap(), LevelState::Missing);
    assert_eq!(copy.level_at(0, 5).unwrap(), LevelState::Low);
    assert_eq!(all(&copy, 0), vec![(10, true)]);

    // A gap wholly outside the range vanishes.
    let source = build(&[false], &[&[]], 40, &[(0, 3)], 0);
    let copy = run(&source, 10, 30).unwrap();
    assert!(copy.gaps().is_empty());
    assert_eq!(copy.status().lost_positions, 0);

    // A gap that covers the whole range becomes the whole copy.
    let source = build(&[false], &[&[]], 40, &[(5, 30)], 0);
    let copy = run(&source, 10, 30).unwrap();
    assert_eq!(
        copy.gaps(),
        &[Gap {
            start: 0,
            length: 20
        }]
    );
    assert_eq!(copy.transition_count(0).unwrap(), 0);
    assert!(!copy.initial_high(0).unwrap());

    // A gap across the end of the range is cut at it.
    let source = build(&[false], &[&[]], 40, &[(25, 10)], 0);
    let copy = run(&source, 10, 30).unwrap();
    assert_eq!(
        copy.gaps(),
        &[Gap {
            start: 15,
            length: 5
        }]
    );
}

#[test]
fn the_flags_are_kept() {
    let source = build(&[false], &[&[3]], 20, &[], 1);
    assert!(source.status().interrupted);
    let copy = run(&source, 2, 10).unwrap();
    assert!(copy.status().interrupted);
    assert_eq!(copy.sample_rate_hz(), source.sample_rate_hz());
}

#[test]
fn the_whole_range_is_a_copy_of_the_source() {
    let source = build(&[false, true], &[&[3, 4, 9], &[1]], 30, &[(12, 3)], 0);
    let copy = run(&source, 0, source.length()).unwrap();
    assert_eq!(copy.length(), source.length());
    assert_eq!(copy.gaps(), source.gaps());
    for c in 0..2 {
        assert_eq!(all(&copy, c), all(&source, c));
        assert_eq!(
            copy.initial_high(c).unwrap(),
            source.initial_high(c).unwrap()
        );
        for p in 0..source.length() + 2 {
            assert_eq!(copy.level_at(c, p).unwrap(), source.level_at(c, p).unwrap());
        }
    }
}

#[test]
fn a_bad_range_is_an_error_not_a_panic() {
    let source = build(&[false], &[&[3]], 20, &[], 0);
    for (start, end) in [
        (5, 5),
        (7, 3),
        (0, 21),
        (20, 30),
        (u64::MAX, 0),
        (0, u64::MAX),
    ] {
        let err = run(&source, start, end).unwrap_err();
        assert_eq!(err.code(), error::INVALID_RESTORE, "{start}..{end}");
    }
    let empty = Capture::new(config(1)).unwrap();
    let mut empty = empty;
    empty.close().unwrap();
    let err = crop(&empty.reader().snapshot(), 0, 1, Limits::default(), &()).unwrap_err();
    assert_eq!(err.code(), error::INVALID_RESTORE);
}

#[test]
fn a_budget_that_is_too_small_is_refused_before_copying() {
    // Four channels flipping at every sample: far more than 1 MiB of transitions.
    let mut capture = Capture::new(config(4)).unwrap();
    let words = common::alternating(120_000, 4);
    common::feed(&mut capture, &words, &[60_000, 60_000]);
    capture.close().unwrap();
    let source = capture.reader().snapshot();
    let tiny = Limits {
        memory_budget_bytes: 1024 * 1024,
        ..Limits::default()
    };
    let polls = Cell::new(0_u32);
    struct Count<'a>(&'a Cell<u32>);
    impl CropControl for Count<'_> {
        fn cancelled(&self) -> bool {
            false
        }
        fn progress(&self, _done: u64, _total: u64) {
            self.0.set(self.0.get() + 1);
        }
    }
    let control = Count(&polls);
    let err = crop(&source, 0, source.length(), tiny, &control).unwrap_err();
    assert_eq!(err.code(), error::MEMORY_LIMIT);
    // The refusal came from the budget check, before any transition was pushed: only the counting
    // pass ran, which reports once per channel (the pushing pass would report more).
    assert!(polls.get() <= 4, "reported progress {} times", polls.get());
}

#[test]
fn a_cancel_builds_nothing() {
    let source = build(&[false], &[&[3, 4, 5]], 20, &[], 0);
    struct Stop;
    impl CropControl for Stop {
        fn cancelled(&self) -> bool {
            true
        }
        fn progress(&self, _done: u64, _total: u64) {}
    }
    let err = crop(&source, 0, 10, Limits::default(), &Stop).unwrap_err();
    assert_eq!(err.code(), error::CANCELLED);
}

#[test]
fn random_captures_and_ranges_agree_with_the_source_everywhere() {
    let mut rng = Rng::new(0xC0FFEE);
    for round in 0..60 {
        let channels = u8::try_from(rng.range(1, 5)).unwrap();
        let mut capture = Capture::new(config(channels)).unwrap();
        let parts = rng.range(1, 4);
        for _ in 0..parts {
            if rng.chance_per_mille(400) {
                capture.record_gap(rng.range(1, 40)).unwrap();
            }
            let len = usize::try_from(rng.range(1, 300)).unwrap();
            let words = common::random(len, channels, rng.range(5, 400), &mut rng);
            capture.append(capture.length(), &words).unwrap();
        }
        capture.close().unwrap();
        let source = capture.reader().snapshot();
        let length = source.length();
        if length < 2 {
            continue;
        }
        for _ in 0..10 {
            let a = rng.below(length);
            let b = rng.range(a + 1, length);
            let copy = run(&source, a, b).unwrap();
            assert_eq!(copy.length(), b - a, "round {round}");
            for c in 0..channels {
                for p in a..b {
                    assert_eq!(
                        copy.level_at(c, p - a).unwrap(),
                        source.level_at(c, p).unwrap(),
                        "round {round} channel {c} range {a}..{b} position {p}"
                    );
                }
                // Past the end the copy has no data.
                assert_eq!(copy.level_at(c, b - a).unwrap(), LevelState::Missing);
            }
            // The gaps are exactly the source gaps clipped to the range.
            let expected: Vec<Gap> = source
                .gaps()
                .iter()
                .filter_map(|g| {
                    let s = g.start.max(a);
                    let e = g.end().unwrap().min(b);
                    (s < e).then(|| Gap {
                        start: s - a,
                        length: e - s,
                    })
                })
                .collect();
            assert_eq!(copy.gaps(), expected.as_slice(), "round {round} {a}..{b}");
        }
    }
}
