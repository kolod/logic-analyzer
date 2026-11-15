//! Lane and overlay plans against a brute-force reference (spec FR-027, SC-003, research R5).
//!
//! Every annotation of the window must be represented exactly once, by a span, a mark or a count
//! in a dense cell, in the column it starts in; an error is never hidden in a dense cell without
//! the error count.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::panic,
    clippy::cast_possible_truncation
)]

mod common;

use common::Lcg;
use la_decoders::lanes::{CROSSING_LIMIT, Cell, LANE_ROWS, OVERLAY_ROWS, Plan, Window, plan};
use la_decoders::model::{Annotation, Kind, Shape, Style, Target, Text};
use la_decoders::store::{Budget, Limits, Store};

fn annotation(kind: Kind, error: bool) -> Annotation {
    Annotation {
        kind,
        target: Target::Lane,
        style: Style {
            colour: 3,
            shape: Shape::Bar,
        },
        text: Text::Literal("t".into()),
        error,
    }
}

fn store_of(all: &[Annotation]) -> Store {
    let mut store = Store::new(Limits::default(), Budget::new(1 << 40));
    for a in all {
        store.insert(a.clone()).unwrap();
    }
    store
}

fn start_of(a: &Annotation) -> u64 {
    a.span().0
}

/// The interval an annotation occupies (a point takes its sample).
fn occupied(a: &Annotation) -> (u64, u64) {
    let (s, e) = a.span();
    (s, e.max(s + 1))
}

fn short_random(rng: &mut Lcg, span: u64, max_len: u64) -> Annotation {
    let start = rng.below(span);
    let kind = match rng.below(6) {
        0 => Kind::Point { at: start },
        1 => Kind::Edge {
            at: start,
            rising: rng.below(2) == 0,
        },
        2 => Kind::Range { start, end: start },
        _ => Kind::Range {
            start,
            end: start + 1 + rng.below(max_len),
        },
    };
    annotation(kind, rng.below(9) == 0)
}

/// Per column: how many annotations the reference says the plan must represent, and how many of
/// them are errors. An annotation is counted in the column it starts in (the first one if it
/// starts before the window).
fn expected_columns(all: &[Annotation], window: &Window) -> (Vec<u64>, Vec<u64>) {
    let mut counts = vec![0_u64; window.columns as usize];
    let mut errors = vec![0_u64; window.columns as usize];
    for a in all {
        let (s, e) = occupied(a);
        if s < window.end && e > window.start {
            let column = if s < window.start {
                0
            } else {
                window.column(s) as usize
            };
            counts[column] += 1;
            errors[column] += u64::from(a.error);
        }
    }
    (counts, errors)
}

/// What the plan says per column, given the annotations (to find where each cell starts).
fn plan_columns(plan: &Plan, all: &[Annotation], window: &Window) -> (Vec<u64>, Vec<u64>) {
    let mut counts = vec![0_u64; window.columns as usize];
    let mut errors = vec![0_u64; window.columns as usize];
    for row in &plan.rows {
        for cell in row {
            let (id, error) = match cell {
                Cell::Span(s) => (s.id, s.error),
                Cell::Mark(m) => (m.id, m.error),
            };
            let s = start_of(&all[id as usize]);
            let column = if s < window.start {
                0
            } else {
                window.column(s) as usize
            };
            counts[column] += 1;
            errors[column] += u64::from(error);
        }
    }
    for d in &plan.dense {
        counts[d.column as usize] += d.count;
        errors[d.column as usize] += d.errors;
        assert!(d.count > 0 && d.errors <= d.count, "{d:?}");
    }
    (counts, errors)
}

fn assert_plan_is_sound(plan: &Plan, all: &[Annotation], window: &Window, rows: usize) {
    // Bounded: at most `rows` rows plus the dense row, every row without overlap, in order.
    assert!(plan.rows.len() <= rows, "{} rows", plan.rows.len());
    let per_column = (rows + 1) * window.columns as usize;
    assert!(
        plan.cell_count() <= per_column,
        "{} cells",
        plan.cell_count()
    );
    assert!(plan.dense.len() <= window.columns as usize);
    for row in &plan.rows {
        let mut right = f64::NEG_INFINITY;
        for cell in row {
            let (left, end) = match cell {
                Cell::Span(s) => (s.x0, s.x1),
                Cell::Mark(m) => (m.x, m.x),
            };
            assert!(left >= right, "cells overlap or are out of order in a row");
            assert!(left >= 0.0 && end <= f64::from(window.columns) + 1e-9);
            right = end;
        }
    }
    assert!(plan.dense.windows(2).all(|w| w[0].column < w[1].column));
    // No annotation twice.
    let mut ids: Vec<u64> = plan
        .rows
        .iter()
        .flatten()
        .map(|c| match c {
            Cell::Span(s) => s.id,
            Cell::Mark(m) => m.id,
        })
        .collect();
    ids.sort_unstable();
    let before = ids.len();
    ids.dedup();
    assert_eq!(ids.len(), before, "an annotation has two cells");
    // Represented exactly once, in its column; errors never hidden.
    let (want_counts, want_errors) = expected_columns(all, window);
    let (got_counts, got_errors) = plan_columns(plan, all, window);
    if plan.clipped {
        // Ranges that cross the left edge beyond the lookup limit are not all counted.
        assert!(got_counts[0] <= want_counts[0]);
        assert_eq!(got_counts[1..], want_counts[1..]);
        assert_eq!(got_errors[1..], want_errors[1..]);
    } else {
        assert_eq!(got_counts, want_counts);
        assert_eq!(got_errors, want_errors);
    }
}

fn windows(rng: &mut Lcg, span: u64) -> Vec<Window> {
    let mut out = vec![Window {
        start: 0,
        end: span,
        columns: 200,
    }];
    for _ in 0..12 {
        let start = rng.below(span);
        let width = 1 + rng.below(span / 3 + 1);
        let columns = [1_u32, 7, 64, 200, 900][rng.below(5) as usize];
        out.push(Window {
            start,
            end: start + width,
            columns,
        });
    }
    out
}

#[test]
fn every_annotation_of_a_sparse_window_is_represented_exactly_once() {
    for seed in 1..=5 {
        let mut rng = Lcg(seed);
        let span = 30_000;
        let all: Vec<Annotation> = (0..4_000)
            .map(|_| short_random(&mut rng, span, 60))
            .collect();
        let mut store = store_of(&all);
        let snapshot = store.snapshot();
        for window in windows(&mut rng, span) {
            for rows in [LANE_ROWS, OVERLAY_ROWS] {
                let p = plan(&snapshot, &window, rows);
                assert_plan_is_sound(&p, &all, &window, rows);
            }
        }
    }
}

#[test]
fn a_window_with_more_annotations_than_cells_is_counted_by_rank() {
    for seed in 1..=3 {
        let mut rng = Lcg(seed);
        let span = 100_000;
        // Many more annotations than the 4 · W cells of a narrow window, with short ranges so
        // that few cross the left edge.
        let all: Vec<Annotation> = (0..60_000)
            .map(|_| short_random(&mut rng, span, 30))
            .collect();
        let mut store = store_of(&all);
        let snapshot = store.snapshot();
        for window in windows(&mut rng, span) {
            for rows in [LANE_ROWS, OVERLAY_ROWS] {
                let p = plan(&snapshot, &window, rows);
                assert_plan_is_sound(&p, &all, &window, rows);
            }
        }
    }
}

#[test]
fn ranges_crossing_the_left_edge_are_shown_up_to_a_limit_and_the_rest_is_flagged() {
    // Twenty long nested ranges all cross position 5,000; the window starts there.
    let mut all: Vec<Annotation> = (0..20)
        .map(|i| {
            annotation(
                Kind::Range {
                    start: 100 + i,
                    end: 9_000 + i,
                },
                false,
            )
        })
        .collect();
    let mut rng = Lcg(1);
    // Enough points inside the window to force the counting mode.
    all.extend((0..5_000).map(|_| {
        annotation(
            Kind::Point {
                at: 5_000 + rng.below(1_000),
            },
            false,
        )
    }));
    let mut store = store_of(&all);
    let snapshot = store.snapshot();
    let window = Window {
        start: 5_000,
        end: 6_000,
        columns: 100,
    };
    let p = plan(&snapshot, &window, LANE_ROWS);
    assert!(
        p.clipped,
        "more than {CROSSING_LIMIT} ranges cross the edge"
    );
    let (got, want) = (p.represented(), 5_020);
    assert!(
        got < want && got >= want - 20 + CROSSING_LIMIT as u64,
        "{got}"
    );
    // With only a few crossing ranges nothing is flagged and all are counted.
    all.truncate(3);
    all.extend((0..5_000).map(|_| {
        annotation(
            Kind::Point {
                at: 5_000 + rng.below(1_000),
            },
            false,
        )
    }));
    let mut store = store_of(&all);
    let p = plan(&store.snapshot(), &window, LANE_ROWS);
    assert!(!p.clipped);
    assert_eq!(p.represented(), 5_003);
}

#[test]
fn one_annotation_alone_in_a_column_has_its_exact_position() {
    let all = vec![
        annotation(Kind::Point { at: 250 }, false),
        annotation(
            Kind::Edge {
                at: 600,
                rising: true,
            },
            true,
        ),
        annotation(
            Kind::Range {
                start: 700,
                end: 900,
            },
            false,
        ),
    ];
    let mut store = store_of(&all);
    let window = Window {
        start: 0,
        end: 1_000,
        columns: 100,
    };
    let p = plan(&store.snapshot(), &window, LANE_ROWS);
    assert!(p.dense.is_empty());
    let cells: Vec<&Cell> = p.rows.iter().flatten().collect();
    assert_eq!(cells.len(), 3);
    let Cell::Mark(point) = cells[0] else {
        panic!("{:?}", cells[0])
    };
    assert!((point.x - 25.0).abs() < 1e-9, "{}", point.x);
    let Cell::Mark(edge) = cells[1] else {
        panic!("{:?}", cells[1])
    };
    assert_eq!(edge.edge, Some(true));
    assert!(edge.error);
    let Cell::Span(span) = cells[2] else {
        panic!("{:?}", cells[2])
    };
    assert!((span.x0 - 70.0).abs() < 1e-9 && (span.x1 - 90.0).abs() < 1e-9);
}

#[test]
fn several_in_one_column_become_a_dense_cell_that_keeps_the_error_count() {
    // Three annotations at positions 10, 11 and 12 share column 1 of a 10-column window of 100.
    let all = vec![
        annotation(Kind::Point { at: 10 }, false),
        annotation(Kind::Point { at: 11 }, true),
        annotation(Kind::Point { at: 12 }, false),
        annotation(Kind::Point { at: 55 }, false),
    ];
    let mut store = store_of(&all);
    let window = Window {
        start: 0,
        end: 100,
        columns: 10,
    };
    let p = plan(&store.snapshot(), &window, LANE_ROWS);
    assert_eq!(p.dense.len(), 1);
    assert_eq!(
        (p.dense[0].column, p.dense[0].count, p.dense[0].errors),
        (1, 3, 1)
    );
    assert_eq!(
        p.rows.iter().flatten().count(),
        1,
        "the one in column 5 keeps its own cell"
    );
}

#[test]
fn more_overlapping_ranges_than_rows_overflow_into_the_dense_cell_of_their_column() {
    // Six ranges that all cover columns 2 to 6: three fit the rows, three are counted.
    let all: Vec<Annotation> = (0..6)
        .map(|i| {
            annotation(
                Kind::Range {
                    start: 20 + i,
                    end: 70 + i,
                },
                i == 5,
            )
        })
        .collect();
    let mut store = store_of(&all);
    let window = Window {
        start: 0,
        end: 100,
        columns: 10,
    };
    let p = plan(&store.snapshot(), &window, LANE_ROWS);
    assert_eq!(p.rows.len(), LANE_ROWS);
    assert_eq!(p.rows.iter().flatten().count(), 3);
    let dense: u64 = p.dense.iter().map(|d| d.count).sum();
    assert_eq!(dense, 3);
    // The last one is an error and is among those counted, so the count says so.
    assert_eq!(p.dense.iter().map(|d| d.errors).sum::<u64>(), 1);
    assert_eq!(p.represented(), 6);
    // An overlay has one row only.
    let p = plan(&store.snapshot(), &window, OVERLAY_ROWS);
    assert_eq!((p.rows.len(), p.represented()), (1, 6));
}

#[test]
fn empty_inputs_give_empty_plans() {
    let mut empty = Store::new(Limits::default(), Budget::new(1 << 30));
    let window = Window {
        start: 0,
        end: 100,
        columns: 10,
    };
    assert_eq!(plan(&empty.snapshot(), &window, LANE_ROWS), Plan::default());
    let mut store = store_of(&[annotation(Kind::Point { at: 5 }, false)]);
    let none = Window {
        start: 50,
        end: 50,
        columns: 10,
    };
    assert_eq!(plan(&store.snapshot(), &none, LANE_ROWS), Plan::default());
    let elsewhere = Window {
        start: 50,
        end: 100,
        columns: 10,
    };
    assert_eq!(
        plan(&store.snapshot(), &elsewhere, LANE_ROWS),
        Plan::default()
    );
}

#[test]
fn the_window_maps_positions_to_columns_consistently() {
    let w = Window {
        start: 100,
        end: 1_100,
        columns: 7,
    };
    // Every position is in the column whose boundaries enclose it.
    for p in 100..1_100 {
        let c = w.column(p);
        assert!(
            w.boundary(c) <= p && p < w.boundary(c + 1),
            "{p} column {c}"
        );
    }
    assert_eq!(w.boundary(0), 100);
    assert_eq!(w.boundary(7), 1_100);
    // More columns than positions: the columns are still ordered.
    let narrow = Window {
        start: 0,
        end: 3,
        columns: 10,
    };
    let columns: Vec<u32> = (0..3).map(|p| narrow.column(p)).collect();
    assert!(columns.windows(2).all(|c| c[0] < c[1]));
}

#[test]
fn a_planted_one_sample_annotation_is_represented_at_every_zoom_among_a_million() {
    const TOTAL: u64 = 1_000_000;
    const PLANTED: u64 = 612_345;
    let mut rng = Lcg(77);
    let mut store = Store::new(Limits::default(), Budget::new(1 << 40));
    for _ in 0..TOTAL {
        let at = rng.below(1_000_000_000);
        // Nothing else sits within 1,000 positions of the planted one.
        if (at as i64 - PLANTED as i64).abs() < 1_000 {
            continue;
        }
        store.insert(annotation(Kind::Point { at }, false)).unwrap();
    }
    let planted = store
        .insert(annotation(
            Kind::Edge {
                at: PLANTED,
                rising: false,
            },
            true,
        ))
        .unwrap();
    let snapshot = store.snapshot();
    let columns = 1_000;
    // From the whole capture down to many pixels per sample.
    let mut width = 1_000_000_000_u64;
    let mut seen_alone = false;
    let mut seen_dense = false;
    while width >= 20 {
        let start = PLANTED.saturating_sub(width / 3);
        let window = Window {
            start,
            end: start + width,
            columns,
        };
        let p = plan(&snapshot, &window, LANE_ROWS);
        let alone = p
            .rows
            .iter()
            .flatten()
            .any(|c| matches!(c, Cell::Mark(m) if m.id == planted && m.error));
        let column = window.column(PLANTED);
        let counted = p
            .dense
            .iter()
            .any(|d| d.column == column && d.count >= 1 && d.errors >= 1);
        assert!(alone || counted, "lost at width {width}");
        seen_alone |= alone;
        seen_dense |= counted;
        assert!(p.cell_count() <= 4 * columns as usize);
        width /= 3;
    }
    assert!(seen_alone, "it gets a cell of its own when zoomed in");
    assert!(seen_dense, "and is counted as an error when zoomed out");
}
