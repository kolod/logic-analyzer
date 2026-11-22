//! Decoding I2C transactions (spec 008, user story 1, task T007).
//!
//! Every expectation is written by hand from the positions the bus builder reports: a byte is a
//! block of eight bit cells (half a period either side of the rising edges of SCL) and an acknowledge
//! block of one cell, a start and a stop are blocks around their SDA edge, the whole transaction is a
//! block below them, every sampled bit is an arrow on SCL, a start and a stop are dots on SDA.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::panic
)]

mod common;

use common::i2c::{
    Bus, Decoded, Expected, GREEN, HALF, ORANGE, SCL, SDA, YELLOW, ack_end, byte_arrows, byte_lane,
    decode, dot, read_tx, restart_block, sorted, start_block, stop_block, whole, write_tx,
};
use la_decoders::model::Shape;

fn assert_marks_name_no_colour(decoded: &Decoded) {
    assert!(!decoded.shapes.is_empty());
    for (channel, shape, colour) in &decoded.shapes {
        assert_eq!(*colour, None, "a mark on channel {channel} names a colour");
        let wanted = if *channel == SCL {
            Shape::Arrow
        } else {
            Shape::Dot
        };
        assert_eq!(*shape, wanted, "channel {channel}");
    }
}

#[test]
fn a_write_of_two_bytes_gives_the_address_the_bytes_the_marks_and_one_transaction() {
    let mut bus = Bus::new();
    bus.idle(64);
    let expected = write_tx(&mut bus, 0x50, &[0x12, 0x34]);
    bus.idle(64);
    let decoded = decode(&bus.words());
    assert_eq!(decoded.all, sorted(expected));
    assert_eq!(decoded.refused, 0);
    assert_marks_name_no_colour(&decoded);
    // 27 arrows: nine for each of the three bytes.
    assert_eq!(decoded.overlay(SCL).len(), 27);
    assert_eq!(decoded.overlay(SDA).len(), 2);
}

#[test]
fn a_read_ends_with_the_controllers_nak_which_is_not_an_error() {
    let mut bus = Bus::new();
    bus.idle(64);
    let expected = read_tx(&mut bus, 0x50, &[0x01, 0x02]);
    bus.idle(64);
    let decoded = decode(&bus.words());
    assert_eq!(decoded.all, sorted(expected));
    assert!(decoded.all.iter().all(|e| !e.error));
}

#[test]
fn a_repeated_start_is_an_event_and_both_parts_are_one_transaction() {
    let mut bus = Bus::new();
    bus.idle(64);
    let mut expected = Vec::new();
    let start = bus.start();
    expected.push(start_block(start, "ann-i2c-start"));
    expected.push(dot(SDA, start, "ann-i2c-start"));
    let a = bus.byte(0xA0, true);
    expected.extend(byte_lane(
        a,
        "ann-i2c-address-write",
        "0x50",
        ORANGE,
        "ann-i2c-ack",
        false,
    ));
    expected.extend(byte_arrows(a, 0xA0, "ann-i2c-ack"));
    let register = bus.byte(0x10, true);
    expected.extend(byte_lane(
        register,
        "ann-i2c-data",
        "0x10",
        ORANGE,
        "ann-i2c-ack",
        false,
    ));
    expected.extend(byte_arrows(register, 0x10, "ann-i2c-ack"));
    let restart = bus.repeated_start();
    // A start with no stop before it begins where the acknowledge before it ended.
    expected.push(restart_block(ack_end(register), restart));
    expected.push(dot(SDA, restart, "ann-i2c-repeated-start"));
    let b = bus.byte(0xA1, true);
    expected.extend(byte_lane(
        b,
        "ann-i2c-address-read",
        "0x50",
        YELLOW,
        "ann-i2c-ack",
        false,
    ));
    expected.extend(byte_arrows(b, 0xA1, "ann-i2c-ack"));
    let data = bus.byte(0x55, false);
    expected.extend(byte_lane(
        data,
        "ann-i2c-data",
        "0x55",
        YELLOW,
        "ann-i2c-nak",
        false,
    ));
    expected.extend(byte_arrows(data, 0x55, "ann-i2c-nak"));
    let stop = bus.stop();
    expected.push(stop_block(ack_end(data), stop));
    expected.push(dot(SDA, stop, "ann-i2c-stop"));
    // One transaction, named after its first part: green, the controller's NAK at the end of a read
    // is not an error.
    expected.push(common::i2c::packet(
        start - HALF,
        stop,
        GREEN,
        false,
        "0x50",
        "0x10 | 0x55",
    ));
    bus.idle(64);
    let decoded = decode(&bus.words());
    assert_eq!(decoded.all, sorted(expected));
}

#[test]
fn a_nak_on_the_address_and_a_nak_on_a_written_byte_are_errors() {
    // The address is not acknowledged: the NAK block is red and an error, and so is the packet. (A
    // good transaction comes first: a single byte has too few clock pulses for the bus period to be
    // found.)
    let mut bus = Bus::new();
    bus.idle(64);
    write_tx(&mut bus, 0x51, &[0x01, 0x02]);
    bus.idle(64);
    let start = bus.start();
    let a = bus.byte(0xA0, false);
    let stop = bus.stop();
    bus.idle(64);
    let decoded = decode(&bus.words());
    let nak = decoded
        .lane()
        .into_iter()
        .find(|e| e.positions == vec![a.ack - HALF, a.ack + HALF])
        .unwrap();
    assert!(nak.error, "{nak:?}");
    // The NAK block and the packet are the errors of the lane.
    assert_eq!(decoded.lane().iter().filter(|e| e.error).count(), 2);
    // The transaction is still one unit from its start to its stop, in red.
    let mut packet = whole(
        start - HALF,
        stop,
        common::i2c::RED,
        "ann-i2c-transaction-write",
        &["0x50"],
    );
    packet.error = true;
    assert!(decoded.lane().contains(&packet));

    // A written data byte is not acknowledged.
    let mut bus = Bus::new();
    bus.idle(64);
    write_tx(&mut bus, 0x51, &[0x01, 0x02]);
    bus.idle(64);
    bus.start();
    bus.byte(0xA0, true);
    let d = bus.byte(0x12, false);
    bus.stop();
    bus.idle(64);
    let decoded = decode(&bus.words());
    let errors: Vec<Expected> = decoded.lane().into_iter().filter(|e| e.error).collect();
    // The NAK block and the red packet.
    assert_eq!(errors.len(), 2);
    assert!(
        errors
            .iter()
            .any(|e| e.positions == vec![d.ack - HALF, d.ack + HALF])
    );
}

#[test]
fn an_sda_change_on_the_sample_scl_falls_is_neither_a_start_nor_a_stop() {
    // Pulses 12 samples apart whose SDA changes on the very sample SCL falls: the edges of one
    // sample are processed SCL first, so the SDA edge finds SCL low.
    let mut bus = Bus::new();
    bus.idle(64);
    let start = bus.start();
    let mut rises = Vec::new();
    let mut bits = Vec::new();
    for byte in [0xA0_u8, 0xFF, 0x00] {
        for i in 0..8 {
            bits.push(byte >> (7 - i) & 1 != 0);
        }
        bits.push(false); // acknowledged
    }
    for bit in &bits {
        bus.raw(*bit, false, 4);
        rises.push(bus.pos());
        bus.raw(*bit, true, 8);
    }
    bus.raw(false, false, 4);
    let stop = bus.stop();
    bus.idle(64);
    let decoded = decode(&bus.words());
    let arrows: Vec<u64> = decoded
        .overlay(SCL)
        .iter()
        .map(|e| e.positions[0])
        .collect();
    assert_eq!(arrows, rises);
    let lane = decoded.lane();
    // The bus period here is 12 samples, so a cell reaches 6 samples either side of the edge; the
    // first clock pulse rises 16 samples after the start, so the start block, which goes on to the
    // block after it, ends 10 samples after the edge.
    assert!(lane.contains(&common::i2c::part(
        start - 6,
        start + 10,
        common::i2c::BLUE,
        "ann-i2c-start",
        &[]
    )));
    assert!(
        lane.iter()
            .any(|e| e.key == "ann-i2c-stop" && e.positions[1] == stop)
    );
    assert!(lane.iter().all(|e| !e.error), "{lane:?}");
    // Three bytes: an address and two data bytes.
    assert_eq!(lane.iter().filter(|e| e.key == "ann-i2c-data").count(), 2);
}

#[test]
fn swapped_channels_give_a_valid_result_and_no_panic() {
    let mut bus = Bus::new();
    bus.idle(64);
    write_tx(&mut bus, 0x50, &[0x12, 0x34]);
    bus.idle(64);
    // SCL on the channel the decoder reads as SDA and the other way round.
    let swapped: Vec<u32> = bus
        .words()
        .iter()
        .map(|w| (w >> SCL & 1) << SDA | (w >> SDA & 1) << SCL)
        .collect();
    let decoded = decode(&swapped);
    assert_eq!(decoded.refused, 0);
    assert_ne!(decoded.all, decode(&bus.words()).all);
}
