//! Conversion of a chunk of sample words into per-channel transitions.
//!
//! The conversion works on local data only; nothing shared is touched, so a chunk that is
//! later rejected (for example because of the memory budget) leaves no trace.

use crate::config::MAX_CHANNELS;

/// Transitions found in one chunk.
#[derive(Debug)]
pub(crate) struct Ingested {
    /// For every channel, the chunk-relative positions of its transitions, increasing.
    pub(crate) per_channel: Vec<Vec<u32>>,
    /// The first sample word of the chunk (masked), used for the initial levels.
    pub(crate) first_word: u32,
    /// The last sample word of the chunk (masked).
    pub(crate) last_word: u32,
}

/// Mask with the lowest `channels` bits set.
pub(crate) fn channel_mask(channels: u8) -> u32 {
    if channels >= MAX_CHANNELS {
        u32::MAX
    } else {
        // channels < 32 here, so the shift cannot overflow.
        (1_u32.checked_shl(u32::from(channels)).unwrap_or(0)).wrapping_sub(1)
    }
}

/// Finds the transitions of `samples` (not empty, at most `u32::MAX` words).
///
/// `previous` is the last sample word before the chunk (`None` for the very first
/// chunk): a difference between it and the first word is a transition at relative
/// position 0, which is also how the first sample after a gap is handled. Bits above
/// `channels` are ignored.
pub(crate) fn ingest_chunk(samples: &[u32], previous: Option<u32>, channels: u8) -> Ingested {
    let mask = channel_mask(channels);
    let mut per_channel: Vec<Vec<u32>> = (0..channels).map(|_| Vec::new()).collect();
    let mut last = previous;
    let mut first_word = 0;
    for (index, &raw) in samples.iter().enumerate() {
        let word = raw & mask;
        if index == 0 {
            first_word = word;
        }
        if let Some(prev) = last {
            let mut changed = prev ^ word;
            if changed != 0 {
                let relative = u32::try_from(index).unwrap_or(u32::MAX);
                while changed != 0 {
                    let channel = usize::try_from(changed.trailing_zeros()).unwrap_or(usize::MAX);
                    if let Some(list) = per_channel.get_mut(channel) {
                        list.push(relative);
                    }
                    changed &= changed.wrapping_sub(1);
                }
            }
        }
        last = Some(word);
    }
    Ingested {
        per_channel,
        first_word,
        last_word: last.unwrap_or(0),
    }
}
