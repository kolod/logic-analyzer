//! Shared by the benchmarks: the baseline file and the artifact.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::panic,
    clippy::print_stdout,
    dead_code
)]

use std::path::PathBuf;

/// A result more than this many times slower than the baseline fails the run.
const SLOWDOWN_LIMIT: f64 = 5.0;

/// Writes the numbers as an artifact, and either records them as the baseline
/// (`--write-baseline`) or compares them with it.
pub fn finish(bench: &str, numbers: &[(&str, f64)]) {
    let mut json = format!("  \"{bench}\": {{\n");
    for (i, (name, value)) in numbers.iter().enumerate() {
        let comma = if i + 1 == numbers.len() { "" } else { "," };
        json.push_str(&format!("    \"{name}\": {value:.0}{comma}\n"));
    }
    json.push_str("  }");
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let artifact_dir = root.join("../../target/la-decoders-bench");
    if std::fs::create_dir_all(&artifact_dir).is_ok() {
        let _ = std::fs::write(
            artifact_dir.join(format!("{bench}.json")),
            format!("{{\n{json}\n}}\n"),
        );
    }
    let path = root.join("benches/baseline.json");
    let mut baseline: serde_json::Value = std::fs::read_to_string(&path)
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_else(|| serde_json::json!({}));
    if std::env::args().any(|a| a == "--write-baseline") {
        let mut entry = serde_json::Map::new();
        for (name, value) in numbers {
            entry.insert((*name).to_owned(), serde_json::json!(value.round()));
        }
        baseline[bench] = serde_json::Value::Object(entry);
        std::fs::write(
            &path,
            serde_json::to_string_pretty(&baseline).unwrap() + "\n",
        )
        .unwrap();
        println!("baseline written to {}", path.display());
    } else {
        for (name, value) in numbers {
            if let Some(base) = baseline[bench][name].as_f64() {
                assert!(
                    *value < base.max(50.0) * SLOWDOWN_LIMIT,
                    "{bench}/{name}: {value:.0} against a baseline of {base:.0}"
                );
            }
        }
    }
}
