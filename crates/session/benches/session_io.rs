//! Session file throughput benchmark (spec SC-007).
//!
//! Dependency-free harness (`harness = false`). Run with
//! `cargo bench -p la-session --bench session_io`; add `-- --write-baseline` to record the
//! current machine in the `session` section of `benches/baseline.json`.
//!
//! A capture is recorded by the virtual device (fast mode, through the real acquisition), then
//! saved and loaded through the real file system. SC-007 asks for a capture of 100 million samples
//! on 16 channels with a typical signal to save and to load in under 10 seconds each.
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

use la_devices::{
    AcquisitionOptions, DeviceConfig, DeviceId, DeviceOptions, DeviceRegistry, RegistryConfig,
    VirtualOptions,
};
use la_session::{Control, LoadLimits, SessionData, load_blocking, save_blocking};

/// SC-007: the limit for the gated case, in seconds.
const LIMIT_SECONDS: f64 = 10.0;
/// A result more than this many times slower than the baseline fails the run.
const SLOWDOWN_LIMIT: f64 = 5.0;

struct Case {
    name: &'static str,
    file_mib: f64,
    save_s: f64,
    load_s: f64,
    million_positions: f64,
    gated: bool,
}

fn run_case(
    registry: &DeviceRegistry,
    dir: &std::path::Path,
    name: &'static str,
    channels: u8,
    mean_gap: u32,
    samples: u64,
    gated: bool,
) -> Case {
    let mask = if channels >= 32 {
        u32::MAX
    } else {
        (1_u32 << channels) - 1
    };
    let acquisition = registry
        .start(
            &DeviceId::new("virtual", "0"),
            DeviceConfig {
                enabled_channels: la_devices::ChannelSet::from_u32(mask),
                sample_rate_hz: 100_000_000,
                trigger: None,
                capture_length: Some(samples),
                options: DeviceOptions::Virtual(VirtualOptions {
                    mean_gap,
                    ..VirtualOptions::default()
                }),
                accept_budget_risk: true,
            },
            AcquisitionOptions::default(),
        )
        .unwrap();
    acquisition.wait_timeout(Duration::from_secs(600)).unwrap();
    let session = SessionData::new(acquisition.description(), acquisition.capture().snapshot());
    let path = dir.join(format!("{name}.lasession"));

    let started = Instant::now();
    let report = save_blocking(&path, &session, &Control::new()).unwrap();
    let save_s = started.elapsed().as_secs_f64();
    let started = Instant::now();
    let loaded = load_blocking(&path, &LoadLimits::default(), &Control::new()).unwrap();
    let load_s = started.elapsed().as_secs_f64();
    assert_eq!(loaded.snapshot().length(), samples);
    let _ = std::fs::remove_file(&path);

    let case = Case {
        name,
        file_mib: report.bytes_written as f64 / (1024.0 * 1024.0),
        save_s,
        load_s,
        million_positions: samples as f64 / 1e6,
        gated,
    };
    println!(
        "{name:<16} {:>8.1} MiB   save {save_s:>6.2} s ({:>7.1} Mpos/s)   load {load_s:>6.2} s ({:>7.1} Mpos/s)",
        case.file_mib,
        case.million_positions / save_s,
        case.million_positions / load_s,
    );
    case
}

/// Reads `"<name>": {"save_s": X` or `"load_s"` from a baseline without a JSON parser.
fn baseline_value(text: &str, name: &str, key: &str) -> Option<f64> {
    let at = text.find(&format!("\"{name}\""))?;
    let rest = &text[at..];
    let key_at = rest.find(&format!("\"{key}\":"))?;
    let number: String = rest[key_at + key.len() + 3..]
        .trim_start()
        .chars()
        .take_while(|c| c.is_ascii_digit() || *c == '.')
        .collect();
    number.parse().ok()
}

fn machine() -> String {
    let cpu = std::env::var("PROCESSOR_IDENTIFIER").unwrap_or_else(|_| "unknown".to_owned());
    let cores = std::thread::available_parallelism().map_or(0, std::num::NonZero::get);
    format!(
        "{{\"os\":\"{}\",\"arch\":\"{}\",\"cpu\":\"{cpu}\",\"cores\":{cores}}}",
        std::env::consts::OS,
        std::env::consts::ARCH
    )
}

fn main() {
    let write_baseline = std::env::args().any(|a| a == "--write-baseline");
    let target: PathBuf = [
        env!("CARGO_MANIFEST_DIR"),
        "..",
        "..",
        "target",
        "la-session-bench",
    ]
    .iter()
    .collect();
    std::fs::create_dir_all(&target).unwrap();
    let registry = DeviceRegistry::new(RegistryConfig::default());
    let cases = vec![
        run_case(&registry, &target, "1ch_typical", 1, 50, 100_000_000, false),
        run_case(
            &registry,
            &target,
            "16ch_typical",
            16,
            50,
            100_000_000,
            true,
        ),
        run_case(
            &registry,
            &target,
            "16ch_sparse",
            16,
            1_000,
            100_000_000,
            false,
        ),
        run_case(
            &registry,
            &target,
            "32ch_typical",
            32,
            50,
            40_000_000,
            false,
        ),
    ];

    let rows: Vec<String> = cases
        .iter()
        .map(|c| {
            format!(
                "    \"{}\": {{\"file_mib\": {:.1}, \"save_s\": {:.2}, \"load_s\": {:.2}}}",
                c.name, c.file_mib, c.save_s, c.load_s
            )
        })
        .collect();
    let section = format!("\"session\": {{\n{}\n  }}", rows.join(",\n"));
    std::fs::write(
        target.join("session_io.json"),
        format!("{{\n  {section}\n}}\n"),
    )
    .unwrap();
    println!("wrote {}", target.join("session_io.json").display());

    let baseline_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("benches")
        .join("baseline.json");
    let existing = std::fs::read_to_string(&baseline_path).unwrap_or_default();
    if write_baseline {
        let head = match existing.find("\"session\"") {
            Some(i) => existing[..i].trim_end().trim_end_matches(',').to_owned(),
            None if existing.trim().is_empty() => format!("{{\n  \"machine\": {}", machine()),
            None => existing
                .trim_end()
                .trim_end_matches('}')
                .trim_end()
                .to_owned(),
        };
        std::fs::write(&baseline_path, format!("{head},\n  {section}\n}}\n")).unwrap();
        println!("baseline written to {}", baseline_path.display());
    }

    let mut failed = false;
    for c in cases.iter().filter(|c| c.gated) {
        println!(
            "SC-007 {:<14} save {:.2} s, load {:.2} s (limit {LIMIT_SECONDS} s each)",
            c.name, c.save_s, c.load_s
        );
        if c.save_s > LIMIT_SECONDS || c.load_s > LIMIT_SECONDS {
            println!("FAIL: SC-007 not met for {}", c.name);
            failed = true;
        }
    }
    if !write_baseline {
        for c in &cases {
            for (key, now) in [("save_s", c.save_s), ("load_s", c.load_s)] {
                if let Some(base) = baseline_value(&existing, c.name, key) {
                    let ratio = now / base.max(0.001);
                    println!("{:<14} {key}: {ratio:.2}x the baseline", c.name);
                    if ratio > SLOWDOWN_LIMIT {
                        println!(
                            "FAIL: {} {key} is {ratio:.1}x slower than the baseline",
                            c.name
                        );
                        failed = true;
                    }
                }
            }
        }
    }
    if failed {
        std::process::exit(1);
    }
}
