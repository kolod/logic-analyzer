//! The length of a capture in seconds: turning what the user types into a number of samples and
//! back (`specs/014-gui-improvements/contracts/host-api-1.2.md`).
//!
//! The configuration of a capture still counts samples; seconds are how the length is entered and
//! shown. Everything is integer arithmetic (`u128` intermediates), so there is exactly one answer
//! for every input and no rounding surprise of floating point.

/// Nanoseconds in a second: the resolution of the text.
const NANOS: u128 = 1_000_000_000;
/// The longest text that is looked at.
const MAX_TEXT: usize = 40;
/// The most digits after the point.
const MAX_DECIMALS: usize = 9;

/// Why a text is not a length.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SecondsError {
    /// The text is empty (or only spaces).
    Empty,
    /// The text is not a decimal number without sign, exponent or separator other than a point
    /// with digits on both sides, or it is too long to be one.
    NotANumber,
    /// More than nine digits after the point.
    TooManyDecimals,
    /// Less than half a sample.
    TooSmall,
    /// More samples than a count can hold.
    TooLarge,
    /// The sample rate is zero.
    NoRate,
}

/// The number of samples of `text` seconds at `rate_hz` samples a second, rounded half up.
///
/// # Errors
///
/// See [`SecondsError`].
pub fn samples_for_seconds(rate_hz: u64, text: &str) -> Result<u64, SecondsError> {
    let text = text.trim();
    if text.is_empty() {
        return Err(SecondsError::Empty);
    }
    if rate_hz == 0 {
        return Err(SecondsError::NoRate);
    }
    if text.len() > MAX_TEXT {
        return Err(SecondsError::NotANumber);
    }
    let (whole, fraction) = match text.split_once('.') {
        Some((whole, fraction)) => (whole, Some(fraction)),
        None => (text, None),
    };
    let digits = |part: &str| !part.is_empty() && part.bytes().all(|b| b.is_ascii_digit());
    if !digits(whole) || fraction.is_some_and(|f| !digits(f)) {
        return Err(SecondsError::NotANumber);
    }
    let fraction = fraction.unwrap_or("");
    if fraction.len() > MAX_DECIMALS {
        return Err(SecondsError::TooManyDecimals);
    }
    let whole: u128 = whole.parse().map_err(|_| SecondsError::TooLarge)?;
    let mut nanos_fraction: u128 = if fraction.is_empty() {
        0
    } else {
        fraction.parse().map_err(|_| SecondsError::NotANumber)?
    };
    for _ in fraction.len()..MAX_DECIMALS {
        nanos_fraction = nanos_fraction.saturating_mul(10);
    }
    let nanos = whole
        .checked_mul(NANOS)
        .and_then(|n| n.checked_add(nanos_fraction))
        .ok_or(SecondsError::TooLarge)?;
    let samples = nanos
        .checked_mul(u128::from(rate_hz))
        .and_then(|n| n.checked_add(NANOS / 2))
        .map(|n| n / NANOS)
        .ok_or(SecondsError::TooLarge)?;
    match u64::try_from(samples) {
        Ok(0) => Err(SecondsError::TooSmall),
        Ok(samples) => Ok(samples),
        Err(_) => Err(SecondsError::TooLarge),
    }
}

/// `samples` at `rate_hz` as seconds in decimal with only the digits they need (at most nine after
/// the point, rounded half up); `"0"` when there are no samples or no rate.
#[must_use]
pub fn seconds_for_samples(rate_hz: u64, samples: u64) -> String {
    if rate_hz == 0 || samples == 0 {
        return "0".to_owned();
    }
    let rate = u128::from(rate_hz);
    let nanos = u128::from(samples)
        .saturating_mul(NANOS * 2)
        .saturating_add(rate)
        .checked_div(rate.saturating_mul(2))
        .unwrap_or(0);
    let (whole, fraction) = (nanos / NANOS, nanos % NANOS);
    if fraction == 0 {
        return whole.to_string();
    }
    let fraction = format!("{fraction:09}");
    format!("{whole}.{}", fraction.trim_end_matches('0'))
}
