//! Generate, encode, decode and compare (spec US6, SC-006; constitution Principle XI).
//!
//! For many seeds the generator of the sample decoder makes a signal, the decoder reads it back and
//! the result must be exactly what `expected()` says; planted errors are reported where they were
//! planted.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::panic
)]

mod common;

use std::sync::atomic::AtomicBool;

use common::{Part, capture_from_parts};
use la_decoders::decoders::clocked_bits::{DESCRIPTOR, ID};
use la_decoders::generators::clocked_bits::{
    OPTION_ERROR_RATE, OPTION_ERRORS, Params, expected, expected_errors, source,
};
use la_decoders::model::{Item, Kind, Text};
use la_decoders::registry::Registry;
use la_decoders::run::{ChunkLimits, Outcome, Pass, Sink, run_pass};
use la_decoders::setup::{Setup, Value};
use la_decoders::store::{Budget, Limits};
use la_devices::SignalSource;

fn seeds() -> u64 {
    std::env::var("LA_ROUND_TRIP_SEEDS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(1_000)
}

fn setup(options: &[(&str, Value)], errors: bool) -> Setup {
    let mut setup = Setup::new(3, ID);
    setup.channels.insert("clock".into(), 2);
    setup.channels.insert("data".into(), 5);
    for (id, value) in options {
        setup.options.insert((*id).into(), value.clone());
    }
    if errors {
        setup.generator.insert(OPTION_ERRORS.into(), 1);
        setup.generator.insert(OPTION_ERROR_RATE.into(), 4);
    }
    setup
}

/// `length` samples from the generator, asked for in pieces of `chunk`.
fn generate(setup: &Setup, seed: u64, length: usize, chunk: usize) -> Vec<u32> {
    let mut source = source(setup, seed).expect("a source");
    let mut words = vec![0_u32; length];
    let mut start = 0;
    for piece in words.chunks_mut(chunk) {
        source.fill(start as u64, piece);
        start += piece.len();
    }
    words
}

struct Decoded {
    items: Vec<Item>,
    errors: Vec<(u64, u64)>,
}

fn decode(setup: &Setup, words: &[u32]) -> Decoded {
    let mut capture = capture_from_parts(8, 1_000_000, &[Part::Samples(words.to_vec())]);
    capture.close().unwrap();
    let registry = Registry::builtin();
    setup.validate(&registry, &[], 8).unwrap();
    let mut pass = Pass::new(
        &DESCRIPTOR,
        setup.clone(),
        registry.create(setup).unwrap(),
        Sink::new(Limits::default(), Budget::new(1 << 30)),
    );
    let outcome = run_pass(
        &mut pass,
        &capture.reader(),
        &ChunkLimits::default(),
        &AtomicBool::new(false),
        &mut || false,
        &mut |_, _| {},
    )
    .unwrap();
    assert_eq!(outcome, Outcome::Complete);
    let results = pass.sink().results();
    let errors = results
        .lane
        .window(0, u64::MAX)
        .iter()
        .filter(|e| e.annotation.error)
        .filter_map(|e| match (&e.annotation.kind, &e.annotation.text) {
            (Kind::Range { start, end }, Text::Message { key, .. })
                if key.ends_with("incomplete") =>
            {
                Some((*start, *end))
            }
            _ => None,
        })
        .collect();
    Decoded {
        items: pass.sink().items().to_vec(),
        errors,
    }
}

/// A frame count and the length that holds exactly that many frames.
fn length_of(setup: &Setup, frames: u64) -> usize {
    let params = Params::from_setup(setup, 0).unwrap();
    usize::try_from(params.frame_length() * frames).unwrap()
}

fn options_for(seed: u64) -> Vec<(&'static str, Value)> {
    vec![
        ("bits", Value::Integer(4 + (seed % 13) as i64)),
        (
            "bit_order",
            Value::Choice(if seed % 2 == 0 { "msb" } else { "lsb" }.into()),
        ),
        (
            "clock_edge",
            Value::Choice(
                if seed / 2 % 2 == 0 {
                    "rising"
                } else {
                    "falling"
                }
                .into(),
            ),
        ),
        ("idle", Value::Integer(20 + (seed % 7) as i64 * 30)),
    ]
}

#[test]
fn the_decoder_reads_back_what_the_generator_wrote_for_many_seeds() {
    for seed in 0..seeds() {
        let setup = setup(&options_for(seed), false);
        let length = length_of(&setup, 12);
        let words = generate(&setup, seed, length, 4_096);
        let decoded = decode(&setup, &words);
        assert_eq!(
            decoded.items,
            expected(&setup, seed, length as u64),
            "seed {seed}"
        );
        assert_eq!(decoded.items.len(), 12, "seed {seed}");
        assert!(
            decoded.errors.is_empty(),
            "seed {seed}: {:?}",
            decoded.errors
        );
    }
}

#[test]
fn planted_errors_are_reported_where_they_were_planted_and_the_bytes_around_them_decode() {
    let mut planted = 0;
    for seed in 0..seeds().min(400) {
        let setup = setup(&options_for(seed), true);
        let length = length_of(&setup, 40);
        let words = generate(&setup, seed, length, 4_096);
        let decoded = decode(&setup, &words);
        assert_eq!(
            decoded.items,
            expected(&setup, seed, length as u64),
            "seed {seed}"
        );
        let errors = expected_errors(&setup, seed, length as u64);
        assert_eq!(decoded.errors, errors, "seed {seed}");
        planted += errors.len();
        assert_eq!(decoded.items.len() + errors.len(), 40, "seed {seed}");
    }
    assert!(
        planted > 100,
        "the rate of errors was too low to test anything: {planted}"
    );
}

#[test]
fn the_signal_does_not_depend_on_how_it_is_asked_for() {
    for seed in [0_u64, 1, 77, u64::MAX] {
        let setup = setup(&options_for(seed), true);
        let length = length_of(&setup, 9);
        let reference = generate(&setup, seed, length, length);
        for chunk in [1, 7, 4_096] {
            assert_eq!(
                generate(&setup, seed, length, chunk),
                reference,
                "seed {seed} chunk {chunk}"
            );
        }
        // The same seed twice gives the same signal; another seed gives another one.
        assert_eq!(generate(&setup, seed, length, 100), reference);
        assert_ne!(
            generate(&setup, seed.wrapping_add(1), length, 100),
            reference
        );
    }
}

#[test]
fn only_the_clock_and_data_channels_carry_the_signal() {
    let setup = setup(&[], false);
    let words = generate(&setup, 5, 3_000, 512);
    let mask = (1_u32 << 2) | (1_u32 << 5);
    assert!(words.iter().all(|w| w & !mask == 0));
    assert!(words.iter().any(|w| w & (1 << 2) != 0));
    assert!(words.iter().any(|w| w & (1 << 5) != 0));
}

#[test]
fn two_instances_with_the_same_seed_differ() {
    let a = setup(&[], false);
    let mut b = setup(&[], false);
    b.instance = 4;
    assert_ne!(generate(&a, 9, 4_000, 4_000), generate(&b, 9, 4_000, 4_000));
}

#[test]
#[ignore = "10,000 seeds: run in release"]
fn ten_thousand_seeds() {
    for seed in 0..10_000 {
        let setup = setup(&options_for(seed), seed % 3 == 0);
        let length = length_of(&setup, 10);
        let words = generate(&setup, seed, length, 4_096);
        let decoded = decode(&setup, &words);
        assert_eq!(
            decoded.items,
            expected(&setup, seed, length as u64),
            "seed {seed}"
        );
        assert_eq!(
            decoded.errors,
            expected_errors(&setup, seed, length as u64),
            "seed {seed}"
        );
    }
}
