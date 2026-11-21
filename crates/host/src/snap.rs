//! Snapping of a marker to the transitions of a signal (`specs/015-markers-rework`, research R3
//! and R4).
//!
//! Everything here asks the storage's indexed `next_transition` / `previous_transition` a fixed
//! number of times, so the cost of a pointer move is O(log N) and does not grow with the distance
//! between the pointer and the answer or with the number of transitions in the snapping zone.
//! Edges next to a gap (lost samples) are not measured edges and are skipped.

use la_storage::Snapshot;

use crate::plan::{real_edge_after, real_edge_before};

/// What lies within the snapping distance of a sample on one channel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Zone {
    /// The nearest edge within the distance (the earlier one on a tie).
    pub nearest: Option<u64>,
    /// Whether a second edge lies within the distance too.
    pub crowded: bool,
}

/// The nearest measured edge of `channel` within `radius` samples of `sample`, and whether the
/// zone holds more than one.
pub(crate) fn zone(snapshot: &Snapshot, channel: u8, sample: u64, radius: u64) -> Zone {
    // At or after the sample, then the next one; before it, then the one before that.
    let after = real_edge_after(snapshot, channel, sample.saturating_sub(1));
    let before = real_edge_before(snapshot, channel, sample);
    let after_second = after.and_then(|a| real_edge_after(snapshot, channel, a));
    let before_second = before.and_then(|b| real_edge_before(snapshot, channel, b));
    let within_after = |p: Option<u64>| p.filter(|p| p.saturating_sub(sample) <= radius);
    let within_before = |p: Option<u64>| p.filter(|p| sample.saturating_sub(*p) <= radius);
    let (a, b) = (within_after(after), within_before(before));
    let nearest = match (b, a) {
        (Some(b), Some(a)) => {
            if sample.saturating_sub(b) <= a.saturating_sub(sample) {
                Some(b)
            } else {
                Some(a)
            }
        }
        (one, other) => one.or(other),
    };
    let count = [
        a,
        within_after(after_second),
        b,
        within_before(before_second),
    ]
    .iter()
    .flatten()
    .count();
    Zone {
        nearest,
        crowded: count >= 2,
    }
}

/// Up to four measured edges around `sample`, in order: the two before it and the two at or after
/// it.
pub(crate) fn nearest_four(snapshot: &Snapshot, channel: u8, sample: u64) -> Vec<u64> {
    let after = real_edge_after(snapshot, channel, sample.saturating_sub(1));
    let before = real_edge_before(snapshot, channel, sample);
    let after_second = after.and_then(|a| real_edge_after(snapshot, channel, a));
    let before_second = before.and_then(|b| real_edge_before(snapshot, channel, b));
    [before_second, before, after, after_second]
        .into_iter()
        .flatten()
        .collect()
}
