//! Protocol violations and the end of a capture in the middle of a transaction (spec 008, user
//! story 3, task T022). Every expectation is written by hand from the positions of the bus builder.
//!
//! A single byte has too few clock pulses for the bus period to be found, so every signal starts
//! with an ordinary write that is not looked at here.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::panic
)]

mod common;

use std::sync::atomic::AtomicBool;

use common::i2c::{
    Bus, Decoded, Expected, byte_lane, collect, decode, decode_capture, erroneous, lane_point,
    lane_range, new_pass, setup, write_tx,
};
use common::{Part, append_parts, capture_from_parts};
use la_decoders::decoders::i2c::items;
use la_decoders::model::{Item, Payload};
use la_decoders::run::{ChunkLimits, End};

/// A bus that has begun with an ordinary write and an idle.
fn lead() -> Bus {
    let mut bus = Bus::new();
    bus.idle(64);
    write_tx(&mut bus, 0x51, &[0x01, 0x02]);
    bus.idle(64);
    bus
}

fn lane(decoded: &Decoded) -> Vec<Expected> {
    decoded.lane()
}

fn errors(decoded: &Decoded) -> Vec<Expected> {
    lane(decoded).into_iter().filter(|e| e.error).collect()
}

/// The item types, directions and acknowledgements after the lead (which has 4 + 2 items: start,
/// address, two data bytes, stop is 5).
fn tail_items(decoded: &Decoded) -> Vec<(u8, u8, u8, bool)> {
    decoded
        .items
        .iter()
        .skip(5)
        .map(|item: &Item| {
            let Payload::Bytes(b) = &item.payload else {
                panic!("not bytes");
            };
            (b[0], b[1], b[2], item.error)
        })
        .collect()
}

#[test]
fn a_nak_on_the_address_is_an_error_where_it_happens_and_the_transaction_ends_at_its_stop() {
    let mut bus = lead();
    let start = bus.start();
    let a = bus.byte(0xA0, false);
    let stop = bus.stop();
    bus.idle(64);
    let decoded = decode(&bus.words());
    let mut nak = lane_point(a.ack, "ann-i2c-error-address-nak");
    nak.args = vec!["0x50".to_owned()];
    assert_eq!(errors(&decoded), vec![erroneous(nak)]);
    assert!(lane(&decoded).contains(&lane_range(
        start,
        stop + 1,
        "ann-i2c-transaction-write",
        &["0x50"]
    )));
    assert_eq!(
        tail_items(&decoded),
        vec![
            (items::START, items::UNKNOWN, items::NONE, false),
            (items::ADDRESS, items::WRITE, items::NAK, true),
            (items::STOP, items::WRITE, items::NONE, false),
        ]
    );
}

#[test]
fn a_nak_on_a_written_data_byte_is_an_error_and_on_the_last_read_byte_it_is_not() {
    let mut bus = lead();
    bus.start();
    bus.byte(0xA0, true);
    let d = bus.byte(0x12, false);
    bus.stop();
    bus.idle(64);
    let decoded = decode(&bus.words());
    let mut nak = lane_point(d.ack, "ann-i2c-error-data-nak");
    nak.args = vec!["0x12".to_owned()];
    assert_eq!(errors(&decoded), vec![erroneous(nak)]);
    assert_eq!(
        tail_items(&decoded)[2],
        (items::DATA, items::WRITE, items::NAK, true)
    );

    // A read that ends with the controller's NAK has no error at all.
    let mut bus = lead();
    bus.start();
    bus.byte(0xA1, true);
    bus.byte(0x12, false);
    bus.stop();
    bus.idle(64);
    assert!(errors(&decode(&bus.words())).is_empty());
}

#[test]
fn a_stop_in_the_middle_of_a_byte_cuts_it_short_and_the_next_transaction_decodes() {
    let mut bus = lead();
    let start = bus.start();
    let a = bus.byte(0xA0, true);
    let first_bit = bus.pos() + 4;
    bus.bits(0x12, 3);
    let stop = bus.stop();
    bus.idle(64);
    let after = bus.pos();
    let expected_next = write_tx(&mut bus, 0x52, &[0x09]);
    bus.idle(64);
    let decoded = decode(&bus.words());
    // The error covers the bits that were read, up to the stop.
    let mut cut = lane_range(first_bit, stop, "ann-i2c-error-byte-cut", &["3"]);
    cut.error = true;
    assert_eq!(errors(&decoded), vec![cut]);
    // The stop is marked and the transaction ends there; the address was read before the cut.
    let lane = lane(&decoded);
    assert!(lane.contains(&lane_point(stop, "ann-i2c-stop")));
    assert!(lane.contains(&lane_range(
        start,
        stop + 1,
        "ann-i2c-transaction-write",
        &["0x50"]
    )));
    assert!(lane.contains(&byte_lane(a, "ann-i2c-address-write", "0x50", "ann-i2c-ack", false)[0]));
    // The byte that was cut short gives no item, and the next transaction is whole.
    assert_eq!(
        tail_items(&decoded)[..3],
        [
            (items::START, items::UNKNOWN, items::NONE, false),
            (items::ADDRESS, items::WRITE, items::ACK, false),
            (items::STOP, items::WRITE, items::NONE, false),
        ]
    );
    for wanted in expected_next {
        assert!(
            decoded.all.contains(&wanted),
            "missing {wanted:?} after {after}"
        );
    }
}

#[test]
fn a_start_in_the_middle_of_a_byte_cuts_it_short_and_begins_a_new_transaction() {
    let mut bus = lead();
    let first = bus.start();
    bus.byte(0xA0, true);
    let first_bit = bus.pos() + 4;
    bus.bits(0x12, 3);
    let second = bus.repeated_start();
    bus.byte(0xA2, true);
    bus.byte(0x77, true);
    let stop = bus.stop();
    bus.idle(64);
    let decoded = decode(&bus.words());
    let mut cut = lane_range(first_bit, second, "ann-i2c-error-byte-cut", &["3"]);
    cut.error = true;
    assert_eq!(errors(&decoded), vec![cut]);
    let lane = lane(&decoded);
    // The first transaction is shown as unfinished, up to the second start, which is a start and
    // not a repeated start; the second one is whole.
    assert!(lane.contains(&lane_range(first, second, "ann-i2c-incomplete", &[])));
    assert!(lane.contains(&lane_point(second, "ann-i2c-start")));
    assert!(!lane.iter().any(|e| e.key == "ann-i2c-repeated-start"));
    assert!(lane.contains(&lane_range(
        second,
        stop + 1,
        "ann-i2c-transaction-write",
        &["0x51"]
    )));
}

#[test]
fn a_start_followed_by_a_stop_without_a_clock_is_an_empty_transaction() {
    let mut bus = lead();
    let start = bus.start();
    let stop = bus.stop();
    bus.idle(64);
    let decoded = decode(&bus.words());
    let mut empty = lane_range(start, stop + 1, "ann-i2c-error-empty", &[]);
    empty.error = true;
    assert_eq!(errors(&decoded), vec![empty]);
    assert_eq!(
        tail_items(&decoded),
        vec![
            (items::START, items::UNKNOWN, items::NONE, false),
            (items::STOP, items::UNKNOWN, items::NONE, false),
        ]
    );
}

#[test]
fn a_gap_in_the_middle_of_a_byte_is_reported_and_decoding_resumes_after_the_idle_that_follows() {
    let mut first = lead();
    let start = first.start();
    first.byte(0xA0, true);
    first.bits(0x12, 4);
    let gap_start = first.pos();
    let mut second = Bus::new();
    second.idle(64);
    let expected_next = write_tx(&mut second, 0x53, &[0x07]);
    second.idle(64);
    let mut capture = capture_from_parts(
        2,
        1_000_000,
        &[
            Part::Samples(first.words()),
            Part::Gap(30),
            Part::Samples(second.words()),
        ],
    );
    capture.close().unwrap();
    let decoded = decode_capture(&capture, &[], &ChunkLimits::default());
    let mut gap = lane_range(gap_start, gap_start + 30, "ann-i2c-error-gap", &[]);
    gap.error = true;
    assert_eq!(errors(&decoded), vec![gap]);
    assert!(lane(&decoded).contains(&lane_range(start, gap_start, "ann-i2c-incomplete", &[])));
    for mut wanted in expected_next {
        for p in &mut wanted.positions {
            *p += gap_start + 30;
        }
        assert!(decoded.all.contains(&wanted), "missing {wanted:?}");
    }
}

#[test]
fn a_capture_that_ends_inside_a_byte_or_between_bytes_shows_the_transaction_as_unfinished() {
    // Inside a byte: three bits of a data byte were read.
    let mut bus = lead();
    let start = bus.start();
    bus.byte(0xA0, true);
    let first_bit = bus.pos() + 4;
    bus.bits(0x12, 3);
    let length = bus.pos();
    let decoded = decode(&bus.words());
    let mut cut = lane_range(first_bit, length, "ann-i2c-error-byte-cut", &["3"]);
    cut.error = true;
    assert_eq!(errors(&decoded), vec![cut]);
    // The unfinished transaction is not an error.
    assert!(lane(&decoded).contains(&lane_range(start, length, "ann-i2c-incomplete", &[])));

    // Between bytes: only the unfinished transaction.
    let mut bus = lead();
    let start = bus.start();
    bus.byte(0xA0, true);
    let length = bus.pos();
    let decoded = decode(&bus.words());
    assert!(errors(&decoded).is_empty());
    assert!(lane(&decoded).contains(&lane_range(start, length, "ann-i2c-incomplete", &[])));
}

#[test]
fn a_transaction_that_continues_while_the_capture_grows_gives_the_result_of_the_whole_capture() {
    let mut bus = lead();
    bus.start();
    bus.byte(0xA0, true);
    let cut_at = bus.pos() as usize;
    bus.byte(0x12, true);
    bus.stop();
    bus.idle(64);
    let words = bus.words();
    let whole = decode(&words);
    assert!(errors(&whole).is_empty());

    let mut capture = capture_from_parts(
        2,
        1_000_000,
        &[Part::Samples(words[..cut_at + 20].to_vec())],
    );
    let setup = setup(&[]);
    let mut pass = new_pass(&setup);
    let limits = ChunkLimits::default();
    let cancel = AtomicBool::new(false);
    pass.advance(
        &capture.reader().snapshot(),
        &limits,
        &cancel,
        &mut |_, _| {},
    )
    .unwrap();
    // While the capture grows nothing is reported as unfinished or cut.
    assert!(errors(&collect(&mut pass)).is_empty());
    append_parts(
        &mut capture,
        &[Part::Samples(words[cut_at + 20..].to_vec())],
    );
    capture.close().unwrap();
    let snapshot = capture.reader().snapshot();
    pass.advance(&snapshot, &limits, &cancel, &mut |_, _| {})
        .unwrap();
    pass.finish(&snapshot, End::Complete).unwrap();
    let grown = collect(&mut pass);
    assert_eq!(grown.all, whole.all);
    assert_eq!(grown.items, whole.items);
}
