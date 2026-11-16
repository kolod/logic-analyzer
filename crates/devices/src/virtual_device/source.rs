//! The signal-source hook.
//!
//! The virtual device normally generates random digital signals. Later features (protocol
//! decoders and their generators, roadmap items 007 and 008) supply their own samples
//! through these traits, without any change to the virtual device or the acquisition path.

/// Produces the samples of one acquisition.
pub trait SignalSource: Send {
    /// Writes the samples for the positions `start .. start + out.len()` of the capture
    /// (bit `i` = channel `i`, bits of disabled channels zero).
    ///
    /// Must be deterministic: a source created with the same arguments produces the same
    /// samples for any chunking of the calls, and the calls arrive in increasing position
    /// order without gaps.
    fn fill(&mut self, start: u64, out: &mut [u32]);
}

/// Creates a [`SignalSource`] for an acquisition.
pub trait SignalSourceFactory: Send + Sync {
    /// Creates a source for the enabled channels, the sample rate in hertz and the seed of
    /// the acquisition.
    fn create(&self, enabled_mask: u32, sample_rate_hz: u64, seed: u64) -> Box<dyn SignalSource>;
}

/// The random signals of the virtual device as a [`SignalSource`], so that a source that makes
/// only some channels (a decoder generator) can leave the others to the same random generator the
/// device uses without a source.
pub struct RandomSource {
    generator: super::generator::RandomGenerator,
}

impl RandomSource {
    /// Random signals on the channels of `enabled_mask`, flipping about every `mean_gap` samples.
    #[must_use]
    pub fn new(enabled_mask: u32, seed: u64, mean_gap: u32) -> Self {
        Self {
            generator: super::generator::RandomGenerator::new(enabled_mask, seed, mean_gap),
        }
    }
}

impl SignalSource for RandomSource {
    fn fill(&mut self, _start: u64, out: &mut [u32]) {
        self.generator.fill(out);
    }
}
