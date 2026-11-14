//! A stress run with hostile input on several threads while the language is switched
//! (spec SC-006). 100,000 operations run with the normal tests; the full million is
//! `#[ignore]`d and run by the weekly workflow in release mode.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::panic
)]

use std::collections::HashSet;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::Duration;

use la_i18n::{Args, Key, Language, Localizer, key};

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    fn below(&mut self, n: u64) -> u64 {
        self.next() % n.max(1)
    }
}

/// A random string: ASCII, syntax characters, bidi controls, other scripts, control characters.
fn hostile_string(rng: &mut Rng, max_len: usize) -> String {
    const PIECES: [&str; 18] = [
        "a",
        "Z",
        "0",
        " ",
        "{",
        "}",
        "$",
        "-",
        ".",
        "\u{202E}",
        "\u{2069}",
        "\u{2066}",
        "ж",
        "日",
        "\u{0}",
        "\n",
        "{ $count }",
        "\u{1F600}",
    ];
    let len = usize::try_from(rng.below(max_len as u64 + 1)).unwrap();
    (0..len)
        .map(|_| PIECES[usize::try_from(rng.below(PIECES.len() as u64)).unwrap()])
        .collect()
}

fn run(operations: u64, seed: u64) {
    let localizer = Localizer::new(Language::En);
    let damaged = key!("error-session-damaged");
    let versions: Arc<HashSet<String>> = Arc::new(
        Language::ALL
            .iter()
            .map(|l| localizer.message_in(*l, &damaged, &Args::new()))
            .collect(),
    );
    let stop = Arc::new(AtomicBool::new(false));
    let done = Arc::new(AtomicU64::new(0));
    let progress = Arc::new(AtomicU64::new(0));

    let watchdog = {
        let (stop, progress) = (Arc::clone(&stop), Arc::clone(&progress));
        std::thread::spawn(move || {
            let mut last = u64::MAX;
            let mut stalled = 0;
            while !stop.load(Ordering::Acquire) {
                std::thread::sleep(Duration::from_secs(1));
                let now = progress.load(Ordering::Acquire);
                stalled = if now == last { stalled + 1 } else { 0 };
                last = now;
                if stalled >= 30 {
                    eprintln!("no progress for 30 seconds: a hang");
                    std::process::abort();
                }
            }
        })
    };
    let switcher = {
        let (localizer, stop) = (localizer.clone(), Arc::clone(&stop));
        std::thread::spawn(move || {
            let mut i = 0_usize;
            while !stop.load(Ordering::Acquire) {
                localizer.set_language(Language::ALL[i % 6]);
                i += 1;
                std::thread::yield_now();
            }
        })
    };
    let threads = 4_u64;
    let workers: Vec<_> = (0..threads)
        .map(|t| {
            let (localizer, versions, done, progress) = (
                localizer.clone(),
                Arc::clone(&versions),
                Arc::clone(&done),
                Arc::clone(&progress),
            );
            std::thread::spawn(move || {
                let mut rng = Rng(seed ^ (t + 1).wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1);
                for _ in 0..operations / threads {
                    match rng.below(9) {
                        0 => {
                            let text =
                                localizer.message(&key!("error-session-damaged"), &Args::new());
                            assert!(versions.contains(&text), "a mixed text: {text}");
                        }
                        1 => {
                            let text = localizer.message(&key!("no-such-key"), &Args::new());
                            assert!(text.contains("no-such-key"));
                        }
                        2 => {
                            let code = hostile_string(&mut rng, 150);
                            let text = localizer.error_message_for_code(&code);
                            assert!(!text.trim().is_empty());
                        }
                        3 => {
                            let tag = hostile_string(&mut rng, 80);
                            let language = Language::negotiate([tag.as_str()]);
                            assert!(Language::ALL.contains(&language));
                            let _ = Language::from_tag(&tag);
                        }
                        4 => {
                            let value = f64::from_bits(rng.next());
                            let _ = localizer.format_number(value);
                            let _ = localizer.format_frequency(value);
                            let _ = localizer.format_duration(value);
                            let _ = localizer.format_size(rng.next());
                            #[allow(clippy::cast_possible_wrap)]
                            let _ = localizer.format_datetime(rng.next() as i64);
                        }
                        5 => {
                            let value = f64::from_bits(rng.next());
                            let text = localizer.message(
                                &key!("count-samples"),
                                &Args::new().float("count", value),
                            );
                            assert!(!text.is_empty());
                        }
                        6 => {
                            // Occasionally a very long value.
                            let len = if rng.below(5_000) == 0 {
                                1_000_000
                            } else {
                                300
                            };
                            let value = hostile_string(&mut rng, len);
                            let text = localizer.message(
                                &key!("error-unexpected"),
                                &Args::new().text("code", &value),
                            );
                            assert!(!text.is_empty());
                        }
                        7 => {
                            let id = hostile_string(&mut rng, 150);
                            let text = localizer.message(&Key::for_error_code(&id), &Args::new());
                            assert!(!text.is_empty());
                        }
                        _ => {
                            let name = hostile_string(&mut rng, 130);
                            let value = hostile_string(&mut rng, 50);
                            let args = Args::new().text(&name, &value).uint("count", rng.next());
                            let text = localizer.message(&key!("count-devices"), &args);
                            assert!(!text.is_empty());
                        }
                    }
                    done.fetch_add(1, Ordering::Relaxed);
                    progress.fetch_add(1, Ordering::Relaxed);
                }
            })
        })
        .collect();
    for worker in workers {
        worker.join().unwrap();
    }
    stop.store(true, Ordering::Release);
    switcher.join().unwrap();
    watchdog.join().unwrap();
    assert!(done.load(Ordering::Relaxed) >= operations - threads);
}

#[test]
fn a_hundred_thousand_hostile_operations_on_four_threads() {
    run(100_000, 0xA11CE);
}

#[test]
#[ignore = "one million operations: run in release, weekly in CI"]
fn a_million_hostile_operations_on_four_threads() {
    run(1_000_000, 0xB0B);
}
