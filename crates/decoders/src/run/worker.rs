//! A pass on a supervised worker thread (research R6).
//!
//! The interface never waits for a decoder: the pass runs on a worker made with
//! [`la_diagnostics::spawn_worker`] (a panic is caught at the thread boundary), publishes the
//! results it has so far through a mutex that is only held to swap a handle, and reports its
//! progress through atomics. The host polls the [`Job`]: it asks for progress and results, cancels
//! it, and collects what it ended with.

use std::sync::atomic::{AtomicBool, AtomicU8, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use la_core::Error;
use la_diagnostics::{WorkerFailure, WorkerHandle, lock_recover, spawn_worker};

use super::{ChunkLimits, Outcome, Pass, Results, Run, Sink, run_pass};
use crate::descriptor::Descriptor;
use crate::input::SnapshotSource;
use crate::setup::Setup;
use crate::store::{Budget, Limits};

/// How often the worker publishes results while it runs.
const PUBLISH_EVERY: Duration = Duration::from_millis(25);
/// How long the worker sleeps when it has caught up with a capture that is still growing.
const FOLLOW_PAUSE: Duration = Duration::from_millis(2);

/// How long a decoder may take before the supervisor gives up on it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Deadlines {
    /// The longest a chunk (or a wait for data) may go without a sign of life.
    pub chunk: Duration,
    /// The longest a worker may take to stop after a cancel.
    pub cancel: Duration,
}

impl Default for Deadlines {
    fn default() -> Self {
        Self {
            chunk: Duration::from_secs(5),
            cancel: Duration::from_secs(1),
        }
    }
}

/// How many abandoned workers are still running (a decoder that never returns keeps its thread).
static ABANDONED_ALIVE: AtomicUsize = AtomicUsize::new(0);
/// With this many stuck workers new passes are refused.
pub const MAX_STUCK_WORKERS: usize = 4;

/// The number of abandoned workers that are still running.
#[must_use]
pub fn stuck_workers() -> usize {
    ABANDONED_ALIVE.load(Ordering::Acquire)
}

/// Why the supervisor gave up on a job.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Overdue {
    /// A chunk took longer than its deadline.
    Chunk,
    /// The worker did not stop in time after a cancel.
    Cancel,
}

const RUNNING: u8 = 0;
const ABANDONED: u8 = 1;
const ENDED: u8 = 2;

/// Everything a pass needs.
pub struct JobSpec {
    /// The decoder.
    pub descriptor: &'static Descriptor,
    /// What the user chose for the instance.
    pub setup: Setup,
    /// The state of the pass.
    pub run: Box<dyn Run>,
    /// The capture to read.
    pub capture: Arc<dyn SnapshotSource>,
    /// The chunk bounds.
    pub chunk_limits: ChunkLimits,
    /// The limits of the instance.
    pub limits: Limits,
    /// The memory budget shared by all instances.
    pub budget: Arc<Budget>,
    /// The deadlines the supervisor enforces.
    pub deadlines: Deadlines,
}

/// How a pass ended.
#[derive(Debug)]
pub struct JobEnd {
    /// The outcome, or the error the decoder ended with.
    pub outcome: Result<Outcome, Error>,
    /// What was found (kept when the pass failed or was cut).
    pub results: Results,
}

/// What a finished job hands back.
#[derive(Debug)]
pub enum Ended {
    /// The pass ran to an end (complete, cancelled, limited or failed).
    Done(JobEnd),
    /// The worker panicked; what it had published before is kept.
    Panicked {
        /// What the worker reported.
        failure: WorkerFailure,
        /// The last results published.
        results: Option<Results>,
    },
}

/// Progress of a running job.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Progress {
    /// Everything before this position has been processed.
    pub covered: u64,
    /// The length of the capture at the last chunk.
    pub length: u64,
}

struct Shared {
    cancel: AtomicBool,
    covered: AtomicU64,
    length: AtomicU64,
    ended: AtomicBool,
    /// Running, abandoned by the supervisor, or ended (decides who counts a stuck worker).
    life: AtomicU8,
    /// Milliseconds since the start when a cancel was asked for, plus one (0: not asked).
    cancelled_at: AtomicU64,
    deadlines: Deadlines,
    /// Milliseconds since the start at the last sign of life (a chunk done).
    beat: AtomicU64,
    started: Instant,
    latest: Mutex<Option<Results>>,
}

/// Sets the flag when the worker's closure ends, however it ends.
struct EndFlag(Arc<Shared>);

impl Drop for EndFlag {
    fn drop(&mut self) {
        self.0.ended.store(true, Ordering::Release);
        if self.0.life.swap(ENDED, Ordering::AcqRel) == ABANDONED {
            ABANDONED_ALIVE.fetch_sub(1, Ordering::AcqRel);
        }
    }
}

/// A running pass.
pub struct Job {
    handle: Option<WorkerHandle<JobEnd>>,
    shared: Arc<Shared>,
}

impl Job {
    /// Starts the pass on a new worker.
    #[must_use]
    pub fn spawn(spec: JobSpec) -> Self {
        let shared = Arc::new(Shared {
            cancel: AtomicBool::new(false),
            covered: AtomicU64::new(0),
            length: AtomicU64::new(0),
            ended: AtomicBool::new(false),
            life: AtomicU8::new(RUNNING),
            cancelled_at: AtomicU64::new(0),
            deadlines: spec.deadlines,
            beat: AtomicU64::new(0),
            started: Instant::now(),
            latest: Mutex::new(None),
        });
        let worker_shared = Arc::clone(&shared);
        let name = format!("decoder-{}", spec.descriptor.id);
        let handle = spawn_worker(&name, move || work(spec, worker_shared));
        Self {
            handle: Some(handle),
            shared,
        }
    }

    /// Asks the pass to stop at the next chunk boundary.
    pub fn cancel(&self) {
        let now = u64::try_from(self.shared.started.elapsed().as_millis()).unwrap_or(u64::MAX);
        let _ = self.shared.cancelled_at.compare_exchange(
            0,
            now.saturating_add(1),
            Ordering::AcqRel,
            Ordering::Acquire,
        );
        self.shared.cancel.store(true, Ordering::Release);
    }

    /// Whether the supervisor must give up on this job now: a chunk has gone on too long, or the
    /// worker did not stop in time after a cancel.
    #[must_use]
    pub fn overdue(&self) -> Option<Overdue> {
        if self.is_finished() {
            return None;
        }
        let now = u64::try_from(self.shared.started.elapsed().as_millis()).unwrap_or(u64::MAX);
        let asked = self.shared.cancelled_at.load(Ordering::Acquire);
        if asked > 0 {
            let waited = now.saturating_sub(asked.saturating_sub(1));
            if u128::from(waited) > self.shared.deadlines.cancel.as_millis() {
                return Some(Overdue::Cancel);
            }
        }
        if self.silent_for() > self.shared.deadlines.chunk {
            return Some(Overdue::Chunk);
        }
        None
    }

    /// Gives up on the worker: whatever it still does is ignored and its thread is left to end by
    /// itself. It counts as stuck until it does.
    pub fn abandon(mut self) {
        self.shared.cancel.store(true, Ordering::Release);
        if self
            .shared
            .life
            .compare_exchange(RUNNING, ABANDONED, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
        {
            ABANDONED_ALIVE.fetch_add(1, Ordering::AcqRel);
        }
        // Dropping the handle detaches the thread.
        self.handle = None;
    }

    /// How far the pass has got.
    #[must_use]
    pub fn progress(&self) -> Progress {
        Progress {
            covered: self.shared.covered.load(Ordering::Acquire),
            length: self.shared.length.load(Ordering::Acquire),
        }
    }

    /// The results published most recently, if any.
    #[must_use]
    pub fn latest(&self) -> Option<Results> {
        lock_recover(&self.shared.latest).clone()
    }

    /// Milliseconds since the last chunk was finished (or since the start), for the supervisor.
    #[must_use]
    pub fn silent_for(&self) -> Duration {
        let now = u64::try_from(self.shared.started.elapsed().as_millis()).unwrap_or(u64::MAX);
        Duration::from_millis(now.saturating_sub(self.shared.beat.load(Ordering::Acquire)))
    }

    /// Whether the worker has ended.
    #[must_use]
    pub fn is_finished(&self) -> bool {
        self.shared.ended.load(Ordering::Acquire)
    }

    /// If the worker has ended, collects how; `None` while it runs.
    pub fn poll(&mut self) -> Option<Ended> {
        if !self.is_finished() {
            return None;
        }
        let handle = self.handle.take()?;
        Some(match handle.join() {
            Ok(end) => Ended::Done(end),
            Err(failure) => Ended::Panicked {
                failure,
                results: self.latest(),
            },
        })
    }
}

fn work(spec: JobSpec, shared: Arc<Shared>) -> JobEnd {
    let _flag = EndFlag(Arc::clone(&shared));
    let JobSpec {
        descriptor,
        setup,
        run,
        capture,
        chunk_limits,
        limits,
        budget,
        deadlines: _,
    } = spec;
    let mut pass = Pass::new(descriptor, setup, run, Sink::new(limits, budget));
    let mut last_publish = Instant::now();
    let ran = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        run_pass(
            &mut pass,
            capture.as_ref(),
            &chunk_limits,
            &shared.cancel,
            &mut || {
                std::thread::sleep(FOLLOW_PAUSE);
                beat(&shared);
                !shared.cancel.load(Ordering::Acquire)
            },
            &mut |position, sink| {
                shared.covered.store(position, Ordering::Release);
                shared
                    .length
                    .store(capture.snapshot().length(), Ordering::Release);
                beat(&shared);
                if last_publish.elapsed() >= PUBLISH_EVERY {
                    last_publish = Instant::now();
                    publish(&shared, sink.results());
                }
            },
        )
    }));
    let outcome = match ran {
        Ok(outcome) => outcome,
        Err(payload) => {
            // What was found before the panic is kept, marked incomplete, before the panic goes on
            // to the supervision of the worker thread.
            let position = pass.position();
            pass.sink().set_coverage(position, true);
            publish(&shared, pass.sink().results());
            std::panic::resume_unwind(payload);
        }
    };
    pass.sink().compact();
    let results = pass.sink().results();
    publish(&shared, results.clone());
    shared
        .covered
        .store(results.coverage.covered, Ordering::Release);
    JobEnd { outcome, results }
}

fn beat(shared: &Shared) {
    let now = u64::try_from(shared.started.elapsed().as_millis()).unwrap_or(u64::MAX);
    shared.beat.store(now, Ordering::Release);
}

/// Builds the new value first and swaps it in, so that a panic never leaves a half-written one.
fn publish(shared: &Shared, results: Results) {
    *lock_recover(&shared.latest) = Some(results);
}
