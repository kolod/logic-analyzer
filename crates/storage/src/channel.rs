//! Per-channel data as seen by a snapshot, rank lookup and finger search.
//!
//! `rank(x)` is the number of transitions with position `<= x`. The level at a sample
//! position is the channel's initial level flipped once per transition at or before it,
//! so the level is `initial XOR (rank(x) mod 2)`.

use std::sync::Arc;

use crate::block::BlockView;
use crate::segment::{self, SegmentList};
use crate::types::Edge;

/// Copy of the unsealed transitions of a channel (at most one block, a few KiB).
#[derive(Debug, Clone, Default)]
pub(crate) struct Tail {
    pub(crate) base: u64,
    pub(crate) first_rank: u64,
    pub(crate) offsets: Arc<[u32]>,
}

/// One channel as visible to a snapshot.
#[derive(Debug, Clone)]
pub(crate) struct ChannelData {
    /// Level of the first stored sample.
    pub(crate) initial_high: bool,
    /// Shared segments holding the sealed blocks.
    pub(crate) segments: SegmentList,
    /// Number of sealed blocks visible to this snapshot.
    pub(crate) sealed: usize,
    /// The unsealed transitions.
    pub(crate) tail: Tail,
}

impl ChannelData {
    /// Number of blocks including the tail (when it holds transitions).
    pub(crate) fn block_count(&self) -> usize {
        if self.tail.offsets.is_empty() {
            self.sealed
        } else {
            self.sealed.saturating_add(1)
        }
    }

    /// Block `index` (sealed blocks first, then the tail).
    pub(crate) fn block(&self, index: usize) -> Option<BlockView<'_>> {
        if index < self.sealed {
            let block = segment::block_ref(&self.segments, index)?;
            Some(BlockView {
                base: block.base,
                first_rank: block.first_rank,
                offsets: &block.offsets,
            })
        } else if index == self.sealed && !self.tail.offsets.is_empty() {
            Some(BlockView {
                base: self.tail.base,
                first_rank: self.tail.first_rank,
                offsets: &self.tail.offsets,
            })
        } else {
            None
        }
    }

    /// Whether the level after the transition with global index `rank_index` is high.
    pub(crate) fn high_after(&self, rank_index: u64) -> bool {
        self.initial_high ^ (rank_index.saturating_add(1) & 1 == 1)
    }

    /// Whether the level after `rank` transitions is high.
    pub(crate) fn high_at_rank(&self, rank: u64) -> bool {
        self.initial_high ^ (rank & 1 == 1)
    }
}

/// Result of resolving a position to a rank.
#[derive(Debug, Clone, Copy)]
pub(crate) struct RankPoint {
    /// Number of transitions with position `<=` the resolved position.
    pub(crate) rank: u64,
    /// Index of the last block whose base is `<=` the position, if any.
    pub(crate) block: Option<usize>,
    /// Number of transitions of that block at positions `<=` the position.
    pub(crate) count_in_block: usize,
}

/// Resolves a non-decreasing sequence of positions to ranks.
///
/// The first position is found with a binary search over the block index, O(log N). Each
/// following position gallops forward from the previous block: O(1) when it is in the
/// same or the next block and O(log of the blocks skipped) otherwise (finger search).
/// Every comparison is counted in the probes passed to `advance_to`.
pub(crate) struct RankCursor<'a> {
    channel: &'a ChannelData,
    /// Number of blocks whose base is `<=` the previous position; `None` before the first.
    blocks_le: Option<usize>,
}

impl<'a> RankCursor<'a> {
    pub(crate) fn new(channel: &'a ChannelData) -> Self {
        Self {
            channel,
            blocks_le: None,
        }
    }

    /// `base(j) <= x`, counting one probe.
    fn base_le(&self, j: usize, x: u64, probes: &mut u64) -> bool {
        *probes = probes.saturating_add(1);
        self.channel.block(j).is_some_and(|b| b.base <= x)
    }

    /// First index in `lo..hi` for which `base(j) <= x` is false (or `hi`).
    fn first_false(&self, mut lo: usize, mut hi: usize, x: u64, probes: &mut u64) -> usize {
        while lo < hi {
            let mid = lo.saturating_add(hi.saturating_sub(lo) / 2);
            if self.base_le(mid, x, probes) {
                lo = mid.saturating_add(1);
            } else {
                hi = mid;
            }
        }
        lo
    }

    /// Number of blocks with `base <= x`, given that it is at least `known`.
    fn count_blocks(&self, known: usize, x: u64, probes: &mut u64) -> usize {
        let total = self.channel.block_count();
        if known >= total {
            return total;
        }
        // Gallop: probe known, known + 1, known + 3, known + 7, ...
        if !self.base_le(known, x, probes) {
            return known;
        }
        let mut lo = known; // base(lo) <= x
        let mut step = 1_usize;
        loop {
            let probe = lo.saturating_add(step);
            if probe >= total {
                return self.first_false(lo.saturating_add(1), total, x, probes);
            }
            if self.base_le(probe, x, probes) {
                lo = probe;
                step = step.saturating_mul(2);
            } else {
                return self.first_false(lo.saturating_add(1), probe, x, probes);
            }
        }
    }

    /// Resolves position `x` (not smaller than the previous one) to a rank.
    pub(crate) fn advance_to(&mut self, x: u64, probes: &mut u64) -> RankPoint {
        let known = self.blocks_le.unwrap_or(0);
        let n = self.count_blocks(known, x, probes);
        self.blocks_le = Some(n);
        let Some(k) = n.checked_sub(1) else {
            return RankPoint {
                rank: 0,
                block: None,
                count_in_block: 0,
            };
        };
        match self.channel.block(k) {
            Some(view) => {
                let count = view.count_le(x, probes);
                RankPoint {
                    rank: view
                        .first_rank
                        .saturating_add(u64::try_from(count).unwrap_or(0)),
                    block: Some(k),
                    count_in_block: count,
                }
            }
            None => RankPoint {
                rank: 0,
                block: None,
                count_in_block: 0,
            },
        }
    }

    /// Position of the first transition after the point (the one with index `rank`).
    #[expect(dead_code, reason = "used by the viewport query (phase 4)")]
    pub(crate) fn next_transition(&self, point: &RankPoint, probes: &mut u64) -> Option<u64> {
        *probes = probes.saturating_add(1);
        match point.block {
            None => self.channel.block(0)?.position(0),
            Some(k) => {
                let view = self.channel.block(k)?;
                if point.count_in_block < view.len() {
                    view.position(point.count_in_block)
                } else {
                    self.channel.block(k.saturating_add(1))?.position(0)
                }
            }
        }
    }
}

/// Iterator over the transitions of one channel with `start < position <= end`.
#[derive(Debug)]
pub struct TransitionIter<'a> {
    channel: &'a ChannelData,
    block: usize,
    index: usize,
    end: u64,
}

impl<'a> TransitionIter<'a> {
    pub(crate) fn new(channel: &'a ChannelData, start: u64, end: u64) -> Self {
        let mut probes = 0;
        let mut cursor = RankCursor::new(channel);
        let point = cursor.advance_to(start, &mut probes);
        let (block, index) = match point.block {
            None => (0, 0),
            Some(k) => match channel.block(k) {
                Some(view) if point.count_in_block >= view.len() => (k.saturating_add(1), 0),
                _ => (k, point.count_in_block),
            },
        };
        Self {
            channel,
            block,
            index,
            end,
        }
    }
}

impl Iterator for TransitionIter<'_> {
    type Item = Edge;

    fn next(&mut self) -> Option<Edge> {
        loop {
            let view = self.channel.block(self.block)?;
            if self.index >= view.len() {
                self.block = self.block.saturating_add(1);
                self.index = 0;
                continue;
            }
            let position = view.position(self.index)?;
            if position > self.end {
                // Make the iterator fused: nothing more will be yielded.
                self.block = usize::MAX;
                return None;
            }
            let rank_index = view
                .first_rank
                .saturating_add(u64::try_from(self.index).unwrap_or(0));
            self.index = self.index.saturating_add(1);
            return Some(Edge {
                position,
                rising: self.channel.high_after(rank_index),
            });
        }
    }
}
