//! The coding of the captured samples: raw (each sample as bytes) and compact (only the changes).
//!
//! A sample of `n` selected channels is `ceil(n / 8)` bytes; bit `i` (byte 0 first, bit 0 first) is the
//! `i`-th channel of the request's list. Nothing here allocates: the caller gives the buffers.

/// The bytes of one raw sample of `channels` selected channels.
#[must_use]
pub fn bytes_per_sample(channels: u8) -> usize {
    usize::from(channels).div_ceil(8)
}

/// Why a buffer could not be coded or decoded.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompactError {
    /// A buffer has the wrong size for what it should hold.
    BadSize,
    /// A channel number is 32 or more.
    BadChannel,
    /// The output buffer is too small.
    OutputTooSmall,
    /// The stream ended inside a record, or before the first sample.
    Truncated,
    /// The stream has more records than the declared number of samples (or a record that changes nothing).
    TooLong,
    /// A distance does not fit in 32 bits.
    BadNumber,
}

impl core::fmt::Display for CompactError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(match self {
            Self::BadSize => "a buffer has the wrong size",
            Self::BadChannel => "a channel number is 32 or more",
            Self::OutputTooSmall => "the output buffer is too small",
            Self::Truncated => "the compact stream ends too early",
            Self::TooLong => "the compact stream is longer than its sample count",
            Self::BadNumber => "a distance in the compact stream is too large",
        })
    }
}

#[cfg(feature = "std")]
impl std::error::Error for CompactError {}

/// Packs the selected channels of each 32-bit word of `words` (bit `c` is channel `c`) into raw samples:
/// bit `i` of an output sample is channel `channels[i]`. Returns the bytes written
/// (`words.len() * bytes_per_sample(channels.len())`).
///
/// # Errors
///
/// [`CompactError::BadChannel`] for a channel of 32 or more, [`CompactError::BadSize`] for more than 32
/// channels, [`CompactError::OutputTooSmall`] if `out` cannot hold the result.
pub fn pack_raw(words: &[u32], channels: &[u8], out: &mut [u8]) -> Result<usize, CompactError> {
    let count = u8::try_from(channels.len()).map_err(|_| CompactError::BadSize)?;
    if count > 32 || channels.iter().any(|&c| c >= 32) {
        return Err(if count > 32 {
            CompactError::BadSize
        } else {
            CompactError::BadChannel
        });
    }
    let per = bytes_per_sample(count);
    let total = words.len().checked_mul(per).ok_or(CompactError::BadSize)?;
    let out = out.get_mut(..total).ok_or(CompactError::OutputTooSmall)?;
    for (word, sample) in words.iter().zip(out.chunks_exact_mut(per.max(1))) {
        sample.fill(0);
        for (i, &channel) in channels.iter().enumerate() {
            if word.checked_shr(u32::from(channel)).unwrap_or(0) & 1 == 1 {
                if let Some(byte) = sample.get_mut(i / 8) {
                    *byte |= 1 << (i % 8);
                }
            }
        }
    }
    Ok(total)
}

/// The reverse of [`pack_raw`]: turns raw samples into 32-bit words with the selected channels at their own
/// numbers and the rest zero. `raw` must hold a whole number of samples and `out` exactly that many words.
///
/// # Errors
///
/// [`CompactError::BadSize`] if `raw` is not a whole number of samples or `out` has another length,
/// [`CompactError::BadChannel`] for a channel of 32 or more.
pub fn unpack_raw(raw: &[u8], channels: &[u8], out: &mut [u32]) -> Result<(), CompactError> {
    let count = u8::try_from(channels.len()).map_err(|_| CompactError::BadSize)?;
    if count == 0 || count > 32 {
        return Err(CompactError::BadSize);
    }
    if channels.iter().any(|&c| c >= 32) {
        return Err(CompactError::BadChannel);
    }
    let per = bytes_per_sample(count);
    if raw.len().checked_rem(per) != Some(0) || raw.len().checked_div(per) != Some(out.len()) {
        return Err(CompactError::BadSize);
    }
    for (sample, word) in raw.chunks_exact(per).zip(out.iter_mut()) {
        let mut value = 0_u32;
        for (i, &channel) in channels.iter().enumerate() {
            if sample.get(i / 8).is_some_and(|b| b >> (i % 8) & 1 == 1) {
                value |= 1_u32.checked_shl(u32::from(channel)).unwrap_or(0);
            }
        }
        *word = value;
    }
    Ok(())
}

fn put_varint(mut value: u32, out: &mut [u8], at: &mut usize) -> Result<(), CompactError> {
    loop {
        let low = (value & 0x7F) as u8;
        value >>= 7;
        let byte = if value == 0 { low } else { low | 0x80 };
        *out.get_mut(*at).ok_or(CompactError::OutputTooSmall)? = byte;
        *at = at.saturating_add(1);
        if value == 0 {
            return Ok(());
        }
    }
}

/// Codes raw samples (`per` bytes each) in the compact form: the first sample, then for every change the
/// distance in samples to it (7 bits per byte, the high bit set on all but the last) and the mask of the
/// channels that change. Returns the bytes written. The coding is not always smaller: the caller sends raw
/// when the result is not shorter than `raw`.
///
/// # Errors
///
/// [`CompactError::BadSize`] if `per` is 0 or `raw` is not a whole number of samples,
/// [`CompactError::OutputTooSmall`] if `out` is too short.
pub fn encode_compact(raw: &[u8], per: usize, out: &mut [u8]) -> Result<usize, CompactError> {
    if raw.len().checked_rem(per) != Some(0) {
        return Err(CompactError::BadSize);
    }
    let mut samples = raw.chunks_exact(per);
    let Some(first) = samples.next() else {
        return Ok(0);
    };
    out.get_mut(..per)
        .ok_or(CompactError::OutputTooSmall)?
        .copy_from_slice(first);
    let mut at = per;
    let (mut previous, mut distance) = (first, 0_u32);
    for sample in samples {
        distance = distance.checked_add(1).ok_or(CompactError::BadNumber)?;
        if sample == previous {
            continue;
        }
        put_varint(distance, out, &mut at)?;
        for (a, b) in previous.iter().zip(sample) {
            *out.get_mut(at).ok_or(CompactError::OutputTooSmall)? = a ^ b;
            at = at.saturating_add(1);
        }
        previous = sample;
        distance = 0;
    }
    Ok(at)
}

/// Decodes a compact stream of `samples` samples of `per` bytes into `out`, which must be exactly
/// `samples * per` bytes (the caller allocates it after checking the declared count against its own limit, so
/// nothing here allocates more than that). The stream must add up exactly.
///
/// # Errors
///
/// [`CompactError::BadSize`] for a wrong `out` length, [`CompactError::Truncated`] for a stream that ends
/// early or inside a record, [`CompactError::TooLong`] for records that go past the last sample or change
/// nothing, [`CompactError::BadNumber`] for a distance over 32 bits.
pub fn decode_compact(
    stream: &[u8],
    per: usize,
    samples: usize,
    out: &mut [u8],
) -> Result<(), CompactError> {
    if per == 0 || samples.checked_mul(per) != Some(out.len()) {
        return Err(CompactError::BadSize);
    }
    if samples == 0 {
        return if stream.is_empty() {
            Ok(())
        } else {
            Err(CompactError::TooLong)
        };
    }
    let first = stream.get(..per).ok_or(CompactError::Truncated)?;
    let mut rest = stream.get(per..).unwrap_or(&[]);
    let mut current = [0_u8; 4];
    let current = current.get_mut(..per).ok_or(CompactError::BadSize)?;
    current.copy_from_slice(first);
    let mut position = 0_usize;
    while !rest.is_empty() {
        let (distance, after) = take_varint(rest)?;
        let mask = after.get(..per).ok_or(CompactError::Truncated)?;
        rest = after.get(per..).unwrap_or(&[]);
        let next = position
            .checked_add(usize::try_from(distance).map_err(|_| CompactError::BadNumber)?)
            .ok_or(CompactError::BadNumber)?;
        if distance == 0 || next >= samples || mask.iter().all(|&b| b == 0) {
            return Err(CompactError::TooLong);
        }
        fill(out, per, position, next, current)?;
        for (c, m) in current.iter_mut().zip(mask) {
            *c ^= m;
        }
        position = next;
    }
    fill(out, per, position, samples, current)
}

/// Writes `value` into the samples `from..to`.
fn fill(
    out: &mut [u8],
    per: usize,
    from: usize,
    to: usize,
    value: &[u8],
) -> Result<(), CompactError> {
    let range = from.checked_mul(per).zip(to.checked_mul(per));
    let (start, end) = range.ok_or(CompactError::BadNumber)?;
    for sample in out
        .get_mut(start..end)
        .ok_or(CompactError::BadSize)?
        .chunks_exact_mut(per)
    {
        sample.copy_from_slice(value);
    }
    Ok(())
}

fn take_varint(bytes: &[u8]) -> Result<(u32, &[u8]), CompactError> {
    let mut value = 0_u32;
    for (index, &byte) in bytes.iter().enumerate() {
        let shift = u32::try_from(index)
            .map_err(|_| CompactError::BadNumber)?
            .saturating_mul(7);
        let part = u32::from(byte & 0x7F);
        if shift > 28 || (shift == 28 && part > 0x0F) {
            return Err(CompactError::BadNumber);
        }
        value |= part << shift;
        if byte & 0x80 == 0 {
            return Ok((value, bytes.get(index.saturating_add(1)..).unwrap_or(&[])));
        }
    }
    Err(CompactError::Truncated)
}
