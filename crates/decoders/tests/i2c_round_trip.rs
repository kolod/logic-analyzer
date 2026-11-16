//! Generate, encode, decode and compare for the I2C decoder (spec 008, user story 2, constitution
//! Principle XI). For many seeds the generator makes a signal, the decoder reads it back and the
//! items must be exactly what `expected()` says.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::panic
)]

mod common;

use common::i2c::{SCL, SDA, decode_with, setup};
use la_decoders::generators::i2c::{Params, expected, source};
use la_decoders::run::ChunkLimits;
use la_decoders::setup::Setup;
use la_devices::SignalSource;

fn seeds() -> u64 {
    std::env::var("LA_ROUND_TRIP_SEEDS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(1_000)
}

/// An I2C setup with these generator options.
fn setup_with(generator: &[(&str, i64)]) -> Setup {
    let mut setup = setup(&[]);
    for (key, value) in generator {
        setup.generator.insert((*key).to_owned(), *value);
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

fn frames(setup: &Setup, count: u64) -> usize {
    let params = Params::from_setup(setup, 0).unwrap();
    usize::try_from(params.frame_length() * count).unwrap()
}

#[test]
fn the_decoder_reads_back_what_the_generator_wrote_for_many_seeds() {
    for seed in 0..seeds() {
        let setup = setup_with(&[("max_bytes", 1 + (seed % 6) as i64)]);
        let length = frames(&setup, 8);
        let words = generate(&setup, seed, length, 4_096);
        let decoded = decode_with(&words, &[], &ChunkLimits::default());
        let items = expected(&setup, seed, length as u64);
        assert!(items.len() >= 16, "seed {seed}: too little traffic to test");
        assert_eq!(decoded.items, items, "seed {seed}");
        assert!(
            decoded.all.iter().all(|e| !e.error),
            "seed {seed}: an error without errors planted"
        );
        assert_eq!(decoded.refused, 0, "seed {seed}");
    }
}

#[test]
fn the_signal_does_not_depend_on_how_it_is_asked_for() {
    for seed in [0_u64, 1, 77, u64::MAX] {
        let setup = setup_with(&[]);
        let length = frames(&setup, 3);
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
fn only_the_two_channels_carry_the_signal_and_a_frame_begins_with_a_quiet_bus() {
    let setup = setup_with(&[]);
    let length = frames(&setup, 4);
    let words = generate(&setup, 5, length, 512);
    let mask = (1_u32 << SCL) | (1_u32 << SDA);
    assert!(words.iter().all(|w| w & !mask == 0));
    assert!(words.iter().any(|w| w & (1 << SCL) != 0));
    assert!(words.iter().any(|w| w & (1 << SDA) == 0));
    let frame = Params::from_setup(&setup, 5).unwrap().frame_length() as usize;
    for k in 0..4 {
        assert!(
            words[k * frame..k * frame + 64].iter().all(|w| *w == mask),
            "frame {k} does not begin with both lines high"
        );
    }
}

#[test]
fn two_instances_with_the_same_seed_differ() {
    let a = setup_with(&[]);
    let mut b = setup_with(&[]);
    b.instance = 4;
    let length = frames(&a, 3);
    assert_ne!(
        generate(&a, 9, length, length),
        generate(&b, 9, length, length)
    );
}

#[test]
fn the_options_are_clamped_and_never_an_error() {
    for options in [
        vec![("max_bytes", 0)],
        vec![("max_bytes", 99)],
        vec![("max_bytes", -5)],
        vec![("error_rate", i64::MIN)],
        vec![("no_such_option", 3)],
    ] {
        let setup = setup_with(&options);
        let params = Params::from_setup(&setup, 1).expect("parameters");
        let length = params.frame_length() as usize * 2;
        let words = generate(&setup, 1, length, length);
        assert_eq!(words.len(), length);
        let decoded = decode_with(&words, &[], &ChunkLimits::default());
        assert_eq!(decoded.refused, 0);
    }
    // Without both channels there is no generator.
    let mut incomplete = setup_with(&[]);
    incomplete.channels.remove("sda");
    assert!(Params::from_setup(&incomplete, 1).is_none());
}

#[test]
#[ignore = "10,000 seeds: run in release"]
fn ten_thousand_seeds() {
    for seed in 0..10_000 {
        let setup = setup_with(&[("max_bytes", 1 + (seed % 6) as i64)]);
        let length = frames(&setup, 6);
        let words = generate(&setup, seed, length, 4_096);
        let decoded = decode_with(&words, &[], &ChunkLimits::default());
        assert_eq!(
            decoded.items,
            expected(&setup, seed, length as u64),
            "seed {seed}"
        );
    }
}

// ---------------------------------------------------------------------------------------------
// Planted errors, truncated captures and start-up (spec 008, user story 3, task T023).
// ---------------------------------------------------------------------------------------------

use common::i2c::Decoded;
use la_decoders::generators::i2c::{PlantedError, expected_errors};
use la_decoders::model::{Item, Payload};

/// The errors the decoder reported, in the form of the planted ones.
fn reported(decoded: &Decoded) -> Vec<PlantedError> {
    let mut list: Vec<PlantedError> = decoded
        .lane()
        .iter()
        .filter(|e| e.error)
        .map(|e| match e.key.as_str() {
            "ann-i2c-error-address-nak" => PlantedError::AddressNak { at: e.positions[0] },
            "ann-i2c-error-data-nak" => PlantedError::DataNak { at: e.positions[0] },
            "ann-i2c-error-byte-cut" => PlantedError::ByteCut {
                start: e.positions[0],
                end: e.positions[1],
            },
            other => panic!(
                "an error that was not planted: {other} at {:?}",
                e.positions
            ),
        })
        .collect();
    list.sort_by_key(PlantedError::position);
    list
}

fn planted(setup: &Setup, seed: u64, length: u64) -> Vec<PlantedError> {
    let mut list = expected_errors(setup, seed, length);
    list.sort_by_key(PlantedError::position);
    list
}

#[test]
fn planted_errors_are_reported_where_they_were_planted_and_nowhere_else() {
    let mut total = 0;
    for seed in 0..400_u64 {
        let setup = setup_with(&[
            ("errors", 1),
            ("error_rate", 2 + (seed % 11) as i64),
            ("max_bytes", 1 + (seed % 6) as i64),
        ]);
        let length = frames(&setup, 8);
        let words = generate(&setup, seed, length, 4_096);
        let decoded = decode_with(&words, &[], &ChunkLimits::default());
        assert_eq!(
            decoded.items,
            expected(&setup, seed, length as u64),
            "seed {seed}"
        );
        let wanted = planted(&setup, seed, length as u64);
        assert_eq!(reported(&decoded), wanted, "seed {seed}");
        total += wanted.len();
    }
    assert!(
        total > 100,
        "the rate of errors was too low to test anything: {total}"
    );
}

#[test]
fn without_errors_nothing_is_planted_and_the_same_seed_plants_the_same_errors() {
    for seed in 0..20_u64 {
        let off = setup_with(&[("errors", 0), ("error_rate", 2)]);
        assert!(expected_errors(&off, seed, 100_000).is_empty());
        let on = setup_with(&[("errors", 1), ("error_rate", 2)]);
        let length = frames(&on, 10);
        assert!(!expected_errors(&on, seed, length as u64).is_empty());
        assert_eq!(
            expected_errors(&on, seed, length as u64),
            expected_errors(&on, seed, length as u64)
        );
        assert_eq!(
            generate(&on, seed, length, 100),
            generate(&on, seed, length, length)
        );
        assert_ne!(
            generate(&on, seed, length, length),
            generate(&off, seed, length, length)
        );
    }
}

#[test]
fn a_capture_cut_at_any_point_gives_the_items_completed_before_it_and_no_other_error() {
    for seed in 0..30_u64 {
        let setup = setup_with(&[("errors", 1), ("error_rate", 3), ("max_bytes", 3)]);
        let frame = Params::from_setup(&setup, seed).unwrap().frame_length() as usize;
        for offset in (0..frame).step_by(97) {
            let length = frame * 3 + offset;
            let words = generate(&setup, seed, length, 4_096);
            let decoded = decode_with(&words, &[], &ChunkLimits::default());
            let items: Vec<Item> = expected(&setup, seed, length as u64);
            assert_eq!(decoded.items, items, "seed {seed} length {length}");
            // Every error is a planted one, or a byte cut short by the end of the capture.
            let wanted = planted(&setup, seed, length as u64);
            for error in reported(&decoded) {
                let cut_by_the_end =
                    matches!(error, PlantedError::ByteCut { end, .. } if end == length as u64);
                assert!(
                    cut_by_the_end || wanted.contains(&error),
                    "seed {seed} length {length}: {error:?}"
                );
            }
            for error in &wanted {
                assert!(
                    reported(&decoded).contains(error),
                    "seed {seed} length {length}: {error:?} not reported"
                );
            }
        }
    }
}

#[test]
fn a_start_up_before_the_interface_is_set_up_leaves_no_trace_and_shifts_everything() {
    for seed in 0..100_u64 {
        let plain = setup_with(&[
            ("max_bytes", 1 + (seed % 5) as i64),
            ("errors", (seed % 2) as i64),
        ]);
        let mut with = plain.clone();
        with.generator.insert("startup".to_owned(), 1);
        let shift = Params::from_setup(&with, seed).unwrap().startup_length();
        assert!(shift > 0 && shift % 16 == 0 && shift <= 128, "{shift}");
        assert_eq!(
            Params::from_setup(&plain, seed).unwrap().startup_length(),
            0
        );
        let frames_length = frames(&plain, 8);
        let length = frames_length + shift as usize;
        let words = generate(&with, seed, length, 4_096);
        let decoded = decode_with(&words, &[], &ChunkLimits::default());
        let items = expected(&with, seed, length as u64);
        assert_eq!(decoded.items, items, "seed {seed}");
        // The same traffic as without a start-up, moved by its length.
        let shifted: Vec<Item> = expected(&plain, seed, frames_length as u64)
            .into_iter()
            .map(|mut i| {
                i.start += shift;
                i.end += shift;
                i
            })
            .collect();
        assert_eq!(items, shifted, "seed {seed}");
        // Nothing is drawn or reported inside the start-up.
        assert!(
            decoded.all.iter().all(|e| e.positions[0] >= shift),
            "seed {seed}: something before the end of the start-up"
        );
        assert_eq!(
            reported(&decoded),
            planted(&with, seed, length as u64),
            "seed {seed}"
        );
        let _: &Payload = &items[0].payload;
    }
}
