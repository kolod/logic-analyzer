//! The head of the data of a capture and its loss marks (feature 017, FR-013): every field at its offset, the
//! rules the marks must follow, and the accounting of stored plus lost samples.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::panic
)]

use la_proto::{
    DataError, DataHeader, Encoding, HEADER_LEN, LossMark, MARK_LEN, MAX_LOSS_MARKS, check_marks,
};

#[test]
fn the_head_is_seven_bytes_with_every_field_at_its_offset() {
    assert_eq!(HEADER_LEN, 7);
    let head = DataHeader {
        encoding: Encoding::Compact,
        samples: 0x0403_0201,
        marks: 0x0105,
    };
    assert_eq!(head.to_bytes(), [1, 0x01, 0x02, 0x03, 0x04, 0x05, 0x01]);
    assert_eq!(DataHeader::from_bytes(head.to_bytes()), Ok(head));
    let raw = DataHeader {
        encoding: Encoding::Raw,
        samples: 16,
        marks: 0,
    };
    assert_eq!(raw.to_bytes(), [0, 16, 0, 0, 0, 0, 0]);
}

#[test]
fn a_bad_encoding_or_too_many_marks_is_refused() {
    assert_eq!(
        DataHeader::from_bytes([2, 0, 0, 0, 0, 0, 0]),
        Err(DataError::BadEncoding)
    );
    let most = u16::try_from(MAX_LOSS_MARKS).unwrap().to_le_bytes();
    assert!(DataHeader::from_bytes([0, 0, 0, 0, 0, most[0], most[1]]).is_ok());
    let more = u16::try_from(MAX_LOSS_MARKS + 1).unwrap().to_le_bytes();
    assert_eq!(
        DataHeader::from_bytes([0, 0, 0, 0, 0, more[0], more[1]]),
        Err(DataError::TooManyMarks)
    );
}

#[test]
fn a_mark_is_the_position_then_the_count() {
    assert_eq!(MARK_LEN, 8);
    let mark = LossMark {
        position: 0x0403_0201,
        lost: 0x0807_0605,
    };
    assert_eq!(mark.to_bytes(), [1, 2, 3, 4, 5, 6, 7, 8]);
    assert_eq!(LossMark::from_bytes(mark.to_bytes()), mark);
}

fn mark(position: u32, lost: u32) -> LossMark {
    LossMark { position, lost }
}

#[test]
fn marks_that_follow_the_rules_give_the_samples_lost() {
    assert_eq!(check_marks(&[], 100, 100), Ok(0));
    // A loss at the start, in the middle and after the last stored sample.
    let marks = [mark(0, 5), mark(40, 10), mark(100, 3)];
    assert_eq!(check_marks(&marks, 100, 118), Ok(18));
    // Stored plus lost may be less than requested (the capture ended early) but never more.
    assert_eq!(check_marks(&marks, 100, 1_000), Ok(18));
}

#[test]
fn marks_that_break_a_rule_are_refused_with_the_rule() {
    assert_eq!(
        check_marks(&[mark(3, 0)], 10, 100),
        Err(DataError::EmptyMark)
    );
    assert_eq!(
        check_marks(&[mark(5, 1), mark(4, 1)], 10, 100),
        Err(DataError::MarksNotAscending)
    );
    assert_eq!(
        check_marks(&[mark(5, 1), mark(5, 1)], 10, 100),
        Err(DataError::MarksNotAscending),
        "two runs at one place are one mark"
    );
    assert_eq!(
        check_marks(&[mark(11, 1)], 10, 100),
        Err(DataError::MarkBeyondSamples)
    );
    assert_eq!(
        check_marks(&[mark(5, 91)], 10, 100),
        Err(DataError::MoreThanRequested)
    );
    // Sums that would overflow are refused, not wrapped.
    assert_eq!(
        check_marks(
            &[mark(1, u32::MAX), mark(2, u32::MAX)],
            10,
            u64::from(u32::MAX)
        ),
        Err(DataError::MoreThanRequested)
    );
}

#[test]
fn the_samples_stored_and_lost_add_up_to_what_the_capture_covers() {
    // For any list of runs, the positions covered are the stored samples plus the lost ones.
    let mut state = 77_u64;
    let mut next = move || {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        state
    };
    for _ in 0..500 {
        let stored = u32::try_from(next() % 1_000).unwrap();
        let mut positions: Vec<u32> = (0..next() % 20)
            .map(|_| u32::try_from(next() % (u64::from(stored) + 1)).unwrap())
            .collect();
        positions.sort_unstable();
        positions.dedup();
        let marks: Vec<LossMark> = positions
            .iter()
            .map(|&p| mark(p, u32::try_from(next() % 50).unwrap() + 1))
            .collect();
        let lost: u64 = marks.iter().map(|m| u64::from(m.lost)).sum();
        let covered = u64::from(stored) + lost;
        assert_eq!(check_marks(&marks, stored, covered), Ok(lost));
        assert_eq!(
            check_marks(&marks, stored, covered - 1),
            Err(DataError::MoreThanRequested)
        );
    }
}
