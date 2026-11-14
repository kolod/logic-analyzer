//! Timing benchmark of viewport queries (spec SC-002, SC-003; research R9).
//!
//! Dependency-free harness (`harness = false`). Run with
//! `cargo bench -p la-storage --bench viewport`; add `-- --write-baseline` to record the
//! current machine as the reference in `benches/baseline.json`.
//!
//! The merge-blocking complexity check is the deterministic probe-count test in
//! `tests/complexity.rs`; this benchmark reports wall-clock numbers, writes them to
//! `target/la-storage-bench/viewport.json`, and fails if a measurement is more than 5x
//! slower than the baseline or if the probe growth ratio breaks the documented bound.
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

use la_storage::testing::synthetic_capture;
use la_storage::{Capture, CaptureConfig, Limits, Snapshot};

const PIXELS: u32 = 2_000;
const CHANNELS: u8 = 16;
const RUNS: usize = 21;
/// A measurement more than this many times slower than the baseline fails the run.
const SLOWDOWN_LIMIT: f64 = 5.0;
/// SC-002: a 2,000-pixel x 16-channel viewport on 10^9 transitions must take under this.
const SC002_LIMIT_US: f64 = 20_000.0;

struct Measurement {
    name: String,
    median_us: f64,
    p95_us: f64,
    probes: u64,
}

/// Times one query over all channels, repeated `RUNS` times after a warm-up.
fn measure(name: &str, snap: &Snapshot, channels: u8) -> Measurement {
    let query = || {
        let mut probes = 0;
        for ch in 0..channels {
            let r = snap.viewport(ch, 0, snap.length(), PIXELS).unwrap();
            probes += r.stats.probes;
            black_box(&r);
        }
        probes
    };
    let probes = query();
    query();
    let mut times: Vec<f64> = (0..RUNS)
        .map(|_| {
            let t = Instant::now();
            black_box(query());
            t.elapsed().as_secs_f64() * 1e6
        })
        .collect();
    times.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let m = Measurement {
        name: name.to_owned(),
        median_us: times[times.len() / 2],
        p95_us: times[(times.len() * 95 / 100).min(times.len() - 1)],
        probes,
    };
    println!(
        "{:<22} median {:>10.1} us  p95 {:>10.1} us  probes {:>9}",
        m.name, m.median_us, m.p95_us, m.probes
    );
    m
}

fn machine() -> String {
    let cpu = std::env::var("PROCESSOR_IDENTIFIER")
        .ok()
        .or_else(|| {
            std::fs::read_to_string("/proc/cpuinfo").ok().and_then(|t| {
                t.lines()
                    .find(|l| l.starts_with("model name"))
                    .and_then(|l| l.split(':').nth(1))
                    .map(|s| s.trim().to_owned())
            })
        })
        .unwrap_or_else(|| "unknown".to_owned());
    let cores = std::thread::available_parallelism().map_or(0, usize::from);
    format!(
        "{{\"os\":\"{}\",\"arch\":\"{}\",\"cpu\":\"{}\",\"cores\":{}}}",
        std::env::consts::OS,
        std::env::consts::ARCH,
        cpu.replace('\\', "/").replace('"', "'"),
        cores
    )
}

fn to_json(results: &[Measurement]) -> String {
    let rows: Vec<String> = results
        .iter()
        .map(|m| {
            format!(
                "    \"{}\": {{\"median_us\": {:.1}, \"p95_us\": {:.1}, \"probes\": {}}}",
                m.name, m.median_us, m.p95_us, m.probes
            )
        })
        .collect();
    format!(
        "{{\n  \"machine\": {},\n  \"pixels\": {},\n  \"channels\": {},\n  \"viewport\": {{\n{}\n  }}\n}}\n",
        machine(),
        PIXELS,
        CHANNELS,
        rows.join(",\n")
    )
}

/// Reads `"<name>": {"median_us": X` from a baseline file without a JSON parser.
fn baseline_median(text: &str, name: &str) -> Option<f64> {
    let at = text.find(&format!("\"{name}\""))?;
    let rest = &text[at..];
    let key = rest.find("\"median_us\":")?;
    let number: String = rest[key + "\"median_us\":".len()..]
        .trim_start()
        .chars()
        .take_while(|c| c.is_ascii_digit() || *c == '.')
        .collect();
    number.parse().ok()
}

fn main() {
    let write_baseline = std::env::args().any(|a| a == "--write-baseline");
    let mut results = Vec::new();

    println!("== synthetic captures ({CHANNELS} channels x {PIXELS} pixels) ==");
    for exp in [6_u32, 7, 8, 9] {
        let n = 10_u64.pow(exp);
        let snap = synthetic_capture(CHANNELS, n).unwrap();
        results.push(measure(&format!("synthetic_1e{exp}"), &snap, CHANNELS));
    }

    println!("== real captures (1 channel, built by appending) ==");
    for n in [1_000_000_usize, 10_000_000] {
        let mut capture = Capture::new(CaptureConfig {
            channels: 1,
            sample_rate_hz: 1_000_000,
            limits: Limits::default(),
        })
        .unwrap();
        // A transition at every sample: n - 1 transitions.
        let chunk: Vec<u32> = (0..65_536_u32).map(|i| i & 1).collect();
        let mut written = 0;
        while written < n {
            let take = chunk.len().min(n - written);
            capture.append(capture.length(), &chunk[..take]).unwrap();
            written += take;
        }
        let snap = capture.reader().snapshot();
        results.push(measure(&format!("real_1e{}", n.ilog10()), &snap, 1));
    }

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
    let out = to_json(&results);
    let _ = std::fs::create_dir_all(&dir);
    std::fs::write(dir.join("viewport.json"), &out).unwrap();
    println!("wrote {}", dir.join("viewport.json").display());

    let baseline_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("benches")
        .join("baseline.json");
    if write_baseline {
        // Merge with any ingest section already recorded.
        let ingest = std::fs::read_to_string(&baseline_path)
            .ok()
            .and_then(|t| t.find("\"ingest\"").map(|i| t[i..].to_owned()));
        let mut text = out.trim_end().trim_end_matches('}').trim_end().to_owned();
        if let Some(section) = ingest {
            text.push_str(",\n  ");
            text.push_str(section.trim_end().trim_end_matches('}').trim_end());
        }
        text.push_str("\n}\n");
        std::fs::write(&baseline_path, text).unwrap();
        println!("baseline written to {}", baseline_path.display());
    }

    let mut failed = false;
    // SC-003 on probes (deterministic).
    let probes = |name: &str| results.iter().find(|m| m.name == name).unwrap().probes as f64;
    let growth = probes("synthetic_1e9") / probes("synthetic_1e6");
    println!("probe growth 1e6 -> 1e9: {growth:.2} (limit 3.0)");
    if growth >= 3.0 {
        println!("FAIL: probe growth ratio breaks SC-003");
        failed = true;
    }
    // SC-002 on the largest synthetic capture.
    let big = results.iter().find(|m| m.name == "synthetic_1e9").unwrap();
    println!(
        "SC-002: {:.1} us for {CHANNELS} channels at 1e9 transitions (limit {SC002_LIMIT_US} us)",
        big.median_us
    );
    if big.median_us >= SC002_LIMIT_US {
        println!("FAIL: SC-002 not met on this machine");
        failed = true;
    }
    // Comparison with the recorded baseline.
    match std::fs::read_to_string(&baseline_path) {
        Ok(text) if !write_baseline => {
            for m in &results {
                if let Some(base) = baseline_median(&text, &m.name) {
                    let ratio = m.median_us / base;
                    println!("{:<22} {:.2}x baseline", m.name, ratio);
                    if ratio > SLOWDOWN_LIMIT {
                        println!("FAIL: {} is {ratio:.1}x slower than the baseline", m.name);
                        failed = true;
                    }
                }
            }
        }
        Ok(_) => {}
        Err(_) => println!("no baseline found at {}", baseline_path.display()),
    }
    if failed {
        std::process::exit(1);
    }
}
