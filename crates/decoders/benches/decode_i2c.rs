//! Decoding throughput of the I2C decoder (spec 008, constitution Principle VIII): generated dense
//! traffic, thirty million samples.
//!
//! Run with `cargo bench -p la-decoders --bench decode_i2c`; add `-- --write-baseline` to record the
//! current machine in `benches/baseline.json`.
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

use la_decoders::decoders::i2c::{DESCRIPTOR, ID};
use la_decoders::generators::i2c::source;
use la_decoders::registry::Registry;
use la_decoders::run::{ChunkLimits, Outcome, Pass, Sink, run_pass};
use la_decoders::setup::Setup;
use la_decoders::store::{Budget, Limits};
use la_devices::SignalSource;
use la_storage::{Capture, CaptureConfig};

const SAMPLES: u64 = 30_000_000;

fn main() {
    let mut setup = Setup::new(1, ID);
    setup.channels.insert("scl".into(), 0);
    setup.channels.insert("sda".into(), 1);
    // A fixed seed: the benchmark is deterministic.
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
    let mut chunk = vec![0_u32; 1 << 16];
    let mut position = 0_u64;
    while position < SAMPLES {
        generator.fill(position, &mut chunk);
        capture.append(position, &chunk).unwrap();
        position += chunk.len() as u64;
    }
    capture.close().unwrap();
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
    assert!(matches!(outcome, Outcome::Complete | Outcome::Limit(_)));
    let mega = pass.position() as f64 / 1e6;
    println!(
        "decoded {mega:.0} M samples in {elapsed:.2} s: {:.0} M samples/s, {} annotations ({outcome:?})",
        mega / elapsed,
        pass.sink().annotation_count()
    );
    common::finish(
        "decode_i2c",
        &[("microseconds_per_million_samples", elapsed * 1e6 / mega)],
    );
}
