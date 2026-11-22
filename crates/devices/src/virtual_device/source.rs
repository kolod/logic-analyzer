//! The signal-source hook.
//!
//! The virtual device normally generates random digital signals. Later features (protocol
//! decoders and their generators, roadmap items 007 and 008) supply their own samples
//! through these traits, without any change to the virtual device or the acquisition path.

use crate::channel_set::ChannelSet;

/// Produces the samples of one acquisition.
pub trait SignalSource: Send {
    /// Writes the samples for the positions `start .. start + out.len()` of the capture
    /// (bit `i` = channel `i`, bits of disabled channels zero).
    ///
    /// Must be deterministic: a source created with the same arguments produces the same
    /// samples for any chunking of the calls, and the calls arrive in increasing position
    /// order without gaps.
    fn fill(&mut self, start: u64, out: &mut [u32]);

    /// Writes the samples for a capture of more than 32 channels: `out` holds whole samples
    /// of `stride` words each (word `k` is channels `32k..32k+31`).
    ///
    /// The default fills channels `0..=31` with [`SignalSource::fill`] and leaves the other
    /// words zero, so a source that knows nothing of wide captures keeps working.
    fn fill_wide(&mut self, start: u64, out: &mut [u32], stride: usize) {
        if stride <= 1 {
            self.fill(start, out);
            return;
        }
        let samples = out.len().checked_div(stride).unwrap_or(0);
        let mut first = vec![0_u32; samples];
        self.fill(start, &mut first);
        for (sample, &word) in out.chunks_exact_mut(stride).zip(&first) {
            for (index, dst) in sample.iter_mut().enumerate() {
                *dst = if index == 0 { word } else { 0 };
            }
        }
    }
}

/// Creates a [`SignalSource`] for an acquisition.
pub trait SignalSourceFactory: Send + Sync {
    /// Creates a source for the enabled channels, the sample rate in hertz and the seed of
    /// the acquisition.
    fn create(&self, enabled: ChannelSet, sample_rate_hz: u64, seed: u64) -> Box<dyn SignalSource>;
}

/// The random signals of the virtual device as a [`SignalSource`], so that a source that makes
/// only some channels (a decoder generator) can leave the others to the same random generator the
/// device uses without a source.
pub struct RandomSource {
    generator: super::generator::RandomGenerator,
}

impl RandomSource {
    /// Random signals on the channels of `enabled`, flipping about every `mean_gap` samples.
    #[must_use]
    pub fn new(enabled: ChannelSet, seed: u64, mean_gap: u32) -> Self {
        Self {
            generator: super::generator::RandomGenerator::new(enabled, seed, mean_gap),
        }
    }
}

impl SignalSource for RandomSource {
    fn fill(&mut self, _start: u64, out: &mut [u32]) {
        self.generator.fill_wide(out, 1);
    }

    fn fill_wide(&mut self, _start: u64, out: &mut [u32], stride: usize) {
        self.generator.fill_wide(out, stride);
    }
}
