//! Queries on a snapshot: levels, transitions and (later) viewports.

use la_core::Result;

use crate::channel::{ChannelData, RankCursor, TransitionIter};
use crate::error;
use crate::math::pixel_boundary;
use crate::published::Snapshot;
use crate::types::{
    Activity, Edge, LevelState, MAX_REPORTED_TRANSITIONS, PixelSummary, QueryStats, ViewportResult,
};

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

    /// The level at sample `x` given `rank(x)`; `Missing` at or beyond the end and inside
    /// gaps.
    fn level_with_rank(&self, data: &ChannelData, x: u64, rank: u64) -> LevelState {
        if x >= self.inner.length || self.inner.gap_containing(x).is_some() {
            LevelState::Missing
        } else if data.high_at_rank(rank) {
            LevelState::High
        } else {
            LevelState::Low
        }
    }

    /// Whether the closed range `[a, b]` touches a gap.
    fn touches_gap(&self, a: u64, b: u64) -> bool {
        let gaps = &self.inner.gaps;
        let first = gaps.partition_point(|g| g.end().unwrap_or(u64::MAX) <= a);
        gaps.get(first).is_some_and(|g| g.start <= b)
    }

    /// Summarizes `pixels` columns of `channel` over the window `[start, end)`.
    ///
    /// Column `i` covers samples `[a_i, a_{i+1})` with `a_i = start + floor(i * len /
    /// pixels)`. The result follows `contracts/viewport-semantics.md`: start and end
    /// level, none/one/many transitions (transitions with `a_i < p <= a_{i+1}`), a
    /// missing-data flag for gaps, and a flag for columns beyond the capture end.
    ///
    /// Cost: `O(log N + W * (1 + log(1 + N_w / W)))` probes (reported in
    /// [`ViewportResult::stats`]), never proportional to the capture size.
    ///
    /// # Errors
    ///
    /// `storage.channel_out_of_range`, `storage.invalid_window` (`start >= end`) or
    /// `storage.invalid_pixels` (zero or above the configured maximum).
    pub fn viewport(
        &self,
        channel: u8,
        start: u64,
        end: u64,
        pixels: u32,
    ) -> Result<ViewportResult> {
        let data = self.channel_data(channel)?;
        let length = end
            .checked_sub(start)
            .filter(|l| *l > 0)
            .ok_or_else(|| error::invalid_window(start, end))?;
        let max_pixels = self.inner.limits.max_pixels;
        if pixels == 0 || pixels > max_pixels {
            return Err(error::invalid_pixels(pixels, max_pixels));
        }

        let mut probes = 0_u64;
        let mut cursor = RankCursor::new(data);
        // `pixels` is validated against the limit, so this allocation is bounded.
        let mut out = Vec::with_capacity(usize::try_from(pixels).unwrap_or(0));

        let mut left_bound = start;
        let mut left = cursor.advance_to(start, &mut probes);
        for i in 0..pixels {
            let right_bound = pixel_boundary(start, length, i.saturating_add(1), pixels)
                .ok_or_else(|| error::internal("pixel boundary out of range"))?;
            let right = cursor.advance_to(right_bound, &mut probes);
            let count = right.rank.saturating_sub(left.rank);
            let activity = match count {
                0 => Activity::None,
                1 => {
                    let position = cursor
                        .next_transition(&left, &mut probes)
                        .ok_or_else(|| error::internal("missing transition"))?;
                    Activity::One(Edge {
                        position,
                        rising: data.high_after(left.rank),
                    })
                }
                n => Activity::Many {
                    count: u32::try_from(n).unwrap_or(MAX_REPORTED_TRANSITIONS),
                },
            };
            out.push(PixelSummary {
                start_level: self.level_with_rank(data, left_bound, left.rank),
                end_level: self.level_with_rank(data, right_bound, right.rank),
                activity,
                missing_data: self.touches_gap(left_bound, right_bound),
                beyond_end: left_bound >= self.inner.length,
            });
            left_bound = right_bound;
            left = right;
        }
        Ok(ViewportResult {
            pixels: out,
            stats: QueryStats { probes },
        })
    }

    /// The level of the first stored sample of `channel` (`false` for a capture that has no
    /// stored sample). Together with the transitions it describes the channel completely.
    ///
    /// # Errors
    ///
    /// `storage.channel_out_of_range` if the channel does not exist.
    pub fn initial_high(&self, channel: u8) -> Result<bool> {
        Ok(self.channel_data(channel)?.initial_high)
    }

    /// The number of transitions stored for `channel`. Cost O(1).
    ///
    /// # Errors
    ///
    /// `storage.channel_out_of_range` if the channel does not exist.
    pub fn transition_count(&self, channel: u8) -> Result<u64> {
        let data = self.channel_data(channel)?;
        let Some(last) = data.block_count().checked_sub(1) else {
            return Ok(0);
        };
        Ok(data.block(last).map_or(0, |view| {
            view.first_rank
                .saturating_add(u64::try_from(view.len()).unwrap_or(0))
        }))
    }

    /// The first transition of `channel` with a position greater than `from`, if any. Cost
    /// O(log N) (a binary search of the block, then of the position inside it).
    ///
    /// # Errors
    ///
    /// `storage.channel_out_of_range` if the channel does not exist.
    pub fn next_transition(&self, channel: u8, from: u64) -> Result<Option<Edge>> {
        let data = self.channel_data(channel)?;
        let mut probes = 0;
        let mut cursor = RankCursor::new(data);
        let point = cursor.advance_to(from, &mut probes);
        Ok(cursor
            .next_transition(&point, &mut probes)
            .map(|position| Edge {
                position,
                rising: data.high_after(point.rank),
            }))
    }

    /// The last transition of `channel` with a position smaller than `before`, if any. Cost
    /// O(log N).
    ///
    /// # Errors
    ///
    /// `storage.channel_out_of_range` if the channel does not exist.
    pub fn previous_transition(&self, channel: u8, before: u64) -> Result<Option<Edge>> {
        let data = self.channel_data(channel)?;
        let Some(last) = before.checked_sub(1) else {
            return Ok(None);
        };
        let mut probes = 0;
        let point = RankCursor::new(data).advance_to(last, &mut probes);
        let Some(index) = point.rank.checked_sub(1) else {
            return Ok(None);
        };
        let position = point.block.and_then(|k| {
            if let Some(i) = point.count_in_block.checked_sub(1) {
                data.block(k)?.position(i)
            } else {
                let view = data.block(k.checked_sub(1)?)?;
                view.position(view.len().checked_sub(1)?)
            }
        });
        Ok(position.map(|position| Edge {
            position,
            rising: data.high_after(index),
        }))
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
