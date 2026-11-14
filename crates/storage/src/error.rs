//! Stable error codes and constructors for the storage crate.
//!
//! Every fallible operation returns [`la_core::Error`]. The codes below are stable
//! identifiers: the localization layer maps them to user-facing text later, and tests
//! compare against them. `detail` is English text for the log only.

use la_core::{Error, ErrorKind};

use crate::config::MAX_CHANNELS;

/// The channel count is zero or above [`MAX_CHANNELS`].
pub const INVALID_CHANNEL_COUNT: &str = "storage.invalid_channel_count";
/// The sample rate is zero.
pub const INVALID_SAMPLE_RATE: &str = "storage.invalid_sample_rate";
/// A limit is outside its allowed range.
pub const INVALID_LIMITS: &str = "storage.invalid_limits";
/// A chunk does not start where the capture ends.
pub const CHUNK_OUT_OF_ORDER: &str = "storage.chunk_out_of_order";
/// A chunk is larger than the configured maximum.
pub const CHUNK_TOO_LARGE: &str = "storage.chunk_too_large";
/// A chunk contains no samples.
pub const CHUNK_EMPTY: &str = "storage.chunk_empty";
/// A position would exceed the largest representable value.
pub const POSITION_OVERFLOW: &str = "storage.position_overflow";
/// Appending would exceed the memory budget.
pub const MEMORY_LIMIT: &str = "storage.memory_limit";
/// The capture was already closed.
pub const CLOSED: &str = "storage.closed";
/// A query window is empty or reversed.
pub const INVALID_WINDOW: &str = "storage.invalid_window";
/// The pixel count is zero or above the limit.
pub const INVALID_PIXELS: &str = "storage.invalid_pixels";
/// The channel index does not exist.
pub const CHANNEL_OUT_OF_RANGE: &str = "storage.channel_out_of_range";
/// A gap has zero length or does not fit.
pub const INVALID_GAP: &str = "storage.invalid_gap";
/// A stored capture violates a rule of the capture data model (restoring a capture).
pub const INVALID_RESTORE: &str = "storage.invalid_restore";
/// A state that cannot be reached from external input; indicates a bug.
pub const INTERNAL: &str = "storage.internal";

pub(crate) fn invalid_restore(detail: impl Into<String>) -> Error {
    Error::new(ErrorKind::InvalidInput, INVALID_RESTORE, detail)
}

pub(crate) fn internal(what: &str) -> Error {
    Error::new(
        ErrorKind::Internal,
        INTERNAL,
        format!("internal invariant violated: {what}"),
    )
}

pub(crate) fn invalid_channel_count(channels: u8) -> Error {
    Error::new(
        ErrorKind::InvalidInput,
        INVALID_CHANNEL_COUNT,
        format!("channel count {channels} is outside 1..={MAX_CHANNELS}"),
    )
}

pub(crate) fn invalid_sample_rate() -> Error {
    Error::new(
        ErrorKind::InvalidInput,
        INVALID_SAMPLE_RATE,
        "the sample rate must be greater than zero",
    )
}

pub(crate) fn invalid_limits(what: &str) -> Error {
    Error::new(
        ErrorKind::InvalidInput,
        INVALID_LIMITS,
        format!("invalid limit: {what}"),
    )
}

pub(crate) fn chunk_out_of_order(expected: u64, got: u64) -> Error {
    Error::new(
        ErrorKind::InvalidInput,
        CHUNK_OUT_OF_ORDER,
        format!("chunk starts at {got} but the capture ends at {expected}"),
    )
}

pub(crate) fn chunk_too_large(samples: usize, max: u32) -> Error {
    Error::new(
        ErrorKind::InvalidInput,
        CHUNK_TOO_LARGE,
        format!("chunk of {samples} samples exceeds the maximum of {max}"),
    )
}

pub(crate) fn chunk_empty() -> Error {
    Error::new(
        ErrorKind::InvalidInput,
        CHUNK_EMPTY,
        "a chunk must contain at least one sample",
    )
}

pub(crate) fn position_overflow() -> Error {
    Error::new(
        ErrorKind::InvalidInput,
        POSITION_OVERFLOW,
        "the position exceeds the largest representable value",
    )
}

pub(crate) fn memory_limit(used: u64, needed: u64, budget: u64) -> Error {
    Error::new(
        ErrorKind::Resource,
        MEMORY_LIMIT,
        format!("appending needs {needed} more bytes with {used} in use; budget is {budget}"),
    )
}

pub(crate) fn closed() -> Error {
    Error::new(
        ErrorKind::InvalidInput,
        CLOSED,
        "the capture is closed and accepts no more data",
    )
}

pub(crate) fn invalid_window(start: u64, end: u64) -> Error {
    Error::new(
        ErrorKind::InvalidInput,
        INVALID_WINDOW,
        format!("window [{start}, {end}) is empty or reversed"),
    )
}

pub(crate) fn invalid_pixels(pixels: u32, max: u32) -> Error {
    Error::new(
        ErrorKind::InvalidInput,
        INVALID_PIXELS,
        format!("pixel count {pixels} is outside 1..={max}"),
    )
}

pub(crate) fn channel_out_of_range(channel: u8, channels: u8) -> Error {
    Error::new(
        ErrorKind::InvalidInput,
        CHANNEL_OUT_OF_RANGE,
        format!("channel {channel} does not exist (the capture has {channels})"),
    )
}

pub(crate) fn invalid_gap(length: u64) -> Error {
    Error::new(
        ErrorKind::InvalidInput,
        INVALID_GAP,
        format!("a gap of {length} positions is not valid"),
    )
}
