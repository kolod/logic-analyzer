//! The default random generator of the virtual device.
//!
//! Each enabled channel keeps its level and the position of its next flip. After every flip
//! the distance to the next one is `1 + draw % (2 * mean_gap - 1)` samples. All arithmetic
//! is integer (SplitMix64 derives a stream per channel from the seed, xorshift64 produces the
//! draws), so the output is identical on every platform, and because every channel advances
//! on its own, identical for any chunking of the output. A chunk is produced by filling
//! constant stretches and applying one XOR per flip, so the cost is O(samples + flips).

/// SplitMix64 step: derives well-mixed values from a counter state.
fn splitmix64(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = *state;
    z = (z ^ z.wrapping_shr(30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ z.wrapping_shr(27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ z.wrapping_shr(31)
}

/// xorshift64 step (state must not be zero).
fn xorshift64(state: &mut u64) -> u64 {
    let mut x = *state;
    x ^= x.wrapping_shl(13);
    x ^= x.wrapping_shr(7);
    x ^= x.wrapping_shl(17);
    *state = x;
    x
}

#[derive(Debug)]
struct ChannelGenerator {
    bit: u32,
    rng: u64,
    next_flip: u64,
}

/// Generates random digital signals for the enabled channels.
#[derive(Debug)]
pub(crate) struct RandomGenerator {
    channels: Vec<ChannelGenerator>,
    modulus: u64,
    levels: u32,
    position: u64,
    scratch: Vec<(u64, u32)>,
}

impl RandomGenerator {
    /// Creates a generator for the channels in `enabled_mask`; `mean_gap` is clamped to
    /// at least 1.
    pub(crate) fn new(enabled_mask: u32, seed: u64, mean_gap: u32) -> Self {
        let modulus = u64::from(mean_gap.max(1))
            .saturating_mul(2)
            .saturating_sub(1)
            .max(1);
        let mut levels = 0_u32;
        let mut channels = Vec::new();
        for channel in 0..32_u32 {
            let bit = 1_u32.wrapping_shl(channel);
            if enabled_mask & bit == 0 {
                continue;
            }
            // One independent stream per channel, derived from the seed and the channel.
            let mut mix = seed ^ u64::from(channel).wrapping_mul(0xD1B5_4A32_D192_ED03);
            let mut rng = splitmix64(&mut mix);
            if rng == 0 {
                rng = 0x1234_5678_9ABC_DEF1;
            }
            if xorshift64(&mut rng) & 1 == 1 {
                levels |= bit;
            }
            let mut generator = ChannelGenerator {
                bit,
                rng,
                next_flip: 0,
            };
            generator.next_flip = gap(&mut generator.rng, modulus);
            channels.push(generator);
        }
        Self {
            channels,
            modulus,
            levels,
            position: 0,
            scratch: Vec::new(),
        }
    }

    /// Fills `out` with the next `out.len()` samples.
    pub(crate) fn fill(&mut self, out: &mut [u32]) {
        let start = self.position;
        let len = u64::try_from(out.len()).unwrap_or(u64::MAX);
        let end = start.saturating_add(len);
        let start_levels = self.levels;

        self.scratch.clear();
        for channel in &mut self.channels {
            while channel.next_flip < end {
                self.scratch.push((channel.next_flip, channel.bit));
                channel.next_flip = channel
                    .next_flip
                    .saturating_add(gap(&mut channel.rng, self.modulus));
                self.levels ^= channel.bit;
            }
        }
        self.scratch.sort_unstable_by_key(|event| event.0);

        let mut current = start_levels;
        let mut index = 0_usize;
        let mut events = self.scratch.iter().peekable();
        while let Some(&(position, bit)) = events.next() {
            let target = usize::try_from(position.saturating_sub(start)).unwrap_or(out.len());
            if let Some(run) = out.get_mut(index..target.min(out.len())) {
                run.fill(current);
            }
            index = target.min(out.len());
            current ^= bit;
            // Events at the same position toggle together.
            while let Some(&&(next_position, next_bit)) = events.peek() {
                if next_position != position {
                    break;
                }
                current ^= next_bit;
                events.next();
            }
        }
        if let Some(rest) = out.get_mut(index..) {
            rest.fill(current);
        }
        self.position = end;
    }

    /// Advances by `count` samples without producing them (used for scripted losses).
    pub(crate) fn skip(&mut self, count: u64) {
        let end = self.position.saturating_add(count);
        for channel in &mut self.channels {
            while channel.next_flip < end {
                channel.next_flip = channel
                    .next_flip
                    .saturating_add(gap(&mut channel.rng, self.modulus));
                self.levels ^= channel.bit;
            }
        }
        self.position = end;
    }
}

/// Distance to the next flip: `1 + draw % modulus`.
fn gap(rng: &mut u64, modulus: u64) -> u64 {
    1_u64.saturating_add(xorshift64(rng).checked_rem(modulus).unwrap_or(0))
}

#[cfg(test)]
#[allow(clippy::arithmetic_side_effects)]
mod tests {
    use super::*;

    fn generate(mask: u32, seed: u64, mean_gap: u32, splits: &[usize], total: usize) -> Vec<u32> {
        let mut g = RandomGenerator::new(mask, seed, mean_gap);
        let mut out = vec![0_u32; total];
        let mut at = 0;
        let mut i = 0;
        while at < total {
            let size = splits[i % splits.len()].min(total - at);
            g.fill(&mut out[at..at + size]);
            at += size;
            i += 1;
        }
        out
    }

    /// Positions where `bit` changes, with the first sample having no transition.
    fn transitions(samples: &[u32], bit: u32) -> Vec<usize> {
        (1..samples.len())
            .filter(|&i| (samples[i] ^ samples[i - 1]) & bit != 0)
            .collect()
    }

    #[test]
    fn disabled_channels_are_always_zero() {
        let mask = 0b1010_0101;
        let out = generate(mask, 7, 5, &[1000], 20_000);
        assert!(out.iter().all(|w| w & !mask == 0));
        assert!(out.iter().any(|w| w & mask != 0));
        // Every enabled channel actually flips.
        for ch in [0, 2, 5, 7] {
            assert!(!transitions(&out, 1 << ch).is_empty(), "channel {ch}");
        }
    }

    #[test]
    fn mean_gap_one_flips_every_enabled_channel_at_every_sample() {
        let mask = 0b111;
        let out = generate(mask, 3, 1, &[333], 1_000);
        for ch in 0..3 {
            assert_eq!(transitions(&out, 1 << ch).len(), 999, "channel {ch}");
        }
    }

    #[test]
    fn gaps_stay_within_bounds_and_average_the_mean() {
        let mean = 50_u32;
        let out = generate(0b1, 11, mean, &[4096], 2_000_000);
        let t = transitions(&out, 1);
        assert!(t.len() > 10_000);
        let gaps: Vec<usize> = t.windows(2).map(|w| w[1] - w[0]).collect();
        assert!(
            gaps.iter()
                .all(|&g| (1..=2 * mean as usize - 1).contains(&g))
        );
        let average = gaps.iter().sum::<usize>() as f64 / gaps.len() as f64;
        assert!(
            (average - f64::from(mean)).abs() < f64::from(mean) * 0.1,
            "{average}"
        );
        // The first flip is at least one sample after the start.
        assert!(t[0] >= 1);
    }

    #[test]
    fn same_seed_is_identical_and_different_seeds_differ() {
        let a = generate(u32::MAX, 42, 20, &[1000], 50_000);
        let b = generate(u32::MAX, 42, 20, &[1000], 50_000);
        let c = generate(u32::MAX, 43, 20, &[1000], 50_000);
        assert_eq!(a, b);
        assert_ne!(a, c);
    }

    #[test]
    fn chunking_does_not_change_the_output() {
        let whole = generate(u32::MAX, 5, 20, &[100_000], 100_000);
        for splits in [
            &[1_usize][..],
            &[7],
            &[4096],
            &[1, 3, 100, 5000, 65_536, 17],
        ] {
            let split = generate(u32::MAX, 5, 20, splits, 100_000);
            assert_eq!(whole, split, "splits {splits:?}");
        }
    }

    #[test]
    fn extreme_seeds_work() {
        for seed in [0_u64, u64::MAX, 1, u64::MAX - 1] {
            let out = generate(u32::MAX, seed, 10, &[1000], 10_000);
            assert!(
                out.iter().any(|&w| w != out[0]),
                "seed {seed} produced a constant"
            );
        }
    }

    #[test]
    fn skipping_equals_generating_and_discarding() {
        let mut a = RandomGenerator::new(u32::MAX, 9, 10);
        let mut b = RandomGenerator::new(u32::MAX, 9, 10);
        let mut discard = vec![0_u32; 5_000];
        a.fill(&mut discard);
        b.skip(5_000);
        let (mut x, mut y) = (vec![0_u32; 1_000], vec![0_u32; 1_000]);
        a.fill(&mut x);
        b.fill(&mut y);
        assert_eq!(x, y);
    }
}
