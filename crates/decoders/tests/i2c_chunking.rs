//! The result for a given I2C signal does not depend on how it was cut into chunks or how fast the
//! capture grew (spec 008, FR-009, SC-005), and the decoder never panics or breaks the output on
//! arbitrary signals (SC-004).
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::panic
)]

mod common;

use std::sync::atomic::AtomicBool;

use common::i2c::{Decoded, collect, decode_with, new_pass, setup};
use common::{Lcg, Part, append_parts, capture_from_parts};
use la_decoders::generators::i2c::{Params, source};
use la_decoders::run::{ChunkLimits, End};
use la_devices::SignalSource;

fn generated(seed: u64, frames: u64) -> Vec<u32> {
    let setup = setup(&[]);
    let length = (Params::from_setup(&setup, seed).unwrap().frame_length() * frames) as usize;
    let mut words = vec![0_u32; length];
    source(&setup, seed).unwrap().fill(0, &mut words);
    words
}

fn same(a: &Decoded, b: &Decoded, what: &str) {
    assert_eq!(a.all, b.all, "{what}: annotations");
    assert_eq!(a.items, b.items, "{what}: items");
}

#[test]
fn every_chunk_size_gives_the_same_result() {
    for seed in 0..40 {
        let words = generated(seed, 5);
        let reference = decode_with(&words, &[], &ChunkLimits::default());
        assert!(!reference.items.is_empty(), "seed {seed}");
        for (max_edges, max_span) in [(1, 1), (1, 1 << 20), (7, 64), (100, 1 << 20), (65_536, 100)]
        {
            let limits = ChunkLimits {
                max_edges,
                max_span,
            };
            let decoded = decode_with(&words, &[], &limits);
            same(
                &decoded,
                &reference,
                &format!("seed {seed} edges {max_edges} span {max_span}"),
            );
        }
    }
}

/// Feeds the signal to a growing capture in pieces and decodes after every piece.
fn grown(words: &[u32], piece: usize) -> Decoded {
    let mut capture = capture_from_parts(2, 1_000_000, &[]);
    let setup = setup(&[]);
    let mut pass = new_pass(&setup);
    let limits = ChunkLimits::default();
    let cancel = AtomicBool::new(false);
    for part in words.chunks(piece) {
        append_parts(&mut capture, &[Part::Samples(part.to_vec())]);
        pass.advance(
            &capture.reader().snapshot(),
            &limits,
            &cancel,
            &mut |_, _| {},
        )
        .unwrap();
    }
    capture.close().unwrap();
    let snapshot = capture.reader().snapshot();
    pass.advance(&snapshot, &limits, &cancel, &mut |_, _| {})
        .unwrap();
    pass.finish(&snapshot, End::Complete).unwrap();
    collect(&mut pass)
}

#[test]
fn a_capture_that_grows_gives_the_result_of_the_whole_capture() {
    for seed in 0..6 {
        let words = generated(seed, 4);
        let reference = decode_with(&words, &[], &ChunkLimits::default());
        for piece in [7, 1_000, 100_000] {
            same(
                &grown(&words, piece),
                &reference,
                &format!("seed {seed} piece {piece}"),
            );
        }
    }
    // Pieces of one sample, for a short capture.
    let words = generated(3, 1);
    let reference = decode_with(&words, &[], &ChunkLimits::default());
    same(&grown(&words, 1), &reference, "pieces of one sample");
}

#[test]
fn random_signals_never_break_the_decoder() {
    // Signals of random runs on both lines, some with a regular clock on SCL, some with gaps; read in
    // chunks of one edge, so that the chunk loop runs more than a million times in all.
    let mut random = Lcg(2026);
    let limits = ChunkLimits {
        max_edges: 1,
        max_span: 1 << 20,
    };
    let mut chunks = 0_u64;
    let mut with_output = 0_u32;
    for round in 0..5_000_u32 {
        let length = 300 + random.below(1_500) as usize;
        let mut words = Vec::with_capacity(length);
        let (mut scl, mut sda) = (random.below(2) == 1, random.below(2) == 1);
        let clocked = round % 3 == 0;
        let period = 4 + random.below(20);
        while words.len() < length {
            if clocked {
                scl = (words.len() as u64 / (period / 2).max(1)) % 2 == 1;
                if random.below(6) == 0 {
                    sda = !sda;
                }
            } else {
                if random.below(7) == 0 {
                    scl = !scl;
                }
                if random.below(9) == 0 {
                    sda = !sda;
                }
            }
            words.push(u32::from(scl) | u32::from(sda) << 1);
        }
        let mut parts = vec![Part::Samples(words[..length / 2].to_vec())];
        if round % 7 == 0 {
            parts.push(Part::Gap(1 + random.below(40)));
        }
        parts.push(Part::Samples(words[length / 2..].to_vec()));
        let mut capture = capture_from_parts(2, 1_000_000, &parts);
        capture.close().unwrap();
        let setup = setup(&[]);
        let mut pass = new_pass(&setup);
        la_decoders::run::run_pass(
            &mut pass,
            &capture.reader(),
            &limits,
            &AtomicBool::new(false),
            &mut || false,
            &mut |_, _| {},
        )
        .unwrap();
        chunks += pass.chunks();
        let decoded = collect(&mut pass);
        assert_eq!(decoded.refused, 0, "round {round}: invalid output");
        if !decoded.all.is_empty() {
            with_output += 1;
        }
    }
    assert!(chunks >= 1_000_000, "only {chunks} chunks");
    assert!(
        with_output > 100,
        "only {with_output} signals gave any output"
    );
}
