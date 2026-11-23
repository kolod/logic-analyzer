//! The accounting of lost samples on the board (feature 017, T032 and FR-013): the marks plus the samples
//! stored are the samples sampled, and what the board sends passes the protocol's own check.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::panic
)]

use la_fw_logic::LossTracker;
use la_proto::{LossMark, MAX_LOSS_MARKS, check_marks};

fn mark(position: u32, lost: u32) -> LossMark {
    LossMark { position, lost }
}

#[test]
fn a_capture_without_an_overrun_has_no_marks() {
    let mut t = LossTracker::new();
    t.stored(1_000);
    t.stored(500);
    assert!(t.marks().is_empty());
    assert_eq!((t.stored_total(), t.lost_total()), (1_500, 0));
}

#[test]
fn a_loss_is_a_mark_at_the_samples_stored_before_it() {
    let mut t = LossTracker::new();
    t.lost(5); // at the start
    t.stored(4);
    assert!(t.lost(100));
    t.stored(6);
    assert!(t.lost(2));
    t.stored(6);
    assert!(t.lost(5)); // after the last
    assert_eq!(
        t.marks(),
        [mark(0, 5), mark(4, 100), mark(10, 2), mark(16, 5)]
    );
    assert_eq!((t.stored_total(), t.lost_total()), (16, 112));
}

#[test]
fn losses_with_nothing_stored_between_them_are_one_mark() {
    let mut t = LossTracker::new();
    t.stored(3);
    t.lost(10);
    t.lost(20);
    t.lost(0); // nothing lost is nothing
    assert_eq!(t.marks(), [mark(3, 30)]);
    t.stored(1);
    t.lost(1);
    assert_eq!(t.marks(), [mark(3, 30), mark(4, 1)]);
}

#[test]
fn what_is_stored_and_lost_adds_up_and_passes_the_protocols_check() {
    let mut state = 11_u64;
    let mut next = move || {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        state
    };
    for _ in 0..300 {
        let mut t = LossTracker::new();
        let mut sampled = 0_u64;
        for _ in 0..(next() % 200) {
            let n = u32::try_from(next() % 1_000).unwrap();
            if next() % 3 == 0 {
                assert!(t.lost(n));
            } else {
                t.stored(n);
            }
            sampled += u64::from(n);
        }
        assert_eq!(u64::from(t.stored_total()) + t.lost_total(), sampled);
        assert_eq!(
            check_marks(t.marks(), t.stored_total(), sampled),
            Ok(t.lost_total())
        );
    }
}

#[test]
fn a_capture_that_needs_more_marks_than_the_protocol_allows_ends() {
    let mut t = LossTracker::new();
    for _ in 0..MAX_LOSS_MARKS {
        t.stored(1);
        assert!(t.lost(1));
    }
    assert!(!t.is_full());
    t.stored(1);
    assert!(!t.lost(1), "the 1025th mark does not fit");
    assert!(t.is_full());
    assert_eq!(t.marks().len(), MAX_LOSS_MARKS);
    // The run that did not fit is not counted, so the accounting still holds for what was sent.
    assert_eq!(t.lost_total(), MAX_LOSS_MARKS as u64);
    assert_eq!(
        check_marks(
            t.marks(),
            t.stored_total(),
            u64::from(t.stored_total()) + t.lost_total()
        ),
        Ok(t.lost_total())
    );
}
