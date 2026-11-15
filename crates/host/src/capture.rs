//! The capture shown in the view, how it ended, and how a running one is observed.

use std::path::PathBuf;
use std::time::Instant;

use la_devices::{Acquisition, AcquisitionOutcome, CaptureDescription, EndReason, error};
use la_storage::Snapshot;

/// Where the capture on screen came from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CaptureOrigin {
    /// Acquired from a device in this run.
    Acquired,
    /// Loaded from this session file.
    File(PathBuf),
}

/// Why a capture ended (the text is `end-reason-*` in `ui.ftl`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaptureEnd {
    /// The configured length was reached.
    Completed,
    /// The user stopped it.
    Stopped,
    /// The memory limit was reached.
    MemoryLimit,
    /// The device was disconnected.
    Disconnected,
    /// The device or its transport failed; the code says how.
    DeviceError(&'static str),
    /// The capture finished but samples were lost on the way.
    SamplesLost,
    /// The capture was loaded from a file.
    Loaded,
}

impl CaptureEnd {
    /// Whether the capture ended before its configured length for a reason other than the user.
    #[must_use]
    pub fn is_abnormal(self) -> bool {
        matches!(
            self,
            CaptureEnd::MemoryLimit | CaptureEnd::Disconnected | CaptureEnd::DeviceError(_)
        )
    }

    /// The end reason of an acquisition outcome.
    #[must_use]
    pub fn from_outcome(outcome: &AcquisitionOutcome) -> Self {
        let limit_error = outcome
            .error
            .as_deref()
            .is_some_and(|e| e.code() == la_storage::error::MEMORY_LIMIT);
        if outcome.capture_status.stopped_by_limit || limit_error {
            return CaptureEnd::MemoryLimit;
        }
        match outcome.end {
            EndReason::Completed if outcome.lost_positions > 0 => CaptureEnd::SamplesLost,
            EndReason::Completed => CaptureEnd::Completed,
            EndReason::Stopped => CaptureEnd::Stopped,
            EndReason::Failed => match outcome.error.as_deref().map(la_core::Error::code) {
                Some(error::DISCONNECTED) => CaptureEnd::Disconnected,
                Some(code) => CaptureEnd::DeviceError(code),
                None => CaptureEnd::DeviceError(crate::error::INTERNAL),
            },
        }
    }
}

/// The capture on screen.
#[derive(Debug, Clone)]
pub struct CaptureView {
    /// The data (an immutable snapshot; a running capture is re-snapshotted while it grows).
    pub snapshot: Snapshot,
    /// What was captured and with which settings.
    pub description: CaptureDescription,
    /// Where it came from.
    pub origin: CaptureOrigin,
    /// How it ended; `None` while it runs.
    pub end: Option<CaptureEnd>,
    /// Whether samples were lost or the capture ended abnormally: it must never be presented as a
    /// clean measurement.
    pub incomplete: bool,
    /// Whether the data is simulated (cannot be changed by the user).
    pub simulated: bool,
}

impl CaptureView {
    /// A view of `snapshot` with the flags derived from the description and the snapshot.
    #[must_use]
    pub fn new(
        snapshot: Snapshot,
        description: CaptureDescription,
        origin: CaptureOrigin,
        end: Option<CaptureEnd>,
    ) -> Self {
        let incomplete = is_incomplete(&snapshot);
        let simulated = description.simulated();
        Self {
            snapshot,
            description,
            origin,
            end,
            incomplete,
            simulated,
        }
    }

    /// The channels that were enabled for the capture and exist in the data, in order.
    #[must_use]
    pub fn enabled_channels(&self) -> Vec<u8> {
        let mask = self.description.enabled_mask();
        (0..self.snapshot.channels())
            .filter(|c| u32::from(*c) < 32 && mask >> u32::from(*c) & 1 == 1)
            .collect()
    }
}

/// Whether a capture has lost samples or ended abnormally.
#[must_use]
pub fn is_incomplete(snapshot: &Snapshot) -> bool {
    let status = snapshot.status();
    status.lost_positions > 0
        || status.stopped_by_limit
        || status.interrupted
        || (status.closed && !status.complete)
}

/// Progress of the running capture (or of the last one, frozen at its end).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProgressView {
    /// Milliseconds since the capture started.
    pub elapsed_ms: u64,
    /// Samples stored so far.
    pub samples: u64,
    /// Positions so far (samples plus lost positions).
    pub positions: u64,
    /// Memory accounted for the stored data, in bytes.
    pub memory_used: u64,
    /// The memory limit of the capture, in bytes.
    pub memory_limit: u64,
    /// The configured length in samples, if finite.
    pub planned: Option<u64>,
}

/// The acquisition the shell is observing.
pub(crate) struct Running {
    pub(crate) acquisition: Acquisition,
    pub(crate) started: Instant,
    pub(crate) memory_limit: u64,
    pub(crate) planned: Option<u64>,
}
