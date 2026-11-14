//! Helpers for tests and benchmarks (feature `test-support`; not part of the stable API).
//!
//! [`synthetic_capture`] builds a capture with up to billions of transitions per channel
//! without the memory a real one would need: all channels share one block table, and all
//! blocks of that table share one payload of offsets (only the base position and rank
//! differ). The timings of such a capture are cache-friendly and therefore optimistic.

use std::sync::Arc;

use la_core::Result;

use crate::block::{BLOCK_CAPACITY, Block};
use crate::channel::{ChannelData, Tail};
use crate::config::{Limits, MAX_CHANNELS};
use crate::error;
use crate::published::{Published, Snapshot};
use crate::segment::{self, SegmentList};
use crate::types::{CaptureStatus, Gap};

/// Distance between two consecutive transitions in a synthetic capture.
pub const SYNTHETIC_SPACING: u64 = 3;
/// Position of the first transition in a synthetic capture.
pub const SYNTHETIC_FIRST: u64 = 3;

/// Builds a closed, complete capture with `transitions_per_channel` transitions on each
/// of `channels` channels. Transitions are evenly spaced
/// ([`SYNTHETIC_SPACING`] apart, the first at [`SYNTHETIC_FIRST`]); every channel is
/// identical and starts low.
///
/// # Errors
///
/// `storage.invalid_channel_count` if `channels` is not in `1..=32`.
pub fn synthetic_capture(channels: u8, transitions_per_channel: u64) -> Result<Snapshot> {
    if channels == 0 || channels > MAX_CHANNELS {
        return Err(error::invalid_channel_count(channels));
    }
    let capacity = u64::try_from(BLOCK_CAPACITY).unwrap_or(1024);
    let full_payload: Arc<[u32]> = (0..capacity)
        .map(|i| u32::try_from(i.saturating_mul(SYNTHETIC_SPACING)).unwrap_or(0))
        .collect();

    let mut segments: SegmentList = segment::new_list();
    let mut sealed = 0_usize;
    let mut remaining = transitions_per_channel;
    let mut rank = 0_u64;
    let mut base = SYNTHETIC_FIRST;
    while remaining > 0 {
        let in_block = remaining.min(capacity);
        let offsets: Arc<[u32]> = if in_block == capacity {
            Arc::clone(&full_payload)
        } else {
            full_payload
                .iter()
                .take(usize::try_from(in_block).unwrap_or(0))
                .copied()
                .collect()
        };
        let block = Block {
            base,
            first_rank: rank,
            offsets,
        };
        if !segment::push_block(&mut segments, sealed, block) {
            return Err(error::internal("a block slot was already written"));
        }
        sealed = sealed.saturating_add(1);
        rank = rank.saturating_add(in_block);
        base = base.saturating_add(in_block.saturating_mul(SYNTHETIC_SPACING));
        remaining = remaining.saturating_sub(in_block);
    }

    let length = if transitions_per_channel == 0 {
        1
    } else {
        SYNTHETIC_FIRST
            .saturating_add(
                transitions_per_channel
                    .saturating_sub(1)
                    .saturating_mul(SYNTHETIC_SPACING),
            )
            .saturating_add(1)
    };
    let channel = ChannelData {
        initial_high: false,
        segments,
        sealed,
        tail: Tail::default(),
    };
    let data: Vec<ChannelData> = (0..channels).map(|_| channel.clone()).collect();
    Ok(Snapshot {
        inner: Arc::new(Published {
            length,
            sample_rate_hz: 1_000_000,
            channels: data,
            gaps: Arc::new(Vec::<Gap>::new()),
            status: CaptureStatus {
                closed: true,
                complete: true,
                lost_positions: 0,
                stopped_by_limit: false,
                interrupted: false,
            },
            limits: Limits::default(),
            used_bytes: 0,
        }),
    })
}
