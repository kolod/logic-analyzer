//! Annotation store insertion benchmark (spec FR-011, research R4).
//!
//! Run with `cargo bench -p la-decoders --bench store_insert`; add `-- --write-baseline` to record
//! the current machine in `benches/baseline.json` (shared with `decode_throughput`).
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::panic,
    clippy::print_stdout
)]

mod common;

use std::time::Instant;

use la_decoders::model::{Annotation, Kind, Shape, Style, Target, Text};
use la_decoders::store::{Budget, Limits, Store};

const COUNT: u64 = 2_000_000;

fn annotation(at: u64) -> Annotation {
    Annotation {
        kind: Kind::Range {
            start: at,
            end: at + 5,
        },
        target: Target::Lane,
        style: Style {
            colour: 0,
            shape: Shape::Bar,
        },
        text: Text::Literal(String::new()),
        error: false,
    }
}

fn time(order: &str, positions: impl Iterator<Item = u64>) -> f64 {
    let mut store = Store::new(
        Limits {
            max_annotations: usize::MAX,
        },
        Budget::new(u64::MAX),
    );
    let started = Instant::now();
    for at in positions {
        store.insert(annotation(at)).unwrap();
    }
    let per_insert = started.elapsed().as_secs_f64() * 1e9 / COUNT as f64;
    println!(
        "{order:<22} {per_insert:>10.0} ns per insert, {} runs",
        store.run_count()
    );
    per_insert
}

fn main() {
    let mut state = 11_u64;
    let in_order = time("in start order", (0..COUNT).map(|i| i * 10));
    let nearly = time(
        "nearly in order",
        (0..COUNT).map(|i| {
            state = state
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1);
            i * 10 + (state >> 40) % 400
        }),
    );
    let mut state = 5_u64;
    let random = time(
        "random order",
        (0..COUNT).map(|_| {
            state = state
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1);
            (state >> 24) % (COUNT * 10)
        }),
    );
    common::finish(
        "store_insert",
        &[
            ("in_order_ns", in_order),
            ("nearly_ns", nearly),
            ("random_ns", random),
        ],
    );
}
