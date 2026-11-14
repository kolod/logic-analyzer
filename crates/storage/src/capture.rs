//! The producer side: building a capture chunk by chunk.

use std::sync::{Arc, PoisonError, RwLock};

use la_core::Result;

use crate::block::{Block, OpenBlock, block_bytes};
use crate::channel::{ChannelData, Tail};
use crate::config::CaptureConfig;
use crate::error;
use crate::ingest::ingest_chunk;
use crate::math::{checked_position_add, chunk_temp_bytes};
use crate::published::{CaptureReader, Published, Shared};
use crate::segment::{self, SEGMENT_BYTES, SEGMENT_SLOTS, SegmentList};
use crate::types::{CaptureStatus, Gap};

/// Producer-side state of one channel.
#[derive(Debug)]
struct ChannelState {
    initial_high: bool,
    segments: SegmentList,
    sealed: usize,
    sealed_bytes: u64,
    open: OpenBlock,
    transitions: u64,
}

impl ChannelState {
    fn new() -> Self {
        Self {
            initial_high: false,
            segments: segment::new_list(),
            sealed: 0,
            sealed_bytes: 0,
            open: OpenBlock::default(),
            transitions: 0,
        }
    }

    /// Exact memory accounted for this channel.
    fn bytes(&self) -> u64 {
        let segments = u64::try_from(self.segments.len()).unwrap_or(u64::MAX);
        self.sealed_bytes
            .saturating_add(self.open.bytes())
            .saturating_add(segments.saturating_mul(SEGMENT_BYTES))
    }

    fn view(&self) -> ChannelData {
        ChannelData {
            initial_high: self.initial_high,
            segments: Arc::clone(&self.segments),
            sealed: self.sealed,
            tail: Tail {
                base: self.open.base,
                first_rank: self.open.first_rank,
                offsets: Arc::from(self.open.offsets.as_slice()),
            },
        }
    }
}

/// The new state of one channel after a chunk, prepared before anything is published.
struct Prepared {
    open: OpenBlock,
    sealed: Vec<Block>,
    transitions: u64,
}

/// A capture being recorded. It has a single owner (the producer); readers use the
/// [`CaptureReader`] returned by [`Capture::reader`].
///
/// Memory: see [`crate::Limits`] for the worst-case cost per sample. A chunk is converted
/// to blocks before it is published, so it is accepted completely or not at all.
#[derive(Debug)]
pub struct Capture {
    config: CaptureConfig,
    channels: Vec<ChannelState>,
    length: u64,
    last_word: Option<u32>,
    used_bytes: u64,
    gaps: Arc<Vec<Gap>>,
    lost_positions: u64,
    stopped_by_limit: bool,
    closed: bool,
    shared: Shared,
}

impl Capture {
    /// Creates an empty capture.
    ///
    /// # Errors
    ///
    /// Returns `storage.invalid_channel_count`, `storage.invalid_sample_rate` or
    /// `storage.invalid_limits` for an invalid configuration.
    pub fn new(config: CaptureConfig) -> Result<Self> {
        config.validate()?;
        let channels = (0..config.channels).map(|_| ChannelState::new()).collect();
        let mut capture = Self {
            config,
            channels,
            length: 0,
            last_word: None,
            used_bytes: 0,
            gaps: Arc::new(Vec::new()),
            lost_positions: 0,
            stopped_by_limit: false,
            closed: false,
            shared: Arc::new(RwLock::new(Arc::new(Published {
                length: 0,
                sample_rate_hz: config.sample_rate_hz,
                channels: Vec::new(),
                gaps: Arc::new(Vec::new()),
                status: CaptureStatus::default(),
                limits: config.limits,
            }))),
        };
        capture.publish();
        tracing::info!(
            target: "la_storage",
            channels = config.channels,
            sample_rate_hz = config.sample_rate_hz,
            memory_budget_bytes = config.limits.memory_budget_bytes,
            "capture created"
        );
        Ok(capture)
    }

    /// A handle that readers use to take snapshots; cloneable, `Send` and `Sync`.
    #[must_use]
    pub fn reader(&self) -> CaptureReader {
        CaptureReader {
            shared: Arc::clone(&self.shared),
        }
    }

    /// Number of positions so far (samples and gap positions).
    #[must_use]
    pub fn length(&self) -> u64 {
        self.length
    }

    /// Exact memory currently accounted for stored data and indexes, in bytes.
    #[must_use]
    pub fn memory_used(&self) -> u64 {
        self.used_bytes
    }

    fn status(&self) -> CaptureStatus {
        CaptureStatus {
            closed: self.closed,
            complete: self.closed && self.lost_positions == 0 && !self.stopped_by_limit,
            lost_positions: self.lost_positions,
            stopped_by_limit: self.stopped_by_limit,
        }
    }

    /// Replaces the published state with the current one.
    fn publish(&mut self) {
        let published = Published {
            length: self.length,
            sample_rate_hz: self.config.sample_rate_hz,
            channels: self.channels.iter().map(ChannelState::view).collect(),
            gaps: Arc::clone(&self.gaps),
            status: self.status(),
            limits: self.config.limits,
        };
        // The lock is held only to swap the pointer.
        let mut guard = self.shared.write().unwrap_or_else(PoisonError::into_inner);
        *guard = Arc::new(published);
    }

    /// Memory accounted for the gap list.
    fn gap_bytes(&self) -> u64 {
        u64::try_from(self.gaps.len())
            .unwrap_or(u64::MAX)
            .saturating_mul(u64::try_from(std::mem::size_of::<Gap>()).unwrap_or(16))
    }

    fn recompute_used(&mut self) {
        let channels: u64 = self.channels.iter().map(ChannelState::bytes).sum();
        self.used_bytes = channels.saturating_add(self.gap_bytes());
    }

    /// Exact total memory after committing `prepared`, computed without committing.
    fn bytes_after(&self, prepared: &[Prepared]) -> u64 {
        let channels = self
            .channels
            .iter()
            .zip(prepared)
            .map(|(state, new)| {
                let new_sealed = new
                    .sealed
                    .iter()
                    .map(|b| block_bytes(b.offsets.len()))
                    .fold(0_u64, u64::saturating_add);
                let blocks = state.sealed.saturating_add(new.sealed.len());
                let segments = u64::try_from(blocks.div_ceil(SEGMENT_SLOTS)).unwrap_or(u64::MAX);
                state
                    .sealed_bytes
                    .saturating_add(new_sealed)
                    .saturating_add(new.open.bytes())
                    .saturating_add(segments.saturating_mul(SEGMENT_BYTES))
            })
            .fold(0_u64, u64::saturating_add);
        channels.saturating_add(self.gap_bytes())
    }

    /// Appends a chunk of consecutive samples for all channels.
    ///
    /// `start` must equal [`Capture::length`]. Bit `i` of every word is the level of
    /// channel `i`; bits above the channel count are ignored. The chunk is accepted
    /// completely or not at all.
    ///
    /// # Errors
    ///
    /// `storage.closed`, `storage.chunk_empty`, `storage.chunk_too_large`,
    /// `storage.chunk_out_of_order` or `storage.position_overflow` for invalid input;
    /// `storage.memory_limit` when the chunk would exceed the memory budget (the capture
    /// is then stopped: stored data stays readable, every later append fails the same
    /// way, and the status reports `stopped_by_limit`) or when converting the chunk could
    /// not fit the budget even in the worst case (`4 × samples × channels` bytes, the
    /// capture is not stopped). A rejected call changes no stored data.
    pub fn append(&mut self, start: u64, samples: &[u32]) -> Result<()> {
        if self.closed {
            return Err(error::closed());
        }
        // Validate before doing any work proportional to the chunk size.
        if samples.is_empty() {
            return Err(error::chunk_empty());
        }
        if samples.len() > usize::try_from(self.config.limits.max_chunk_samples).unwrap_or(0) {
            return Err(error::chunk_too_large(
                samples.len(),
                self.config.limits.max_chunk_samples,
            ));
        }
        if start != self.length {
            return Err(error::chunk_out_of_order(self.length, start));
        }
        let budget = self.config.limits.memory_budget_bytes;
        if self.stopped_by_limit {
            // Once stopped by the memory budget, the capture stays stopped.
            return Err(error::memory_limit(self.used_bytes, 0, budget));
        }
        // Converting a chunk needs at most 4 bytes of temporary memory per sample and
        // channel. A chunk whose worst case alone exceeds the whole budget is rejected
        // before anything is allocated; this does not stop the capture, a smaller chunk
        // is still accepted.
        let temp = chunk_temp_bytes(samples.len(), self.config.channels).unwrap_or(u64::MAX);
        if temp > budget {
            return Err(error::memory_limit(self.used_bytes, temp, budget));
        }
        let samples_len = u64::try_from(samples.len()).map_err(|_| error::position_overflow())?;
        let new_length = checked_position_add(self.length, samples_len)?;

        let ingested = ingest_chunk(samples, self.last_word, self.config.channels);

        // Prepare the new state of every channel without touching shared state.
        // Bounded by the validated channel count (at most 32); no allocation below is sized
        // from an unvalidated caller value (FR-018): the chunk length was checked against
        // `max_chunk_samples` and the memory budget above.
        let mut prepared = Vec::with_capacity(self.channels.len());
        for (state, relative) in self.channels.iter().zip(&ingested.per_channel) {
            let mut open = state.open.clone();
            let mut sealed = Vec::new();
            let mut rank = state.transitions;
            for &offset in relative {
                let position = checked_position_add(start, u64::from(offset))?;
                open.push(position, rank, &mut sealed);
                rank = rank.saturating_add(1);
            }
            prepared.push(Prepared {
                open,
                sealed,
                transitions: rank,
            });
        }

        // Memory budget: the chunk is accepted completely or not at all. A rejection
        // discards the prepared blocks and publishes only the "stopped" flag.
        let total_after = self.bytes_after(&prepared);
        if total_after > budget {
            self.stopped_by_limit = true;
            self.publish();
            tracing::warn!(
                target: "la_storage",
                used_bytes = self.used_bytes,
                budget_bytes = budget,
                rejected_samples = samples_len,
                "memory limit reached"
            );
            return Err(error::memory_limit(
                self.used_bytes,
                total_after.saturating_sub(self.used_bytes),
                budget,
            ));
        }

        // Commit: nothing below can fail for external reasons.
        let first_ever = self.last_word.is_none();
        for (channel, (state, new)) in self.channels.iter_mut().zip(prepared).enumerate() {
            if first_ever {
                let bit = u32::try_from(channel)
                    .ok()
                    .and_then(|c| ingested.first_word.checked_shr(c));
                state.initial_high = bit.is_some_and(|w| w & 1 == 1);
            }
            for block in new.sealed {
                let bytes = block_bytes(block.offsets.len());
                if !segment::push_block(&mut state.segments, state.sealed, block) {
                    return Err(error::internal("a block slot was already written"));
                }
                state.sealed = state.sealed.saturating_add(1);
                state.sealed_bytes = state.sealed_bytes.saturating_add(bytes);
            }
            state.open = new.open;
            state.transitions = new.transitions;
        }
        self.length = new_length;
        self.last_word = Some(ingested.last_word);
        self.recompute_used();
        self.publish();
        Ok(())
    }

    /// Ends the capture; later appends fail with `storage.closed`. Closing twice is
    /// harmless and returns the same status.
    ///
    /// # Errors
    ///
    /// Currently never fails; the `Result` leaves room for future checks.
    pub fn close(&mut self) -> Result<CaptureStatus> {
        if !self.closed {
            self.closed = true;
            self.publish();
            let status = self.status();
            tracing::info!(
                target: "la_storage",
                length = self.length,
                complete = status.complete,
                lost_positions = status.lost_positions,
                stopped_by_limit = status.stopped_by_limit,
                "capture closed"
            );
        }
        Ok(self.status())
    }
}
