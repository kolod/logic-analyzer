//! The chunk loop: cutting the capture into bounded chunks and driving a decoder through them
//! (research R6). This is the synchronous core; the supervised worker around it is built on top.

use std::sync::atomic::{AtomicBool, Ordering};

use la_core::Error;
use la_storage::{CaptureReader, Snapshot};

use super::{End, Run, Sink};
use crate::descriptor::Descriptor;
use crate::error;
use crate::input::{Chunk, SignalReader};
use crate::model::Bounds;
use crate::setup::Setup;
use crate::store::Full;

/// The default most edges in one chunk.
pub const DEFAULT_CHUNK_EDGES: usize = 65_536;
/// The default longest span of positions in one chunk.
pub const DEFAULT_CHUNK_SPAN: u64 = 1 << 22;

/// How big a chunk may be, so that one call of a decoder takes milliseconds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChunkLimits {
    /// The most edges (of all assigned channels together) in a chunk, at least 1.
    pub max_edges: usize,
    /// The longest span of positions in a chunk, at least 1.
    pub max_span: u64,
}

impl Default for ChunkLimits {
    fn default() -> Self {
        Self {
            max_edges: DEFAULT_CHUNK_EDGES,
            max_span: DEFAULT_CHUNK_SPAN,
        }
    }
}

/// How an [`Pass::advance`] ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Advance {
    /// Everything of the snapshot has been processed.
    CaughtUp,
    /// The cancel flag was seen at a chunk boundary.
    Cancelled,
    /// A limit was reached; the decoder was told to return.
    Limit(Full),
}

/// How a whole pass ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    /// The capture is closed and all of it was processed.
    Complete,
    /// Cancelled (or the capture stopped growing and the caller gave up waiting).
    Cancelled,
    /// A limit was reached; what was found is kept, incomplete.
    Limit(Full),
}

/// One pass of a decoder over a capture.
pub struct Pass {
    run: Box<dyn Run>,
    descriptor: &'static Descriptor,
    setup: Setup,
    sink: Sink,
    position: u64,
    chunks: u64,
}

impl Pass {
    /// A pass at position 0.
    #[must_use]
    pub fn new(
        descriptor: &'static Descriptor,
        setup: Setup,
        run: Box<dyn Run>,
        sink: Sink,
    ) -> Self {
        Self {
            run,
            descriptor,
            setup,
            sink,
            position: 0,
            chunks: 0,
        }
    }

    /// Everything before this position has been processed.
    #[must_use]
    pub fn position(&self) -> u64 {
        self.position
    }

    /// How many chunks have been processed.
    #[must_use]
    pub fn chunks(&self) -> u64 {
        self.chunks
    }

    /// The output.
    #[must_use]
    pub fn sink(&mut self) -> &mut Sink {
        &mut self.sink
    }

    /// Consumes the pass into its output.
    #[must_use]
    pub fn into_sink(self) -> Sink {
        self.sink
    }

    /// The end of the chunk that starts at the current position: at most `max_span` positions and
    /// at most `max_edges` edges, and not beyond the snapshot.
    fn chunk_end(&self, reader: &SignalReader, limits: &ChunkLimits) -> u64 {
        let max_edges = limits.max_edges.max(1);
        let ceiling = self
            .position
            .saturating_add(limits.max_span.max(1))
            .min(reader.length());
        // The edge after the allowed number starts the next chunk.
        match reader.edges(self.position, ceiling).nth(max_edges) {
            Some(edge) if edge.position > self.position => edge.position,
            Some(_) => self.position.saturating_add(1).min(ceiling),
            None => ceiling,
        }
    }

    /// Processes the snapshot chunk by chunk until it is exhausted, a cancel is seen or a limit is
    /// reached. `on_chunk` is told the position reached after every chunk.
    ///
    /// # Errors
    ///
    /// `decoder.failed` if the decoder returned an error (what it stored before is kept).
    pub fn advance(
        &mut self,
        snapshot: &Snapshot,
        limits: &ChunkLimits,
        cancel: &AtomicBool,
        on_chunk: &mut dyn FnMut(u64),
    ) -> Result<Advance, Error> {
        let reader = SignalReader::from_setup(snapshot.clone(), self.descriptor, &self.setup);
        self.sink.set_bounds(Bounds {
            length: snapshot.length(),
            channels: snapshot.channels(),
        });
        loop {
            if cancel.load(Ordering::Acquire) {
                return Ok(Advance::Cancelled);
            }
            if self.position >= snapshot.length() {
                return Ok(Advance::CaughtUp);
            }
            let end = self.chunk_end(&reader, limits);
            let chunk = Chunk {
                start: self.position,
                end,
                capture_length: snapshot.length(),
                gaps: reader.gaps_in(self.position, end),
            };
            let result = self.run.process(&chunk, &reader, &mut self.sink);
            if let Some(full) = self.sink.full() {
                self.sink.set_coverage(self.position, true);
                return Ok(Advance::Limit(full));
            }
            if let Err(e) = result {
                self.sink.set_coverage(self.position, true);
                tracing::warn!(decoder = self.descriptor.id, "a decoder returned an error");
                return Err(error::failed(format!(
                    "decoder `{}` failed at position {}: {}",
                    self.descriptor.id, self.position, e.detail
                )));
            }
            self.position = end;
            self.chunks = self.chunks.saturating_add(1);
            self.sink.set_coverage(self.position, false);
            on_chunk(self.position);
        }
    }

    /// Tells the decoder the pass is over so that it can flush what is pending.
    ///
    /// # Errors
    ///
    /// `decoder.failed` if the decoder returned an error.
    pub fn finish(&mut self, snapshot: &Snapshot, end: End) -> Result<(), Error> {
        let reader = SignalReader::from_setup(snapshot.clone(), self.descriptor, &self.setup);
        self.sink.set_bounds(Bounds {
            length: snapshot.length(),
            channels: snapshot.channels(),
        });
        let result = self.run.finish(end, &reader, &mut self.sink);
        self.sink
            .set_coverage(self.position, end == End::Cut || self.sink.full().is_some());
        if self.sink.full().is_some() {
            // The limit was already reached: a flush that cannot be stored is not a failure.
            return Ok(());
        }
        result.map_err(|e| {
            error::failed(format!(
                "decoder `{}` failed when finishing: {}",
                self.descriptor.id, e.detail
            ))
        })
    }
}

/// Runs a pass to its end over a capture that may still be growing.
///
/// A new snapshot is taken every round, so growth is followed. When the pass has caught up with a
/// capture that is not closed, `wait` is called; it returns `false` to stop waiting (the pass is
/// then cut). `on_chunk` is told the position reached after every chunk.
///
/// # Errors
///
/// `decoder.failed` if the decoder returned an error.
pub fn run_pass(
    pass: &mut Pass,
    capture: &CaptureReader,
    limits: &ChunkLimits,
    cancel: &AtomicBool,
    wait: &mut dyn FnMut() -> bool,
    on_chunk: &mut dyn FnMut(u64),
) -> Result<Outcome, Error> {
    loop {
        let snapshot = capture.snapshot();
        match pass.advance(&snapshot, limits, cancel, on_chunk)? {
            Advance::Cancelled => {
                pass.finish(&snapshot, End::Cut)?;
                return Ok(Outcome::Cancelled);
            }
            Advance::Limit(full) => {
                pass.finish(&snapshot, End::Cut)?;
                return Ok(Outcome::Limit(full));
            }
            Advance::CaughtUp => {
                if snapshot.status().closed {
                    pass.finish(&snapshot, End::Complete)?;
                    return Ok(Outcome::Complete);
                }
                if !wait() {
                    pass.finish(&snapshot, End::Cut)?;
                    return Ok(Outcome::Cancelled);
                }
            }
        }
    }
}
