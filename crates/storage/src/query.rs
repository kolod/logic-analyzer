//! Queries on a snapshot: levels, transitions and (later) viewports.

use la_core::Result;

use crate::channel::{ChannelData, RankCursor, TransitionIter};
use crate::error;
use crate::published::Snapshot;
use crate::types::LevelState;

impl Snapshot {
    /// The data of `channel`, or `storage.channel_out_of_range`.
    pub(crate) fn channel_data(&self, channel: u8) -> Result<&ChannelData> {
        self.inner
            .channels
            .get(usize::from(channel))
            .ok_or_else(|| error::channel_out_of_range(channel, self.channels()))
    }

    /// The level of `channel` at `position`: [`LevelState::Missing`] inside a gap and at
    /// or beyond the capture length. Cost O(log N).
    ///
    /// # Errors
    ///
    /// `storage.channel_out_of_range` if the channel does not exist.
    pub fn level_at(&self, channel: u8, position: u64) -> Result<LevelState> {
        let data = self.channel_data(channel)?;
        if position >= self.inner.length || self.inner.gap_containing(position).is_some() {
            return Ok(LevelState::Missing);
        }
        let mut probes = 0;
        let point = RankCursor::new(data).advance_to(position, &mut probes);
        Ok(if data.high_at_rank(point.rank) {
            LevelState::High
        } else {
            LevelState::Low
        })
    }

    /// Iterates over the transitions of `channel` with `start < position <= end`, in
    /// increasing order, each exactly once. Finding the first one costs O(log N); every
    /// further one is O(1).
    ///
    /// # Errors
    ///
    /// `storage.channel_out_of_range`, or `storage.invalid_window` if `start >= end`.
    pub fn transitions(&self, channel: u8, start: u64, end: u64) -> Result<TransitionIter<'_>> {
        let data = self.channel_data(channel)?;
        if start >= end {
            return Err(error::invalid_window(start, end));
        }
        Ok(TransitionIter::new(data, start, end))
    }
}
