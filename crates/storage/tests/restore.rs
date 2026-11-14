//! Rebuilding a closed capture from stored transitions (spec of item 003, data model).
//!
//! A capture restored from the transitions of a recorded one must answer every query exactly
//! like the original, and every rule of the data model must be enforced.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::panic
)]

mod common;

use common::Rng;
use la_storage::{
    Capture, CaptureConfig, CaptureRestorer, Gap, Limits, RestoredStatus, Snapshot, error,
};

/// One recorded part: an optional gap before it and the number of samples.
type Part = (Option<u64>, usize);

fn config(channels: u8, limits: Limits) -> CaptureConfig {
    CaptureConfig {
        channels,
        sample_rate_hz: 2_500_000,
        limits,
    }
}

/// Records a capture: `parts` are chunks of random words separated by gaps of the given lengths
/// (`None` = no gap before the chunk). Returns the closed capture's snapshot.
fn record(channels: u8, per_mille: u64, parts: &[Part], close: u8, seed: u64) -> Snapshot {
    let mut capture = Capture::new(config(channels, Limits::default())).unwrap();
    let mut rng = Rng::new(seed);
    for (gap, len) in parts {
        if let Some(gap) = gap {
            capture.record_gap(*gap).unwrap();
        }
        if *len > 0 {
            let words = common::random(*len, channels, per_mille, &mut rng);
            capture.append(capture.length(), &words).unwrap();
        }
    }
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

/// Rebuilds a capture from everything a snapshot reports.
fn restore(snapshot: &Snapshot, limits: Limits) -> la_core::Result<Capture> {
    let status = snapshot.status();
    let mut restorer = CaptureRestorer::new(
        CaptureConfig {
            channels: snapshot.channels(),
            sample_rate_hz: snapshot.sample_rate_hz(),
            limits,
        },
        snapshot.length(),
        RestoredStatus {
            stopped_by_limit: status.stopped_by_limit,
            interrupted: status.interrupted || !status.closed,
        },
        snapshot.gaps().to_vec(),
    )?;
    for channel in 0..snapshot.channels() {
        let count = snapshot.transition_count(channel)?;
        restorer.begin_channel(channel, snapshot.initial_high(channel)?, count)?;
        if count > 0 {
            for edge in snapshot.transitions(channel, 0, snapshot.length())? {
                restorer.push_transition(edge.position)?;
            }
        }
    }
    restorer.finish()
}

/// Asserts that two snapshots answer every query identically.
fn assert_same(a: &Snapshot, b: &Snapshot) {
    assert_eq!(a.length(), b.length());
    assert_eq!(a.channels(), b.channels());
    assert_eq!(a.sample_rate_hz(), b.sample_rate_hz());
    assert_eq!(a.gaps(), b.gaps());
    let (sa, sb) = (a.status(), b.status());
    assert_eq!(sa.lost_positions, sb.lost_positions);
    assert_eq!(sa.stopped_by_limit, sb.stopped_by_limit);
    let mut rng = Rng::new(99);
    for ch in 0..a.channels() {
        assert_eq!(
            a.transition_count(ch).unwrap(),
            b.transition_count(ch).unwrap()
        );
        let len = a.length();
        if len >= 2 && a.transition_count(ch).unwrap() > 0 {
            let ea: Vec<_> = a.transitions(ch, 0, len).unwrap().collect();
            let eb: Vec<_> = b.transitions(ch, 0, len).unwrap().collect();
            assert_eq!(ea, eb, "channel {ch}");
            assert_eq!(a.initial_high(ch).unwrap(), b.initial_high(ch).unwrap());
        }
        for _ in 0..200 {
            let p = rng.below(len + 5);
            assert_eq!(
                a.level_at(ch, p).unwrap(),
                b.level_at(ch, p).unwrap(),
                "channel {ch} position {p}"
            );
        }
        if len >= 8 {
            for (start, end, px) in [(0, len, 97_u32), (len / 3, len - 1, 33), (0, 8, 4)] {
                let va = a.viewport(ch, start, end, px).unwrap();
                let vb = b.viewport(ch, start, end, px).unwrap();
                assert_eq!(va.pixels, vb.pixels, "viewport of channel {ch}");
                assert_eq!(va.stats, vb.stats, "probe counts of channel {ch}");
            }
        }
    }
}

#[test]
fn a_restored_capture_answers_every_query_like_the_recorded_one() {
    let cases: Vec<(u8, u64, Vec<Part>, u8)> = vec![
        (1, 20, vec![(None, 5_000)], 0),
        (8, 50, vec![(None, 40_000)], 0),
        (32, 30, vec![(None, 65_000), (None, 1_000)], 0),
        (
            4,
            100,
            vec![(None, 3_000), (Some(50), 3_000), (Some(1), 10)],
            0,
        ),
        (3, 80, vec![(Some(25), 4_000), (Some(7), 2_000)], 1),
        (5, 1, vec![(None, 60_000)], 0),
        (2, 400, vec![(None, 2)], 0),
        (6, 40, vec![(None, 9_000), (Some(500), 0)], 2),
    ];
    for (seed, (channels, per_mille, parts, close)) in cases.into_iter().enumerate() {
        let recorded = record(channels, per_mille, &parts, close, seed as u64 + 1);
        let restored = restore(&recorded, Limits::default()).unwrap();
        let restored = restored.reader().snapshot();
        assert_same(&recorded, &restored);
        let status = restored.status();
        assert!(status.closed, "a restored capture is closed");
        assert_eq!(
            status.complete,
            status.lost_positions == 0 && !status.stopped_by_limit && !status.interrupted
        );
        if close == 1 || close == 2 {
            assert!(status.interrupted, "case {seed}: flags carry over");
            assert!(!status.complete);
        }
    }
}

#[test]
fn an_empty_and_a_one_sample_capture_restore() {
    for len in [0_usize, 1] {
        let recorded = record(3, 50, &[(None, len)], 0, 5);
        let restored = restore(&recorded, Limits::default()).unwrap();
        assert_same(&recorded, &restored.reader().snapshot());
        assert_eq!(restored.length(), len as u64);
    }
}

#[test]
fn a_capture_made_of_a_single_gap_restores_and_keeps_its_gap() {
    let recorded = {
        let mut c = Capture::new(config(2, Limits::default())).unwrap();
        c.record_gap(1_000).unwrap();
        c.close_interrupted().unwrap();
        c.reader().snapshot()
    };
    let restored = restore(&recorded, Limits::default()).unwrap();
    let snap = restored.reader().snapshot();
    assert_eq!(
        snap.gaps(),
        &[Gap {
            start: 0,
            length: 1_000
        }]
    );
    assert_eq!(snap.length(), 1_000);
    assert!(!snap.status().complete);
}

#[test]
fn a_restored_capture_is_closed_and_rejects_appends() {
    let recorded = record(2, 50, &[(None, 100)], 0, 1);
    let mut restored = restore(&recorded, Limits::default()).unwrap();
    let e = restored.append(restored.length(), &[1]).unwrap_err();
    assert_eq!(e.code(), error::CLOSED);
    assert_eq!(
        restored.memory_used(),
        {
            // The recorder's own accounting for the same data.
            let mut c = Capture::new(config(2, Limits::default())).unwrap();
            let words = common::random(100, 2, 50, &mut Rng::new(1));
            c.append(0, &words).unwrap();
            c.memory_used()
        },
        "restored and recorded captures account the same memory"
    );
}

// ---- every rule of the data model -----------------------------------------------------------

fn restorer(
    channels: u8,
    length: u64,
    status: RestoredStatus,
    gaps: Vec<Gap>,
) -> la_core::Result<CaptureRestorer> {
    CaptureRestorer::new(config(channels, Limits::default()), length, status, gaps)
}

fn plain() -> RestoredStatus {
    RestoredStatus::default()
}

fn gap(start: u64, length: u64) -> Gap {
    Gap { start, length }
}

fn code(result: la_core::Result<impl std::fmt::Debug>) -> &'static str {
    result.unwrap_err().code()
}

/// Runs a one-channel restore with the given transitions and returns the first error code.
fn one_channel(
    length: u64,
    gaps: Vec<Gap>,
    initial_high: bool,
    declared: u64,
    positions: &[u64],
) -> Result<(), &'static str> {
    let mut r = restorer(1, length, plain(), gaps).map_err(|e| e.code())?;
    r.begin_channel(0, initial_high, declared)
        .map_err(|e| e.code())?;
    for p in positions {
        r.push_transition(*p).map_err(|e| e.code())?;
    }
    r.finish().map(|_| ()).map_err(|e| e.code())
}

#[test]
fn valid_transitions_are_accepted_including_one_right_after_a_gap() {
    assert_eq!(one_channel(100, vec![], false, 3, &[1, 50, 99]), Ok(()));
    // A transition at the first position after a gap is legal.
    assert_eq!(
        one_channel(100, vec![gap(10, 20)], true, 2, &[5, 30]),
        Ok(())
    );
}

#[test]
fn transitions_that_break_the_rules_are_rejected() {
    let invalid = error::INVALID_RESTORE;
    // Position 0, not increasing, equal, at or beyond the length.
    assert_eq!(one_channel(100, vec![], false, 1, &[0]), Err(invalid));
    assert_eq!(one_channel(100, vec![], false, 2, &[10, 5]), Err(invalid));
    assert_eq!(one_channel(100, vec![], false, 2, &[10, 10]), Err(invalid));
    assert_eq!(one_channel(100, vec![], false, 1, &[100]), Err(invalid));
    assert_eq!(
        one_channel(100, vec![], false, 1, &[u64::MAX]),
        Err(invalid)
    );
    // Inside a gap (first, middle and last position of the gap).
    for p in [10, 15, 29] {
        assert_eq!(
            one_channel(100, vec![gap(10, 20)], false, 1, &[p]),
            Err(invalid),
            "position {p} is inside the gap"
        );
    }
    // Before the first stored position of a capture that starts with a gap.
    assert_eq!(
        one_channel(100, vec![gap(0, 20)], false, 1, &[5]),
        Err(invalid)
    );
    assert_eq!(
        one_channel(100, vec![gap(0, 20)], false, 1, &[20]),
        Err(invalid),
        "the first stored sample has no earlier sample to differ from"
    );
    assert_eq!(one_channel(100, vec![gap(0, 20)], false, 1, &[21]), Ok(()));
    // More transitions than the declared count, fewer than the declared count.
    assert_eq!(one_channel(100, vec![], false, 1, &[5, 6]), Err(invalid));
    assert_eq!(one_channel(100, vec![], false, 3, &[5, 6]), Err(invalid));
}

#[test]
fn a_declared_count_that_cannot_fit_the_length_is_rejected_before_any_transition() {
    let mut r = restorer(1, 10, plain(), vec![]).unwrap();
    // At most length - 1 transitions fit.
    assert_eq!(code(r.begin_channel(0, false, 10)), error::INVALID_RESTORE);
    let mut r = restorer(1, 10, plain(), vec![]).unwrap();
    assert_eq!(
        code(r.begin_channel(0, false, u64::MAX)),
        error::INVALID_RESTORE
    );
}

#[test]
fn gaps_that_break_the_rules_are_rejected() {
    let invalid = error::INVALID_RESTORE;
    assert_eq!(code(restorer(1, 100, plain(), vec![gap(5, 0)])), invalid);
    assert_eq!(code(restorer(1, 100, plain(), vec![gap(90, 11)])), invalid);
    assert_eq!(
        code(restorer(1, 100, plain(), vec![gap(u64::MAX, 2)])),
        invalid
    );
    // Overlapping, adjacent and unsorted.
    assert_eq!(
        code(restorer(1, 100, plain(), vec![gap(10, 10), gap(15, 10)])),
        invalid
    );
    assert_eq!(
        code(restorer(1, 100, plain(), vec![gap(10, 10), gap(20, 5)])),
        invalid,
        "adjacent gaps are merged by the recorder, so an adjacent pair is damage"
    );
    assert_eq!(
        code(restorer(1, 100, plain(), vec![gap(50, 5), gap(10, 5)])),
        invalid
    );
    // A gap in an empty capture.
    assert_eq!(code(restorer(1, 0, plain(), vec![gap(0, 1)])), invalid);
    // Valid: separated gaps, a gap at the very start and at the very end.
    assert!(restorer(1, 100, plain(), vec![gap(0, 5), gap(10, 5), gap(95, 5)]).is_ok());
}

#[test]
fn a_channel_without_stored_samples_must_start_low_and_have_no_transitions() {
    let invalid = error::INVALID_RESTORE;
    // Empty capture.
    assert_eq!(one_channel(0, vec![], true, 0, &[]), Err(invalid));
    assert_eq!(one_channel(0, vec![], false, 0, &[]), Ok(()));
    // A capture that is one big gap.
    assert_eq!(
        one_channel(50, vec![gap(0, 50)], true, 0, &[]),
        Err(invalid)
    );
    assert_eq!(one_channel(50, vec![gap(0, 50)], false, 0, &[]), Ok(()));
}

#[test]
fn channels_must_be_declared_in_order_and_completely() {
    let invalid = error::INVALID_RESTORE;
    // Pushing before begin_channel.
    let mut r = restorer(2, 100, plain(), vec![]).unwrap();
    assert_eq!(code(r.push_transition(5)), invalid);
    // Finishing before every channel was declared.
    let mut r = restorer(2, 100, plain(), vec![]).unwrap();
    r.begin_channel(0, false, 0).unwrap();
    assert_eq!(code(r.finish()), invalid);
    // Starting the next channel before the current one is complete.
    let mut r = restorer(2, 100, plain(), vec![]).unwrap();
    r.begin_channel(0, false, 2).unwrap();
    r.push_transition(5).unwrap();
    assert_eq!(code(r.begin_channel(1, false, 0)), invalid);
    // More channels than the capture has.
    let mut r = restorer(1, 100, plain(), vec![]).unwrap();
    r.begin_channel(0, false, 0).unwrap();
    assert_eq!(code(r.begin_channel(1, false, 0)), invalid);
}

#[test]
fn an_invalid_configuration_is_rejected_with_the_storage_codes() {
    let bad_rate = CaptureConfig {
        channels: 1,
        sample_rate_hz: 0,
        limits: Limits::default(),
    };
    assert_eq!(
        CaptureRestorer::new(bad_rate, 10, plain(), vec![])
            .unwrap_err()
            .code(),
        error::INVALID_SAMPLE_RATE
    );
    assert_eq!(
        CaptureRestorer::new(config(0, Limits::default()), 10, plain(), vec![])
            .unwrap_err()
            .code(),
        error::INVALID_CHANNEL_COUNT
    );
}

#[test]
fn the_memory_budget_is_enforced_before_and_while_transitions_are_pushed() {
    let tiny = Limits {
        memory_budget_bytes: 1024 * 1024,
        ..Limits::default()
    };
    // The declared count alone needs more than 1 MiB: rejected before any push.
    let length = 10_000_000;
    let mut r = CaptureRestorer::new(config(1, tiny), length, plain(), vec![]).unwrap();
    assert_eq!(
        code(r.begin_channel(0, false, 5_000_000)),
        error::MEMORY_LIMIT
    );
    // A count that fits when declared but whose blocks grow past the budget as they are
    // built (offsets too far apart for one block) is caught while pushing.
    let mut r = CaptureRestorer::new(config(1, tiny), u64::MAX, plain(), vec![]).unwrap();
    let declared = 200_000;
    // 200,000 transitions cost about 800 KB + blocks: fits the estimate...
    r.begin_channel(0, false, declared).unwrap();
    let mut position = 1_u64;
    let mut failed = None;
    // ...but every transition 2^32 apart seals a block of one transition (4 + 32 bytes).
    for _ in 0..declared {
        match r.push_transition(position) {
            Ok(()) => position += 1 << 33,
            Err(e) => {
                failed = Some(e.code());
                break;
            }
        }
    }
    assert_eq!(failed, Some(error::MEMORY_LIMIT));
}

#[test]
fn a_rejected_restore_leaves_nothing_behind_and_the_flags_are_applied() {
    // A rejected restore returns an error and no capture; a second restorer is independent.
    assert!(one_channel(100, vec![], false, 1, &[0]).is_err());
    assert_eq!(one_channel(100, vec![], false, 1, &[3]), Ok(()));

    let status = RestoredStatus {
        stopped_by_limit: true,
        interrupted: true,
    };
    let mut r = restorer(1, 100, status, vec![gap(10, 5)]).unwrap();
    r.begin_channel(0, true, 1).unwrap();
    r.push_transition(50).unwrap();
    let capture = r.finish().unwrap();
    let s = capture.reader().snapshot().status();
    assert!(s.closed && s.stopped_by_limit && s.interrupted && !s.complete);
    assert_eq!(s.lost_positions, 5);
}

#[test]
fn accessors_report_the_initial_level_and_the_transition_count() {
    let mut c = Capture::new(config(2, Limits::default())).unwrap();
    // Channel 0 starts high and toggles twice; channel 1 starts low and never changes.
    c.append(0, &[0b01, 0b01, 0b00, 0b00, 0b01]).unwrap();
    c.close().unwrap();
    let snap = c.reader().snapshot();
    assert!(snap.initial_high(0).unwrap());
    assert!(!snap.initial_high(1).unwrap());
    assert_eq!(snap.transition_count(0).unwrap(), 2);
    assert_eq!(snap.transition_count(1).unwrap(), 0);
    assert_eq!(
        snap.initial_high(2).unwrap_err().code(),
        error::CHANNEL_OUT_OF_RANGE
    );
    assert_eq!(
        snap.transition_count(2).unwrap_err().code(),
        error::CHANNEL_OUT_OF_RANGE
    );
    // Counted across several sealed blocks and the open tail.
    let mut c = Capture::new(config(1, Limits::default())).unwrap();
    c.append(0, &common::alternating(5_000, 1)).unwrap();
    let snap = c.reader().snapshot();
    assert_eq!(snap.transition_count(0).unwrap(), 4_999);
}
