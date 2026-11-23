//! The compact coding on the board (feature 017, T032): the streaming coder gives exactly the bytes of the
//! protocol crate's coder over the whole, in any size of input and output pieces, and the stream decodes to the
//! samples.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::panic
)]

use la_fw_logic::CompactEncoder;
use la_proto::{decode_compact, encode_compact};

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
}

/// Samples of `per` bytes in the four kinds the board meets: constant, sparse, random and the worst case.
fn signal(kind: &str, per: usize, samples: usize, rng: &mut Rng) -> Vec<u8> {
    let mut out = Vec::with_capacity(per * samples);
    let mut level = vec![0_u8; per];
    for i in 0..samples {
        match kind {
            "constant" => {}
            "sparse" => {
                if rng.next() % 200 == 0 {
                    level[(rng.next() % per as u64) as usize] ^= 1 << (rng.next() % 8);
                }
            }
            "random" => {
                for b in &mut level {
                    *b = rng.next() as u8;
                }
            }
            _ => {
                level.fill(if i % 2 == 0 { 0x00 } else { 0xFF });
            }
        }
        out.extend_from_slice(&level);
    }
    out
}

/// Codes `raw` with the streaming coder, feeding input pieces of `input` samples into an output of `output`
/// bytes that is emptied after each call.
fn stream(raw: &[u8], per: usize, input: usize, output: usize) -> Vec<u8> {
    let mut encoder = CompactEncoder::new(per).unwrap();
    let mut coded = Vec::new();
    let mut out = vec![0_u8; output];
    for piece in raw.chunks(input * per) {
        let mut rest = piece;
        while !rest.is_empty() {
            let pushed = encoder.push(rest, &mut out);
            coded.extend_from_slice(&out[..pushed.written]);
            assert!(pushed.consumed <= rest.len() && pushed.consumed % per == 0);
            assert!(
                pushed.consumed > 0,
                "an output of {output} bytes must take at least one sample"
            );
            rest = &rest[pushed.consumed..];
        }
    }
    coded
}

#[test]
fn the_streaming_coder_gives_the_bytes_of_the_whole_coder_in_any_pieces() {
    let mut rng = Rng(2026);
    for kind in ["constant", "sparse", "random", "worst"] {
        for per in 1..=4_usize {
            let raw = signal(kind, per, 700, &mut rng);
            let mut whole = vec![0_u8; raw.len() * 8 + 16];
            let n = encode_compact(&raw, per, &mut whole).unwrap();
            whole.truncate(n);
            for input in [1_usize, 2, 7, 64, 700] {
                for output in [per + 5, 16, 33, 4096] {
                    assert_eq!(
                        stream(&raw, per, input, output),
                        whole,
                        "{kind}, {per} bytes, input {input}, output {output}"
                    );
                }
            }
        }
    }
}

#[test]
fn what_the_streaming_coder_writes_decodes_to_the_samples() {
    let mut rng = Rng(7);
    for kind in ["constant", "sparse", "random", "worst"] {
        for per in [1_usize, 3, 4] {
            let raw = signal(kind, per, 1_000, &mut rng);
            let coded = stream(&raw, per, 33, 40);
            let mut back = vec![0_u8; raw.len()];
            decode_compact(&coded, per, 1_000, &mut back).unwrap();
            assert_eq!(back, raw, "{kind}, {per} bytes");
        }
    }
}

#[test]
fn a_sample_size_the_coder_cannot_take_is_refused() {
    assert!(CompactEncoder::new(0).is_none());
    assert!(CompactEncoder::new(5).is_none());
    assert!(CompactEncoder::new(1).is_some() && CompactEncoder::new(4).is_some());
}

#[test]
fn an_output_that_is_too_small_takes_nothing_and_never_overruns() {
    let mut encoder = CompactEncoder::new(4).unwrap();
    let mut out = [0_u8; 3];
    let pushed = encoder.push(&[1, 2, 3, 4], &mut out);
    assert_eq!((pushed.consumed, pushed.written), (0, 0));
    let mut out = [0_u8; 4];
    let pushed = encoder.push(&[1, 2, 3, 4, 5, 6, 7, 8], &mut out);
    assert_eq!(
        (pushed.consumed, pushed.written),
        (4, 4),
        "the first sample is taken, the next needs room for a change"
    );
}
