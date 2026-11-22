//! Markers benchmark (constitution Principle VIII, `specs/015-markers-rework`).
//!
//! Dependency-free harness (`harness = false`). Run with `cargo bench -p la-host --bench markers`;
//! add `-- --write-baseline` to record the current machine in `benches/markers_baseline.json`.
//!
//! Two things must not depend on the length of the capture or on how many transitions lie near
//! the pointer: the frame plan with 64 markers (32 pairs) on it, and the resolution of a pointer
//! to a position and a transition (snapping), which asks the storage's indexed queries a fixed
//! number of times. A result more than five times slower than the baseline fails the run, and
//! so does a resolution of a capture 8 times longer that takes more than 4 times as long.
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

use la_host::markers::{Kind, Pointer};
use la_host::window::ViewSize;
use la_host::{Activity, Command, ConfigEdit, Shell, ShellConfig, SignalProfile};

const SLOWDOWN_LIMIT: f64 = 5.0;
const LENGTH_GROWTH_LIMIT: f64 = 4.0;

fn record(shell: &mut Shell, samples: u64) {
    for edit in [
        ConfigEdit::Realtime(false),
        ConfigEdit::Channels(la_devices::ChannelSet::from_u32(0xffff)),
        ConfigEdit::SampleRate(100_000_000),
        ConfigEdit::Length(samples),
        ConfigEdit::Seed(Some(1)),
        ConfigEdit::Profile(SignalProfile::Dense),
        ConfigEdit::AcceptBudgetRisk(true),
    ] {
        shell.apply(Command::EditConfig(edit));
    }
    shell.apply(Command::StartCapture);
    while shell.state().activity != Activity::Idle {
        shell.poll(Instant::now());
        std::thread::sleep(Duration::from_millis(2));
    }
    shell.apply(Command::SetViewSize(ViewSize {
        pixels: 2_000,
        scale: 1.0,
    }));
}

/// Puts 32 single markers and 16 pairs (48 of the 64 that fit) over the view.
fn mark(shell: &mut Shell, samples: u64) {
    for i in 0..32_u64 {
        let x = ((i * 60 + 20) as f32).min(1_990.0);
        shell.apply(Command::BeginPlacement(Kind::Single));
        shell.apply(Command::FixPlacement {
            pointer: Pointer { x, lane: None },
        });
    }
    for i in 0..16_u64 {
        shell.apply(Command::BeginPlacement(Kind::Pair));
        for x in [i * 120 + 10, i * 120 + 70] {
            shell.apply(Command::FixPlacement {
                pointer: Pointer {
                    x: x as f32,
                    lane: None,
                },
            });
        }
    }
    assert_eq!(shell.state().markers.list.len(), 48, "{samples}");
}

fn median(mut times: Vec<f64>) -> f64 {
    times.sort_by(f64::total_cmp);
    times[times.len() / 2]
}

fn time_plan(shell: &Shell) -> f64 {
    let times = (0..21)
        .map(|_| {
            let start = Instant::now();
            let plan = shell.frame_plan(ViewSize {
                pixels: 2_000,
                scale: 1.0,
            });
            std::hint::black_box(&plan);
            start.elapsed().as_secs_f64() * 1e6
        })
        .collect();
    median(times)
}

fn time_resolve(shell: &Shell) -> f64 {
    let times = (0..21)
        .map(|round| {
            let start = Instant::now();
            for k in 0..200 {
                let x = ((round * 200 + k) % 1_990) as f32 + 3.5;
                let resolved = shell.resolve_pointer(Pointer { x, lane: Some(0) });
                std::hint::black_box(resolved);
            }
            start.elapsed().as_secs_f64() * 1e6 / 200.0
        })
        .collect();
    median(times)
}

fn baseline_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("benches/markers_baseline.json")
}

fn main() {
    let write = std::env::args().any(|a| a == "--write-baseline");
    let dir = std::env::temp_dir().join(format!("la-bench-markers-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let mut rows: Vec<(String, f64)> = Vec::new();
    let mut resolve_times = Vec::new();
    for samples in [1_000_000_u64, 8_000_000] {
        let mut shell = Shell::new(ShellConfig::for_tests(&dir));
        record(&mut shell, samples);
        mark(&mut shell, samples);
        let plan = time_plan(&shell);
        let resolve = time_resolve(&shell);
        println!(
            "{} M samples: plan with 48 markers {plan:.0} us; resolve a pointer {resolve:.2} us",
            samples / 1_000_000
        );
        rows.push((
            format!("{}M plan with 48 markers", samples / 1_000_000),
            plan,
        ));
        rows.push((
            format!("{}M resolve a pointer", samples / 1_000_000),
            resolve,
        ));
        resolve_times.push(resolve);
    }
    let _ = std::fs::remove_dir_all(&dir);
    let growth = resolve_times[1] / resolve_times[0].max(1e-9);
    println!("resolving a pointer in a capture 8 times longer: x{growth:.2}");
    assert!(
        growth <= LENGTH_GROWTH_LIMIT,
        "resolving grows with the length of the capture: x{growth:.2}"
    );
    if write {
        let body: Vec<String> = rows
            .iter()
            .map(|(name, us)| format!("    \"{name}\": {{\"us\": {us:.1}}}"))
            .collect();
        let text = format!("{{\n  \"markers\": {{\n{}\n  }}\n}}\n", body.join(",\n"));
        std::fs::write(baseline_path(), text).unwrap();
        println!("baseline written to {}", baseline_path().display());
        return;
    }
    let Ok(text) = std::fs::read_to_string(baseline_path()) else {
        println!("no baseline: run with -- --write-baseline to record one");
        return;
    };
    for (name, us) in &rows {
        let Some(at) = text.find(&format!("\"{name}\"")) else {
            continue;
        };
        let rest = &text[at..];
        let Some(start) = rest.find("\"us\":") else {
            continue;
        };
        let number: String = rest[start + 5..]
            .trim_start()
            .chars()
            .take_while(|c| c.is_ascii_digit() || *c == '.')
            .collect();
        let recorded: f64 = number.parse().unwrap_or(0.0);
        if recorded > 0.0 {
            assert!(
                *us <= recorded * SLOWDOWN_LIMIT,
                "{name}: {us:.1} us is more than {SLOWDOWN_LIMIT} times the baseline {recorded:.1} us"
            );
        }
    }
}
