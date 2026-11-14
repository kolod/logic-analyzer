//! Ingestion throughput benchmark (spec SC-006; research R9).
//!
//! Dependency-free harness (`harness = false`). Run with
//! `cargo bench -p la-storage --bench ingest`; add `-- --write-baseline` to record the
//! current machine in the `ingest` section of `benches/baseline.json`.
//!
//! SC-006 asks the store to sustain a 100 MHz sample rate on 32 channels, i.e. at least
//! 100 million samples per second, for realistic signals. The alternating worst case
//! (a level change on every channel at every sample) is memory-bound, is reported for
//! information, and is not held to that target.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::panic,
    clippy::print_stdout
)]

use std::hint::black_box;
use std::path::PathBuf;
use std::time::Instant;

use la_storage::{Capture, CaptureConfig, Limits};

const CHUNK: usize = 65_536;
/// SC-006 target in million samples per second for sparse and typical signals.
const TARGET_MSPS: f64 = 100.0;
/// A result more than this many times slower than the baseline fails the run.
const SLOWDOWN_LIMIT: f64 = 5.0;

struct Case {
    name: String,
    msps: f64,
    realistic: bool,
}

/// Deterministic xorshift generator.
fn next(state: &mut u64) -> u64 {
    *state ^= *state << 13;
    *state ^= *state >> 7;
    *state ^= *state << 17;
    *state
}

fn mask(channels: u8) -> u32 {
    if channels >= 32 {
        u32::MAX
    } else {
        (1_u32 << channels) - 1
    }
}

/// One chunk where every channel flips with the given probability per mille.
fn chunk(channels: u8, per_mille: u64, state: &mut u64) -> Vec<u32> {
    let mut word = 0_u32;
    (0..CHUNK)
        .map(|_| {
            for ch in 0..channels {
                if next(state) % 1000 < per_mille {
                    word ^= 1 << ch;
                }
            }
            word & mask(channels)
        })
        .collect()
}

fn alternating_chunk(channels: u8) -> Vec<u32> {
    (0..CHUNK)
        .map(|i| if i % 2 == 0 { 0 } else { mask(channels) })
        .collect()
}

fn run_case(name: &str, channels: u8, data: &[u32], total_samples: usize, realistic: bool) -> Case {
    let mut capture = Capture::new(CaptureConfig {
        channels,
        sample_rate_hz: 100_000_000,
        limits: Limits::default(),
    })
    .unwrap();
    let started = Instant::now();
    let mut written = 0;
    while written < total_samples {
        capture.append(capture.length(), black_box(data)).unwrap();
        written += data.len();
    }
    let seconds = started.elapsed().as_secs_f64();
    let msps = written as f64 / seconds / 1e6;
    println!(
        "{name:<22} {msps:>9.1} MS/s   ({written} samples in {seconds:.2} s, {:.1} MiB used)",
        capture.memory_used() as f64 / (1024.0 * 1024.0)
    );
    Case {
        name: name.to_owned(),
        msps,
        realistic,
    }
}

/// Reads `"<name>": {"msps": X` from a baseline without a JSON parser.
fn baseline_msps(text: &str, name: &str) -> Option<f64> {
    let at = text.find(&format!("\"{name}\""))?;
    let rest = &text[at..];
    let key = rest.find("\"msps\":")?;
    let number: String = rest[key + "\"msps\":".len()..]
        .trim_start()
        .chars()
        .take_while(|c| c.is_ascii_digit() || *c == '.')
        .collect();
    number.parse().ok()
}

fn main() {
    let write_baseline = std::env::args().any(|a| a == "--write-baseline");
    let mut state = 0x1234_5678_9abc_def1_u64;
    let mut cases = Vec::new();

    for channels in [1_u8, 8, 16, 32] {
        let sparse = chunk(channels, 1, &mut state); // about 0.1% flips per channel
        cases.push(run_case(
            &format!("{channels}ch_sparse"),
            channels,
            &sparse,
            40_000_000,
            true,
        ));
        // Typical busy buses: about 2% flips per channel and sample, i.e. one change per
        // 50 samples (a 2 MHz signal on every channel at 100 MS/s). Gated by SC-006.
        let typical = chunk(channels, 20, &mut state);
        cases.push(run_case(
            &format!("{channels}ch_typical"),
            channels,
            &typical,
            20_000_000,
            true,
        ));
        // Stress: about 5% flips per channel and sample (5 MHz on every channel).
        // Reported for information only.
        let stress = chunk(channels, 50, &mut state);
        cases.push(run_case(
            &format!("{channels}ch_stress"),
            channels,
            &stress,
            10_000_000,
            false,
        ));
        let worst = alternating_chunk(channels);
        cases.push(run_case(
            &format!("{channels}ch_alternating"),
            channels,
            &worst,
            2_000_000,
            false,
        ));
    }

    let rows: Vec<String> = cases
        .iter()
        .map(|c| format!("    \"{}\": {{\"msps\": {:.1}}}", c.name, c.msps))
        .collect();
    let section = format!("\"ingest\": {{\n{}\n  }}", rows.join(",\n"));
    // The workspace target directory (a bench runs in the crate directory), so CI finds the file.
    let dir: PathBuf = [
        env!("CARGO_MANIFEST_DIR"),
        "..",
        "..",
        "target",
        "la-storage-bench",
    ]
    .iter()
    .collect();
    let _ = std::fs::create_dir_all(&dir);
    std::fs::write(dir.join("ingest.json"), format!("{{\n  {section}\n}}\n")).unwrap();
    println!("wrote {}", dir.join("ingest.json").display());

    let baseline_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("benches")
        .join("baseline.json");
    let existing = std::fs::read_to_string(&baseline_path).unwrap_or_default();
    if write_baseline {
        let head = match existing.find("\"ingest\"") {
            Some(i) => existing[..i].trim_end().trim_end_matches(',').to_owned(),
            None => existing
                .trim_end()
                .trim_end_matches('}')
                .trim_end()
                .to_owned(),
        };
        let head = if head.is_empty() {
            "{".to_owned()
        } else {
            head
        };
        std::fs::write(&baseline_path, format!("{head},\n  {section}\n}}\n")).unwrap();
        println!("baseline written to {}", baseline_path.display());
    }

    let mut failed = false;
    for c in cases
        .iter()
        .filter(|c| c.realistic && c.name.starts_with("32ch"))
    {
        println!(
            "SC-006 {:<14} {:.1} MS/s (target {TARGET_MSPS})",
            c.name, c.msps
        );
        if c.msps < TARGET_MSPS {
            println!("FAIL: SC-006 not met for {}", c.name);
            failed = true;
        }
    }
    if !write_baseline {
        for c in &cases {
            if let Some(base) = baseline_msps(&existing, &c.name) {
                let ratio = base / c.msps;
                println!("{:<22} {:.2}x slower than baseline", c.name, ratio);
                if ratio > SLOWDOWN_LIMIT {
                    println!("FAIL: {} is {ratio:.1}x slower than the baseline", c.name);
                    failed = true;
                }
            }
        }
    }
    if failed {
        std::process::exit(1);
    }
}
