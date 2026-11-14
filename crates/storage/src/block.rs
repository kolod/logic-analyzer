//! Immutable blocks of transitions.
//!
//! A block stores up to [`BLOCK_CAPACITY`] transitions of one channel as 32-bit offsets
//! from a 64-bit base position, so a transition costs 4 bytes and the offsets inside a
//! block can be binary-searched. A block is closed early when the next offset would not
//! fit in 32 bits, so very long flat stretches simply start a new block.

use std::sync::Arc;

/// Maximum number of transitions in one block.
pub(crate) const BLOCK_CAPACITY: usize = 1024;

/// Fixed per-block overhead used for memory accounting (base, first rank, pointers).
pub(crate) const BLOCK_HEADER_BYTES: u64 = 32;

/// Memory accounted for a block (or open block) holding `len` transitions.
pub(crate) fn block_bytes(len: usize) -> u64 {
    u64::try_from(len)
        .unwrap_or(u64::MAX)
        .saturating_mul(4)
        .saturating_add(BLOCK_HEADER_BYTES)
}

/// A sealed block. `offsets` may be shared between several blocks (test support).
#[derive(Debug)]
pub(crate) struct Block {
    /// Position that the offsets are relative to (position of the first transition).
    pub(crate) base: u64,
    /// Number of transitions of the channel before this block.
    pub(crate) first_rank: u64,
    /// Strictly increasing offsets from `base`; the first one is 0.
    pub(crate) offsets: Arc<[u32]>,
}

/// A borrowed read-only view of a block or of the open tail.
#[derive(Debug, Clone, Copy)]
pub(crate) struct BlockView<'a> {
    pub(crate) base: u64,
    pub(crate) first_rank: u64,
    pub(crate) offsets: &'a [u32],
}

impl BlockView<'_> {
    /// Number of transitions in the block.
    pub(crate) fn len(&self) -> usize {
        self.offsets.len()
    }

    /// Absolute position of transition `index` of this block.
    pub(crate) fn position(&self, index: usize) -> Option<u64> {
        self.base.checked_add(u64::from(*self.offsets.get(index)?))
    }

    /// Number of transitions of this block at positions `<= x`, found by binary search.
    /// Every comparison adds one to `probes`.
    pub(crate) fn count_le(&self, x: u64, probes: &mut u64) -> usize {
        if x < self.base {
            return 0;
        }
        let distance = x.saturating_sub(self.base);
        let Ok(distance) = u32::try_from(distance) else {
            // Every offset fits in u32, so all of them are <= distance.
            return self.offsets.len();
        };
        let mut lo = 0_usize;
        let mut hi = self.offsets.len();
        while lo < hi {
            let mid = lo.saturating_add(hi.saturating_sub(lo) / 2);
            *probes = probes.saturating_add(1);
            match self.offsets.get(mid) {
                Some(&offset) if offset <= distance => lo = mid.saturating_add(1),
                _ => hi = mid,
            }
        }
        lo
    }
}

/// The currently open (unsealed) block of a channel while the capture is written.
#[derive(Debug, Clone, Default)]
pub(crate) struct OpenBlock {
    pub(crate) base: u64,
    pub(crate) first_rank: u64,
    pub(crate) offsets: Vec<u32>,
}

impl OpenBlock {
    /// Adds the transition with global index `rank` at `position`. Positions must be
    /// strictly increasing. Blocks that become full (or cannot hold the offset) are
    /// sealed and appended to `sealed`.
    pub(crate) fn push(&mut self, position: u64, rank: u64, sealed: &mut Vec<Block>) {
        let offset = if self.offsets.is_empty() || self.offsets.len() >= BLOCK_CAPACITY {
            None
        } else {
            position
                .checked_sub(self.base)
                .and_then(|d| u32::try_from(d).ok())
        };
        let offset = match offset {
            Some(offset) => offset,
            None => {
                if !self.offsets.is_empty() {
                    sealed.push(self.seal());
                }
                self.base = position;
                self.first_rank = rank;
                0
            }
        };
        self.offsets.push(offset);
    }

    /// Turns the open block into a sealed block and leaves the open block empty.
    fn seal(&mut self) -> Block {
        Block {
            base: self.base,
            first_rank: self.first_rank,
            offsets: Arc::from(std::mem::take(&mut self.offsets)),
        }
    }

    /// Memory accounted for the open block (zero when empty).
    pub(crate) fn bytes(&self) -> u64 {
        if self.offsets.is_empty() {
            0
        } else {
            block_bytes(self.offsets.len())
        }
    }
}
