//! Capture configuration and limits.

use la_core::Result;

use crate::error;

/// The largest supported channel count (one bit of a 32-bit sample word each).
pub const MAX_CHANNELS: u8 = 32;

/// Smallest accepted memory budget.
pub const MIN_MEMORY_BUDGET_BYTES: u64 = 1024 * 1024;
/// Largest accepted chunk size in samples.
pub const MAX_CHUNK_SAMPLES_LIMIT: u32 = 1_048_576;
/// Largest accepted pixel count limit.
pub const MAX_PIXELS_LIMIT: u32 = 65_536;

/// Resource limits of a capture.
///
/// # Memory
///
/// Worst case (a level change on every channel at every sample) a transition costs
/// 4 bytes plus 32 bytes per 1,024-transition block: about 4.03 bytes per channel and
/// sample, so about 129 bytes per sample for 32 channels. Converting one chunk needs
/// at most `max_chunk_samples × channels × 4` bytes of temporary memory (8 MiB at the
/// defaults with 32 channels).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limits {
    /// Upper bound on the memory used for stored signal data and its indexes.
    pub memory_budget_bytes: u64,
    /// Largest chunk, in samples, accepted by one append.
    pub max_chunk_samples: u32,
    /// Largest pixel count accepted by one viewport query.
    pub max_pixels: u32,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            memory_budget_bytes: 1024 * 1024 * 1024,
            max_chunk_samples: 65_536,
            max_pixels: 16_384,
        }
    }
}

impl Limits {
    /// Checks that every limit is inside its allowed range.
    ///
    /// # Errors
    ///
    /// Returns `storage.invalid_limits` naming the offending limit.
    pub fn validate(&self) -> Result<()> {
        if self.memory_budget_bytes < MIN_MEMORY_BUDGET_BYTES {
            return Err(error::invalid_limits(
                "memory_budget_bytes is below the 1 MiB minimum",
            ));
        }
        if self.max_chunk_samples == 0 || self.max_chunk_samples > MAX_CHUNK_SAMPLES_LIMIT {
            return Err(error::invalid_limits(
                "max_chunk_samples must be in 1..=1048576",
            ));
        }
        if self.max_pixels == 0 || self.max_pixels > MAX_PIXELS_LIMIT {
            return Err(error::invalid_limits("max_pixels must be in 1..=65536"));
        }
        Ok(())
    }
}

/// Parameters of a new capture.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CaptureConfig {
    /// Number of channels, `1..=`[`MAX_CHANNELS`].
    pub channels: u8,
    /// Sample rate in hertz (greater than zero); consumers use it to convert positions
    /// to time.
    pub sample_rate_hz: u64,
    /// Resource limits.
    pub limits: Limits,
}

impl CaptureConfig {
    /// Checks the whole configuration.
    ///
    /// # Errors
    ///
    /// Returns `storage.invalid_channel_count`, `storage.invalid_sample_rate` or
    /// `storage.invalid_limits`.
    pub fn validate(&self) -> Result<()> {
        if self.channels == 0 || self.channels > MAX_CHANNELS {
            return Err(error::invalid_channel_count(self.channels));
        }
        if self.sample_rate_hz == 0 {
            return Err(error::invalid_sample_rate());
        }
        self.limits.validate()
    }
}
