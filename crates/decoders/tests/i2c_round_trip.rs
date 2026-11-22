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

use common::i2c::{Decoded, GREEN, GREY, HALF, RED, YELLOW};
use la_decoders::generators::i2c::{PlantedError, expected_errors};
use la_decoders::model::{Item, Payload};

/// The errors the decoder reported, in the form of the planted ones: the text of an unacknowledged
/// bit is on the arrow of that bit (SCL), a byte cut short is a block of the lane that ends where the
/// next stop begins (or at the end of the capture, `length`). The red blocks that only repeat an error
/// (the NAK block, the packet) are checked by [`check_packets`].
fn reported(decoded: &Decoded, length: u64) -> Vec<PlantedError> {
    let mut list: Vec<PlantedError> = Vec::new();
    for e in decoded.overlay(SCL).iter().filter(|e| e.error) {
        list.push(match e.key.as_str() {
            "ann-i2c-error-address-nak" => PlantedError::AddressNak { at: e.positions[0] },
            "ann-i2c-error-data-nak" => PlantedError::DataNak { at: e.positions[0] },
            other => panic!(
                "an error on an arrow that was not planted: {other} at {:?}",
                e.positions
            ),
        });
    }
    let lane = decoded.lane();
    for e in lane.iter().filter(|e| e.error) {
        match e.key.as_str() {
            "ann-i2c-error-byte-cut" => {
                let end = lane
                    .iter()
                    .filter(|b| b.key == "ann-i2c-stop" && b.positions[1] > e.positions[0])
                    .map(|b| b.positions[1])
                    .min()
                    .unwrap_or(length);
                list.push(PlantedError::ByteCut {
                    start: e.positions[0] + HALF,
                    end,
                });
            }
            // The blocks that repeat the error of a NAK or of a cut byte.
            "ann-i2c-nak"
            | "ann-i2c-transaction-write"
            | "ann-i2c-transaction-read"
            | "ann-i2c-incomplete" => {}
            other => panic!(
                "an error that was not planted: {other} at {:?}",
                e.positions
            ),
        }
    }
    list.sort_by_key(PlantedError::position);
    list
}

/// SC-001: a packet block is red exactly when an error lies inside it, and green or grey otherwise;
/// the controller's NAK that ends a read is red without being an error.
fn check_packets(decoded: &Decoded, length: u64, seed: u64) {
    let errors = reported(decoded, length);
    for packet in decoded
        .lane()
        .iter()
        .filter(|e| matches!(&e.block, Some((band, _)) if band == "whole"))
    {
        let (from, to) = (packet.positions[0], packet.positions[1]);
        let inside = errors.iter().any(|e| {
            let at = e.position();
            at >= from && at < to
        });
        let (_, colour) = packet.block.clone().unwrap();
        assert_eq!(
            packet.error, inside,
            "seed {seed}: the packet {from}..{to} has the wrong error flag"
        );
        let wanted = if inside {
            RED
        } else if packet.key == "ann-i2c-incomplete" {
            GREY
        } else {
            GREEN
        };
        assert_eq!(colour, wanted, "seed {seed}: the packet {from}..{to}");
    }
}

/// The blocks of the upper band that the items of a decode call for (research R2, `T` = 16): the
/// address and data blocks and their acknowledge blocks, the starts and the stops. 10-bit addresses
/// span two bytes in one item and are left to the hand-built tests.
fn check_blocks(decoded: &Decoded, items: &[Item], seed: u64) {
    let lane = decoded.lane();
    // No two blocks of the upper band overlap, and each lies inside the packet block below it.
    let mut parts: Vec<&common::i2c::Expected> = lane
        .iter()
        .filter(|e| matches!(&e.block, Some((band, _)) if band == "part"))
        .collect();
    parts.sort_by_key(|e| e.positions[0]);
    for pair in parts.windows(2) {
        assert!(
            pair[0].positions[1] <= pair[1].positions[0],
            "seed {seed}: {:?} overlaps {:?}",
            pair[0].positions,
            pair[1].positions
        );
    }
    let packets: Vec<&common::i2c::Expected> = lane
        .iter()
        .filter(|e| matches!(&e.block, Some((band, _)) if band == "whole"))
        .collect();
    for block in &parts {
        assert!(
            packets
                .iter()
                .any(|w| w.positions[0] <= block.positions[0]
                    && block.positions[1] <= w.positions[1]),
            "seed {seed}: {:?} {} lies in no packet",
            block.positions,
            block.key
        );
    }
    let has = |from: u64, to: u64, colour: u8, key: &str| {
        lane.iter().any(|e| {
            e.key == key
                && e.positions == vec![from, to]
                && e.block == Some(("part".to_owned(), colour))
        })
    };
    for item in items {
        let Payload::Bytes(b) = &item.payload else {
            continue;
        };
        let ten_bit = b[3] & 1 == 1;
        let colour = if b[1] == 1 { GREEN } else { YELLOW };
        match b[0] {
            1 | 2 => {
                let key = if b[0] == 1 {
                    "ann-i2c-start"
                } else {
                    "ann-i2c-repeated-start"
                };
                assert!(
                    lane.iter()
                        .any(|e| e.key == key && e.positions[0] == item.start - HALF),
                    "seed {seed}: no {key} block at {}",
                    item.start
                );
            }
            3 | 4 if !ten_bit => {
                let key = if b[0] == 3 {
                    if b[1] == 1 {
                        "ann-i2c-address-read"
                    } else {
                        "ann-i2c-address-write"
                    }
                } else {
                    "ann-i2c-data"
                };
                let ack = item.end - 1;
                assert!(
                    has(item.start - HALF, ack - HALF, colour, key),
                    "seed {seed}: no {key} block for the item at {}",
                    item.start
                );
                let ack_key = if b[2] == 1 {
                    "ann-i2c-nak"
                } else {
                    "ann-i2c-ack"
                };
                let ack_colour = if b[2] == 1 { RED } else { GREEN };
                assert!(
                    has(ack - HALF, ack + HALF, ack_colour, ack_key),
                    "seed {seed}: no {ack_key} block at {ack}"
                );
            }
            5 => assert!(
                lane.iter()
                    .any(|e| e.key == "ann-i2c-stop" && e.positions[1] == item.start),
                "seed {seed}: no stop block ending at {}",
                item.start
            ),
            _ => {}
        }
    }
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
        assert_eq!(reported(&decoded, length as u64), wanted, "seed {seed}");
        check_packets(&decoded, length as u64, seed);
        check_blocks(&decoded, &decoded.items, seed);
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
            for error in reported(&decoded, length as u64) {
                let cut_by_the_end =
                    matches!(error, PlantedError::ByteCut { end, .. } if end == length as u64);
                assert!(
                    cut_by_the_end || wanted.contains(&error),
                    "seed {seed} length {length}: {error:?}"
                );
            }
            for error in &wanted {
                assert!(
                    reported(&decoded, length as u64).contains(error),
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
            reported(&decoded, length as u64),
            planted(&with, seed, length as u64),
            "seed {seed}"
        );
        let _: &Payload = &items[0].payload;
    }
}

#[test]
fn ten_bit_addresses_round_trip_with_and_without_errors() {
    let mut ten_bit_items = 0;
    for seed in 0..300_u64 {
        let setup = setup_with(&[
            ("ten_bit", 1),
            ("max_bytes", 1 + (seed % 6) as i64),
            ("errors", (seed % 2) as i64),
            ("error_rate", 3),
        ]);
        let length = frames(&setup, 8);
        let words = generate(&setup, seed, length, 4_096);
        let decoded = decode_with(&words, &[], &ChunkLimits::default());
        let items = expected(&setup, seed, length as u64);
        assert_eq!(decoded.items, items, "seed {seed}");
        assert_eq!(
            reported(&decoded, length as u64),
            planted(&setup, seed, length as u64),
            "seed {seed}"
        );
        check_packets(&decoded, length as u64, seed);
        ten_bit_items += items
            .iter()
            .filter(|i| matches!(&i.payload, Payload::Bytes(b) if b[0] == 3 && b[3] == 1))
            .count();
    }
    assert!(ten_bit_items > 400, "{ten_bit_items}");
}

/// FR-025: SDA changes in the sample where SCL falls, or, in a start, a repeated start and a stop, 8
/// samples from an edge of SCL while SCL is high; and no clock pulse is narrower than a data pulse.
#[test]
fn sda_changes_with_the_falling_edge_of_scl_or_eight_samples_from_an_edge_in_start_and_stop() {
    for seed in 0..seeds().min(300) {
        let setup = setup_with(&[
            ("max_bytes", 1 + (seed % 6) as i64),
            ("ten_bit", (seed % 2) as i64),
            ("errors", 1),
            ("error_rate", 2 + (seed % 3) as i64),
        ]);
        let words = generate(&setup, seed, frames(&setup, 6), 4_096);
        let bit = |word: u32, line: u8| word >> line & 1;
        let scl_at = |at: usize| bit(words[at], SCL);
        let (mut sda_edges, mut scl_edges) = (Vec::new(), Vec::new());
        for at in 1..words.len() {
            if bit(words[at], SDA) != bit(words[at - 1], SDA) {
                sda_edges.push(at);
            }
            if scl_at(at) != scl_at(at - 1) {
                scl_edges.push(at);
            }
        }
        for &sda in &sda_edges {
            let with_fall = scl_at(sda - 1) == 1 && scl_at(sda) == 0;
            let from_edge = scl_at(sda) == 1
                && (scl_edges.contains(&(sda + 8)) || scl_edges.contains(&(sda - 8)));
            assert!(
                with_fall || from_edge,
                "seed {seed}: SDA changes at {sda} with SCL {}",
                scl_at(sda)
            );
        }
        // A clock pulse with a low before and after it is never narrower than eight samples.
        for pair in scl_edges.windows(2) {
            if scl_at(pair[0]) == 1 {
                assert!(
                    pair[1] - pair[0] >= 8,
                    "seed {seed}: a pulse at {}",
                    pair[0]
                );
            }
        }
    }
}
