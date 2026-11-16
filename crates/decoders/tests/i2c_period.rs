//! Finding the bus clock period from the signal (spec 008, research R1, task T006).
//!
//! Expected values are worked out by hand: a quarter-octave bin of an interval `d` is
//! `4 * floor(log2 d) + ((d >> (floor(log2 d) - 2)) & 3)`, so 15 is bin 15, 16 to 19 are bin 16 and 20
//! is bin 17; the period is the mean (integer division) of the intervals of the best bin and its two
//! neighbours once they hold at least 16 of the last 32 intervals.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::panic
)]

use la_decoders::decoders::i2c::period::Period;

/// Feeds rising-edge positions made from `intervals` (the first edge is at `start`) and returns the
/// index of the interval after which the period was found, with the period.
fn feed(period: &mut Period, start: u64, intervals: &[u64]) -> Option<(usize, u64)> {
    let mut position = start;
    assert_eq!(period.edge(position), None);
    for (index, interval) in intervals.iter().enumerate() {
        position += interval;
        if let Some(found) = period.edge(position) {
            return Some((index + 1, found));
        }
    }
    None
}

#[test]
fn a_steady_clock_is_found_after_sixteen_intervals() {
    let mut period = Period::new();
    let found = feed(&mut period, 100, &[16; 40]);
    assert_eq!(found, Some((16, 16)));
    assert_eq!(period.found(), Some(16));
}

#[test]
fn fifteen_intervals_are_not_enough() {
    let mut period = Period::new();
    assert_eq!(feed(&mut period, 5, &[16; 15]), None);
    assert_eq!(period.found(), None);
}

#[test]
fn junk_before_the_clock_does_not_delay_it_beyond_sixteen_clock_intervals() {
    // 41 junk intervals, 1, 2, 4, ... 2^40, each alone in its bin, then a steady 16: the last 32
    // intervals hold 16 of them after 16 steady ones, however long the junk was.
    let junk: Vec<u64> = (0..=40).map(|k| 1_u64 << k).collect();
    let mut intervals = junk.clone();
    intervals.extend([16; 60]);
    let mut period = Period::new();
    let found = feed(&mut period, 1_000, &intervals);
    assert_eq!(found, Some((41 + 16, 16)));
    // 15 steady intervals are not enough: the last 32 hold 15 of them and 17 junk ones (the junk 16
    // itself is by then 52 intervals back and out of the last 32).
    let mut intervals = junk;
    intervals.extend([16; 15]);
    let mut period = Period::new();
    assert_eq!(feed(&mut period, 1_000, &intervals), None);
}

#[test]
fn a_very_long_junk_start_up_does_not_matter_either() {
    // 100,000 intervals spread over the octaves, then the clock.
    let mut x = 12_345_u64;
    let mut intervals = Vec::new();
    for _ in 0..100_000 {
        x = x
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        intervals.push(1 + ((x >> 33) % (1 << ((x >> 20) % 12))));
    }
    let junk = intervals.len();
    intervals.extend([40; 30]);
    let mut period = Period::new();
    let (index, _) = feed(&mut period, 0, &intervals).unwrap();
    assert!(index <= junk + 16, "found only after {index}");
}

#[test]
fn neighbouring_bins_are_averaged() {
    // 15 is bin 15, 16 and 17 are bin 16: the window around bin 16 holds all of them. After 16
    // intervals (five rounds of 15, 16, 17 and one more 15) the sum is 5 * 48 + 15 = 255, mean 15.
    let mut intervals = Vec::new();
    for _ in 0..8 {
        intervals.extend([15, 16, 17]);
    }
    let mut period = Period::new();
    assert_eq!(feed(&mut period, 0, &intervals), Some((16, 15)));
}

#[test]
fn stretched_clocks_do_not_move_the_period() {
    // Eight intervals of 16 and one of 40 (clock stretching), repeated: after 17 intervals there are
    // 16 of 16 (more than half of 17).
    let mut intervals = Vec::new();
    for _ in 0..4 {
        intervals.extend([16; 8]);
        intervals.push(40);
    }
    let mut period = Period::new();
    assert_eq!(feed(&mut period, 7, &intervals), Some((17, 16)));
}

#[test]
fn a_break_adds_no_interval() {
    // 17 edges 16 apart with a break after the 9th: one interval is lost, 15 remain.
    let mut period = Period::new();
    let mut position = 0;
    for index in 0..17 {
        if index == 9 {
            period.break_at_gap();
        }
        assert_eq!(period.edge(position), None, "edge {index}");
        position += 16;
    }
    // The 18th edge gives the 16th interval.
    assert_eq!(period.edge(position), Some(16));
}

#[test]
fn once_found_the_period_never_changes() {
    let mut period = Period::new();
    assert_eq!(feed(&mut period, 0, &[16; 20]), Some((16, 16)));
    let mut position = 16 * 20;
    for _ in 0..500 {
        position += 1_000;
        assert_eq!(period.edge(position), Some(16));
    }
    assert_eq!(period.found(), Some(16));
}

#[test]
fn awkward_positions_do_not_panic_or_overflow() {
    let mut period = Period::new();
    // The same position twice, a position that goes back, huge positions and the largest distance.
    for position in [
        5,
        5,
        3,
        0,
        1,
        u64::MAX / 2,
        u64::MAX - 1,
        u64::MAX,
        u64::MAX,
    ] {
        let _ = period.edge(position);
    }
    period.break_at_gap();
    for position in 0..300 {
        let _ = period.edge(position * (u64::MAX / 300));
    }
    // A found period of enormous intervals is still a number.
    let mut period = Period::new();
    let found = feed(&mut period, 0, &[1 << 58; 20]);
    assert_eq!(found, Some((16, 1 << 58)));
}
