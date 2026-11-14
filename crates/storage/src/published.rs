//! Published (immutable) capture state, readers and snapshots.
//!
//! The producer builds new blocks outside any lock and publishes a chunk by replacing one
//! `Arc<Published>` behind a `RwLock` that is held only to clone or swap the pointer
//! (nanoseconds). A [`Snapshot`] is a clone of that `Arc`: it sees exactly the chunks
//! published before it was taken, stays valid while the producer continues, and is
//! queried without any lock, so readers can never delay the producer.

use std::sync::{Arc, PoisonError, RwLock};

use crate::channel::ChannelData;
use crate::config::Limits;
use crate::types::{CaptureStatus, Gap};

/// Everything a snapshot needs; immutable once published.
#[derive(Debug)]
pub(crate) struct Published {
    pub(crate) length: u64,
    pub(crate) sample_rate_hz: u64,
    pub(crate) channels: Vec<ChannelData>,
    pub(crate) gaps: Arc<Vec<Gap>>,
    pub(crate) status: CaptureStatus,
    pub(crate) limits: Limits,
}

impl Published {
    /// The gap that contains `position`, if any.
    pub(crate) fn gap_containing(&self, position: u64) -> Option<&Gap> {
        let index = self
            .gaps
            .partition_point(|g| g.end().unwrap_or(u64::MAX) <= position);
        self.gaps.get(index).filter(|g| g.start <= position)
    }
}

pub(crate) type Shared = Arc<RwLock<Arc<Published>>>;

/// Handle for readers: cloneable, `Send` and `Sync`.
#[derive(Debug, Clone)]
pub struct CaptureReader {
    pub(crate) shared: Shared,
}

impl CaptureReader {
    /// Takes a consistent snapshot of everything published so far. Cheap (one `Arc`
    /// clone); never blocks on a running query.
    #[must_use]
    pub fn snapshot(&self) -> Snapshot {
        let guard = self.shared.read().unwrap_or_else(PoisonError::into_inner);
        Snapshot {
            inner: Arc::clone(&guard),
        }
    }
}

/// An immutable view of a capture at one moment. Cheap to clone; all queries are methods
/// of the snapshot.
#[derive(Debug, Clone)]
pub struct Snapshot {
    pub(crate) inner: Arc<Published>,
}

impl Snapshot {
    /// Number of positions that are fully available (samples and gap positions).
    #[must_use]
    pub fn length(&self) -> u64 {
        self.inner.length
    }

    /// Sample rate in hertz.
    #[must_use]
    pub fn sample_rate_hz(&self) -> u64 {
        self.inner.sample_rate_hz
    }

    /// Number of channels.
    #[must_use]
    pub fn channels(&self) -> u8 {
        u8::try_from(self.inner.channels.len()).unwrap_or(u8::MAX)
    }

    /// Completeness of the capture at the time of the snapshot.
    #[must_use]
    pub fn status(&self) -> CaptureStatus {
        self.inner.status
    }

    /// The recorded gaps, sorted by position.
    #[must_use]
    pub fn gaps(&self) -> &[Gap] {
        &self.inner.gaps
    }
}
