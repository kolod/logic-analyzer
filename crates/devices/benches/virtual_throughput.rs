//! Virtual device throughput benchmark (spec SC-010).
//!
//! Dependency-free harness (`harness = false`). Run with
//! `cargo bench -p la-devices --bench virtual_throughput`; add `-- --write-baseline` to record
//! the current machine in the `virtual` section of `benches/baseline.json`.
//!
//! The virtual device runs in fast mode through the real acquisition (driver thread, bounded
//! channel, ingest thread) into a capture. SC-010 asks for at least 100 million samples per
//! second on 32 channels with a sparse signal and no recorded loss. The typical signal is
//! reported for information.
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
    AcquisitionOptions, DeviceConfig, DeviceId, DeviceOptions, DeviceRegistry, EndReason,
    RegistryConfig, VirtualOptions,
};

/// SC-010 target in million samples per second for the sparse signal on 32 channels.
const TARGET_MSPS: f64 = 100.0;
/// A result more than this many times slower than the baseline fails the run.
const SLOWDOWN_LIMIT: f64 = 5.0;

struct Case {
    name: String,
    msps: f64,
    lost: u64,
    gated: bool,
}

fn mask(channels: u8) -> u32 {
    if channels >= 32 {
        u32::MAX
    } else {
        (1_u32 << channels) - 1
    }
}

fn run_case(
    registry: &DeviceRegistry,
    name: &str,
    channels: u8,
    mean_gap: u32,
    samples: u64,
    gated: bool,
) -> Case {
    let config = DeviceConfig {
        enabled_channels: la_devices::ChannelSet::from_u32(mask(channels)),
        sample_rate_hz: 100_000_000,
        trigger: None,
        capture_length: Some(samples),
        options: DeviceOptions::Virtual(VirtualOptions {
            mean_gap,
            ..VirtualOptions::default()
        }),
        accept_budget_risk: true,
    };
    let started = Instant::now();
    let acquisition = registry
        .start(
            &DeviceId::new("virtual", "0"),
            config,
            AcquisitionOptions::default(),
        )
        .unwrap();
    let outcome = acquisition
        .wait_timeout(Duration::from_secs(600))
        .expect("the acquisition must end");
    let seconds = started.elapsed().as_secs_f64();
    assert_eq!(outcome.end, EndReason::Completed, "{name} did not complete");
    let msps = outcome.delivered_samples as f64 / seconds / 1e6;
    println!(
        "{name:<18} {msps:>9.1} MS/s   ({} samples in {seconds:.2} s, {} lost)",
        outcome.delivered_samples, outcome.lost_positions
    );
    Case {
        name: name.to_owned(),
        msps,
        lost: outcome.lost_positions,
        gated,
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
    let registry = DeviceRegistry::new(RegistryConfig::default());
    let mut cases = Vec::new();
    for channels in [1_u8, 8, 16, 32] {
        // Sparse: a flip about every 1,000 samples per channel. Gated on 32 channels (SC-010).
        cases.push(run_case(
            &registry,
            &format!("{channels}ch_sparse"),
            channels,
            1_000,
            100_000_000,
            channels == 32,
        ));
        // Typical busy buses: a flip about every 50 samples per channel. Informational.
        cases.push(run_case(
            &registry,
            &format!("{channels}ch_typical"),
            channels,
            50,
            40_000_000,
            false,
        ));
    }

    let rows: Vec<String> = cases
        .iter()
        .map(|c| {
            format!(
                "    \"{}\": {{\"msps\": {:.1}, \"lost\": {}}}",
                c.name, c.msps, c.lost
            )
        })
        .collect();
    let section = format!("\"virtual\": {{\n{}\n  }}", rows.join(",\n"));
    // The workspace target directory (a bench runs in the crate directory), so CI finds the file.
    let dir: PathBuf = [
        env!("CARGO_MANIFEST_DIR"),
        "..",
        "..",
        "target",
        "la-devices-bench",
    ]
    .iter()
    .collect();
    let _ = std::fs::create_dir_all(&dir);
    std::fs::write(dir.join("virtual.json"), format!("{{\n  {section}\n}}\n")).unwrap();
    println!("wrote {}", dir.join("virtual.json").display());

    let baseline_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("benches")
        .join("baseline.json");
    let existing = std::fs::read_to_string(&baseline_path).unwrap_or_default();
    if write_baseline {
        let head = match existing.find("\"virtual\"") {
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
            "SC-010 {:<14} {:.1} MS/s, {} lost (target {TARGET_MSPS}, no loss)",
            c.name, c.msps, c.lost
        );
        if c.msps < TARGET_MSPS || c.lost != 0 {
            println!("FAIL: SC-010 not met for {}", c.name);
            failed = true;
        }
    }
    if !write_baseline {
        for c in &cases {
            if let Some(base) = baseline_msps(&existing, &c.name) {
                let ratio = base / c.msps;
                println!("{:<18} {:.2}x slower than baseline", c.name, ratio);
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
