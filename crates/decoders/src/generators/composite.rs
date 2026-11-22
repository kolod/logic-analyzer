//! A signal source that gives some channels to decoder generators and leaves the others to the
//! random generator of the virtual device (research R14, constitution Principle XI).

use std::sync::Arc;

use la_devices::{ChannelSet, RandomSource, SignalSource, SignalSourceFactory};

/// Creates the source of one generator for the seed of the capture.
pub type MakeSource = Arc<dyn Fn(u64) -> Option<Box<dyn SignalSource>> + Send + Sync>;

/// One generator: the channels it fills and how to make its source.
#[derive(Clone)]
pub struct Claim {
    /// The channels it fills (a bit mask).
    pub channels: u32,
    /// Makes its source.
    pub make: MakeSource,
}

/// Fills the channels of the claims with their generators and every other enabled channel with
/// the random signals of the virtual device.
pub struct CompositeFactory {
    claims: Vec<Claim>,
    mean_gap: u32,
}

impl CompositeFactory {
    /// A factory for these claims; `mean_gap` is the random signals' mean gap.
    #[must_use]
    pub fn new(claims: Vec<Claim>, mean_gap: u32) -> Self {
        Self { claims, mean_gap }
    }
}

struct Composite {
    random: RandomSource,
    parts: Vec<(u32, Box<dyn SignalSource>)>,
    enabled: u32,
    scratch: Vec<u32>,
}

impl SignalSource for Composite {
    fn fill(&mut self, start: u64, out: &mut [u32]) {
        self.fill_wide(start, out, 1);
    }

    fn fill_wide(&mut self, start: u64, out: &mut [u32], stride: usize) {
        let stride = stride.max(1);
        self.random.fill_wide(start, out, stride);
        // The generators know channels 0..=31 only: they go into the first word of a sample.
        let samples = out.len().checked_div(stride).unwrap_or(0);
        for (channels, part) in &mut self.parts {
            self.scratch.clear();
            self.scratch.resize(samples, 0);
            part.fill(start, &mut self.scratch);
            let keep = *channels & self.enabled;
            for (sample, generated) in out.chunks_exact_mut(stride).zip(&self.scratch) {
                if let Some(word) = sample.first_mut() {
                    *word |= generated & keep;
                }
            }
        }
    }
}

impl SignalSourceFactory for CompositeFactory {
    fn create(
        &self,
        enabled: ChannelSet,
        _sample_rate_hz: u64,
        seed: u64,
    ) -> Box<dyn SignalSource> {
        let enabled_mask = enabled.to_u32();
        let claimed = self.claims.iter().fold(0, |mask, c| mask | c.channels);
        // Channels above 31 are never claimed by a generator: they stay random.
        let mut random_set = enabled;
        for channel in 0..32_u8 {
            if claimed & 1_u32.wrapping_shl(u32::from(channel)) != 0 {
                random_set.remove(channel);
            }
        }
        let parts = self
            .claims
            .iter()
            .filter_map(|c| (c.make)(seed).map(|source| (c.channels, source)))
            .collect();
        Box::new(Composite {
            random: RandomSource::new(random_set, seed, self.mean_gap),
            parts,
            enabled: enabled_mask,
            scratch: Vec::new(),
        })
    }
}
