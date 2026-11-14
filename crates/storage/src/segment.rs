//! Append-only segments of block slots.
//!
//! Sealed blocks live in segments of [`SEGMENT_SLOTS`] write-once slots. A segment is
//! shared by the producer and all snapshots; a slot is written once, before the block
//! becomes visible to readers, so readers never need a lock to read it.

use std::sync::{Arc, OnceLock};

use crate::block::Block;

/// Number of block slots in one segment.
pub(crate) const SEGMENT_SLOTS: usize = 256;

/// Memory accounted for one segment.
pub(crate) const SEGMENT_BYTES: u64 = 4096 + 64;

/// A fixed array of write-once block slots.
#[derive(Debug)]
pub(crate) struct Segment {
    slots: Vec<OnceLock<Arc<Block>>>,
}

impl Segment {
    fn new() -> Self {
        Self {
            slots: (0..SEGMENT_SLOTS).map(|_| OnceLock::new()).collect(),
        }
    }

    fn get(&self, index: usize) -> Option<&Arc<Block>> {
        self.slots.get(index)?.get()
    }

    /// Writes a slot once; returns `false` if the slot does not exist or is taken.
    fn set(&self, index: usize, block: Arc<Block>) -> bool {
        self.slots
            .get(index)
            .is_some_and(|slot| slot.set(block).is_ok())
    }
}

/// The shared, growing list of segments of one channel.
pub(crate) type SegmentList = Arc<Vec<Arc<Segment>>>;

/// A new empty segment list.
pub(crate) fn new_list() -> SegmentList {
    Arc::new(Vec::new())
}

/// The sealed block with global block index `index`, if it exists.
pub(crate) fn block_ref(list: &SegmentList, index: usize) -> Option<&Arc<Block>> {
    list.get(index / SEGMENT_SLOTS)?.get(index % SEGMENT_SLOTS)
}

/// Stores `block` as sealed block number `index` (the next free one). Creates a new
/// segment, copying only the small list of segment pointers, when the last one is full.
/// Returns `false` only if an internal invariant is broken.
pub(crate) fn push_block(list: &mut SegmentList, index: usize, block: Block) -> bool {
    let segment_index = index / SEGMENT_SLOTS;
    if segment_index >= list.len() {
        Arc::make_mut(list).push(Arc::new(Segment::new()));
    }
    list.get(segment_index)
        .is_some_and(|segment| segment.set(index % SEGMENT_SLOTS, Arc::new(block)))
}
