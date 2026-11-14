//! Rebuilding a closed capture from stored transitions (used by session files).
//!
//! A [`CaptureRestorer`] builds the same blocks the recorder builds, so a restored capture has
//! the same layout, the same memory accounting and the same query cost as a recorded one. Every
//! rule of the capture data model is checked while it is built, and the memory budget is
//! enforced before and while transitions are pushed, so hostile input cannot make it allocate
//! more than the budget.

use std::sync::Arc;

use la_core::Result;

use crate::block::{BLOCK_CAPACITY, block_bytes};
use crate::capture::Capture;
use crate::config::CaptureConfig;
use crate::error;
use crate::segment::{self, SEGMENT_BYTES, SEGMENT_SLOTS};
use crate::types::Gap;

/// The completeness flags of a capture that cannot be derived from its gaps.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RestoredStatus {
    /// An append was rejected because of the memory budget when the capture was recorded.
    pub stopped_by_limit: bool,
    /// The capture was ended by a failure outside it, or was saved while still recording.
    pub interrupted: bool,
}

/// The channel whose transitions are being pushed.
#[derive(Debug)]
struct Current {
    index: usize,
    declared: u64,
    pushed: u64,
    last_position: u64,
    gap_cursor: usize,
}

/// Builds a closed [`Capture`] from the transitions of every channel.
///
/// Call order: [`new`](Self::new), then for every channel in order
/// [`begin_channel`](Self::begin_channel) followed by exactly the declared number of
/// [`push_transition`](Self::push_transition) calls, then [`finish`](Self::finish). A restorer
/// that returned an error must be dropped; nothing of a failed restore is visible anywhere.
#[derive(Debug)]
pub struct CaptureRestorer {
    capture: Capture,
    length: u64,
    gaps: Vec<Gap>,
    /// Position of the first stored sample; `None` if the capture holds no sample.
    first_stored: Option<u64>,
    status: RestoredStatus,
    next_channel: usize,
    current: Option<Current>,
    /// Exact memory of the channels that are complete.
    done_bytes: u64,
    gap_bytes: u64,
    /// Estimated memory of the channels declared so far (lower bound of the final size).
    estimated: u64,
}

fn gap_entry_bytes() -> u64 {
    u64::try_from(std::mem::size_of::<Gap>()).unwrap_or(16)
}

/// Memory the recorder accounts for `transitions` transitions of one channel, assuming every
/// block is full (a lower bound: blocks that are closed early cost more).
fn estimate_channel_bytes(transitions: u64) -> u64 {
    let capacity = u64::try_from(BLOCK_CAPACITY).unwrap_or(1024);
    let full = transitions.checked_div(capacity).unwrap_or(0);
    let rest = transitions.checked_rem(capacity).unwrap_or(0);
    let mut bytes = full.saturating_mul(block_bytes(BLOCK_CAPACITY));
    let mut blocks = full;
    if rest > 0 {
        bytes = bytes.saturating_add(block_bytes(usize::try_from(rest).unwrap_or(0)));
        blocks = blocks.saturating_add(1);
    }
    let segments = blocks
        .checked_add(
            u64::try_from(SEGMENT_SLOTS)
                .unwrap_or(256)
                .saturating_sub(1),
        )
        .and_then(|n| n.checked_div(u64::try_from(SEGMENT_SLOTS).unwrap_or(256)))
        .unwrap_or(0);
    bytes.saturating_add(segments.saturating_mul(SEGMENT_BYTES))
}

impl CaptureRestorer {
    /// Starts restoring a capture of `length` positions with the given gaps and flags.
    ///
    /// # Errors
    ///
    /// The configuration errors of [`Capture::new`]; `storage.invalid_restore` if a gap has
    /// zero length, does not fit the length, or overlaps, touches or precedes its predecessor;
    /// `storage.memory_limit` if the gap list alone exceeds the memory budget.
    pub fn new(
        config: CaptureConfig,
        length: u64,
        status: RestoredStatus,
        gaps: Vec<Gap>,
    ) -> Result<Self> {
        let capture = Capture::new(config)?;
        let mut previous_end: Option<u64> = None;
        for gap in &gaps {
            if gap.length == 0 {
                return Err(error::invalid_restore("a gap has zero length"));
            }
            let Some(end) = gap.end().filter(|end| *end <= length) else {
                return Err(error::invalid_restore(
                    "a gap does not fit the capture length",
                ));
            };
            if previous_end.is_some_and(|prev| gap.start <= prev) {
                return Err(error::invalid_restore(
                    "gaps must be sorted and separated by at least one stored position",
                ));
            }
            previous_end = Some(end);
        }
        let gap_bytes = u64::try_from(gaps.len())
            .unwrap_or(u64::MAX)
            .saturating_mul(gap_entry_bytes());
        let budget = config.limits.memory_budget_bytes;
        if gap_bytes > budget {
            return Err(error::memory_limit(0, gap_bytes, budget));
        }
        let first_stored = match gaps.first() {
            _ if length == 0 => None,
            Some(first) if first.start == 0 => first.end().filter(|end| *end < length),
            _ => Some(0),
        };
        Ok(Self {
            capture,
            length,
            gaps,
            first_stored,
            status,
            next_channel: 0,
            current: None,
            done_bytes: 0,
            gap_bytes,
            estimated: gap_bytes,
        })
    }

    /// Checks, before any transition is pushed, that channels with the given transition counts
    /// (one entry per channel, in order) fit the memory budget together with the gap list.
    ///
    /// # Errors
    ///
    /// `storage.memory_limit` if the lower bound of the memory they need exceeds the budget.
    pub fn check_budget(&self, transitions_per_channel: &[u64]) -> Result<()> {
        let budget = self.capture.config.limits.memory_budget_bytes;
        let needed = transitions_per_channel
            .iter()
            .fold(self.gap_bytes, |sum, count| {
                sum.saturating_add(estimate_channel_bytes(*count))
            });
        if needed > budget {
            return Err(error::memory_limit(
                self.gap_bytes,
                needed.saturating_sub(self.gap_bytes),
                budget,
            ));
        }
        Ok(())
    }

    /// Declares the next channel: its level at the first stored sample and how many
    /// transitions will follow.
    ///
    /// # Errors
    ///
    /// `storage.invalid_restore` if the previous channel is incomplete, all channels were
    /// already declared, a channel without stored samples has a high initial level or
    /// transitions, or the count cannot fit the capture length; `storage.memory_limit` if the
    /// declared transitions cannot fit the memory budget (checked before any is pushed).
    pub fn begin_channel(
        &mut self,
        channel: u8,
        initial_high: bool,
        transitions: u64,
    ) -> Result<()> {
        if self.current.is_some() {
            return Err(error::invalid_restore(
                "the previous channel did not receive all its transitions",
            ));
        }
        let index = self.next_channel;
        if usize::from(channel) != index || index >= self.capture.channels.len() {
            return Err(error::invalid_restore(
                "channels must be declared once each, in order",
            ));
        }
        let room = match self.first_stored {
            None => {
                if initial_high || transitions > 0 {
                    return Err(error::invalid_restore(
                        "a capture without stored samples has no transitions and starts low",
                    ));
                }
                0
            }
            Some(first) => self.length.saturating_sub(first).saturating_sub(1),
        };
        if transitions > room {
            return Err(error::invalid_restore(
                "more transitions are declared than the capture length can hold",
            ));
        }
        let estimated = self
            .estimated
            .saturating_add(estimate_channel_bytes(transitions));
        let budget = self.capture.config.limits.memory_budget_bytes;
        if estimated > budget {
            return Err(error::memory_limit(
                self.estimated,
                estimated.saturating_sub(self.estimated),
                budget,
            ));
        }
        self.estimated = estimated;
        if let Some(state) = self.capture.channels.get_mut(index) {
            state.initial_high = initial_high;
        }
        self.current = Some(Current {
            index,
            declared: transitions,
            pushed: 0,
            last_position: self.first_stored.unwrap_or(0),
            gap_cursor: 0,
        });
        self.next_channel = index.saturating_add(1);
        if transitions == 0 {
            self.complete_current();
        }
        Ok(())
    }

    /// Adds the next transition of the current channel at `position`.
    ///
    /// # Errors
    ///
    /// `storage.invalid_restore` if no channel is open, more transitions arrive than were
    /// declared, or the position is not strictly increasing, is not after the first stored
    /// sample, is not below the length, or lies inside a gap; `storage.memory_limit` if the
    /// blocks built so far exceed the memory budget.
    pub fn push_transition(&mut self, position: u64) -> Result<()> {
        let Some(current) = self.current.as_mut() else {
            return Err(error::invalid_restore("no channel is open for transitions"));
        };
        if current.pushed >= current.declared {
            return Err(error::invalid_restore(
                "more transitions arrived than were declared",
            ));
        }
        if position <= current.last_position || position >= self.length {
            return Err(error::invalid_restore(
                "a transition position is not increasing, not after the first stored sample, or beyond the length",
            ));
        }
        while self
            .gaps
            .get(current.gap_cursor)
            .is_some_and(|g| g.end().unwrap_or(u64::MAX) <= position)
        {
            current.gap_cursor = current.gap_cursor.saturating_add(1);
        }
        if self
            .gaps
            .get(current.gap_cursor)
            .is_some_and(|g| g.start <= position)
        {
            return Err(error::invalid_restore("a transition lies inside a gap"));
        }
        let rank = current.pushed;
        current.pushed = current.pushed.saturating_add(1);
        current.last_position = position;
        let index = current.index;
        let complete = current.pushed == current.declared;

        let Some(state) = self.capture.channels.get_mut(index) else {
            return Err(error::internal("the open channel does not exist"));
        };
        let mut sealed = Vec::new();
        state.open.push(position, rank, &mut sealed);
        for block in sealed {
            let bytes = block_bytes(block.offsets.len());
            if !segment::push_block(&mut state.segments, state.sealed, block) {
                return Err(error::internal("a block slot was already written"));
            }
            state.sealed = state.sealed.saturating_add(1);
            state.sealed_bytes = state.sealed_bytes.saturating_add(bytes);
        }
        state.transitions = rank.saturating_add(1);
        let used = self
            .done_bytes
            .saturating_add(self.gap_bytes)
            .saturating_add(state.bytes());
        let budget = self.capture.config.limits.memory_budget_bytes;
        if used > budget {
            return Err(error::memory_limit(used, 0, budget));
        }
        if complete {
            self.complete_current();
        }
        Ok(())
    }

    /// Ends the current channel and adds its exact memory to the total.
    fn complete_current(&mut self) {
        if let Some(current) = self.current.take() {
            if let Some(state) = self.capture.channels.get(current.index) {
                self.done_bytes = self.done_bytes.saturating_add(state.bytes());
            }
        }
    }

    /// Finishes the restore and returns the closed capture.
    ///
    /// # Errors
    ///
    /// `storage.invalid_restore` if a channel is missing or incomplete; `storage.memory_limit`
    /// if the final accounting exceeds the budget.
    pub fn finish(mut self) -> Result<Capture> {
        if self.current.is_some() || self.next_channel != self.capture.channels.len() {
            return Err(error::invalid_restore(
                "every channel must be declared and receive all its transitions",
            ));
        }
        let lost = self
            .gaps
            .iter()
            .fold(0_u64, |sum, gap| sum.saturating_add(gap.length));
        self.capture.length = self.length;
        self.capture.gaps = Arc::new(std::mem::take(&mut self.gaps));
        self.capture.lost_positions = lost;
        self.capture.stopped_by_limit = self.status.stopped_by_limit;
        self.capture.interrupted = self.status.interrupted;
        self.capture.closed = true;
        self.capture.recompute_used();
        let budget = self.capture.config.limits.memory_budget_bytes;
        if self.capture.used_bytes > budget {
            return Err(error::memory_limit(self.capture.used_bytes, 0, budget));
        }
        self.capture.publish();
        tracing::info!(
            target: "la_storage",
            length = self.length,
            channels = self.capture.channels.len(),
            memory_used = self.capture.used_bytes,
            "capture restored"
        );
        Ok(self.capture)
    }
}
