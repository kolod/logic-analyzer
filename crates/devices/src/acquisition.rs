//! The acquisition handle: what the UI holds while a capture runs.
//!
//! Every method returns at once and none ever waits for a device: progress is read from
//! atomics, `stop` only sets a flag, and results are waited for with an explicit timeout.

use std::sync::atomic::{AtomicBool, AtomicU8, AtomicU64, Ordering};
use std::sync::mpsc::{Receiver, SyncSender};
use std::sync::{Arc, Condvar, Mutex, PoisonError};
use std::time::{Duration, Instant};

use la_core::Error;
use la_diagnostics::lock_recover;
use la_storage::{CaptureReader, CaptureStatus, Limits};

use crate::config::AppliedConfig;
use crate::description::CaptureDescription;

/// Tuning of one acquisition. The defaults suit normal use; tests shorten the timeouts.
#[derive(Debug, Clone, Copy)]
pub struct AcquisitionOptions {
    /// The capture's memory budget and chunk limit.
    pub limits: Limits,
    /// How long the driver may go without a sign of life (a loop iteration of the driver
    /// thread, including `Idle` reads) before it is declared unresponsive. Also bounds how
    /// long `Device::start` and `Device::stop` may take.
    pub stall_timeout: Duration,
    /// How many chunks may wait between the driver thread and the ingest thread; when the
    /// store is slower than the device the excess is recorded as lost positions.
    pub channel_capacity: usize,
    /// How many samples to ask the device for in one read (at most `limits.max_chunk_samples`).
    pub chunk_samples: usize,
}

impl Default for AcquisitionOptions {
    fn default() -> Self {
        Self {
            limits: Limits::default(),
            stall_timeout: Duration::from_secs(2),
            channel_capacity: 8,
            chunk_samples: 16_384,
        }
    }
}

/// Where an acquisition is in its life.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AcquisitionState {
    /// The driver is being opened and started.
    Starting,
    /// Samples are flowing.
    Running,
    /// A stop was requested and is taking effect.
    Stopping,
    /// Ended normally (length reached or stopped by the user).
    Finished,
    /// Ended by a failure.
    Failed,
}

impl AcquisitionState {
    fn to_u8(self) -> u8 {
        match self {
            Self::Starting => 0,
            Self::Running => 1,
            Self::Stopping => 2,
            Self::Finished => 3,
            Self::Failed => 4,
        }
    }

    fn from_u8(value: u8) -> Self {
        match value {
            0 => Self::Starting,
            1 => Self::Running,
            2 => Self::Stopping,
            3 => Self::Finished,
            _ => Self::Failed,
        }
    }
}

/// A snapshot of how far an acquisition has come; cheap to read at any time.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Progress {
    /// The current state.
    pub state: AcquisitionState,
    /// Samples delivered by the device and stored so far.
    pub samples_captured: u64,
    /// Positions in the capture so far (samples plus lost positions).
    pub positions: u64,
}

/// Something that happened during an acquisition. Delivered best effort: a consumer that is
/// slow or absent never slows the acquisition and may miss events; the
/// [`AcquisitionOutcome`] is authoritative.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AcquisitionEvent {
    /// The device started; this is the configuration in effect.
    Started {
        /// What the device applied.
        applied: AppliedConfig,
    },
    /// Positions were lost and recorded as a gap.
    GapRecorded {
        /// First lost position.
        position: u64,
        /// Number of lost positions.
        length: u64,
    },
    /// The acquisition ended normally.
    Finished,
    /// The acquisition ended with a failure.
    Failed {
        /// The stable error code.
        code: &'static str,
    },
}

/// How an acquisition ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EndReason {
    /// A finite capture length was reached.
    Completed,
    /// The user stopped it.
    Stopped,
    /// A failure ended it.
    Failed,
}

/// The authoritative result of an acquisition; set once and never changed.
#[derive(Debug, Clone)]
pub struct AcquisitionOutcome {
    /// How it ended.
    pub end: EndReason,
    /// The failure, for [`EndReason::Failed`]: a stable code and an actionable `detail`.
    pub error: Option<Arc<Error>>,
    /// The final status of the capture.
    pub capture_status: CaptureStatus,
    /// Positions in the capture (samples stored plus lost positions).
    pub positions: u64,
    /// Samples delivered by the device and stored.
    pub delivered_samples: u64,
    /// Positions recorded as lost.
    pub lost_positions: u64,
}

/// State shared by the handle and the two supervised threads.
pub(crate) struct Control {
    state: AtomicU8,
    pub(crate) samples: AtomicU64,
    pub(crate) positions: AtomicU64,
    pub(crate) stop: AtomicBool,
    outcome: Mutex<Option<AcquisitionOutcome>>,
    done: Condvar,
    description: Mutex<CaptureDescription>,
    epoch: Instant,
    heartbeat_ms: AtomicU64,
    events: SyncSender<AcquisitionEvent>,
}

impl Control {
    pub(crate) fn new(
        description: CaptureDescription,
        events: SyncSender<AcquisitionEvent>,
    ) -> Self {
        Self {
            state: AtomicU8::new(AcquisitionState::Starting.to_u8()),
            samples: AtomicU64::new(0),
            positions: AtomicU64::new(0),
            stop: AtomicBool::new(false),
            outcome: Mutex::new(None),
            done: Condvar::new(),
            description: Mutex::new(description),
            epoch: Instant::now(),
            heartbeat_ms: AtomicU64::new(0),
            events,
        }
    }

    pub(crate) fn state(&self) -> AcquisitionState {
        AcquisitionState::from_u8(self.state.load(Ordering::Acquire))
    }

    pub(crate) fn set_state(&self, state: AcquisitionState) {
        self.state.store(state.to_u8(), Ordering::Release);
    }

    /// Marks the driver as alive now.
    pub(crate) fn beat(&self) {
        let ms = u64::try_from(self.epoch.elapsed().as_millis()).unwrap_or(u64::MAX);
        self.heartbeat_ms.store(ms, Ordering::Release);
    }

    /// Time since the driver last gave a sign of life.
    pub(crate) fn silence(&self) -> Duration {
        let now = u64::try_from(self.epoch.elapsed().as_millis()).unwrap_or(u64::MAX);
        Duration::from_millis(now.saturating_sub(self.heartbeat_ms.load(Ordering::Acquire)))
    }

    /// Best-effort event; never blocks.
    pub(crate) fn emit(&self, event: AcquisitionEvent) {
        let _ = self.events.try_send(event);
    }

    pub(crate) fn apply_to_description(&self, applied: &AppliedConfig) {
        lock_recover(&self.description)
            .apply(applied.sample_rate_hz, applied.samples_before_trigger);
    }

    /// Publishes the outcome (once) and wakes every waiter.
    pub(crate) fn finish(&self, outcome: AcquisitionOutcome) {
        let state = if outcome.end == EndReason::Failed {
            AcquisitionState::Failed
        } else {
            AcquisitionState::Finished
        };
        {
            let mut slot = lock_recover(&self.outcome);
            if slot.is_some() {
                return;
            }
            self.positions.store(outcome.positions, Ordering::Release);
            *slot = Some(outcome);
        }
        self.set_state(state);
        self.done.notify_all();
    }
}

/// A running or finished acquisition. Cheap to share between threads.
pub struct Acquisition {
    pub(crate) control: Arc<Control>,
    pub(crate) events: Mutex<Receiver<AcquisitionEvent>>,
    pub(crate) reader: CaptureReader,
}

impl std::fmt::Debug for Acquisition {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Acquisition")
            .field("progress", &self.progress())
            .finish_non_exhaustive()
    }
}

impl Acquisition {
    /// The current state and counters; reads atomics only.
    #[must_use]
    pub fn progress(&self) -> Progress {
        Progress {
            state: self.control.state(),
            samples_captured: self.control.samples.load(Ordering::Acquire),
            positions: self.control.positions.load(Ordering::Acquire),
        }
    }

    /// Where the data comes from. The sample rate in effect and the trigger delay are filled
    /// in once the device has started.
    #[must_use]
    pub fn description(&self) -> CaptureDescription {
        lock_recover(&self.control.description).clone()
    }

    /// The capture's reader: take snapshots from it while the acquisition runs.
    #[must_use]
    pub fn capture(&self) -> CaptureReader {
        self.reader.clone()
    }

    /// Takes every event that is waiting (never blocks).
    #[must_use]
    pub fn drain_events(&self) -> Vec<AcquisitionEvent> {
        lock_recover(&self.events).try_iter().collect()
    }

    /// Requests a stop and returns at once. Harmless if the acquisition already ended or a
    /// stop was already requested.
    pub fn stop(&self) {
        self.control.stop.store(true, Ordering::Release);
        let _ = self.control.state.compare_exchange(
            AcquisitionState::Running.to_u8(),
            AcquisitionState::Stopping.to_u8(),
            Ordering::AcqRel,
            Ordering::Acquire,
        );
    }

    /// The result, once the acquisition has ended.
    #[must_use]
    pub fn outcome(&self) -> Option<AcquisitionOutcome> {
        lock_recover(&self.control.outcome).clone()
    }

    /// Waits at most `timeout` for the result (never forever).
    #[must_use]
    pub fn wait_timeout(&self, timeout: Duration) -> Option<AcquisitionOutcome> {
        let guard = lock_recover(&self.control.outcome);
        let (guard, _) = self
            .control
            .done
            .wait_timeout_while(guard, timeout, |slot| slot.is_none())
            .unwrap_or_else(PoisonError::into_inner);
        guard.clone()
    }
}
