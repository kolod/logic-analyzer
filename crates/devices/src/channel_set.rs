//! A set of channel numbers, as wide as the largest capture.

use la_storage::{MAX_CHANNELS, MAX_WORDS_PER_SAMPLE};

/// A set of channel numbers below [`MAX_CHANNELS`].
///
/// It is used only through its methods; inside it is a fixed bitset whose word `k` bit `i`
/// is channel `32k + i`, the same layout as a sample. It is a small `Copy` value. A channel
/// number at or above [`MAX_CHANNELS`] is never a member: inserting it does nothing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct ChannelSet {
    words: [u32; MAX_WORDS_PER_SAMPLE],
}

impl ChannelSet {
    /// The empty set.
    #[must_use]
    pub const fn empty() -> Self {
        Self {
            words: [0; MAX_WORDS_PER_SAMPLE],
        }
    }

    /// The first `n` channels, `0..n` (`n` is limited to [`MAX_CHANNELS`]).
    #[must_use]
    pub fn all(n: u8) -> Self {
        let mut set = Self::empty();
        for channel in 0..n.min(MAX_CHANNELS) {
            set.insert(channel);
        }
        set
    }

    /// A set whose channels `0..=31` are the bits of `mask` (for code that handles up to 32).
    #[must_use]
    pub const fn from_u32(mask: u32) -> Self {
        let mut set = Self::empty();
        set.words[0] = mask;
        set
    }

    /// The channels `0..=31` as a mask; channels above 31 are not in it.
    #[must_use]
    pub const fn to_u32(&self) -> u32 {
        self.words[0]
    }

    /// Word `k` of the set (channels `32k..32k+31`); zero for a `k` past the last word.
    #[must_use]
    pub fn word(&self, k: usize) -> u32 {
        self.words.get(k).copied().unwrap_or(0)
    }

    /// A set made from words laid out like a sample; words past the last are ignored and
    /// bits at or above [`MAX_CHANNELS`] are dropped.
    #[must_use]
    pub fn from_words(words: &[u32]) -> Self {
        let mut set = Self::empty();
        for (dst, &src) in set.words.iter_mut().zip(words) {
            *dst = src;
        }
        for channel in MAX_CHANNELS..=u8::MAX {
            set.remove(channel);
        }
        set
    }

    /// The words of the set, the layout of a sample.
    #[must_use]
    pub fn words(&self) -> &[u32; MAX_WORDS_PER_SAMPLE] {
        &self.words
    }

    fn slot(channel: u8) -> Option<(usize, u32)> {
        if channel >= MAX_CHANNELS {
            return None;
        }
        Some((usize::from(channel) / 32, 1_u32 << (channel % 32)))
    }

    /// Whether `channel` is in the set.
    #[must_use]
    pub fn contains(&self, channel: u8) -> bool {
        Self::slot(channel).is_some_and(|(k, bit)| self.word(k) & bit != 0)
    }

    /// Adds `channel`; a channel at or above [`MAX_CHANNELS`] is ignored.
    pub fn insert(&mut self, channel: u8) {
        if let Some((k, bit)) = Self::slot(channel)
            && let Some(word) = self.words.get_mut(k)
        {
            *word |= bit;
        }
    }

    /// Removes `channel`.
    pub fn remove(&mut self, channel: u8) {
        if let Some((k, bit)) = Self::slot(channel)
            && let Some(word) = self.words.get_mut(k)
        {
            *word &= !bit;
        }
    }

    /// Whether the set has no channel.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.words.iter().all(|&w| w == 0)
    }

    /// The number of channels in the set.
    #[must_use]
    pub fn count(&self) -> usize {
        self.words
            .iter()
            .map(|w| usize::try_from(w.count_ones()).unwrap_or(0))
            .sum()
    }

    /// The channels in ascending order.
    pub fn iter(&self) -> impl Iterator<Item = u8> + '_ {
        (0..MAX_CHANNELS).filter(|&c| self.contains(c))
    }

    /// The channels in both sets.
    #[must_use]
    pub fn and(&self, other: &Self) -> Self {
        let mut set = Self::empty();
        for ((dst, &a), &b) in set.words.iter_mut().zip(&self.words).zip(&other.words) {
            *dst = a & b;
        }
        set
    }

    /// The highest channel in the set, `None` for the empty set.
    #[must_use]
    pub fn highest(&self) -> Option<u8> {
        (0..MAX_CHANNELS).rev().find(|&c| self.contains(c))
    }
}
