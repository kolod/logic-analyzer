//! Cancellation and progress of one save or load.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU8, AtomicU64, Ordering};

use la_core::Result;

use crate::error;

/// Where a save or load is in its life.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OperationState {
    /// Still working.
    Running,
    /// Finished successfully.
    Succeeded,
    /// Failed with an error.
    Failed,
    /// Ended because it was cancelled.
    Cancelled,
}

impl OperationState {
    fn code(self) -> u8 {
        match self {
            Self::Running => 0,
            Self::Succeeded => 1,
            Self::Failed => 2,
            Self::Cancelled => 3,
        }
    }

    fn from_code(code: u8) -> Self {
        match code {
            1 => Self::Succeeded,
            2 => Self::Failed,
            3 => Self::Cancelled,
            _ => Self::Running,
        }
    }
}

/// How far a save or load has come.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Progress {
    /// The state of the operation.
    pub state: OperationState,
    /// Bytes written or read so far; never decreases and never exceeds `total_bytes`.
    pub done_bytes: u64,
    /// The size of the file being written or read (zero until it is known).
    pub total_bytes: u64,
}

#[derive(Debug)]
struct Inner {
    cancel: AtomicBool,
    done: AtomicU64,
    total: AtomicU64,
    state: AtomicU8,
}

/// Cancellation flag and progress of one operation. Cheap to clone; every clone shares the same
/// state, so one thread can cancel and watch what another thread does.
#[derive(Debug, Clone)]
pub struct Control {
    inner: Arc<Inner>,
}

impl Default for Control {
    fn default() -> Self {
        Self::new()
    }
}

impl Control {
    /// A new control: not cancelled, nothing done.
    #[must_use]
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Inner {
                cancel: AtomicBool::new(false),
                done: AtomicU64::new(0),
                total: AtomicU64::new(0),
                state: AtomicU8::new(OperationState::Running.code()),
            }),
        }
    }

    /// Asks the operation to stop; it ends promptly with `session.cancelled`.
    pub fn cancel(&self) {
        self.inner.cancel.store(true, Ordering::Release);
    }

    /// `true` once [`cancel`](Self::cancel) was called.
    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        self.inner.cancel.load(Ordering::Acquire)
    }

    /// The current progress.
    #[must_use]
    pub fn progress(&self) -> Progress {
        let total = self.inner.total.load(Ordering::Acquire);
        let done = self.inner.done.load(Ordering::Acquire).min(total);
        Progress {
            state: OperationState::from_code(self.inner.state.load(Ordering::Acquire)),
            done_bytes: done,
            total_bytes: total,
        }
    }

    /// Fails with `session.cancelled` if the operation was cancelled.
    pub(crate) fn check(&self) -> Result<()> {
        if self.is_cancelled() {
            Err(error::cancelled())
        } else {
            Ok(())
        }
    }

    pub(crate) fn set_total(&self, total: u64) {
        self.inner.total.store(total, Ordering::Release);
    }

    /// Records that `done` bytes are finished (monotonic: a smaller value is ignored).
    pub(crate) fn set_done(&self, done: u64) {
        self.inner.done.fetch_max(done, Ordering::AcqRel);
    }

    /// Sets the final state from the result of the operation.
    pub(crate) fn finish<T>(&self, result: &Result<T>) {
        let state = match result {
            Ok(_) => {
                // A successful operation is complete by definition.
                let total = self.inner.total.load(Ordering::Acquire);
                self.set_done(total);
                OperationState::Succeeded
            }
            Err(e) if e.code() == error::CANCELLED => OperationState::Cancelled,
            Err(_) => OperationState::Failed,
        };
        self.inner.state.store(state.code(), Ordering::Release);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cancellation_and_progress_are_shared_between_clones() {
        let a = Control::new();
        let b = a.clone();
        assert!(!b.is_cancelled());
        assert!(b.check().is_ok());
        a.cancel();
        assert!(b.is_cancelled());
        assert_eq!(b.check().unwrap_err().code(), error::CANCELLED);
        a.set_total(100);
        a.set_done(40);
        a.set_done(30);
        let p = b.progress();
        assert_eq!(
            (p.done_bytes, p.total_bytes),
            (40, 100),
            "progress never decreases"
        );
        assert_eq!(p.state, OperationState::Running);
        a.set_done(500);
        assert_eq!(b.progress().done_bytes, 100, "never above the total");
    }

    #[test]
    fn the_final_state_follows_the_result() {
        let ok = Control::new();
        ok.set_total(10);
        ok.finish(&Ok::<(), la_core::Error>(()));
        let p = ok.progress();
        assert_eq!((p.state, p.done_bytes), (OperationState::Succeeded, 10));
        let failed = Control::new();
        failed.finish::<()>(&Err(error::damaged("x")));
        assert_eq!(failed.progress().state, OperationState::Failed);
        let cancelled = Control::new();
        cancelled.finish::<()>(&Err(error::cancelled()));
        assert_eq!(cancelled.progress().state, OperationState::Cancelled);
    }
}
