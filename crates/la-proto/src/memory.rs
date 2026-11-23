//! Where a capture goes: the internal memory or the external one (research R6).
//!
//! The internal memory takes every capture that fits. The external memory is slower, so it takes only captures
//! whose data rate it can write (`PSRAMRATE`, the highest rate at which 32 channels can go to it) and that do
//! not fit the internal one, unless the request names it.

use crate::info::DeviceInfo;
use crate::request::{CaptureRequest2, Memory};

/// What the board's memories can hold and how fast the external one can be written.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MemoryLimits {
    /// The size of the internal buffer in bytes.
    pub internal_bytes: u64,
    /// The size of the external memory in bytes, 0 for none.
    pub psram_bytes: u64,
    /// The highest rate in hertz at which a capture of 32 channels (4 bytes a sample) can go to the external
    /// memory.
    pub psram_rate_hz: u32,
}

/// The memory a capture goes to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Chosen {
    /// The internal memory.
    Internal,
    /// The external memory.
    External,
}

/// Why no memory can take a request (`ERR_NO_MEMORY`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryError {
    /// It does not fit the internal memory, and the request asked for it.
    TooLargeForInternal,
    /// The board has no external memory, and the request asked for it.
    NoExternal,
    /// It does not fit the external memory.
    TooLargeForExternal,
    /// The external memory cannot be written at this rate with this many bytes per sample.
    TooFastForExternal,
    /// It fits neither memory (or the external one cannot take its rate).
    FitsNowhere,
}

impl core::fmt::Display for MemoryError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(match self {
            Self::TooLargeForInternal => "the capture does not fit the internal memory",
            Self::NoExternal => "the board has no external memory",
            Self::TooLargeForExternal => "the capture does not fit the external memory",
            Self::TooFastForExternal => "the external memory cannot be written at this rate",
            Self::FitsNowhere => "the capture fits no memory the board can use at this rate",
        })
    }
}

#[cfg(feature = "std")]
impl std::error::Error for MemoryError {}

impl MemoryLimits {
    /// The limits a board reported: its buffer is counted in samples of four bytes (32 channels).
    #[must_use]
    pub fn from_info(info: &DeviceInfo) -> Self {
        Self {
            internal_bytes: u64::from(info.buffer).saturating_mul(4),
            psram_bytes: info.psram_bytes,
            psram_rate_hz: info.psram_rate,
        }
    }

    fn external_takes_rate(&self, rate_hz: u32, per: usize) -> bool {
        let per = u64::try_from(per).unwrap_or(u64::MAX);
        u64::from(rate_hz).saturating_mul(per) <= u64::from(self.psram_rate_hz).saturating_mul(4)
    }

    /// The most samples of `per` bytes a capture at `rate_hz` can have: the larger of what the internal memory
    /// holds and what the external one holds if it can be written at that rate.
    #[must_use]
    pub fn depth_samples(&self, per: usize, rate_hz: u32) -> u64 {
        let bytes_per_sample = u64::try_from(per.max(1)).unwrap_or(u64::MAX);
        let internal = self
            .internal_bytes
            .checked_div(bytes_per_sample)
            .unwrap_or(0);
        let external = if self.psram_bytes > 0 && self.external_takes_rate(rate_hz, per) {
            self.psram_bytes.checked_div(bytes_per_sample).unwrap_or(0)
        } else {
            0
        };
        internal.max(external)
    }
}

/// The bytes a request needs: the samples before and after the trigger, as they are stored.
#[must_use]
pub fn needed_bytes(request: &CaptureRequest2) -> u64 {
    let samples =
        u64::from(request.samples_before).saturating_add(u64::from(request.samples_after));
    samples.saturating_mul(u64::try_from(request.bytes_per_sample()).unwrap_or(u64::MAX))
}

/// Chooses the memory of a request: the one it names if it can take it; for `Auto`, the internal memory if the
/// capture fits, else the external one if it fits and can be written at the rate.
///
/// # Errors
///
/// [`MemoryError`] naming why no memory can take it.
pub fn choose(request: &CaptureRequest2, limits: &MemoryLimits) -> Result<Chosen, MemoryError> {
    let needed = needed_bytes(request);
    let fits_internal = needed <= limits.internal_bytes;
    let external = if limits.psram_bytes == 0 {
        Err(MemoryError::NoExternal)
    } else if needed > limits.psram_bytes {
        Err(MemoryError::TooLargeForExternal)
    } else if !limits.external_takes_rate(request.frequency, request.bytes_per_sample()) {
        Err(MemoryError::TooFastForExternal)
    } else {
        Ok(Chosen::External)
    };
    match request.memory {
        Memory::Internal => {
            if fits_internal {
                Ok(Chosen::Internal)
            } else {
                Err(MemoryError::TooLargeForInternal)
            }
        }
        Memory::External => external,
        Memory::Auto => {
            if fits_internal {
                Ok(Chosen::Internal)
            } else {
                external.map_err(|_| MemoryError::FitsNowhere)
            }
        }
    }
}
