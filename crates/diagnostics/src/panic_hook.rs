//! The process-wide panic hook.
//!
//! The hook runs on the panicking thread before unwinding starts. It captures the
//! panic location (and a backtrace when `RUST_BACKTRACE` asks for one) so the worker
//! boundary can report them after the unwind, and it logs panics on threads that are
//! not supervised workers (for example the main thread). Panics inside workers are
//! logged exactly once, by the worker boundary, so there are no duplicate entries.

use std::any::Any;
use std::backtrace::{Backtrace, BacktraceStatus};
use std::cell::{Cell, RefCell};
use std::panic::PanicHookInfo;
use std::sync::Once;

/// What the hook learned about the most recent panic on this thread.
#[derive(Debug, Default, Clone)]
pub(crate) struct PanicNote {
    pub(crate) location: Option<String>,
    pub(crate) backtrace: Option<String>,
}

thread_local! {
    static IN_WORKER: Cell<bool> = const { Cell::new(false) };
    static IN_HOOK: Cell<bool> = const { Cell::new(false) };
    static LAST_NOTE: RefCell<Option<PanicNote>> = const { RefCell::new(None) };
}

/// Marks the current thread as a supervised worker (its panics are logged by the
/// worker boundary, not by the hook).
pub(crate) fn mark_worker_thread() {
    let _ = IN_WORKER.try_with(|w| w.set(true));
}

/// Takes the note the hook stored for the most recent panic on this thread.
pub(crate) fn take_last_note() -> PanicNote {
    LAST_NOTE
        .try_with(|n| n.try_borrow_mut().ok().and_then(|mut n| n.take()))
        .ok()
        .flatten()
        .unwrap_or_default()
}

/// Renders a panic payload as text. Non-string payloads get a fixed placeholder.
pub(crate) fn payload_message(payload: &(dyn Any + Send)) -> String {
    if let Some(text) = payload.downcast_ref::<&str>() {
        (*text).to_owned()
    } else if let Some(text) = payload.downcast_ref::<String>() {
        text.clone()
    } else {
        "<non-string panic payload>".to_owned()
    }
}

fn capture_backtrace() -> Option<String> {
    let backtrace = Backtrace::capture();
    (backtrace.status() == BacktraceStatus::Captured).then(|| backtrace.to_string())
}

fn describe_location(info: &PanicHookInfo<'_>) -> Option<String> {
    info.location()
        .map(|l| format!("{}:{}:{}", l.file(), l.line(), l.column()))
}

fn handle_panic(info: &PanicHookInfo<'_>) {
    let note = PanicNote {
        location: describe_location(info),
        backtrace: capture_backtrace(),
    };
    let in_worker = IN_WORKER.try_with(Cell::get).unwrap_or(false);
    if !in_worker {
        let thread = std::thread::current();
        let message = payload_message(info.payload());
        tracing::error!(
            target: "la_diagnostics::panic",
            thread = thread.name().unwrap_or("<unnamed>"),
            panic_message = %message,
            location = note.location.as_deref().unwrap_or("<unknown>"),
            backtrace = note.backtrace.as_deref(),
            "panic on a thread that is not a supervised worker"
        );
    }
    let _ = LAST_NOTE.try_with(|n| {
        if let Ok(mut n) = n.try_borrow_mut() {
            *n = Some(note);
        }
    });
}

/// Installs the process-wide panic hook. Safe to call more than once: only the first
/// call installs it.
///
/// The hook never panics itself and guards against re-entrancy. If no `tracing`
/// subscriber is installed, the previous hook (the standard stderr message) still
/// runs, so a panic is never silent.
pub fn install_panic_hook() {
    static INSTALLED: Once = Once::new();
    INSTALLED.call_once(|| {
        let previous = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            let already_in_hook = IN_HOOK.try_with(Cell::get).unwrap_or(true);
            if already_in_hook {
                return;
            }
            let _ = IN_HOOK.try_with(|h| h.set(true));
            handle_panic(info);
            if !tracing::dispatcher::has_been_set() {
                previous(info);
            }
            let _ = IN_HOOK.try_with(|h| h.set(false));
        }));
    });
}
