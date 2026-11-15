//! Turning the annotations of a window into drawing cells without losing any
//! (research R5, `data-model.md` "Lane plan and overlay plan").
//!
//! The window is divided into the same `W` columns as the waveforms. Everything that intersects the
//! window is represented exactly once, by one of:
//!
//! * a **span** (a range at least one column wide, with its text),
//! * a **mark** (a point, an edge, or a range narrower than a column, with its exact position),
//! * a **dense cell**: the count of the annotations that could not get a cell of their own, with how
//!   many of them are errors (an error is never hidden inside a dense cell without the error count).
//!
//! Spans and marks are packed into at most [`LANE_ROWS`] (lane) or [`OVERLAY_ROWS`] (overlay) rows
//! without overlapping in a row; dense cells have a row of their own. A plan therefore has at most
//! `4 · W` cells for a lane and `2 · W` for the overlays of one channel.
//!
//! The cost does not depend on how many annotations the store holds: when the window has more
//! annotations than cells, the counts come from rank queries on the sorted starts instead of from
//! enumerating them.

use std::collections::BTreeMap;

use crate::model::{Kind, Style, Text};
use crate::store::{Entry, Snapshot};

/// Rows of spans and marks in a lane (and one more row holds the dense cells).
pub const LANE_ROWS: usize = 3;
/// Rows of spans and marks in the overlay of a channel (and one more row holds the dense cells).
pub const OVERLAY_ROWS: usize = 1;
/// How many ranges that start before the window and cross its left edge are looked up when the
/// window holds more annotations than cells.
pub const CROSSING_LIMIT: usize = 4;

/// The part of the capture that is drawn and the number of columns it is drawn in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Window {
    /// The first position (inclusive).
    pub start: u64,
    /// One past the last position.
    pub end: u64,
    /// The number of columns (at least 1).
    pub columns: u32,
}

impl Window {
    fn span(&self) -> u64 {
        self.end.saturating_sub(self.start).max(1)
    }

    /// The horizontal position of `position` in columns, clamped to `0..=columns`.
    #[must_use]
    pub fn x(&self, position: u64) -> f64 {
        let offset = position.saturating_sub(self.start).min(self.span());
        #[allow(clippy::cast_precision_loss)]
        let x = offset as f64 * f64::from(self.columns) / self.span() as f64;
        x
    }

    /// The column that holds `position` (which must lie in the window).
    #[must_use]
    pub fn column(&self, position: u64) -> u32 {
        let offset = u128::from(position.saturating_sub(self.start).min(self.span()));
        let column = offset
            .saturating_mul(u128::from(self.columns))
            .checked_div(u128::from(self.span()))
            .unwrap_or(0);
        u32::try_from(column)
            .unwrap_or(u32::MAX)
            .min(self.columns.saturating_sub(1))
    }

    /// The first position of column `index` (`index == columns` gives the end of the window).
    #[must_use]
    pub fn boundary(&self, index: u32) -> u64 {
        if index >= self.columns {
            return self.end;
        }
        let numerator = u128::from(self.span()).saturating_mul(u128::from(index));
        let columns = u128::from(self.columns.max(1));
        let step = numerator
            .saturating_add(columns.saturating_sub(1))
            .checked_div(columns)
            .unwrap_or(0);
        self.start
            .saturating_add(u64::try_from(step).unwrap_or(u64::MAX))
            .min(self.end)
    }
}

/// A range drawn as a bar with its text.
#[derive(Debug, Clone, PartialEq)]
pub struct SpanCell {
    /// The id of the annotation in its store.
    pub id: u64,
    /// Its first position (with the id it finds the annotation again).
    pub start: u64,
    /// The left edge in columns (clamped to the window).
    pub x0: f64,
    /// The right edge in columns (clamped to the window).
    pub x1: f64,
    /// Colour and shape.
    pub style: Style,
    /// The text.
    pub text: Text,
    /// Whether it is an error.
    pub error: bool,
}

/// A point, an edge or a very short range drawn at its exact position.
#[derive(Debug, Clone, PartialEq)]
pub struct MarkCell {
    /// The id of the annotation in its store.
    pub id: u64,
    /// Its position (with the id it finds the annotation again).
    pub start: u64,
    /// The position in columns.
    pub x: f64,
    /// Colour and shape.
    pub style: Style,
    /// For an edge, its direction (`true` for rising).
    pub edge: Option<bool>,
    /// Whether it is an error.
    pub error: bool,
}

/// A cell of a row.
#[derive(Debug, Clone, PartialEq)]
pub enum Cell {
    /// A range.
    Span(SpanCell),
    /// A point, edge or short range.
    Mark(MarkCell),
}

/// Several annotations in one column (or ones that found no room), counted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DenseCell {
    /// The column.
    pub column: u32,
    /// How many annotations it stands for.
    pub count: u64,
    /// How many of them are errors.
    pub errors: u64,
}

/// What to draw for one store in one window.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Plan {
    /// Rows of spans and marks, each ordered by column and without overlap.
    pub rows: Vec<Vec<Cell>>,
    /// The dense cells, ordered by column, one per column.
    pub dense: Vec<DenseCell>,
    /// More ranges cross the left edge of the window than were looked up (only when the window
    /// holds more annotations than cells).
    pub clipped: bool,
}

impl Plan {
    /// The number of cells of the plan.
    #[must_use]
    pub fn cell_count(&self) -> usize {
        self.rows
            .iter()
            .map(Vec::len)
            .sum::<usize>()
            .saturating_add(self.dense.len())
    }

    /// How many annotations the plan represents.
    #[must_use]
    pub fn represented(&self) -> u64 {
        let cells: usize = self.rows.iter().map(Vec::len).sum();
        let dense: u64 = self.dense.iter().map(|d| d.count).sum();
        u64::try_from(cells)
            .unwrap_or(u64::MAX)
            .saturating_add(dense)
    }
}

/// The column an annotation starting at `start` is counted in: the first for one that starts
/// before the window.
fn start_column(window: &Window, start: u64) -> u32 {
    if start < window.start {
        0
    } else {
        window.column(start)
    }
}

/// One annotation to place: what it is and the columns it needs.
struct Piece {
    cell: Cell,
    first: u32,
    last: u32,
    id: u64,
    error: bool,
}

fn piece(entry: &Entry, window: &Window) -> Piece {
    let annotation = &entry.annotation;
    let (start, end) = annotation.span();
    if let Kind::Range { .. } = annotation.kind {
        let x0 = window.x(start);
        let x1 = window.x(end);
        if x1 - x0 >= 1.0 {
            let first = start_column(window, start);
            let last = window
                .column(end.min(window.end).saturating_sub(1))
                .max(first);
            return Piece {
                cell: Cell::Span(SpanCell {
                    id: entry.id,
                    start,
                    x0,
                    x1,
                    style: annotation.style,
                    text: annotation.text.clone(),
                    error: annotation.error,
                }),
                first,
                last,
                id: entry.id,
                error: annotation.error,
            };
        }
    }
    let x = window.x(start);
    let column = start_column(window, start);
    let edge = match annotation.kind {
        Kind::Edge { rising, .. } => Some(rising),
        _ => None,
    };
    Piece {
        cell: Cell::Mark(MarkCell {
            id: entry.id,
            start,
            x,
            style: annotation.style,
            edge,
            error: annotation.error,
        }),
        first: column,
        last: column,
        id: entry.id,
        error: annotation.error,
    }
}

/// Plans one store in one window with `rows` rows of spans and marks.
#[must_use]
pub fn plan(snapshot: &Snapshot, window: &Window, rows: usize) -> Plan {
    let columns = window.columns.max(1);
    let window = Window { columns, ..*window };
    let mut result = Plan::default();
    if window.end <= window.start || snapshot.is_empty() {
        return result;
    }
    let limit = usize::try_from(columns)
        .unwrap_or(usize::MAX)
        .saturating_mul(4);
    let mut probes = 0;
    let (entries, truncated) =
        snapshot.window_limited(window.start, window.end, limit, &mut probes);
    // Annotations that need a place, and the counts per column of those that share one.
    let mut pieces: Vec<Piece> = Vec::new();
    let mut dense: BTreeMap<u32, (u64, u64)> = BTreeMap::new();
    if truncated {
        let (crossing, more) = snapshot.crossing(window.start, CROSSING_LIMIT, &mut probes);
        result.clipped = more;
        pieces.extend(crossing.iter().map(|e| piece(e, &window)));
        count_by_column(snapshot, &window, &mut pieces, &mut dense);
    } else {
        let mut by_column: BTreeMap<u32, Vec<Piece>> = BTreeMap::new();
        for entry in entries {
            let p = piece(entry, &window);
            if matches!(p.cell, Cell::Span(_)) {
                pieces.push(p);
            } else {
                by_column.entry(p.first).or_default().push(p);
            }
        }
        for (column, mut group) in by_column {
            if group.len() == 1 {
                pieces.extend(group.pop());
            } else {
                let errors = group.iter().filter(|p| p.error).count();
                add_dense(&mut dense, column, group.len(), errors);
            }
        }
    }
    pack(pieces, rows.max(1), &mut result, &mut dense);
    result.dense = dense
        .into_iter()
        .map(|(column, (count, errors))| DenseCell {
            column,
            count,
            errors,
        })
        .collect();
    result
}

fn add_dense(dense: &mut BTreeMap<u32, (u64, u64)>, column: u32, count: usize, errors: usize) {
    let slot = dense.entry(column).or_insert((0, 0));
    slot.0 = slot
        .0
        .saturating_add(u64::try_from(count).unwrap_or(u64::MAX));
    slot.1 = slot
        .1
        .saturating_add(u64::try_from(errors).unwrap_or(u64::MAX));
}

/// Dense mode: per column, how many annotations start in it (two rank queries per boundary), and
/// the annotation itself for a column that holds exactly one.
fn count_by_column(
    snapshot: &Snapshot,
    window: &Window,
    pieces: &mut Vec<Piece>,
    dense: &mut BTreeMap<u32, (u64, u64)>,
) {
    let mut probes = 0;
    let mut previous = snapshot.rank(window.start, &mut probes);
    let mut boundary = window.start;
    for column in 0..window.columns {
        let next_boundary = window.boundary(column.saturating_add(1));
        if next_boundary <= boundary {
            continue;
        }
        let rank = snapshot.rank(next_boundary, &mut probes);
        let count = rank.count.saturating_sub(previous.count);
        let errors = rank.errors.saturating_sub(previous.errors);
        match count {
            0 => {}
            1 => match snapshot.first_from(boundary, &mut probes) {
                Some(entry) => pieces.push(piece(entry, window)),
                None => add_dense(dense, column, 1, usize::try_from(errors).unwrap_or(0)),
            },
            _ => add_dense(
                dense,
                column,
                usize::try_from(count).unwrap_or(usize::MAX),
                usize::try_from(errors).unwrap_or(usize::MAX),
            ),
        }
        previous = rank;
        boundary = next_boundary;
    }
}

/// First-fit packing of spans and marks into rows by column; what does not fit is counted in the
/// dense cell of its first column.
fn pack(
    mut pieces: Vec<Piece>,
    rows: usize,
    result: &mut Plan,
    dense: &mut BTreeMap<u32, (u64, u64)>,
) {
    pieces.sort_by_key(|p| (p.first, p.id));
    // The last column used in each row, plus one (0 when the row is empty).
    let mut used: Vec<u32> = Vec::new();
    for p in pieces {
        let row = used.iter().position(|end| *end <= p.first);
        let row = match row {
            Some(row) => Some(row),
            None if used.len() < rows => {
                used.push(0);
                result.rows.push(Vec::new());
                Some(used.len().saturating_sub(1))
            }
            None => None,
        };
        match row {
            Some(row) => {
                if let (Some(end), Some(cells)) = (used.get_mut(row), result.rows.get_mut(row)) {
                    *end = p.last.saturating_add(1);
                    cells.push(p.cell);
                }
            }
            None => add_dense(dense, p.first, 1, usize::from(p.error)),
        }
    }
}
