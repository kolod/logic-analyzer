//! A deterministic mutation fuzzer for the loader (spec US3, FR-033, SC-005).
//!
//! Mutated copies of valid and crafted files are loaded from memory. Every outcome must be a typed
//! error or a session that passes the storage core's consistency rules; nothing may panic, hang
//! or allocate beyond the budget. Half of the mutations get their checksums repaired so that the
//! structural and semantic checks behind the checksums are reached. The seed is fixed; a failing
//! input is written to `target/fuzz-failures/`.
//!
//! `fuzz_default` runs 20,000 inputs; `cargo test --release --test fuzz -- --ignored` runs one
//! million (the weekly workflow does).
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::panic
)]

mod common;

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::Duration;

use common::{
    Counting, End, Rng, TempDir, build_file, data, file_with_capture, load_bytes, peak_allocation,
    real_file, record, repair_checksums, small_capt, split_sections,
};
use la_session::{Control, LoadLimits, Section, SectionTag, Session, save_blocking};
use la_storage::Limits;

#[global_allocator]
static ALLOCATOR: Counting = Counting;

const BUDGET: u64 = 4 * 1024 * 1024;

fn limits() -> LoadLimits {
    LoadLimits {
        storage: Limits {
            memory_budget_bytes: BUDGET,
            ..Limits::default()
        },
        ..LoadLimits::default()
    }
}

fn base_files() -> Vec<Vec<u8>> {
    let dir = TempDir::new("fuzz");
    let save = |name: &str, session: &la_session::SessionData| {
        let path = dir.join(name);
        save_blocking(&path, session, &Control::new()).unwrap();
        std::fs::read(path).unwrap()
    };
    let small = record(
        3,
        80,
        &[(None, 600), (Some(40), 300)],
        End::Interrupted,
        1,
        Limits::default(),
    );
    let busy = record(6, 60, &[(None, 3_000)], End::Closed, 2, Limits::default());
    let extra = data(&small, true, Some(9))
        .with_section(Section {
            tag: SectionTag::new("TEST").unwrap(),
            critical: false,
            payload: (0..40).collect(),
        })
        .unwrap();
    vec![
        save("a", &data(&small, true, Some(7))),
        save("b", &data(&busy, false, None)),
        save("c", &extra),
        real_file(),
        file_with_capture(&small_capt().bytes()),
        build_file(&split_sections(&real_file())),
    ]
}

const BOUNDARY: [u64; 9] = [0, 1, 127, 128, 255, 65_535, 1 << 32, 0xFFFF_FFFF, u64::MAX];

fn mutate(rng: &mut Rng, bases: &[Vec<u8>]) -> Vec<u8> {
    let mut bytes = bases[rng.below(bases.len() as u64) as usize].clone();
    for _ in 0..=rng.below(3) {
        if bytes.is_empty() {
            break;
        }
        let len = bytes.len() as u64;
        // Half of the targeted offsets are in the first 100 bytes, where the framing is.
        let at = if rng.below(2) == 0 {
            rng.below(len.min(100))
        } else {
            rng.below(len)
        } as usize;
        match rng.below(9) {
            0 => bytes[at] ^= 1 << rng.below(8),
            1 => bytes[at] = rng.next_u64() as u8,
            2 => bytes.truncate(at),
            3 => {
                let n = 1 + rng.below(8) as usize;
                let fill: Vec<u8> = (0..n).map(|_| rng.next_u64() as u8).collect();
                bytes.splice(at..at, fill);
            }
            4 => {
                let end = (at + 1 + rng.below(8) as usize).min(bytes.len());
                bytes.drain(at..end);
            }
            5 => {
                let end = (at + 1 + rng.below(64) as usize).min(bytes.len());
                let copy = bytes[at..end].to_vec();
                let to = rng.below(len) as usize;
                bytes.splice(to..to, copy);
            }
            6 => {
                let other = &bases[rng.below(bases.len() as u64) as usize];
                let from = rng.below(other.len() as u64) as usize;
                bytes.truncate(at);
                bytes.extend_from_slice(&other[from..]);
            }
            7 => {
                let value = BOUNDARY[rng.below(BOUNDARY.len() as u64) as usize];
                let width = [1_usize, 2, 4, 8][rng.below(4) as usize];
                let le = value.to_le_bytes();
                for (i, b) in le.iter().take(width).enumerate() {
                    if let Some(slot) = bytes.get_mut(at + i) {
                        *slot = *b;
                    }
                }
            }
            _ => {
                let end = (at + 1 + rng.below(16) as usize).min(bytes.len());
                for b in &mut bytes[at..end] {
                    *b = 0;
                }
            }
        }
    }
    if rng.below(2) == 0 {
        repair_checksums(&mut bytes);
    }
    bytes
}

/// The rules of the capture data model, checked on whatever the loader accepted.
fn assert_consistent(session: &Session) {
    let snapshot = session.snapshot();
    let length = snapshot.length();
    assert!(snapshot.status().closed);
    let mut previous_end = None::<u64>;
    for gap in snapshot.gaps() {
        assert!(gap.length >= 1);
        assert!(gap.end().is_some_and(|end| end <= length));
        assert!(
            previous_end.is_none_or(|end| gap.start > end),
            "gaps overlap or touch"
        );
        previous_end = gap.end();
    }
    let first_stored = match snapshot.gaps().first() {
        Some(g) if g.start == 0 => g.end().unwrap(),
        _ => 0,
    };
    for channel in 0..snapshot.channels() {
        let count = snapshot.transition_count(channel).unwrap();
        if count == 0 || length < 2 {
            continue;
        }
        let mut last = first_stored;
        let mut seen = 0_u64;
        for edge in snapshot.transitions(channel, 0, length).unwrap() {
            assert!(
                edge.position > last && edge.position < length,
                "position {}",
                edge.position
            );
            assert!(
                !snapshot
                    .gaps()
                    .iter()
                    .any(|g| g.start <= edge.position && edge.position < g.end().unwrap()),
                "a transition lies inside a gap"
            );
            last = edge.position;
            seen += 1;
        }
        assert_eq!(seen, count);
    }
}

fn run(iterations: u64, seed: u64) {
    let bases = base_files();
    let finished = Arc::new(AtomicBool::new(false));
    let heartbeat = Arc::new(AtomicU64::new(0));
    let watchdog = {
        let (finished, heartbeat) = (Arc::clone(&finished), Arc::clone(&heartbeat));
        std::thread::spawn(move || {
            let mut last = u64::MAX;
            let mut stalled = 0;
            while !finished.load(Ordering::Acquire) {
                std::thread::sleep(Duration::from_secs(1));
                let now = heartbeat.load(Ordering::Acquire);
                stalled = if now == last { stalled + 1 } else { 0 };
                last = now;
                if stalled >= 8 {
                    eprintln!("the loader made no progress for 8 seconds: a hang");
                    std::process::abort();
                }
            }
        })
    };
    let mut rng = Rng::new(seed);
    let (mut loaded, mut rejected) = (0_u64, 0_u64);
    for i in 0..iterations {
        heartbeat.store(i, Ordering::Release);
        let input = mutate(&mut rng, &bases);
        let (outcome, peak) =
            peak_allocation(|| catch_unwind(AssertUnwindSafe(|| load_bytes(&input, &limits()))));
        let fail = |what: &str| -> ! {
            let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/fuzz-failures");
            let _ = std::fs::create_dir_all(&dir);
            let path = dir.join(format!("{seed}-{i}.bin"));
            let _ = std::fs::write(&path, &input);
            panic!(
                "seed {seed} input {i}: {what} (saved to {})",
                path.display()
            );
        };
        match outcome {
            Err(_) => fail("the loader panicked"),
            Ok(Err(error)) => {
                assert!(error.code().starts_with("session."), "{}", error.code());
                rejected += 1;
            }
            Ok(Ok(session)) => {
                if catch_unwind(AssertUnwindSafe(|| assert_consistent(&session))).is_err() {
                    fail("the loader accepted an inconsistent capture");
                }
                loaded += 1;
            }
        }
        let allowed =
            i64::try_from(BUDGET).unwrap() + 3 * i64::try_from(input.len()).unwrap() + 1_048_576;
        if peak > allowed {
            fail(&format!(
                "the loader allocated {peak} bytes (allowed {allowed})"
            ));
        }
    }
    finished.store(true, Ordering::Release);
    watchdog.join().unwrap();
    println!("{iterations} inputs: {rejected} rejected, {loaded} loaded");
    assert!(
        rejected > iterations / 2,
        "most mutations should be rejected"
    );
}

#[test]
fn fuzz_default() {
    run(20_000, 0x5E55_10F0);
}

#[test]
#[ignore = "one million inputs: run in release, weekly in CI"]
fn fuzz_one_million() {
    run(1_000_000, 0xF022_1234);
}
