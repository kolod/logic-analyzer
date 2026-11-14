//! Supervised worker threads: a panic is contained, logged and reported.
//!
//! # Updating shared state safely
//!
//! A panic can interrupt a thread while it holds a lock. To keep shared state
//! consistent, workers MUST update it transactionally: build the complete new value
//! first, then swap it in under the lock. A panic then leaves either the old or the
//! new value, never a half-updated one. After a panic, other threads take the lock
//! with [`lock_recover`], which recovers a poisoned `Mutex` instead of propagating
//! the poison.

use std::fmt;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::{Mutex, MutexGuard};
use std::thread::JoinHandle;

use crate::panic_hook::{mark_worker_thread, payload_message, take_last_note};

/// Why a worker did not produce a result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkerFailure {
    /// Name given to [`spawn_worker`].
    pub worker: String,
    /// Panic message (`<non-string panic payload>` for non-text payloads).
    pub message: String,
    /// `file:line:column` of the panic, if the panic hook was installed.
    pub location: Option<String>,
}

impl fmt::Display for WorkerFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "worker `{}` failed: {}", self.worker, self.message)?;
        if let Some(location) = &self.location {
            write!(f, " ({location})")?;
        }
        Ok(())
    }
}

impl std::error::Error for WorkerFailure {}

enum Inner<T> {
    Running {
        name: String,
        handle: JoinHandle<Result<T, WorkerFailure>>,
    },
    SpawnFailed(WorkerFailure),
}

/// Handle to a supervised worker. Dropping it detaches the worker.
#[must_use = "join the handle to learn whether the worker failed"]
pub struct WorkerHandle<T> {
    inner: Inner<T>,
}

impl<T> WorkerHandle<T> {
    /// Waits for the worker and returns its value, or why it failed.
    ///
    /// # Errors
    ///
    /// Returns a [`WorkerFailure`] if the worker panicked or could not be started.
    pub fn join(self) -> Result<T, WorkerFailure> {
        match self.inner {
            Inner::SpawnFailed(failure) => Err(failure),
            Inner::Running { name, handle } => match handle.join() {
                Ok(result) => result,
                // The closure catches panics itself, so this only happens if the
                // thread died outside it; report it like any other failure.
                Err(payload) => Err(WorkerFailure {
                    worker: name,
                    message: payload_message(&*payload),
                    location: None,
                }),
            },
        }
    }
}

/// Runs `work` on a new thread named `name`.
///
/// A panic in `work` is caught at the thread boundary, logged once at `ERROR` level
/// (worker name, message, location) and returned by [`WorkerHandle::join`] as a
/// [`WorkerFailure`]. It does not terminate the application and does not affect other
/// workers. Panic containment requires `panic = "unwind"` (the workspace default).
pub fn spawn_worker<T, F>(name: &str, work: F) -> WorkerHandle<T>
where
    F: FnOnce() -> T + Send + 'static,
    T: Send + 'static,
{
    let worker = name.to_owned();
    let thread_worker = worker.clone();
    let spawned = std::thread::Builder::new()
        .name(worker.clone())
        .spawn(move || {
            mark_worker_thread();
            match catch_unwind(AssertUnwindSafe(work)) {
                Ok(value) => Ok(value),
                Err(payload) => {
                    let note = take_last_note();
                    let failure = WorkerFailure {
                        worker: thread_worker,
                        message: payload_message(&*payload),
                        location: note.location,
                    };
                    tracing::error!(
                        target: "la_diagnostics::worker",
                        worker = %failure.worker,
                        panic_message = %failure.message,
                        location = failure.location.as_deref().unwrap_or("<unknown>"),
                        backtrace = note.backtrace.as_deref(),
                        "worker panicked"
                    );
                    Err(failure)
                }
            }
        });
    let inner = match spawned {
        Ok(handle) => Inner::Running {
            name: worker,
            handle,
        },
        Err(error) => {
            let failure = WorkerFailure {
                worker,
                message: format!("the operating system refused to start the thread: {error}"),
                location: None,
            };
            tracing::error!(
                target: "la_diagnostics::worker",
                worker = %failure.worker,
                reason = %failure.message,
                "worker could not be started"
            );
            Inner::SpawnFailed(failure)
        }
    };
    WorkerHandle { inner }
}

/// Locks `mutex`, recovering (and logging a warning) if a panic poisoned it.
///
/// The poison flag is cleared so later locks are not affected. See the module
/// documentation for the transactional-update rule that keeps the data consistent.
pub fn lock_recover<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    match mutex.lock() {
        Ok(guard) => guard,
        Err(poisoned) => {
            tracing::warn!(
                target: "la_diagnostics::worker",
                "recovered a mutex poisoned by a panicking thread"
            );
            mutex.clear_poison();
            poisoned.into_inner()
        }
    }
}
