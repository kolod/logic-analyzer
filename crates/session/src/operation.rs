//! Background saving and loading: cancellable operations with progress and exactly one outcome.
//!
//! The work runs on a supervised worker (a panic is contained and reported as
//! `session.worker_failed`); the caller gets a handle at once and never waits for the disk.

use std::path::PathBuf;
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

use la_core::Error;
use la_diagnostics::{lock_recover, spawn_worker};

use crate::control::{Control, Progress};
use crate::error;
use crate::fs::{FileSystem, RealFileSystem};
use crate::limits::LoadLimits;
use crate::read::load_with;
use crate::session::{Session, SessionData};
use crate::write::{SaveReport, save_with};

/// The result of an operation as every reader sees it.
type Outcome<T> = Result<T, Arc<Error>>;

/// The slot an outcome is stored in, once.
struct Slot<T> {
    value: Mutex<Option<Outcome<T>>>,
    ready: Condvar,
}

impl<T: Clone> Slot<T> {
    fn new() -> Self {
        Self {
            value: Mutex::new(None),
            ready: Condvar::new(),
        }
    }

    /// Stores the outcome unless one was stored already.
    fn set(&self, outcome: Outcome<T>) {
        let mut guard = lock_recover(&self.value);
        if guard.is_none() {
            *guard = Some(outcome);
        }
        drop(guard);
        self.ready.notify_all();
    }

    fn get(&self) -> Option<Outcome<T>> {
        lock_recover(&self.value).clone()
    }

    fn wait(&self, timeout: Duration) -> Option<Outcome<T>> {
        let deadline = Instant::now().checked_add(timeout);
        let mut guard = lock_recover(&self.value);
        loop {
            if let Some(outcome) = guard.as_ref() {
                return Some(outcome.clone());
            }
            let left = match deadline {
                Some(deadline) => deadline.saturating_duration_since(Instant::now()),
                None => timeout,
            };
            if left.is_zero() {
                return None;
            }
            guard = match self.ready.wait_timeout(guard, left) {
                Ok((guard, _)) => guard,
                Err(poisoned) => poisoned.into_inner().0,
            };
        }
    }
}

/// Runs `work` on a supervised worker and stores its outcome in `slot`; a panic of the worker
/// becomes `session.worker_failed`.
fn run<T, F>(name: &str, control: &Control, slot: &Arc<Slot<T>>, work: F)
where
    T: Clone + Send + Sync + 'static,
    F: FnOnce() -> la_core::Result<T> + Send + 'static,
{
    let handle = spawn_worker(name, work);
    let (control, slot) = (control.clone(), Arc::clone(slot));
    let joiner = std::thread::Builder::new()
        .name(format!("{name}-join"))
        .spawn({
            let (control, slot) = (control.clone(), Arc::clone(&slot));
            move || {
                let result = match handle.join() {
                    Ok(result) => result,
                    Err(failure) => Err(error::worker_failed(failure.to_string())),
                };
                control.finish(&result);
                slot.set(result.map_err(Arc::new));
            }
        });
    if joiner.is_err() {
        let failure: la_core::Result<T> = Err(error::worker_failed(
            "the supervising thread could not be started",
        ));
        control.finish(&failure);
        slot.set(failure.map_err(Arc::new));
    }
}

/// A save running in the background.
#[derive(Debug)]
pub struct SaveOperation {
    control: Control,
    slot: Arc<Slot<SaveReport>>,
}

/// A load running in the background.
#[derive(Debug)]
pub struct LoadOperation {
    control: Control,
    slot: Arc<Slot<Arc<Session>>>,
}

impl<T> std::fmt::Debug for Slot<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Slot").finish_non_exhaustive()
    }
}

/// Starts saving `data` to `path` in the background; returns at once.
#[must_use]
pub fn start_save(path: PathBuf, data: SessionData) -> SaveOperation {
    start_save_with(Arc::new(RealFileSystem), path, data)
}

/// Starts loading `path` in the background; returns at once.
#[must_use]
pub fn start_load(path: PathBuf, limits: LoadLimits) -> LoadOperation {
    start_load_with(Arc::new(RealFileSystem), path, limits)
}

/// Like [`start_save`] on an arbitrary file system (used by tests).
#[must_use]
pub fn start_save_with(fs: Arc<dyn FileSystem>, path: PathBuf, data: SessionData) -> SaveOperation {
    let control = Control::new();
    let slot = Arc::new(Slot::new());
    let worker_control = control.clone();
    run("session-save", &control, &slot, move || {
        save_with(&*fs, &path, &data, &worker_control)
    });
    SaveOperation { control, slot }
}

/// Like [`start_load`] on an arbitrary file system (used by tests).
#[must_use]
pub fn start_load_with(
    fs: Arc<dyn FileSystem>,
    path: PathBuf,
    limits: LoadLimits,
) -> LoadOperation {
    let control = Control::new();
    let slot = Arc::new(Slot::new());
    let worker_control = control.clone();
    run("session-load", &control, &slot, move || {
        load_with(&*fs, &path, &limits, &worker_control).map(Arc::new)
    });
    LoadOperation { control, slot }
}

impl SaveOperation {
    /// How far the save has come. Lock-free; never blocks.
    #[must_use]
    pub fn progress(&self) -> Progress {
        self.control.progress()
    }

    /// Asks the save to stop; returns at once. The operation ends with `session.cancelled`
    /// (or with its result if it was already finished).
    pub fn cancel(&self) {
        self.control.cancel();
    }

    /// The result, once available.
    #[must_use]
    pub fn outcome(&self) -> Option<Result<SaveReport, Arc<Error>>> {
        self.slot.get()
    }

    /// Waits at most `timeout` for the result (never forever).
    #[must_use]
    pub fn wait_timeout(&self, timeout: Duration) -> Option<Result<SaveReport, Arc<Error>>> {
        self.slot.wait(timeout)
    }
}

impl LoadOperation {
    /// How far the load has come. Lock-free; never blocks.
    #[must_use]
    pub fn progress(&self) -> Progress {
        self.control.progress()
    }

    /// Asks the load to stop; returns at once. The operation ends with `session.cancelled`
    /// (or with its result if it was already finished) and no partial session.
    pub fn cancel(&self) {
        self.control.cancel();
    }

    /// The result, once available.
    #[must_use]
    pub fn outcome(&self) -> Option<Result<Arc<Session>, Arc<Error>>> {
        self.slot.get()
    }

    /// Waits at most `timeout` for the result (never forever).
    #[must_use]
    pub fn wait_timeout(&self, timeout: Duration) -> Option<Result<Arc<Session>, Arc<Error>>> {
        self.slot.wait(timeout)
    }
}
