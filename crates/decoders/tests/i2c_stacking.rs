//! What a decoder stacked on the I2C decoder receives (spec 008, user story 5, task T033;
//! `specs/008-i2c-decoder-generator/contracts/i2c-items.md`).
//!
//! The upper decoder is a test decoder that writes every item it is given as an annotation: its span
//! and its payload as hexadecimal text, a trailing `!` for an item with the error flag.
#![cfg(feature = "test-support")]
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::panic
)]

mod common;

use std::sync::atomic::AtomicBool;

use common::i2c::{Bus, SCL, SDA, setup, write_tx};
use common::{Part, append_parts, capture_from_parts};
use la_decoders::decoders::i2c::{self, items};
use la_decoders::generators::i2c::{Params, expected, source};
use la_decoders::model::{Item, Payload, Text};
use la_decoders::registry::Registry;
use la_decoders::run::{ChunkLimits, End, Pass, Sink, run_item_pass, run_pass};
use la_decoders::setup::Setup;
use la_decoders::stack::ItemStream;
use la_decoders::store::{Budget, Limits};
use la_decoders::testing::{self, I2C_READER};
use la_devices::SignalSource;

fn registry() -> Registry {
    let mut registry = Registry::builtin();
    for (descriptor, create) in testing::fixtures() {
        if descriptor.id == I2C_READER.id {
            registry.register_decoder(descriptor, create).unwrap();
        }
    }
    registry
}

/// The text the test decoder shows for an item.
fn shown(item: &Item) -> (u64, u64, String) {
    let Payload::Bytes(bytes) = &item.payload else {
        panic!("not bytes");
    };
    let mut text: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
    if item.error {
        text.push('!');
    }
    (item.start, item.end, text)
}

fn generated(setup: &Setup, seed: u64, frames: u64) -> (Vec<u32>, u64) {
    let length = Params::from_setup(setup, seed).unwrap().frame_length() * frames;
    let mut words = vec![0_u32; length as usize];
    source(setup, seed).unwrap().fill(0, &mut words);
    (words, length)
}

/// Runs the I2C decoder and the test decoder above it over a closed capture.
fn stacked(setup: &Setup, words: Vec<u32>) -> (Vec<Item>, Vec<(u64, u64, String)>) {
    let mut capture = capture_from_parts(2, 1_000_000, &[Part::Samples(words)]);
    capture.close().unwrap();
    let registry = registry();
    let stream = ItemStream::new();
    let mut lower = Pass::new(
        &i2c::DESCRIPTOR,
        setup.clone(),
        registry.create(setup).unwrap(),
        Sink::with_stream(Limits::default(), Budget::new(1 << 30), stream.clone()),
    );
    run_pass(
        &mut lower,
        &capture.reader(),
        &ChunkLimits::default(),
        &AtomicBool::new(false),
        &mut || false,
        &mut |_, _| {},
    )
    .unwrap();
    stream.end(true);
    let upper_setup = testing::setup_above(&I2C_READER, 2, setup.instance);
    let mut upper = Pass::new(
        &I2C_READER,
        upper_setup.clone(),
        registry.create(&upper_setup).unwrap(),
        Sink::new(Limits::default(), Budget::new(1 << 30)),
    );
    run_item_pass(
        &mut upper,
        &stream,
        &capture.reader(),
        &AtomicBool::new(false),
        &mut || false,
        &mut |_, _| {},
    )
    .unwrap();
    let results = upper.sink().results();
    let mut received: Vec<(u64, u64, String)> = results
        .lane
        .window(0, u64::MAX)
        .iter()
        .map(|e| {
            let (start, end) = e.annotation.span();
            let Text::Literal(text) = &e.annotation.text else {
                panic!("not a literal");
            };
            (start, end, text.clone())
        })
        .collect();
    received.sort();
    (lower.sink().items(), received)
}

#[test]
fn the_decoder_above_receives_exactly_the_transactions_the_generator_made() {
    for seed in 0..120_u64 {
        let mut setup = setup(&[]);
        setup
            .generator
            .insert("errors".to_owned(), (seed % 2) as i64);
        setup.generator.insert("error_rate".to_owned(), 3);
        setup
            .generator
            .insert("ten_bit".to_owned(), ((seed / 2) % 2) as i64);
        let (words, length) = generated(&setup, seed, 8);
        let (produced, received) = stacked(&setup, words);
        let wanted = expected(&setup, seed, length);
        assert_eq!(produced, wanted, "seed {seed}: the items");
        let mut should: Vec<(u64, u64, String)> = wanted.iter().map(shown).collect();
        should.sort();
        assert_eq!(received, should, "seed {seed}: what the decoder above got");
        // In order, the layout of the contract: type, direction, acknowledgement, flags, value.
        for item in &produced {
            let Payload::Bytes(bytes) = &item.payload else {
                panic!("not bytes");
            };
            assert_eq!(bytes.len(), 6);
            assert!((items::START..=items::STOP).contains(&bytes[0]));
            assert!(bytes[1] <= items::UNKNOWN && bytes[2] <= items::NONE);
        }
        assert!(produced.windows(2).all(|w| w[0].end <= w[1].end));
    }
}

#[test]
fn a_byte_cut_short_gives_no_item_and_an_unfinished_transaction_gives_what_completed() {
    let mut bus = Bus::new();
    bus.idle(64);
    write_tx(&mut bus, 0x51, &[0x01, 0x02]);
    bus.idle(64);
    bus.start();
    bus.byte(0xA0, true);
    bus.bits(0x12, 3);
    bus.stop();
    bus.idle(64);
    // Then a transaction that is not finished when the capture ends.
    bus.start();
    bus.byte(0xA4, true);
    bus.byte(0x55, true);
    bus.bits(0x01, 2);
    let (produced, received) = stacked(&setup(&[]), bus.words());
    let kinds: Vec<(u8, u8)> = produced
        .iter()
        .map(|i| match &i.payload {
            Payload::Bytes(b) => (b[0], b[1]),
            _ => panic!(),
        })
        .collect();
    assert_eq!(
        kinds,
        vec![
            // The ordinary write: start, address, two data bytes, stop.
            (items::START, items::UNKNOWN),
            (items::ADDRESS, items::WRITE),
            (items::DATA, items::WRITE),
            (items::DATA, items::WRITE),
            (items::STOP, items::WRITE),
            // The cut one: start, address, no item for the byte that was cut, stop.
            (items::START, items::UNKNOWN),
            (items::ADDRESS, items::WRITE),
            (items::STOP, items::WRITE),
            // The unfinished one: start, address, one data byte, and nothing more.
            (items::START, items::UNKNOWN),
            (items::ADDRESS, items::WRITE),
            (items::DATA, items::WRITE),
        ]
    );
    assert_eq!(received.len(), produced.len());
}

#[test]
fn while_the_period_is_unknown_the_decoder_above_gets_nothing_and_then_everything_in_order() {
    let mut bus = Bus::new();
    bus.idle(64);
    bus.start();
    bus.byte(0xA0, true);
    let first_part = bus.pos() as usize;
    bus.byte(0x12, true);
    bus.stop();
    bus.idle(64);
    write_tx(&mut bus, 0x51, &[0x77, 0x88]);
    bus.idle(64);
    let words = bus.words();

    let mut capture =
        capture_from_parts(2, 1_000_000, &[Part::Samples(words[..first_part].to_vec())]);
    let setup = setup(&[]);
    let stream = ItemStream::new();
    let mut lower = Pass::new(
        &i2c::DESCRIPTOR,
        setup.clone(),
        registry().create(&setup).unwrap(),
        Sink::with_stream(Limits::default(), Budget::new(1 << 30), stream.clone()),
    );
    let limits = ChunkLimits::default();
    let cancel = AtomicBool::new(false);
    lower
        .advance(
            &capture.reader().snapshot(),
            &limits,
            &cancel,
            &mut |_, _| {},
        )
        .unwrap();
    // Nine pulses: the period is not known, so nothing is final and nothing is handed on.
    let read = stream.read(0, usize::MAX);
    assert!(read.items.is_empty());
    assert_eq!(read.covered, 0);

    append_parts(&mut capture, &[Part::Samples(words[first_part..].to_vec())]);
    capture.close().unwrap();
    let snapshot = capture.reader().snapshot();
    lower
        .advance(&snapshot, &limits, &cancel, &mut |_, _| {})
        .unwrap();
    lower.finish(&snapshot, End::Complete).unwrap();
    stream.end(true);
    let read = stream.read(0, usize::MAX);
    assert_eq!(read.items.len(), 9);
    assert!(read.items.windows(2).all(|w| w[0].end <= w[1].end));
    assert_eq!(read.covered, words.len() as u64);
    // The channels are the ones the builder uses.
    assert_eq!((SCL, SDA), (0, 1));
}
