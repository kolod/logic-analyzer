//! The accounting of the samples lost on an overrun, as the loss marks of the protocol.

use la_proto::{LossMark, MAX_LOSS_MARKS};

/// Counts the samples stored and lost while a capture runs and keeps the loss marks.
///
/// The board calls [`stored`](Self::stored) for each run of samples it kept and [`lost`](Self::lost) for each
/// run it missed. At the end, the marks are sent before the samples, and the samples stored plus the samples
/// lost are the positions the capture covered.
#[derive(Debug)]
pub struct LossTracker {
    marks: [LossMark; MAX_LOSS_MARKS],
    len: usize,
    stored: u32,
    lost: u64,
    full: bool,
}

impl Default for LossTracker {
    fn default() -> Self {
        Self::new()
    }
}

impl LossTracker {
    /// A tracker with nothing counted.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            marks: [LossMark {
                position: 0,
                lost: 0,
            }; MAX_LOSS_MARKS],
            len: 0,
            stored: 0,
            lost: 0,
            full: false,
        }
    }

    /// `count` more samples were stored.
    pub fn stored(&mut self, count: u32) {
        self.stored = self.stored.saturating_add(count);
    }

    /// `count` samples were missed at the present position. Returns `false` when the capture has to end
    /// because there is no room for another mark (the run is not counted then).
    pub fn lost(&mut self, count: u32) -> bool {
        if count == 0 {
            return true;
        }
        // A run right after another with no stored sample between them is one run.
        if self.len > 0 {
            if let Some(last) = self.marks.get_mut(self.len.saturating_sub(1)) {
                if last.position == self.stored {
                    last.lost = last.lost.saturating_add(count);
                    self.lost = self.lost.saturating_add(u64::from(count));
                    return true;
                }
            }
        }
        match self.marks.get_mut(self.len) {
            Some(slot) if self.len < MAX_LOSS_MARKS => {
                *slot = LossMark {
                    position: self.stored,
                    lost: count,
                };
                self.len = self.len.saturating_add(1);
                self.lost = self.lost.saturating_add(u64::from(count));
                true
            }
            _ => {
                self.full = true;
                false
            }
        }
    }

    /// The marks so far, in order.
    #[must_use]
    pub fn marks(&self) -> &[LossMark] {
        self.marks.get(..self.len).unwrap_or(&[])
    }

    /// The samples stored.
    #[must_use]
    pub fn stored_total(&self) -> u32 {
        self.stored
    }

    /// The samples lost.
    #[must_use]
    pub fn lost_total(&self) -> u64 {
        self.lost
    }

    /// Whether a run could not be counted because the marks were full.
    #[must_use]
    pub fn is_full(&self) -> bool {
        self.full
    }
}
