//! Decoding throughput benchmark (spec SC-004): the sample decoder over a hundred million samples.
//!
//! Run with `cargo bench -p la-decoders --bench decode_throughput`; add `-- --write-baseline` to
//! record the current machine in `benches/baseline.json`.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::panic,
    clippy::print_stdout,
    clippy::cast_precision_loss
)]

mod common;

use std::sync::atomic::AtomicBool;
use std::time::Instant;

use la_decoders::decoders::clocked_bits::{DESCRIPTOR, ID};
use la_decoders::generators::clocked_bits::source;
use la_decoders::registry::Registry;
use la_decoders::run::{ChunkLimits, Outcome, Pass, Sink, run_pass};
use la_decoders::setup::Setup;
use la_decoders::store::{Budget, Limits};
use la_devices::SignalSource;
use la_storage::{Capture, CaptureConfig};

const SAMPLES: u64 = 100_000_000;

fn main() {
    let mut setup = Setup::new(1, ID);
    setup.channels.insert("clock".into(), 0);
    setup.channels.insert("data".into(), 1);
    // Long idle stretches between frames keep the annotation count below the limit of an instance.
    setup
        .options
        .insert("idle".into(), la_decoders::setup::Value::Integer(4_000));
    let mut generator = source(&setup, 3).unwrap();
    let mut capture = Capture::new(CaptureConfig {
        channels: 2,
        sample_rate_hz: 100_000_000,
        limits: la_storage::Limits {
            memory_budget_bytes: 8 << 30,
            ..la_storage::Limits::default()
        },
    })
    .unwrap();
    let started = Instant::now();
    let mut chunk = vec![0_u32; 1 << 16];
    let mut position = 0_u64;
    while position < SAMPLES {
        generator.fill(position, &mut chunk);
        capture.append(position, &chunk).unwrap();
        position += chunk.len() as u64;
    }
    capture.close().unwrap();
    let generate_s = started.elapsed().as_secs_f64();
    println!("generated and stored {SAMPLES} samples in {generate_s:.1} s");
    let registry = Registry::builtin();
    let mut pass = Pass::new(
        &DESCRIPTOR,
        setup.clone(),
        registry.create(&setup).unwrap(),
        Sink::new(Limits::default(), Budget::new(u64::MAX)),
    );
    let started = Instant::now();
    let outcome = run_pass(
        &mut pass,
        &capture.reader(),
        &ChunkLimits::default(),
        &AtomicBool::new(false),
        &mut || false,
        &mut |_, _| {},
    )
    .unwrap();
    let elapsed = started.elapsed().as_secs_f64();
    // The 5 million annotations an instance may hold are reached long before 100 M samples are
    // decoded with the default frame length; either end is a valid way for a pass to finish.
    assert!(matches!(outcome, Outcome::Complete | Outcome::Limit(_)));
    let covered = pass.position();
    let mega = covered as f64 / 1e6;
    println!(
        "decoded {mega:.0} M samples in {elapsed:.2} s: {:.0} M samples/s, {} annotations ({outcome:?})",
        mega / elapsed,
        pass.sink().annotation_count()
    );
    common::finish(
        "decode_throughput",
        &[("microseconds_per_million_samples", elapsed * 1e6 / mega)],
    );
}
