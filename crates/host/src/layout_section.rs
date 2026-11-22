//! The `LAYT` section of a session file: the heights of the signals
//! (`specs/014-gui-improvements/contracts/layt-section.md`) and, from version 2, the heights of the
//! lanes of decoder instances (`specs/016-i2c-lane-blocks/contracts/layt-section-2.md`).
//!
//! The reader treats every byte as untrusted and accepts only the canonical form, so an accepted
//! payload always re-encodes to the same bytes. Rejection never fails the opening of a file: the
//! caller shows the capture with automatic heights and a warning.

use la_core::Result;

use crate::error;
use crate::presentation::{MAX_HEIGHT, MIN_HEIGHT, Presentation};

/// The section tag.
pub const TAG: &str = "LAYT";
/// The newest version of the payload this code reads; it writes version 2 only when a decoder lane has
/// a height of its own, so that a file without one is byte for byte what version 1 wrote.
pub const VERSION: u8 = 2;
/// The most channels a payload can describe.
pub const MAX_CHANNELS: u64 = la_storage::MAX_CHANNELS as u64;
/// The most decoder lanes a payload can describe.
pub const MAX_DECODERS: u64 = 64;

/// What a payload holds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Decoded {
    /// The heights chosen by the user.
    Heights {
        /// The channels with a custom height, in ascending channel order.
        channels: Vec<(u8, u16)>,
        /// The decoder instances with a custom lane height, in ascending instance order.
        decoders: Vec<(u32, u16)>,
    },
    /// A newer version of the section: kept as it is, not interpreted.
    Newer,
}

fn put_varint(mut value: u64, out: &mut Vec<u8>) {
    loop {
        let byte = u8::try_from(value & 0x7f).unwrap_or(0);
        value >>= 7;
        if value == 0 {
            out.push(byte);
            return;
        }
        out.push(byte | 0x80);
    }
}

/// The payload for the heights of `presentation`, or `None` when nothing has a custom one (the
/// section is then not written). Deterministic: the same heights give the same bytes.
#[must_use]
pub fn encode(presentation: &Presentation) -> Option<Vec<u8>> {
    let mut heights: Vec<(u8, u16)> = presentation
        .channels
        .iter()
        .filter(|s| s.height != 0 && u64::from(s.index) < MAX_CHANNELS)
        .map(|s| (s.index, s.height))
        .collect();
    let decoders: Vec<(u32, u16)> = presentation
        .decoder_heights
        .iter()
        .filter(|(_, height)| **height != 0)
        .take(usize::try_from(MAX_DECODERS).unwrap_or(0))
        .map(|(instance, height)| (*instance, *height))
        .collect();
    if heights.is_empty() && decoders.is_empty() {
        return None;
    }
    heights.sort_unstable();
    let mut out = vec![if decoders.is_empty() { 1 } else { VERSION }];
    put_varint(heights.len() as u64, &mut out);
    for (channel, height) in heights {
        put_varint(u64::from(channel), &mut out);
        put_varint(u64::from(height), &mut out);
    }
    if !decoders.is_empty() {
        put_varint(decoders.len() as u64, &mut out);
        // `BTreeMap` iterates in ascending order of instance.
        for (instance, height) in decoders {
            put_varint(u64::from(instance), &mut out);
            put_varint(u64::from(height), &mut out);
        }
    }
    Some(out)
}

struct Reader<'a> {
    data: &'a [u8],
    at: usize,
}

fn invalid<T>(detail: &str) -> Result<T> {
    Err(error::layout_invalid(detail))
}

impl Reader<'_> {
    fn byte(&mut self) -> Result<u8> {
        let Some(&b) = self.data.get(self.at) else {
            return invalid("the section ends too early");
        };
        self.at = self.at.saturating_add(1);
        Ok(b)
    }

    /// A canonical varint: no more bytes than needed, no overlong zero padding.
    fn varint(&mut self) -> Result<u64> {
        let mut value = 0_u64;
        for index in 0..10_u32 {
            let byte = self.byte()?;
            let bits = u64::from(byte & 0x7f);
            let Some(shifted) = bits.checked_shl(index.saturating_mul(7)) else {
                return invalid("a number is too large");
            };
            if index > 0 && byte == 0 {
                return invalid("a number is not in canonical form");
            }
            if shifted >> index.saturating_mul(7) != bits {
                return invalid("a number is too large");
            }
            value |= shifted;
            if byte & 0x80 == 0 {
                return Ok(value);
            }
        }
        invalid("a number is too long")
    }
}

/// Reads a payload; `channels` is the number of channels of the capture it belongs to.
///
/// # Errors
///
/// `host.layout_invalid` for a payload that does not follow the contract.
pub fn decode(payload: &[u8], channels: u8) -> Result<Decoded> {
    let mut reader = Reader {
        data: payload,
        at: 0,
    };
    let version = reader.byte()?;
    if version > VERSION {
        return Ok(Decoded::Newer);
    }
    if version != 1 && version != VERSION {
        return invalid("the version is not known");
    }
    let count = reader.varint()?;
    if count > MAX_CHANNELS {
        return invalid("more channels than a capture can have");
    }
    let mut heights = Vec::new();
    let mut previous: Option<u64> = None;
    for _ in 0..count {
        let channel = reader.varint()?;
        if channel >= u64::from(channels) || previous.is_some_and(|p| channel <= p) {
            return invalid("a channel is out of range, repeated or out of order");
        }
        previous = Some(channel);
        let height = reader.varint()?;
        if height < u64::from(MIN_HEIGHT) || height > u64::from(MAX_HEIGHT) {
            return invalid("a height is out of range");
        }
        let (Ok(channel), Ok(height)) = (u8::try_from(channel), u16::try_from(height)) else {
            return invalid("a number is too large");
        };
        heights.push((channel, height));
    }
    let mut decoders = Vec::new();
    if version >= 2 {
        let count = reader.varint()?;
        if count == 0 || count > MAX_DECODERS {
            return invalid("the number of decoder lanes is out of range");
        }
        let mut previous: Option<u64> = None;
        for _ in 0..count {
            let instance = reader.varint()?;
            if previous.is_some_and(|p| instance <= p) {
                return invalid("a decoder lane is repeated or out of order");
            }
            previous = Some(instance);
            let height = reader.varint()?;
            if height < u64::from(MIN_HEIGHT) || height > u64::from(MAX_HEIGHT) {
                return invalid("a height is out of range");
            }
            let (Ok(instance), Ok(height)) = (u32::try_from(instance), u16::try_from(height))
            else {
                return invalid("a number is too large");
            };
            decoders.push((instance, height));
        }
    }
    if reader.at != payload.len() {
        return invalid("bytes remain after the last height");
    }
    Ok(Decoded::Heights {
        channels: heights,
        decoders,
    })
}
