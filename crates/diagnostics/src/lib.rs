//! Logging, panic containment and worker supervision.
//!
//! # Logging
//!
//! [`init_logging`] writes structured JSON-lines diagnostics to a size-bounded local
//! file (see `specs/001-workspace-ci-foundation/contracts/log-entry-format.md`).
//!
//! **Never log captured signal data.** Log only metadata such as sizes, counts and
//! identifiers: payloads would bloat the log and leak measurement content (spec
//! FR-019). Nothing in this crate sends log data over the network.

pub mod bounded_writer;
mod config;
mod logging;
mod panic_hook;
mod paths;
mod worker;

pub use config::{LogConfig, parse_level};
pub use logging::{LogGuard, LogStatus, build_logging, init_logging};
pub use panic_hook::install_panic_hook;
pub use worker::{WorkerFailure, WorkerHandle, lock_recover, spawn_worker};
