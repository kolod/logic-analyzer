//! Conversion of a chunk of sample words into per-channel transitions.
//!
//! The conversion works on local data only; nothing shared is touched, so a chunk that is
//! later rejected (for example because of the memory budget) leaves no trace.

use crate::config::{MAX_WORDS_PER_SAMPLE, words_per_sample};

/// One sample as words (only the first `words_per_sample` are used, the rest are zero).
pub(crate) type Word = [u32; MAX_WORDS_PER_SAMPLE];

/// Transitions found in one chunk.
#[derive(Debug)]
pub(crate) struct Ingested {
    /// For every channel, the chunk-relative positions of its transitions, increasing.
    pub(crate) per_channel: Vec<Vec<u32>>,
    /// The first sample of the chunk (masked), used for the initial levels.
    pub(crate) first_word: Word,
    /// The last sample of the chunk (masked).
    pub(crate) last_word: Word,
}

/// Mask of the bits of word `index` that belong to a capture of `channels` channels.
fn word_mask(channels: u8, index: usize) -> u32 {
    let below = u32::try_from(index).unwrap_or(u32::MAX).saturating_mul(32);
    let left = u32::from(channels).saturating_sub(below);
    if left >= 32 {
        u32::MAX
    } else {
        // left < 32 here, so the shift cannot overflow.
        (1_u32.checked_shl(left).unwrap_or(0)).wrapping_sub(1)
    }
}

/// Finds the transitions of `samples` (not empty, a whole number of samples of
/// `words_per_sample(channels)` words, at most `u32::MAX` samples).
///
/// `previous` is the last sample before the chunk (`None` for the very first chunk): a
/// difference between it and the first sample is a transition at relative position 0,
/// which is also how the first sample after a gap is handled. Bits above `channels` are
/// ignored.
pub(crate) fn ingest_chunk(samples: &[u32], previous: Option<Word>, channels: u8) -> Ingested {
    let stride = words_per_sample(channels);
    let mut masks: Word = [0; MAX_WORDS_PER_SAMPLE];
    for (index, mask) in masks.iter_mut().enumerate().take(stride) {
        *mask = word_mask(channels, index);
    }
    let mut per_channel: Vec<Vec<u32>> = (0..channels).map(|_| Vec::new()).collect();
    let mut last = previous;
    let mut first_word: Word = [0; MAX_WORDS_PER_SAMPLE];
    for (index, sample) in samples.chunks_exact(stride).enumerate() {
        let mut word: Word = [0; MAX_WORDS_PER_SAMPLE];
        for ((dst, &raw), &mask) in word.iter_mut().zip(sample).zip(&masks) {
            *dst = raw & mask;
        }
        if index == 0 {
            first_word = word;
        }
        if let Some(prev) = last {
            let relative = u32::try_from(index).unwrap_or(u32::MAX);
            for (k, (&before, &now)) in prev.iter().zip(&word).enumerate().take(stride) {
                let mut changed = before ^ now;
                while changed != 0 {
                    let bit = usize::try_from(changed.trailing_zeros()).unwrap_or(usize::MAX);
                    let channel = k.saturating_mul(32).saturating_add(bit);
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
        last_word: last.unwrap_or([0; MAX_WORDS_PER_SAMPLE]),
    }
}
