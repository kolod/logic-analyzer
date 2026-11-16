//! Running a decoder: the contract a decoder implements, the sink it writes to and the chunk loop
//! that drives it (`contracts/decoder-api.md`).

mod job;
mod sink;
mod worker;

pub use job::{
    Advance, ChunkLimits, DEFAULT_CHUNK_EDGES, DEFAULT_CHUNK_SPAN, Outcome, Pass, run_item_pass,
    run_pass,
};
pub use sink::{Limit, Refused, Results, Sink};
pub use worker::{
    Deadlines, Ended, Job, JobEnd, JobSpec, MAX_STUCK_WORKERS, Overdue, Progress, stuck_workers,
};

use crate::input::{Chunk, ItemChunk, SignalReader};

/// Why a pass is being ended, as `finish` is told.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum End {
    /// The capture is closed and all of it was processed.
    Complete,
    /// The pass was cancelled or stopped by a limit; flush what is pending and stop.
    Cut,
}

/// A decoder that cannot continue (a problem in the signal is an error annotation instead).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecodeError {
    /// A short technical detail for the log; never shown to the user, never signal data.
    pub detail: String,
}

impl DecodeError {
    /// An error with a log detail.
    #[must_use]
    pub fn new(detail: impl Into<String>) -> Self {
        Self {
            detail: detail.into(),
        }
    }
}

impl From<Limit> for DecodeError {
    fn from(_: Limit) -> Self {
        Self::new("the results limit was reached")
    }
}

/// The state of one pass of a decoder over a capture: a resumable state machine called with one
/// chunk after another.
pub trait Run: Send {
    /// Processes the chunk `[chunk.start, chunk.end)`; `chunk.start` is the previous `end`.
    ///
    /// # Errors
    ///
    /// Only a decoder that cannot continue returns an error; a [`Limit`] from the sink must be
    /// returned at once (it converts into a `DecodeError`).
    fn process(
        &mut self,
        chunk: &Chunk,
        reader: &SignalReader,
        sink: &mut Sink,
    ) -> Result<(), DecodeError>;

    /// Processes items of the decoder below (for a decoder with `Input::Items`): the items whose
    /// end is at or below the watermark `chunk.covered`, in order.
    ///
    /// # Errors
    ///
    /// As for `process`.
    fn process_items(&mut self, chunk: &ItemChunk, sink: &mut Sink) -> Result<(), DecodeError> {
        let _ = (chunk, sink);
        Ok(())
    }

    /// The position up to which this decoder's output is final: nothing it will still produce
    /// begins or ends before it. `None` (the default) means the end of the last chunk, which is what
    /// every decoder of version 1.0 means. A decoder that holds work back (it is waiting for
    /// information that arrives later in the capture, or is catching up on a part of the capture it
    /// has already been given) returns the position it has actually processed.
    ///
    /// The framework reports `min(end of the chunk, this)` as the coverage of the results and never
    /// lets the coverage go back. (Decoder API 1.1.)
    fn final_through(&self) -> Option<u64> {
        None
    }

    /// Called once when the pass ends; flushes a pending construct.
    ///
    /// # Errors
    ///
    /// As for `process`.
    fn finish(
        &mut self,
        end: End,
        reader: &SignalReader,
        sink: &mut Sink,
    ) -> Result<(), DecodeError>;
}
