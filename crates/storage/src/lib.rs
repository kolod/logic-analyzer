//! Storage of captured digital signals with fast viewport queries.
//!
//! The store keeps every channel as its transitions in immutable blocks, so memory
//! depends on the number of level changes, not on the duration of the capture, and a
//! viewport query costs a number of probes that is polylogarithmic in the capture size
//! (see `specs/002-signal-storage-core`).
//!
//! **Never log signal data.** Log only metadata such as sizes, counts and positions.

#![deny(missing_docs)]

mod block;
mod capture;
mod channel;
mod config;
pub mod error;
mod ingest;
mod math;
mod published;
mod query;
mod restore;
mod segment;
#[cfg(feature = "test-support")]
pub mod testing;
mod types;

pub use capture::Capture;
pub use channel::TransitionIter;
pub use config::{CaptureConfig, Limits, MAX_CHANNELS};
pub use published::{CaptureReader, Snapshot};
pub use restore::{CaptureRestorer, RestoredStatus};
pub use types::{
    Activity, CaptureStatus, Edge, Gap, LevelState, MAX_REPORTED_TRANSITIONS, PixelSummary,
    QueryStats, ViewportResult,
};
