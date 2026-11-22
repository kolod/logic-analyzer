//! Synchronising on the bus: what happens before the first valid start condition (spec 008,
//! clarifications 1 to 3, research R2, task T008).
//!
//! A start counts as the first one only after both lines have been high, without a break, for at
//! least one bus period (here 16 samples: the builder's clock). Everything before it is ignored
//! without a trace. The period itself is found from the clock pulses of the signal, so every signal
//! below holds enough of them (a byte is nine pulses and 16 intervals are needed).
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::panic
)]

mod common;

use std::sync::atomic::AtomicBool;

use common::i2c::{Bus, SCL, decode, decode_capture, new_pass, setup, sorted, write_tx};
use common::{Part, append_parts, capture_from_parts};
use la_decoders::run::{ChunkLimits, End};

fn mix(mut x: u64) -> u64 {
    x = x.wrapping_add(0x9E37_79B9_7F4A_7C15);
    x = (x ^ (x >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    x ^ (x >> 31)
}

/// A signal that has the ordinary transaction (`0x50`, `0x12`, `0x34`) after `lead`, and the
/// expectation of it.
fn after(lead: impl FnOnce(&mut Bus)) -> (Bus, Vec<common::i2c::Expected>) {
    let mut bus = Bus::new();
    lead(&mut bus);
    let expected = write_tx(&mut bus, 0x50, &[0x12, 0x34]);
    bus.idle(64);
    (bus, expected)
}

#[test]
fn a_capture_that_begins_in_the_middle_of_a_byte_decodes_from_the_first_valid_start() {
    let (bus, expected) = after(|bus| {
        // SCL low, SDA low (the middle of something), five clock pulses, then the lines are
        // released and the bus is idle.
        bus.raw(false, false, 6);
        bus.bits(0xA5, 5);
        bus.raw(true, false, 4);
        bus.raw(true, true, 4);
        bus.idle(64);
    });
    let decoded = decode(&bus.words());
    assert_eq!(decoded.all, sorted(expected));
}

#[test]
fn lines_held_low_before_the_interface_is_set_up_leave_no_trace() {
    let (bus, expected) = after(|bus| {
        bus.raw(false, false, 3_000);
        bus.idle(40);
    });
    assert_eq!(decode(&bus.words()).all, sorted(expected));
}

#[test]
fn a_start_after_too_short_an_idle_is_ignored_and_the_next_real_one_is_taken() {
    // Ten samples of idle, then a whole write whose start is therefore not accepted (so none of it is
    // decoded), then a stop, a long idle and the ordinary transaction.
    let (bus, expected) = after(|bus| {
        bus.idle(10);
        bus.start();
        bus.byte(0xA0, true);
        bus.byte(0x77, true);
        bus.stop();
        bus.idle(64);
    });
    assert_eq!(decode(&bus.words()).all, sorted(expected));
}

#[test]
fn a_stop_like_edge_before_the_first_start_is_ignored() {
    // SDA rises while SCL is high with the bus not yet idle: not a stop, nothing to stop.
    let (bus, expected) = after(|bus| {
        bus.raw(false, true, 5);
        bus.idle(80);
    });
    assert_eq!(decode(&bus.words()).all, sorted(expected));
}

#[test]
fn an_idle_that_begins_at_position_zero_counts_and_sixteen_samples_are_enough() {
    // 16 samples of idle before the start: exactly one period, accepted.
    let (bus, expected) = after(|bus| {
        bus.idle(16);
    });
    assert_eq!(decode(&bus.words()).all, sorted(expected));
    // 15 are not: the start is ignored and nothing at all is decoded.
    let (bus, _) = after(|bus| {
        bus.idle(15);
    });
    assert!(decode(&bus.words()).all.is_empty());
}

#[test]
fn a_gap_in_the_capture_breaks_an_idle_run() {
    // Idle 30, a gap of 5, idle 10, then the transaction: the run since the gap is 10 samples, not
    // enough, so nothing is decoded. Positions after the gap are shifted by its length.
    let mut bus = Bus::new();
    bus.idle(10);
    write_tx(&mut bus, 0x50, &[0x12, 0x34]);
    bus.idle(64);
    let mut capture = capture_from_parts(
        2,
        1_000_000,
        &[
            Part::Samples(Bus::new().idle(30).words()),
            Part::Gap(5),
            Part::Samples(bus.words()),
        ],
    );
    capture.close().unwrap();
    let decoded = decode_capture(&capture, &[], &ChunkLimits::default());
    assert!(
        decoded
            .lane()
            .iter()
            .all(|e| e.key != "ann-i2c-address-write"),
        "{:?}",
        decoded.lane()
    );

    // With enough idle after the gap the same transaction is decoded, shifted by 35.
    let mut bus = Bus::new();
    bus.idle(40);
    let expected = write_tx(&mut bus, 0x50, &[0x12, 0x34]);
    bus.idle(64);
    let mut capture = capture_from_parts(
        2,
        1_000_000,
        &[
            Part::Samples(Bus::new().idle(30).words()),
            Part::Gap(5),
            Part::Samples(bus.words()),
        ],
    );
    capture.close().unwrap();
    let decoded = decode_capture(&capture, &[], &ChunkLimits::default());
    let shifted: Vec<_> = expected
        .into_iter()
        .map(|mut e| {
            for p in &mut e.positions {
                *p += 35;
            }
            e
        })
        .collect();
    assert_eq!(decoded.all, sorted(shifted));
}

#[test]
fn too_few_clock_pulses_give_an_empty_result_and_no_error() {
    // One address byte: nine pulses, eight intervals, the period is never known.
    let mut bus = Bus::new();
    bus.idle(64);
    bus.start();
    bus.byte(0xA0, true);
    bus.stop();
    bus.idle(64);
    let decoded = decode(&bus.words());
    assert!(decoded.all.is_empty());
    assert!(decoded.items.is_empty());
    assert_eq!(decoded.refused, 0);
}

#[test]
fn while_the_period_is_unknown_nothing_is_shown_and_nothing_is_covered() {
    let mut bus = Bus::new();
    bus.idle(64);
    let start = bus.start();
    bus.byte(0xA0, true);
    let first_part = bus.pos();
    // The rest of the transaction and a second one arrive later.
    bus.byte(0x12, true);
    bus.byte(0x34, true);
    bus.stop();
    bus.idle(64);
    write_tx(&mut bus, 0x51, &[0x77, 0x88]);
    bus.idle(64);
    let words = bus.words();

    // Fed in two pieces to a growing capture: the first holds nine pulses.
    let mut capture = capture_from_parts(
        2,
        1_000_000,
        &[Part::Samples(words[..first_part as usize].to_vec())],
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
    let results = pass.sink().results();
    assert_eq!(results.coverage.covered, 0, "nothing is final yet");
    assert!(results.lane.is_empty());
    assert!(pass.sink().items().is_empty());

    // The rest arrives: now the period is known and everything from the start is decoded.
    append_parts(
        &mut capture,
        &[Part::Samples(words[first_part as usize..].to_vec())],
    );
    capture.close().unwrap();
    pass.advance(
        &capture.reader().snapshot(),
        &limits,
        &cancel,
        &mut |_, _| {},
    )
    .unwrap();
    pass.finish(&capture.reader().snapshot(), End::Complete)
        .unwrap();
    let grown = common::i2c::collect(&mut pass);

    let whole = decode(&words);
    assert_eq!(grown.all, whole.all);
    assert!(
        grown
            .lane()
            .iter()
            .any(|e| e.positions.first() == Some(&(start - common::i2c::HALF)))
    );
    assert_eq!(pass.sink().results().coverage.covered, words.len() as u64);
}

#[test]
fn a_long_start_up_of_noise_is_scanned_in_bounded_steps_and_the_transaction_after_it_decodes() {
    // 150,000 clock pulses (300,000 edges) with SCL high for one sample and low for one of 24
    // values, one in each quarter octave from 4 to 224 (never a steady clock: a window of three bins
    // holds at most three of them), SDA low, then the ordinary transaction.
    let mut junk = Bus::new();
    for i in 0..150_000_u64 {
        let j = mix(i) % 24;
        junk.raw(false, true, 1);
        junk.raw(false, false, (4 + j % 4) << (j / 4));
    }
    junk.raw(false, false, 3);
    junk.idle(40);
    let lead = junk.pos();
    let mut bus = junk;
    let expected = write_tx(&mut bus, 0x50, &[0x12, 0x34]);
    bus.idle(64);
    let words = bus.words();
    let mut capture = capture_from_parts(2, 1_000_000, &[Part::Samples(words)]);
    capture.close().unwrap();

    let setup = setup(&[]);
    let mut pass = new_pass(&setup);
    let mut calls = 0_u32;
    let mut coverage = Vec::new();
    la_decoders::run::run_pass(
        &mut pass,
        &capture.reader(),
        &ChunkLimits::default(),
        &AtomicBool::new(false),
        &mut || false,
        &mut |_, sink| {
            calls += 1;
            coverage.push(sink.results().coverage.covered);
        },
    )
    .unwrap();
    assert!(calls > 4, "the scan was done in {calls} calls");
    assert!(coverage.windows(2).all(|w| w[0] <= w[1]));
    let decoded = common::i2c::collect(&mut pass);
    assert_eq!(decoded.all, sorted(expected));
    // Nothing before the start block, which begins half a period before the first start.
    assert!(
        decoded
            .all
            .iter()
            .all(|e| e.positions[0] + common::i2c::HALF >= lead)
    );
    // Silence the unused import when this module is compiled on its own.
    let _ = SCL;
}
