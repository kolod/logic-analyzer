//! Lane and overlay plan benchmark (spec SC-003, constitution Principle VIII).
//!
//! Dependency-free harness (`harness = false`). Run with
//! `cargo bench -p la-host --bench annotation_plan`; add `-- --write-baseline` to record the current
//! machine in `benches/annotation_baseline.json`.
//!
//! Finished stores of one million and five million annotations (the most an instance can hold)
//! spread over captures of 100 million and 1,000 million samples are planned for the whole capture,
//! for one percent of it and for four pixels per sample, at 2000 and 4000 pixels. The plan must stay
//! inside the frame budget, the five times larger store must not be more than twice as slow as the
//! small one, and a run is compared with the baseline of the recorded machine.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::panic,
    clippy::print_stdout,
    clippy::cast_precision_loss
)]

use std::path::PathBuf;
use std::time::Instant;

use la_decoders::lanes::{LANE_ROWS, Window, plan};
use la_decoders::model::{Annotation, Kind, Shape, Style, Target, Text};
use la_decoders::store::{Budget, Limits, Store};

/// A result more than this many times slower than the baseline fails the run.
const SLOWDOWN_LIMIT: f64 = 5.0;
/// The plan of the larger store may take at most this many times as long as that of the smaller.
const SIZE_GROWTH_LIMIT: f64 = 2.0;
/// The budget of a whole frame in milliseconds; the plan of annotations must fit well inside it.
const FRAME_BUDGET_MS: f64 = 16.0;

struct Row {
    case: String,
    view: &'static str,
    pixels: u32,
    micros: f64,
    cells: usize,
}

fn lcg(state: &mut u64) -> u64 {
    *state = state
        .wrapping_mul(6_364_136_223_846_793_005)
        .wrapping_add(1_442_695_040_888_963_407);
    *state >> 11
}

/// A store of `count` annotations spread over `length` positions: mostly points and short ranges,
/// an error now and then.
fn store(count: u64, length: u64) -> Store {
    let mut store = Store::new(
        Limits {
            max_annotations: usize::MAX,
        },
        Budget::new(u64::MAX),
    );
    let mut rng = 7_u64;
    // Emitted in time order, like a decoder does.
    let step = (length / count).max(1);
    for i in 0..count {
        let start = (i * step + lcg(&mut rng) % step).min(length - 2);
        let kind = match lcg(&mut rng) % 4 {
            0 => Kind::Point { at: start },
            1 => Kind::Edge {
                at: start,
                rising: true,
            },
            _ => Kind::Range {
                start,
                end: (start + 1 + lcg(&mut rng) % (step * 3)).min(length - 1),
            },
        };
        store
            .insert(Annotation {
                kind,
                target: Target::Lane,
                style: Style {
                    colour: Some(1),
                    shape: Shape::Dot,
                },
                text: Text::Literal(String::new()),
                error: lcg(&mut rng) % 50 == 0,
            })
            .unwrap();
    }
    store.compact();
    store
}

fn main() {
    let write_baseline = std::env::args().any(|a| a == "--write-baseline");
    let mut rows: Vec<Row> = Vec::new();
    let mut whole: Vec<(u64, f64)> = Vec::new();
    for (count, length) in [
        (1_000_000_u64, 100_000_000_u64),
        (1_000_000, 1_000_000_000),
        (5_000_000, 100_000_000),
        (5_000_000, 1_000_000_000),
    ] {
        let mut store = store(count, length);
        let snapshot = store.snapshot();
        let case = format!(
            "{}M annotations / {}M samples",
            count / 1_000_000,
            length / 1_000_000
        );
        for pixels in [2000_u32, 4000] {
            for (view, start, end) in [
                ("whole capture", 0, length),
                ("1 % window", length / 2, length / 2 + length / 100),
                (
                    "4 px per sample",
                    length / 2,
                    length / 2 + u64::from(pixels) / 4,
                ),
            ] {
                let window = Window {
                    start,
                    end,
                    columns: pixels,
                };
                let _ = plan(&snapshot, &window, LANE_ROWS);
                let runs = 20;
                let started = Instant::now();
                let mut cells = 0;
                for _ in 0..runs {
                    cells = plan(&snapshot, &window, LANE_ROWS).cell_count();
                }
                let micros = started.elapsed().as_secs_f64() * 1e6 / f64::from(runs);
                assert!(cells <= 4 * pixels as usize, "{cells} cells");
                if view == "whole capture" && pixels == 2000 {
                    whole.push((count, micros));
                }
                rows.push(Row {
                    case: case.clone(),
                    view,
                    pixels,
                    micros,
                    cells,
                });
            }
        }
    }

    println!(
        "{:<34} {:<16} {:>6} {:>12} {:>8}",
        "store", "view", "px", "plan [us]", "cells"
    );
    for row in &rows {
        println!(
            "{:<34} {:<16} {:>6} {:>12.0} {:>8}",
            row.case, row.view, row.pixels, row.micros, row.cells
        );
    }

    // The larger store is not (much) slower than the small one.
    let small = whole
        .iter()
        .filter(|(c, _)| *c == 1_000_000)
        .map(|(_, t)| *t)
        .fold(0.0, f64::max);
    let large = whole
        .iter()
        .filter(|(c, _)| *c == 5_000_000)
        .map(|(_, t)| *t)
        .fold(0.0, f64::max);
    println!(
        "whole capture, 5M against 1M annotations: x{:.2}",
        large / small
    );
    assert!(
        large < small * SIZE_GROWTH_LIMIT + 200.0,
        "x{:.2}",
        large / small
    );
    let worst = rows.iter().map(|r| r.micros).fold(0.0, f64::max);
    println!(
        "slowest plan: {:.2} ms (budget {FRAME_BUDGET_MS} ms for a whole frame)",
        worst / 1000.0
    );
    assert!(
        worst / 1000.0 < FRAME_BUDGET_MS,
        "the plan alone exceeds the frame budget"
    );

    let mut json = String::from("{\n  \"annotation_plan\": {\n");
    for (i, row) in rows.iter().enumerate() {
        let comma = if i + 1 == rows.len() { "" } else { "," };
        json.push_str(&format!(
            "    \"{} / {} / {}px\": {{\"plan_us\": {:.0}, \"cells\": {}}}{comma}\n",
            row.case, row.view, row.pixels, row.micros, row.cells
        ));
    }
    json.push_str("  }\n}\n");
    let artifact_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/la-host-bench");
    if std::fs::create_dir_all(&artifact_dir).is_ok() {
        let _ = std::fs::write(artifact_dir.join("annotation_plan.json"), &json);
    }
    let baseline_path =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("benches/annotation_baseline.json");
    if write_baseline {
        std::fs::write(&baseline_path, json).unwrap();
        println!("baseline written to {}", baseline_path.display());
    } else if let Ok(text) = std::fs::read_to_string(&baseline_path) {
        let parsed: serde_json::Value = serde_json::from_str(&text).unwrap();
        for row in &rows {
            let key = format!("{} / {} / {}px", row.case, row.view, row.pixels);
            if let Some(base) = parsed["annotation_plan"][&key]["plan_us"].as_f64() {
                assert!(
                    row.micros < base.max(50.0) * SLOWDOWN_LIMIT,
                    "{key}: {:.0} us against a baseline of {base:.0} us",
                    row.micros
                );
            }
        }
    }
}
