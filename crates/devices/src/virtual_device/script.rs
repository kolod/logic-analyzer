//! The failure script and the trigger search of the virtual device.

use la_core::Result;

use super::ScriptedFailure;
use crate::capabilities::TriggerKind;
use crate::config::Trigger;
use crate::error;

/// The failures that are still to happen, in position order. Nothing fails unless it is
/// listed here.
#[derive(Debug, Default)]
pub(crate) struct FailureScript {
    pending: Vec<ScriptedFailure>,
    next: usize,
}

impl FailureScript {
    /// Creates a script; failures are applied in position order, ties in listing order.
    pub(crate) fn new(mut failures: Vec<ScriptedFailure>) -> Self {
        failures.sort_by_key(|failure| failure.position);
        Self {
            pending: failures,
            next: 0,
        }
    }

    /// The position of the next failure still to happen.
    pub(crate) fn next_position(&self) -> Option<u64> {
        self.pending.get(self.next).map(|failure| failure.position)
    }

    /// Takes the next failure if its position has been reached.
    pub(crate) fn take_due(&mut self, position: u64) -> Option<ScriptedFailure> {
        let failure = *self.pending.get(self.next)?;
        if failure.position <= position {
            self.next = self.next.saturating_add(1);
            Some(failure)
        } else {
            None
        }
    }
}

/// Where the trigger condition was met.
pub(crate) struct TriggerFound {
    /// Samples generated and discarded before the trigger sample.
    pub(crate) skipped: u64,
    /// The rest of the generated chunk, starting with the trigger sample.
    pub(crate) carry: Vec<u32>,
    /// How many samples the generator has produced in total (including `carry`).
    pub(crate) generated: u64,
}

/// Generates and discards samples until `trigger` holds, looking at the first `limit` samples.
///
/// For an edge the trigger sample is the first sample with the new level; for a level it is
/// the first sample with that level. `fill(start, out)` produces the samples for the
/// generator positions `start ..`.
///
/// # Errors
///
/// `device.trigger_timeout` if the condition does not occur within `limit` samples.
pub(crate) fn search_trigger(
    fill: &mut dyn FnMut(u64, &mut [u32]),
    trigger: Trigger,
    limit: u64,
    stride: usize,
) -> Result<TriggerFound> {
    const CHUNK: usize = 4_096;
    let stride = stride.max(1);
    let word = usize::from(trigger.channel) / 32;
    let bit = 1_u32
        .checked_shl(u32::from(trigger.channel) % 32)
        .unwrap_or(0);
    let mut buffer = vec![0_u32; CHUNK.saturating_mul(stride)];
    let mut position = 0_u64;
    let mut previous: Option<bool> = None;
    while position < limit {
        let len = usize::try_from(limit.saturating_sub(position)).map_or(CHUNK, |l| l.min(CHUNK));
        let Some(part) = buffer.get_mut(..len.saturating_mul(stride)) else {
            break;
        };
        fill(position, part);
        for (index, sample) in part.chunks_exact(stride).enumerate() {
            let level = sample.get(word).is_some_and(|w| w & bit != 0);
            let hit = match trigger.kind {
                TriggerKind::Level(high) => level == high,
                TriggerKind::RisingEdge => previous == Some(false) && level,
                TriggerKind::FallingEdge => previous == Some(true) && !level,
                TriggerKind::AnyEdge => previous.is_some_and(|p| p != level),
            };
            if hit {
                let offset = u64::try_from(index).unwrap_or(0);
                return Ok(TriggerFound {
                    skipped: position.saturating_add(offset),
                    carry: part
                        .get(index.saturating_mul(stride)..)
                        .map(<[u32]>::to_vec)
                        .unwrap_or_default(),
                    generated: position.saturating_add(u64::try_from(len).unwrap_or(0)),
                });
            }
            previous = Some(level);
        }
        position = position.saturating_add(u64::try_from(len).unwrap_or(0));
    }
    Err(error::trigger_timeout(limit))
}
