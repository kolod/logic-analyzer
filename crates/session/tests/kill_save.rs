//! A process killed in the middle of a save never leaves a damaged file (spec US2, SC-003).
//!
//! The test binary runs itself as a child process that saves two different sessions to the same
//! path in a loop; the parent kills it at random moments (after a temporary file appeared, so
//! the kill lands inside a save) and checks that the target is always one of the two complete
//! versions.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::panic
)]

mod common;

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use common::{End, Rng, TempDir, data, record};
use la_session::{Control, LoadLimits, SessionData, load_blocking, save_blocking};
use la_storage::Limits;

const TARGET: &str = "LA_SESSION_KILL_TARGET";
const SOURCE_A: &str = "LA_SESSION_KILL_A";
const SOURCE_B: &str = "LA_SESSION_KILL_B";

/// Kills in a debug build take long (process start-up), so fewer are done there; a release run
/// does the 1,000 of the success criterion. `LA_KILL_ITERATIONS` overrides both.
fn iterations() -> usize {
    std::env::var("LA_KILL_ITERATIONS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(if cfg!(debug_assertions) { 150 } else { 1_000 })
}

/// The child: loads the two sources and saves them over the target until it is killed.
#[test]
fn kill_save_child() {
    let Ok(target) = std::env::var(TARGET) else {
        return; // an ordinary run of the test suite: nothing to do
    };
    let load = |var: &str| {
        let path = std::env::var(var).unwrap();
        let session =
            load_blocking(Path::new(&path), &LoadLimits::default(), &Control::new()).unwrap();
        SessionData::from_session(&session)
    };
    let (a, b) = (load(SOURCE_A), load(SOURCE_B));
    let started = Instant::now();
    while started.elapsed() < Duration::from_secs(60) {
        for session in [&a, &b] {
            let _ = save_blocking(Path::new(&target), session, &Control::new());
        }
    }
}

fn temporaries(dir: &Path) -> Vec<PathBuf> {
    std::fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.file_name().is_some_and(|n| n != "target"))
        .collect()
}

#[test]
fn killed_saves_never_leave_a_damaged_or_mixed_file() {
    let sources = TempDir::new("kill-src");
    let work = TempDir::new("kill-work");
    let session = |seed: u64| {
        let capture = record(
            16,
            250,
            &[(None, 40_000), (Some(17), 20_000)],
            End::Closed,
            seed,
            Limits::default(),
        );
        data(&capture, true, Some(seed))
    };
    let (a, b) = (session(31), session(32));
    let (path_a, path_b) = (sources.join("a"), sources.join("b"));
    save_blocking(&path_a, &a, &Control::new()).unwrap();
    save_blocking(&path_b, &b, &Control::new()).unwrap();
    let bytes_a = std::fs::read(&path_a).unwrap();
    let bytes_b = std::fs::read(&path_b).unwrap();
    assert_ne!(bytes_a, bytes_b);

    let target = work.join("target");
    std::fs::write(&target, &bytes_a).unwrap();
    let exe = std::env::current_exe().unwrap();
    let mut rng = Rng::new(0xC0FFEE);
    let mut kills_inside_a_save = 0;
    let rounds = iterations();
    for round in 0..rounds {
        for leftover in temporaries(work.path()) {
            let _ = std::fs::remove_file(leftover);
        }
        let mut child = Command::new(&exe)
            .args([
                "--exact",
                "kill_save_child",
                "--nocapture",
                "--test-threads=1",
            ])
            .env(TARGET, &target)
            .env(SOURCE_A, &path_a)
            .env(SOURCE_B, &path_b)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("the test binary can start itself");
        // Wait until the child is inside a save (a temporary file exists), then kill it after a
        // random extra delay of up to 3 ms.
        let started = Instant::now();
        let mut inside = false;
        while started.elapsed() < Duration::from_secs(20) {
            if !temporaries(work.path()).is_empty() {
                inside = true;
                break;
            }
            if child.try_wait().unwrap().is_some() {
                break;
            }
            std::thread::sleep(Duration::from_micros(200));
        }
        std::thread::sleep(Duration::from_micros(rng.below(3_000)));
        let _ = child.kill();
        let _ = child.wait();
        kills_inside_a_save += usize::from(inside);

        let bytes = std::fs::read(&target).unwrap_or_default();
        assert!(
            bytes == bytes_a || bytes == bytes_b,
            "round {round}: the target is neither complete version ({} bytes)",
            bytes.len()
        );
        load_blocking(&target, &LoadLimits::default(), &Control::new())
            .unwrap_or_else(|e| panic!("round {round}: the target does not load: {e}"));
    }
    assert!(
        kills_inside_a_save * 10 >= rounds * 9,
        "only {kills_inside_a_save} of {rounds} kills landed inside a save"
    );
    // Whatever the kills left behind never blocks the next save.
    for leftover in temporaries(work.path()) {
        let _ = std::fs::remove_file(leftover);
    }
    save_blocking(&target, &a, &Control::new()).unwrap();
    assert_eq!(std::fs::read(&target).unwrap(), bytes_a);
}
