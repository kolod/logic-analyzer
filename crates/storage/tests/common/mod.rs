//! Shared test helpers: a naive reference model and pattern generators.
//!
//! `ReferenceCapture` stores every sample word and answers all queries by brute force,
//! following `contracts/viewport-semantics.md`. Tests compare the store against it.
#![allow(
    dead_code,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::panic
)]

use la_storage::{Activity, Capture, Edge, Gap, LevelState, PixelSummary};

/// Deterministic xorshift generator: no extra dependency.
#[derive(Debug, Clone)]
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        Self(seed | 1)
    }

    pub fn next_u64(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    /// A value in `0..n` (n > 0).
    pub fn below(&mut self, n: u64) -> u64 {
        self.next_u64() % n
    }

    pub fn range(&mut self, lo: u64, hi_inclusive: u64) -> u64 {
        lo + self.below(hi_inclusive - lo + 1)
    }

    pub fn chance_per_mille(&mut self, per_mille: u64) -> bool {
        self.below(1000) < per_mille
    }
}

/// Mask with the lowest `channels` bits set.
pub fn mask(channels: u8) -> u32 {
    if channels >= 32 {
        u32::MAX
    } else {
        (1_u32 << channels) - 1
    }
}

// ---- pattern generators ----------------------------------------------------------

pub fn constant(len: usize, word: u32) -> Vec<u32> {
    vec![word; len]
}

/// Every channel flips at every sample.
pub fn alternating(len: usize, channels: u8) -> Vec<u32> {
    (0..len)
        .map(|i| if i % 2 == 0 { 0 } else { mask(channels) })
        .collect()
}

/// Each channel flips independently with the given probability per mille at each sample.
pub fn random(len: usize, channels: u8, per_mille: u64, rng: &mut Rng) -> Vec<u32> {
    let mut word = rng.next_u64() as u32 & mask(channels);
    let mut out = Vec::with_capacity(len);
    for _ in 0..len {
        for ch in 0..channels {
            if rng.chance_per_mille(per_mille) {
                word ^= 1 << ch;
            }
        }
        out.push(word);
    }
    out
}

/// Long runs of random words: each word is held for 1..=max_run samples.
pub fn runs(len: usize, channels: u8, max_run: u64, rng: &mut Rng) -> Vec<u32> {
    let mut out = Vec::with_capacity(len);
    while out.len() < len {
        let word = rng.next_u64() as u32 & mask(channels);
        let run = rng.range(1, max_run) as usize;
        for _ in 0..run.min(len - out.len()) {
            out.push(word);
        }
    }
    out
}

/// Flat low signal on all channels with a single-sample high glitch of `glitch_mask`
/// at position `at`.
pub fn glitch(len: usize, at: usize, glitch_mask: u32) -> Vec<u32> {
    let mut out = vec![0; len];
    if let Some(w) = out.get_mut(at) {
        *w = glitch_mask;
    }
    out
}

/// Splits `total` into chunk sizes cycling through `sizes` (each at least 1).
pub fn split_sizes(total: usize, sizes: &[usize]) -> Vec<usize> {
    let mut out = Vec::new();
    let mut left = total;
    let mut i = 0;
    while left > 0 {
        let s = sizes[i % sizes.len()].max(1).min(left);
        out.push(s);
        left -= s;
        i += 1;
    }
    out
}

/// Appends `samples` to `capture` in chunks of the given sizes.
pub fn feed(capture: &mut Capture, samples: &[u32], sizes: &[usize]) {
    let mut at = 0;
    for size in split_sizes(samples.len(), sizes) {
        let start = capture.length();
        capture.append(start, &samples[at..at + size]).unwrap();
        at += size;
    }
}

// ---- reference model -------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct ReferenceCapture {
    pub channels: u8,
    /// One entry per position: `Some(word)` or `None` for a lost (gap) position.
    pub samples: Vec<Option<u32>>,
}

impl ReferenceCapture {
    pub fn new(channels: u8) -> Self {
        Self {
            channels,
            samples: Vec::new(),
        }
    }

    pub fn push_chunk(&mut self, words: &[u32]) {
        let m = mask(self.channels);
        self.samples.extend(words.iter().map(|w| Some(w & m)));
    }

    pub fn push_gap(&mut self, length: u64) {
        self.samples
            .extend(std::iter::repeat_n(None, length as usize));
    }

    pub fn len(&self) -> u64 {
        self.samples.len() as u64
    }

    pub fn level(&self, ch: u8, pos: u64) -> LevelState {
        match self.samples.get(pos as usize) {
            Some(Some(w)) => {
                if (w >> ch) & 1 == 1 {
                    LevelState::High
                } else {
                    LevelState::Low
                }
            }
            _ => LevelState::Missing,
        }
    }

    /// Positions of all transitions of a channel, increasing. A transition is stored at
    /// `p` when sample `p` differs from the last known sample before it (this also
    /// covers the first sample after a gap).
    pub fn all_transitions(&self, ch: u8) -> Vec<u64> {
        let mut out = Vec::new();
        let mut last: Option<u32> = None;
        for (p, s) in self.samples.iter().enumerate() {
            if let Some(w) = s {
                if let Some(prev) = last {
                    if (prev ^ w) >> ch & 1 == 1 {
                        out.push(p as u64);
                    }
                }
                last = Some(*w);
            }
        }
        out
    }

    /// Transitions with `start < p <= end`.
    pub fn transitions(&self, ch: u8, start: u64, end: u64) -> Vec<Edge> {
        self.all_transitions(ch)
            .into_iter()
            .filter(|&p| p > start && p <= end)
            .map(|p| Edge {
                position: p,
                rising: self.level(ch, p) == LevelState::High,
            })
            .collect()
    }

    /// Number of transitions with position `<= x`.
    pub fn rank(&self, ch: u8, x: u64) -> u64 {
        self.all_transitions(ch).iter().filter(|&&p| p <= x).count() as u64
    }

    pub fn gaps(&self) -> Vec<Gap> {
        let mut out: Vec<Gap> = Vec::new();
        for (p, s) in self.samples.iter().enumerate() {
            if s.is_none() {
                match out.last_mut() {
                    Some(g) if g.start + g.length == p as u64 => g.length += 1,
                    _ => out.push(Gap {
                        start: p as u64,
                        length: 1,
                    }),
                }
            }
        }
        out
    }

    /// Brute-force pixel summaries per `contracts/viewport-semantics.md`.
    pub fn viewport(&self, ch: u8, start: u64, end: u64, pixels: u32) -> Vec<PixelSummary> {
        let len = end - start;
        let trans = self.all_transitions(ch);
        let gaps = self.gaps();
        let bound = |i: u32| -> u64 {
            start + (u128::from(len) * u128::from(i) / u128::from(pixels)) as u64
        };
        let rank = |x: u64| trans.iter().filter(|&&p| p <= x).count() as u64;
        (0..pixels)
            .map(|i| {
                let (a, b) = (bound(i), bound(i + 1));
                let count = rank(b) - rank(a);
                let activity = match count {
                    0 => Activity::None,
                    1 => {
                        let p = *trans.iter().find(|&&p| p > a && p <= b).unwrap();
                        Activity::One(Edge {
                            position: p,
                            rising: self.level(ch, p) == LevelState::High,
                        })
                    }
                    n => Activity::Many {
                        count: u32::try_from(n).unwrap_or(u32::MAX),
                    },
                };
                PixelSummary {
                    start_level: self.level(ch, a),
                    end_level: self.level(ch, b),
                    activity,
                    missing_data: gaps.iter().any(|g| a < g.start + g.length && b >= g.start),
                    beyond_end: a >= self.len(),
                }
            })
            .collect()
    }
}
