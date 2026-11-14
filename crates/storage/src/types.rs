//! Value types returned by the store.

/// The level of a channel at a position.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LevelState {
    /// Low level.
    Low,
    /// High level.
    High,
    /// No data: the position lies inside a gap or at or beyond the capture length.
    Missing,
}

/// A transition: the change between sample `position - 1` and sample `position`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Edge {
    /// Position of the first sample with the new level.
    pub position: u64,
    /// `true` for a change from low to high.
    pub rising: bool,
}

/// The largest transition count a pixel summary reports; larger counts saturate.
pub const MAX_REPORTED_TRANSITIONS: u32 = u32::MAX;

/// Activity inside one pixel column.
///
/// A consumer draws `None` as a flat line at the column's level, `One` as a single
/// clean edge at the reported position, and `Many` as a dense-activity mark, so a
/// narrow glitch is never hidden by zooming out.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Activity {
    /// No transition in the column.
    None,
    /// Exactly one transition, with its exact position and direction.
    One(Edge),
    /// Two or more transitions; `count` saturates at [`MAX_REPORTED_TRANSITIONS`].
    Many {
        /// Number of transitions in the column (saturating).
        count: u32,
    },
}

/// Everything needed to draw one pixel column faithfully.
///
/// A consumer draws a column from its `start_level` (the line before any edge) and its
/// [`Activity`]: a flat line for `None`, one clean edge at the exact position for `One`,
/// and a dense-activity mark for `Many`. `end_level` is the level the next column starts
/// with, so adjacent columns always join without a visual jump. Columns with
/// `missing_data` or `beyond_end` are drawn as "no data", never as a level that was
/// not captured. The store only supplies the data; it draws nothing itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PixelSummary {
    /// Level at the first sample of the column.
    pub start_level: LevelState,
    /// Level at the first sample of the next column (equals its `start_level`).
    pub end_level: LevelState,
    /// Transitions inside the column.
    pub activity: Activity,
    /// The column touches lost data (a gap); levels are never invented there.
    pub missing_data: bool,
    /// The whole column lies at or beyond the capture length.
    pub beyond_end: bool,
}

/// Cost counters of one query, used by the complexity tests.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct QueryStats {
    /// Number of block-index and offset comparisons performed.
    pub probes: u64,
}

/// The result of a viewport query.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ViewportResult {
    /// One summary per requested pixel column.
    pub pixels: Vec<PixelSummary>,
    /// Cost counters.
    pub stats: QueryStats,
}

/// A range of positions for which samples were lost.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Gap {
    /// First lost position.
    pub start: u64,
    /// Number of lost positions.
    pub length: u64,
}

impl Gap {
    /// The first position after the gap, or `None` if it would overflow.
    #[must_use]
    pub fn end(&self) -> Option<u64> {
        self.start.checked_add(self.length)
    }
}

/// Completeness of a capture.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CaptureStatus {
    /// The producer closed the capture.
    pub closed: bool,
    /// Closed, with no lost positions, not stopped by the memory limit and not
    /// interrupted by a failure.
    pub complete: bool,
    /// Sum of all gap lengths.
    pub lost_positions: u64,
    /// An append was rejected because of the memory budget.
    pub stopped_by_limit: bool,
    /// The capture was ended by a failure outside it (for example a device was
    /// unplugged); data that should have followed is missing even if no gap was recorded.
    pub interrupted: bool,
}
