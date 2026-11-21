//! A cropped copy of a capture: the positions `start..end` of a closed capture as a new closed
//! capture.
//!
//! Used by the marker actions that delete data to one side of a marker or keep only a pair (see
//! `specs/015-markers-rework`, research R6). The copy is built with the [`CaptureRestorer`], so
//! every rule of the capture data model holds for it and the memory budget is enforced before
//! anything is copied. The work is O(kept transitions) and can be cancelled between blocks of
//! transitions.

use la_core::Result;

use crate::capture::Capture;
use crate::config::{CaptureConfig, Limits};
use crate::error;
use crate::published::Snapshot;
use crate::restore::{CaptureRestorer, RestoredStatus};
use crate::types::{Gap, LevelState};

/// How many transitions are copied between two looks at the cancel flag.
const POLL_EVERY: u64 = 4096;

/// What the caller of [`crop`] can do while it runs: ask it to stop and watch its progress.
pub trait CropControl {
    /// Whether the user asked to stop; the copy is then dropped and `storage.cancelled` returned.
    fn cancelled(&self) -> bool;
    /// How far the work has come: `done` of `total` steps (a step is one channel of one of the
    /// two passes).
    fn progress(&self, done: u64, total: u64);
}

/// No control: never cancelled, progress ignored.
impl CropControl for () {
    fn cancelled(&self) -> bool {
        false
    }

    fn progress(&self, _done: u64, _total: u64) {}
}

/// The gaps of `snapshot` clipped to `start..end` and shifted so that `start` is 0.
fn clipped_gaps(snapshot: &Snapshot, start: u64, end: u64) -> Vec<Gap> {
    snapshot
        .gaps()
        .iter()
        .filter_map(|gap| {
            let from = gap.start.max(start);
            let to = gap.end()?.min(end);
            (from < to).then(|| Gap {
                start: from.saturating_sub(start),
                length: to.saturating_sub(from),
            })
        })
        .collect()
}

/// The position of the first stored sample of a capture of `length` positions with these gaps
/// (the rule of the restorer).
fn first_stored(length: u64, gaps: &[Gap]) -> Option<u64> {
    if length == 0 {
        return None;
    }
    match gaps.first() {
        Some(first) if first.start == 0 => first.end().filter(|end| *end < length),
        _ => Some(0),
    }
}

/// The transitions of `channel` that belong to the copy: those with `first < position < end`
/// (the one at `first` is folded into the initial level).
fn kept(
    snapshot: &Snapshot,
    channel: u8,
    first: u64,
    end: u64,
) -> Result<impl Iterator<Item = u64> + '_> {
    let last = end.saturating_sub(1);
    let iter = if first < last {
        Some(snapshot.transitions(channel, first, last)?)
    } else {
        None
    };
    Ok(iter.into_iter().flatten().map(|edge| edge.position))
}

/// Builds the closed capture of the positions `start..end` of `snapshot`.
///
/// Positions are shifted so that `start` becomes 0. Gaps are clipped to the range, the status
/// flags (limit reached, interrupted), the sample rate and the channel count are kept. A
/// transition exactly at `start` is not a transition of the copy: it is part of the level the
/// copy starts at.
///
/// # Errors
///
/// `storage.invalid_restore` if the range is empty or not inside the capture;
/// `storage.memory_limit` if the transitions to copy cannot fit `limits` (checked before any is
/// copied); `storage.cancelled` if `control` asked to stop.
pub fn crop(
    snapshot: &Snapshot,
    start: u64,
    end: u64,
    limits: Limits,
    control: &dyn CropControl,
) -> Result<Capture> {
    if start >= end || end > snapshot.length() {
        return Err(error::invalid_restore(
            "the range to keep is empty or not inside the capture",
        ));
    }
    let length = end.saturating_sub(start);
    let gaps = clipped_gaps(snapshot, start, end);
    let first = first_stored(length, &gaps).map(|first| start.saturating_add(first));
    let channels = snapshot.channels();
    let status = snapshot.status();
    let mut restorer = CaptureRestorer::new(
        CaptureConfig {
            channels,
            sample_rate_hz: snapshot.sample_rate_hz(),
            limits,
        },
        length,
        RestoredStatus {
            stopped_by_limit: status.stopped_by_limit,
            interrupted: status.interrupted || !status.closed,
        },
        gaps,
    )?;
    let steps = u64::from(channels).saturating_mul(2);

    // Pass 1: count what will be copied, so the budget can be checked before any copying.
    let mut counts = Vec::with_capacity(usize::from(channels));
    for channel in 0..channels {
        if control.cancelled() {
            return Err(error::cancelled());
        }
        let mut count = 0_u64;
        if let Some(first) = first {
            for _ in kept(snapshot, channel, first, end)? {
                count = count.saturating_add(1);
                if count % POLL_EVERY == 0 && control.cancelled() {
                    return Err(error::cancelled());
                }
            }
        }
        counts.push(count);
        control.progress(u64::from(channel).saturating_add(1), steps);
    }
    restorer.check_budget(&counts)?;

    // Pass 2: copy.
    for (channel, count) in (0..channels).zip(counts) {
        if control.cancelled() {
            return Err(error::cancelled());
        }
        let initial_high = match first {
            Some(first) => match snapshot.level_at(channel, first)? {
                LevelState::High => true,
                LevelState::Low => false,
                LevelState::Missing => {
                    return Err(error::invalid_restore(
                        "the first stored sample of the range has no level",
                    ));
                }
            },
            None => false,
        };
        restorer.begin_channel(channel, initial_high, count)?;
        if let Some(first) = first {
            let mut done = 0_u64;
            for position in kept(snapshot, channel, first, end)? {
                restorer.push_transition(position.saturating_sub(start))?;
                done = done.saturating_add(1);
                if done % POLL_EVERY == 0 && control.cancelled() {
                    return Err(error::cancelled());
                }
            }
        }
        control.progress(
            u64::from(channels)
                .saturating_add(u64::from(channel))
                .saturating_add(1),
            steps,
        );
    }
    restorer.finish()
}
