//! The I2C lane as blocks (spec 016, tasks T012 to T014 and T027, T028): the two bands, the places of
//! the blocks from the bus period (research R2) and the awkward cases (a stop that comes at once, two
//! packets close together, odd periods). Every expectation is written by hand from the positions the
//! bus builder reports.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::panic
)]

mod common;

use common::i2c::{
    BLUE, Bus, Expected, GREEN, GREY, HALF, RED, YELLOW, ack_end, decode, erroneous, part,
    start_block, stop_block, whole, write_tx,
};

/// The blocks of a kind of band, in order of their start.
fn band(lane: &[Expected], name: &str) -> Vec<Expected> {
    let mut list: Vec<Expected> = lane
        .iter()
        .filter(|e| matches!(&e.block, Some((b, _)) if b == name))
        .cloned()
        .collect();
    list.sort_by_key(|e| e.positions[0]);
    list
}

/// A bus that has begun with an ordinary write and an idle (a single byte has too few clock pulses
/// for the bus period to be found).
fn lead() -> Bus {
    let mut bus = Bus::new();
    bus.idle(64);
    write_tx(&mut bus, 0x51, &[0x01, 0x02]);
    bus.idle(64);
    bus
}

#[test]
fn a_good_write_is_six_blocks_side_by_side_and_one_block_below_them() {
    let mut bus = Bus::new();
    bus.idle(64);
    write_tx(&mut bus, 0x50, &[0x12]);
    bus.idle(64);
    let lane = decode(&bus.words()).lane();
    // Everything in the lane is a block of one of the two bands.
    assert!(lane.iter().all(|e| e.block.is_some()), "{lane:?}");
    let upper = band(&lane, "part");
    let lower = band(&lane, "whole");
    let keys: Vec<&str> = upper.iter().map(|e| e.key.as_str()).collect();
    assert_eq!(
        keys,
        [
            "ann-i2c-start",
            "ann-i2c-address-write",
            "ann-i2c-ack",
            "ann-i2c-data",
            "ann-i2c-ack",
            "ann-i2c-stop"
        ]
    );
    let colours: Vec<u8> = upper.iter().map(|e| e.block.clone().unwrap().1).collect();
    assert_eq!(colours, [BLUE, YELLOW, GREEN, YELLOW, GREEN, BLUE]);
    // They follow each other with no gap and no overlap.
    for pair in upper.windows(2) {
        assert_eq!(pair[0].positions[1], pair[1].positions[0], "{pair:?}");
    }
    // The block below runs from the beginning of the start block to the end of the stop block.
    assert_eq!(lower.len(), 1);
    assert_eq!(lower[0].positions[0], upper[0].positions[0]);
    assert_eq!(lower[0].positions[1], upper[5].positions[1]);
    assert_eq!(lower[0].block.clone().unwrap().1, GREEN);
    assert!(!lower[0].error);
}

#[test]
fn a_stop_right_after_the_eighth_bit_cuts_the_byte_and_begins_where_it_ends() {
    // The SDA of the ninth pulse rises three samples into the pulse: a stop, with no acknowledge.
    let mut bus = lead();
    let start = bus.start();
    let a = bus.byte(0xA0, true);
    let first = bus.pos() + 4;
    bus.bits(0x12, 8);
    bus.raw(false, false, 4);
    bus.raw(false, true, 3);
    let stop = bus.pos();
    bus.raw(true, true, 64);
    let lane = decode(&bus.words()).lane();
    let cut = erroneous(part(
        first - HALF,
        first + 7 * 16 + HALF,
        YELLOW,
        "ann-i2c-error-byte-cut",
        &["8"],
    ));
    assert!(lane.contains(&cut), "{lane:?}");
    // The stop block begins where the cut block ends (a period before the stop would be earlier)
    // and ends at the stop.
    assert!(
        lane.contains(&stop_block(cut.positions[1], stop)),
        "{lane:?}"
    );
    assert!(lane.contains(&erroneous(whole(
        start - HALF,
        stop,
        RED,
        "ann-i2c-transaction-write",
        &["0x50"]
    ))));
    let _ = a;
}

#[test]
fn a_stop_and_the_next_start_less_than_a_period_apart_never_overlap() {
    let mut bus = lead();
    let first_start = bus.start();
    let a = bus.byte(0xA0, true);
    let stop = bus.stop();
    // The next start one sample after the stop: its block would begin inside the stop block.
    let second_start = bus.start();
    let b = bus.byte(0xA2, true);
    let second_stop = bus.stop();
    bus.idle(64);
    let lane = decode(&bus.words()).lane();
    assert!(second_start - stop <= 2, "{} {}", stop, second_start);
    // The stop block is cut where the start block begins.
    assert!(lane.contains(&part(
        ack_end(a),
        second_start - HALF,
        BLUE,
        "ann-i2c-stop",
        &[]
    )));
    assert!(lane.contains(&start_block(second_start, "ann-i2c-start")));
    // The packets still run from their starts to their stops.
    assert!(lane.contains(&whole(
        first_start - HALF,
        stop,
        GREEN,
        "ann-i2c-transaction-write",
        &["0x50"]
    )));
    assert!(lane.contains(&whole(
        second_start - HALF,
        second_stop,
        GREEN,
        "ann-i2c-transaction-write",
        &["0x51"]
    )));
    let _ = b;
    let upper = band(&lane, "part");
    for pair in upper.windows(2) {
        assert!(pair[0].positions[1] <= pair[1].positions[0], "{pair:?}");
    }
}

/// A bus with a clock period of `period` samples (at least 13): the pulse is 8 samples high.
fn odd_bus(period: u64, bytes: &[u8]) -> (Bus, Vec<(u64, u64)>, u64, u64) {
    let mut bus = Bus::new();
    bus.idle(64);
    let start = bus.raw(false, true, period).pos() - period;
    let mut cells = Vec::new();
    for value in bytes {
        let mut first = 0;
        for i in 0..9 {
            let bit = if i < 8 {
                value >> (7 - i) & 1 != 0
            } else {
                false
            };
            bus.raw(bit, false, 4);
            if i == 0 {
                first = bus.pos();
            }
            bus.raw(bit, true, 8);
            bus.raw(bit, false, period - 12);
        }
        cells.push((first, first + 8 * period));
    }
    bus.raw(false, false, 4);
    bus.raw(false, true, 4);
    let stop = bus.pos();
    bus.raw(true, true, 64);
    (bus, cells, start, stop)
}

#[test]
fn with_an_odd_period_a_cell_reaches_half_the_period_rounded_down_before_the_edge() {
    for period in [15_u64, 17] {
        let before = period / 2;
        let after = period - before;
        let (bus, cells, start, stop) = odd_bus(period, &[0xA0, 0x12, 0x34]);
        let lane = decode(&bus.words()).lane();
        let upper = band(&lane, "part");
        // Start, three bytes with an acknowledge each, stop.
        assert_eq!(upper.len(), 8, "period {period}: {upper:?}");
        assert_eq!(upper[0].positions, vec![start - before, start + after]);
        for (n, (first, ack)) in cells.iter().enumerate() {
            let byte = &upper[1 + 2 * n];
            let ack_cell = &upper[2 + 2 * n];
            assert_eq!(
                byte.positions,
                vec![first - before, first + 7 * period + after],
                "period {period} byte {n}"
            );
            assert_eq!(
                ack_cell.positions,
                vec![ack - before, ack + after],
                "period {period} ack {n}"
            );
            // The two touch.
            assert_eq!(byte.positions[1], ack_cell.positions[0]);
        }
        assert_eq!(upper[7].positions[1], stop);
        let lower = band(&lane, "whole");
        assert_eq!(lower.len(), 1);
        assert_eq!(lower[0].positions, vec![start - before, stop]);
    }
}

#[test]
fn a_capture_that_ends_after_the_ack_is_a_grey_packet_up_to_the_end_of_the_ack_block() {
    let mut bus = lead();
    let start = bus.start();
    let a = bus.byte(0xA0, true);
    let lane = decode(&bus.words()).lane();
    assert!(lane.contains(&whole(
        start - HALF,
        ack_end(a),
        GREY,
        "ann-i2c-incomplete",
        &[]
    )));
    assert!(lane.iter().all(|e| !e.error));
}
