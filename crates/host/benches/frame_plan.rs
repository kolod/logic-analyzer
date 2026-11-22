//! Frame-plan benchmark (spec SC-002, constitution Principle VIII).
//!
//! Dependency-free harness (`harness = false`). Run with
//! `cargo bench -p la-host --bench frame_plan`; add `-- --write-baseline` to record the current
//! machine in `benches/baseline.json`.
//!
//! A 16-channel capture is recorded by the virtual device (fast mode, through the real shell),
//! then the frame plan is built for the whole capture, for a window of one percent of it and for
//! a window where four pixels show one sample, at 2000 and 4000 pixels. The time of a plan and
//! its index probes are reported; the probes must not grow with the capture length (a run fails
//! if the plan of a 100 M-sample capture needs more than twice the probes of a 1 M-sample one),
//! and the time must stay within a generous factor of the recorded baseline.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::panic,
    clippy::print_stdout
)]

use std::path::PathBuf;
use std::time::{Duration, Instant};

use la_host::window::ViewSize;
use la_host::{Activity, Command, ConfigEdit, Shell, ShellConfig, SignalProfile};

/// A result more than this many times slower than the baseline fails the run.
const SLOWDOWN_LIMIT: f64 = 5.0;
/// The plan of the longest capture may need at most this many times the probes of the shortest.
const PROBE_GROWTH_LIMIT: f64 = 2.0;
/// The target of spec SC-002 for the gated case, in milliseconds per plan.
const FRAME_BUDGET_MS: f64 = 16.0;

struct Row {
    capture: String,
    view: &'static str,
    pixels: u32,
    micros: f64,
    probes: u64,
}

fn record(shell: &mut Shell, samples: u64, profile: SignalProfile) {
    for edit in [
        ConfigEdit::Realtime(false),
        ConfigEdit::Channels(la_devices::ChannelSet::from_u32(0xffff)),
        ConfigEdit::SampleRate(100_000_000),
        ConfigEdit::Length(samples),
        ConfigEdit::Seed(Some(1)),
        ConfigEdit::Profile(profile),
        ConfigEdit::AcceptBudgetRisk(true),
    ] {
        shell.apply(Command::EditConfig(edit));
    }
    shell.apply(Command::StartCapture);
    while shell.state().activity != Activity::Idle {
        shell.poll(Instant::now());
        std::thread::sleep(Duration::from_millis(2));
    }
}

fn plan_time(shell: &Shell, pixels: u32) -> (f64, u64) {
    let size = ViewSize { pixels, scale: 1.0 };
    let _ = shell.frame_plan(size);
    let runs = 20;
    let started = Instant::now();
    let mut probes = 0;
    for _ in 0..runs {
        probes = shell.frame_plan(size).stats.probes;
    }
    (
        started.elapsed().as_secs_f64() * 1e6 / f64::from(runs),
        probes,
    )
}

fn main() {
    let write_baseline = std::env::args().any(|a| a == "--write-baseline");
    let dir = std::env::temp_dir().join(format!("la-host-bench-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let mut rows: Vec<Row> = Vec::new();
    let mut whole_probes: Vec<(u64, u64)> = Vec::new();
    for (label, samples, profile) in [
        ("1M sparse", 1_000_000_u64, SignalProfile::Sparse),
        ("10M sparse", 10_000_000, SignalProfile::Sparse),
        ("100M sparse", 100_000_000, SignalProfile::Sparse),
        ("10M normal", 10_000_000, SignalProfile::Normal),
    ] {
        let mut shell = Shell::new(ShellConfig::for_tests(&dir));
        record(&mut shell, samples, profile);
        for pixels in [2000_u32, 4000] {
            shell.apply(Command::SetViewSize(ViewSize { pixels, scale: 1.0 }));
            shell.apply(Command::FitAll);
            let (micros, probes) = plan_time(&shell, pixels);
            if pixels == 2000 && label.ends_with("sparse") {
                whole_probes.push((samples, probes));
            }
            rows.push(Row {
                capture: label.to_owned(),
                view: "whole capture",
                pixels,
                micros,
                probes,
            });
            shell.apply(Command::ZoomToRange {
                start: samples / 2,
                end: samples / 2 + samples / 100,
            });
            let (micros, probes) = plan_time(&shell, pixels);
            rows.push(Row {
                capture: label.to_owned(),
                view: "1 % window",
                pixels,
                micros,
                probes,
            });
            let narrow = u64::from(pixels) / 4;
            shell.apply(Command::ZoomToRange {
                start: samples / 2,
                end: samples / 2 + narrow,
            });
            let (micros, probes) = plan_time(&shell, pixels);
            rows.push(Row {
                capture: label.to_owned(),
                view: "4 px per sample",
                pixels,
                micros,
                probes,
            });
        }
    }
    let _ = std::fs::remove_dir_all(&dir);

    println!(
        "{:<12} {:<16} {:>6} {:>12} {:>10}",
        "capture", "view", "px", "plan [us]", "probes"
    );
    for row in &rows {
        println!(
            "{:<12} {:<16} {:>6} {:>12.0} {:>10}",
            row.capture, row.view, row.pixels, row.micros, row.probes
        );
    }

    // The probes do not grow with the capture length.
    let first = whole_probes.first().copied().unwrap();
    let last = whole_probes.last().copied().unwrap();
    let growth = last.1 as f64 / first.1 as f64;
    println!(
        "probe growth from {} to {} samples: x{growth:.2}",
        first.0, last.0
    );
    assert!(growth < PROBE_GROWTH_LIMIT, "probes grew by x{growth:.2}");

    // The gated case of SC-002: the whole 100M capture at 2000 pixels, sparse.
    let gated = rows
        .iter()
        .find(|r| r.capture == "100M sparse" && r.view == "whole capture" && r.pixels == 2000)
        .unwrap();
    println!(
        "gated case: {:.2} ms (budget {FRAME_BUDGET_MS} ms for the whole frame, plan stage only)",
        gated.micros / 1000.0
    );

    // The numbers of this run, as an artifact for the benchmark workflow.
    let artifact_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/la-host-bench");
    if std::fs::create_dir_all(&artifact_dir).is_ok() {
        let mut json = String::from("{\n  \"frame_plan\": {\n");
        for (i, row) in rows.iter().enumerate() {
            let comma = if i + 1 == rows.len() { "" } else { "," };
            json.push_str(&format!(
                "    \"{} / {} / {}px\": {{\"plan_us\": {:.0}, \"probes\": {}}}{comma}\n",
                row.capture, row.view, row.pixels, row.micros, row.probes
            ));
        }
        json.push_str("  }\n}\n");
        let _ = std::fs::write(artifact_dir.join("frame_plan.json"), json);
    }

    let baseline_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("benches/baseline.json");
    if write_baseline {
        let mut json = String::from("{\n  \"frame_plan\": {\n");
        for (i, row) in rows.iter().enumerate() {
            let comma = if i + 1 == rows.len() { "" } else { "," };
            json.push_str(&format!(
                "    \"{} / {} / {}px\": {{\"plan_us\": {:.0}, \"probes\": {}}}{comma}\n",
                row.capture, row.view, row.pixels, row.micros, row.probes
            ));
        }
        json.push_str("  }\n}\n");
        std::fs::write(&baseline_path, json).unwrap();
        println!("baseline written to {}", baseline_path.display());
    } else if let Ok(text) = std::fs::read_to_string(&baseline_path) {
        let parsed: serde_json::Value = serde_json::from_str(&text).unwrap();
        for row in &rows {
            let key = format!("{} / {} / {}px", row.capture, row.view, row.pixels);
            if let Some(base) = parsed["frame_plan"][&key]["plan_us"].as_f64() {
                assert!(
                    row.micros < base.max(50.0) * SLOWDOWN_LIMIT,
                    "{key}: {:.0} us against a baseline of {base:.0} us",
                    row.micros
                );
            }
        }
    }
}
