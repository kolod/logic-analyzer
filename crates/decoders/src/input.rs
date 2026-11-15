//! What a decoder is given to read: chunks of positions and a read-only reader of the assigned
//! channels over one snapshot of the capture (`contracts/decoder-api.md`).
//!
//! The reader holds a snapshot, so every answer is consistent with the moment the chunk was cut
//! and nothing can be written through it.

use std::iter::Peekable;

use la_storage::{Gap, LevelState, Snapshot, TransitionIter};

use crate::descriptor::Descriptor;
use crate::model::Item;
use crate::setup::Setup;

/// The level of a channel at a position.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Level {
    /// Low.
    Low,
    /// High.
    High,
    /// No data: inside a gap, at or beyond the snapshot length, or the role has no channel.
    Missing,
}

/// One edge of a signal: the change between `position - 1` and `position`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Edge {
    /// The first position with the new level.
    pub position: u64,
    /// The signal role whose channel changed.
    pub role: &'static str,
    /// `true` for a change from low to high.
    pub rising: bool,
}

/// A range of positions to process.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Chunk {
    /// The first position (equal to the end of the previous chunk).
    pub start: u64,
    /// One past the last position.
    pub end: u64,
    /// The length of the capture at the snapshot the chunk was cut from.
    pub capture_length: u64,
    /// The gaps (lost samples) that intersect the chunk, sorted by position.
    pub gaps: Vec<Gap>,
}

/// What a decoder that reads the output of another instance is given instead of edges.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ItemChunk {
    /// The items of the kind it reads whose end lies in the chunk, in order.
    pub items: Vec<Item>,
    /// The position up to which the source has finished (its watermark).
    pub covered: u64,
}

/// Read-only access to the channels assigned to the roles of one instance.
#[derive(Debug, Clone)]
pub struct SignalReader {
    snapshot: Snapshot,
    roles: Vec<(&'static str, u8)>,
}

impl SignalReader {
    /// A reader of `snapshot` for the given `(role, channel)` pairs; the order of the pairs is
    /// the order in which simultaneous edges are reported.
    #[must_use]
    pub fn new(snapshot: Snapshot, roles: Vec<(&'static str, u8)>) -> Self {
        Self { snapshot, roles }
    }

    /// A reader for the roles of `descriptor` that `setup` assigns to channels.
    #[must_use]
    pub fn from_setup(snapshot: Snapshot, descriptor: &Descriptor, setup: &Setup) -> Self {
        let roles = descriptor
            .signals
            .iter()
            .filter_map(|s| setup.channels.get(s.role).map(|c| (s.role, *c)))
            .collect();
        Self::new(snapshot, roles)
    }

    /// The number of positions of the snapshot.
    #[must_use]
    pub fn length(&self) -> u64 {
        self.snapshot.length()
    }

    /// The snapshot this reader reads.
    #[must_use]
    pub fn snapshot(&self) -> &Snapshot {
        &self.snapshot
    }

    /// Whether the role has a channel.
    #[must_use]
    pub fn role_assigned(&self, role: &str) -> bool {
        self.roles.iter().any(|(r, _)| *r == role)
    }

    /// The level of the channel of `role` at `position`. Cost O(log N).
    #[must_use]
    pub fn level(&self, role: &str, position: u64) -> Level {
        let Some((_, channel)) = self.roles.iter().find(|(r, _)| *r == role) else {
            return Level::Missing;
        };
        match self.snapshot.level_at(*channel, position) {
            Ok(LevelState::High) => Level::High,
            Ok(LevelState::Low) => Level::Low,
            Ok(LevelState::Missing) | Err(_) => Level::Missing,
        }
    }

    /// The edges of the assigned channels at positions in `[start, end)`, merged in position
    /// order (simultaneous edges in the order of the roles). An edge belongs to exactly one range
    /// of any partition of the positions.
    #[must_use]
    pub fn edges(&self, start: u64, end: u64) -> Edges<'_> {
        // The storage reports transitions with `from < position <= to`; position 0 has none.
        let from = start.saturating_sub(1);
        let to = end.saturating_sub(1);
        let mut sources = Vec::new();
        if from < to {
            for (role, channel) in &self.roles {
                if let Ok(iter) = self.snapshot.transitions(*channel, from, to) {
                    sources.push((*role, iter.peekable()));
                }
            }
        }
        Edges { sources, start }
    }

    /// The gaps that intersect `[start, end)`, sorted.
    #[must_use]
    pub fn gaps_in(&self, start: u64, end: u64) -> Vec<Gap> {
        self.snapshot
            .gaps()
            .iter()
            .filter(|g| g.start < end && g.end().is_none_or(|e| e > start))
            .copied()
            .collect()
    }
}

/// The merged edges of a [`SignalReader`] over a range.
pub struct Edges<'a> {
    sources: Vec<(&'static str, Peekable<TransitionIter<'a>>)>,
    start: u64,
}

impl Iterator for Edges<'_> {
    type Item = Edge;

    fn next(&mut self) -> Option<Edge> {
        loop {
            let mut best: Option<(usize, u64)> = None;
            for (index, (_, source)) in self.sources.iter_mut().enumerate() {
                if let Some(edge) = source.peek() {
                    if best.is_none_or(|(_, position)| edge.position < position) {
                        best = Some((index, edge.position));
                    }
                }
            }
            let (index, _) = best?;
            let (role, source) = self.sources.get_mut(index)?;
            let edge = source.next()?;
            if edge.position < self.start {
                continue;
            }
            return Some(Edge {
                position: edge.position,
                role,
                rising: edge.rising,
            });
        }
    }
}
