//! Helpers shared by the tests of `la-decoders` (not a test target itself).
//!
//! Captures are built from plain descriptions of signals, so that a test states in a few lines what
//! the signal is and then checks what a decoder, the reader or the store makes of it.
#![allow(
    dead_code,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::panic
)]

pub mod i2c;

use la_storage::{Capture, CaptureConfig, Limits};

/// A tiny deterministic generator (tests must not depend on a random crate).
pub struct Lcg(pub u64);

impl Lcg {
    pub fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        self.0 >> 20
    }

    /// A value in `0..bound` (0 when `bound` is 0).
    pub fn below(&mut self, bound: u64) -> u64 {
        self.next() % bound.max(1)
    }
}

/// A piece of a capture: samples (bit `i` of a word is channel `i`) or a gap of lost positions.
#[derive(Debug, Clone)]
pub enum Part {
    Samples(Vec<u32>),
    Gap(u64),
}

/// The sample words of `length` positions for a list of edges `(position, channel, high)`.
///
/// Every channel starts low; an edge sets the level from its position on. Edges must be in
/// non-decreasing position order and may not repeat a level (an edge to the level a channel
/// already has is a mistake in the test and is rejected).
pub fn words_from_edges(length: u64, edges: &[(u64, u8, bool)]) -> Vec<u32> {
    let size = usize::try_from(length).unwrap();
    let mut words = vec![0_u32; size];
    let mut level = 0_u32;
    let mut next = 0_usize;
    for (position, channel, high) in edges {
        let at = usize::try_from(*position).unwrap();
        assert!(at >= next, "edges out of order at {position}");
        for word in &mut words[next..at.min(size)] {
            *word = level;
        }
        next = at;
        let bit = 1_u32 << channel;
        assert_eq!(level & bit != 0, !high, "edge to the level the channel has");
        if *high {
            level |= bit;
        } else {
            level &= !bit;
        }
    }
    for word in &mut words[next.min(size)..] {
        *word = level;
    }
    words
}

/// An open capture made of `parts` (not closed: it can still be appended to).
pub fn capture_from_parts(channels: u8, sample_rate_hz: u64, parts: &[Part]) -> Capture {
    let mut capture = Capture::new(CaptureConfig {
        channels,
        sample_rate_hz,
        limits: Limits::default(),
    })
    .unwrap();
    append_parts(&mut capture, parts);
    capture
}

/// Appends `parts` to an open capture.
pub fn append_parts(capture: &mut Capture, parts: &[Part]) {
    for part in parts {
        match part {
            Part::Samples(words) => {
                let mut start = capture.length();
                for chunk in words.chunks(60_000) {
                    capture.append(start, chunk).unwrap();
                    start += chunk.len() as u64;
                }
            }
            Part::Gap(length) => capture.record_gap(*length).unwrap(),
        }
    }
}

/// A closed capture of `length` positions with the given edges.
pub fn capture_from_edges(
    channels: u8,
    sample_rate_hz: u64,
    length: u64,
    edges: &[(u64, u8, bool)],
) -> Capture {
    let words = words_from_edges(length, edges);
    let mut capture = capture_from_parts(channels, sample_rate_hz, &[Part::Samples(words)]);
    capture.close().unwrap();
    capture
}
