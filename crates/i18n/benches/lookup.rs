//! Timing of message lookup, formatting and the language switch (spec SC-005).
//!
//! Dependency-free harness (`harness = false`). Run with `cargo bench -p la-i18n --bench lookup`;
//! add `-- --write-baseline` to record the machine in `benches/baseline.json`. A lookup must stay
//! under 100 microseconds and a language switch under 10 milliseconds.
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

use la_i18n::{Args, Language, Localizer, key};

/// The longest a lookup may take, in nanoseconds (SC-005).
const LOOKUP_LIMIT_NS: f64 = 100_000.0;
/// The longest a switch may take, in nanoseconds (SC-005).
const SWITCH_LIMIT_NS: f64 = 10_000_000.0;
/// A result more than this many times slower than the baseline fails the run.
const SLOWDOWN_LIMIT: f64 = 5.0;

fn time(name: &'static str, iterations: u32, mut work: impl FnMut()) -> (&'static str, f64) {
    for _ in 0..iterations / 10 {
        work();
    }
    let started = Instant::now();
    for _ in 0..iterations {
        work();
    }
    let ns = started.elapsed().as_secs_f64() * 1e9 / f64::from(iterations);
    println!("{name:<24} {ns:>10.0} ns");
    (name, ns)
}

fn baseline_ns(text: &str, name: &str) -> Option<f64> {
    let at = text.find(&format!("\"{name}\""))?;
    let rest = &text[at..];
    let key = rest.find("\"ns\":")?;
    let number: String = rest[key + 5..]
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
    let english = Localizer::new(Language::En);
    let ukrainian = Localizer::new(Language::Uk);
    let n = 200_000;

    let results = vec![
        time("plain_message", n, || {
            black_box(english.message(&key!("error-session-damaged"), &Args::new()));
        }),
        time("counted_english", n, || {
            black_box(english.message(&key!("count-samples"), &Args::new().uint("count", 1_234)));
        }),
        time("counted_ukrainian", n, || {
            black_box(ukrainian.message(&key!("count-samples"), &Args::new().uint("count", 22)));
        }),
        time("error_message", n, || {
            black_box(english.error_message_for_code("storage.memory_limit"));
        }),
        time("format_frequency", n, || {
            black_box(english.format_frequency(black_box(12_500_000.0)));
        }),
        time("format_datetime", n, || {
            black_box(english.format_datetime(black_box(1_794_672_309)));
        }),
        time("user_text_argument", n, || {
            black_box(english.message(
                &key!("error-unexpected"),
                &Args::new().text("code", "a channel named { $x }"),
            ));
        }),
        {
            let l = Localizer::new(Language::En);
            let mut i = 0_usize;
            time("language_switch", n, move || {
                l.set_language(Language::ALL[i % 6]);
                i += 1;
            })
        },
    ];

    let rows: Vec<String> = results
        .iter()
        .map(|(name, ns)| format!("    \"{name}\": {{\"ns\": {ns:.0}}}"))
        .collect();
    let section = format!("\"lookup\": {{\n{}\n  }}", rows.join(",\n"));
    let dir: PathBuf = [
        env!("CARGO_MANIFEST_DIR"),
        "..",
        "..",
        "target",
        "la-i18n-bench",
    ]
    .iter()
    .collect();
    let _ = std::fs::create_dir_all(&dir);
    std::fs::write(dir.join("lookup.json"), format!("{{\n  {section}\n}}\n")).unwrap();
    println!("wrote {}", dir.join("lookup.json").display());

    let baseline_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("benches")
        .join("baseline.json");
    let existing = std::fs::read_to_string(&baseline_path).unwrap_or_default();
    if write_baseline {
        let head = match existing.find("\"lookup\"") {
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
    for (name, ns) in &results {
        let limit = if *name == "language_switch" {
            SWITCH_LIMIT_NS
        } else {
            LOOKUP_LIMIT_NS
        };
        if *ns > limit {
            println!("FAIL: SC-005: {name} takes {ns:.0} ns (limit {limit:.0})");
            failed = true;
        }
        if !write_baseline {
            if let Some(base) = baseline_ns(&existing, name) {
                let ratio = ns / base.max(1.0);
                println!("{name:<24} {ratio:.2}x the baseline");
                if ratio > SLOWDOWN_LIMIT {
                    println!("FAIL: {name} is {ratio:.1}x slower than the baseline");
                    failed = true;
                }
            }
        }
    }
    if failed {
        std::process::exit(1);
    }
}
