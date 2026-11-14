//! Helpers shared by the host tests.
#![allow(
    dead_code,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects
)]

use la_storage::{Capture, CaptureConfig, Limits, Snapshot};

/// A tiny deterministic generator (tests must not depend on a random crate).
pub struct Lcg(pub u64);

impl Lcg {
    pub fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        self.0 >> 20
    }

    pub fn below(&mut self, bound: u64) -> u64 {
        self.next() % bound.max(1)
    }
}

/// What to put into a test capture.
pub struct Shape {
    pub channels: u8,
    pub samples: u64,
    /// The longest run of constant levels on a channel (1 gives a transition at every sample).
    pub max_run: u64,
    /// How many gaps to record at random places.
    pub gaps: u32,
}

/// Builds a closed capture of the given shape from `seed`; the sample rate is 1 MHz.
pub fn capture_snapshot(seed: u64, shape: &Shape) -> Snapshot {
    let mut rng = Lcg(seed);
    let mut capture = Capture::new(CaptureConfig {
        channels: shape.channels,
        sample_rate_hz: 1_000_000,
        limits: Limits::default(),
    })
    .unwrap();
    // Per channel: the level and the number of samples until it flips.
    let mut level = vec![false; usize::from(shape.channels)];
    let mut left: Vec<u64> = (0..shape.channels)
        .map(|_| 1 + rng.below(shape.max_run))
        .collect();
    let mut gap_at: Vec<u64> = (0..shape.gaps)
        .map(|_| rng.below(shape.samples.max(1)))
        .collect();
    gap_at.sort_unstable();
    let mut position = 0_u64;
    while position < shape.samples {
        if gap_at.first().is_some_and(|g| *g <= position) {
            gap_at.remove(0);
            capture.record_gap(1 + rng.below(50)).unwrap();
            position = capture.length();
            continue;
        }
        let chunk_len = (shape.samples - position).min(4096);
        let mut chunk = Vec::with_capacity(usize::try_from(chunk_len).unwrap());
        for _ in 0..chunk_len {
            let mut word = 0_u32;
            for (c, (lvl, remaining)) in level.iter_mut().zip(left.iter_mut()).enumerate() {
                if *remaining == 0 {
                    *lvl = !*lvl;
                    *remaining = 1 + rng.below(shape.max_run);
                }
                *remaining -= 1;
                if *lvl {
                    word |= 1 << c;
                }
            }
            chunk.push(word);
        }
        capture.append(capture.length(), &chunk).unwrap();
        position = capture.length();
    }
    capture.close().unwrap();
    capture.reader().snapshot()
}

/// A scratch directory removed on drop.
pub struct TempDir(pub std::path::PathBuf);

impl TempDir {
    pub fn new(name: &str) -> Self {
        use std::sync::atomic::{AtomicU64, Ordering};
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let unique = COUNTER.fetch_add(1, Ordering::Relaxed);
        let path =
            std::env::temp_dir().join(format!("la-host-{name}-{}-{unique}", std::process::id()));
        std::fs::create_dir_all(&path).unwrap();
        TempDir(path)
    }

    pub fn path(&self) -> &std::path::Path {
        &self.0
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

use la_host::window::ViewSize;
use la_host::{Activity, Command, ConfigEdit, Outcome, Shell, ShellConfig};

/// A shell that has captured `length` samples on the channels of `mask` with `seed`, unpaced.
pub fn finished_shell(dir: &TempDir, mask: u32, length: u64, seed: u64) -> Shell {
    let mut shell = Shell::new(ShellConfig::for_tests(dir.path()));
    for edit in [
        ConfigEdit::Realtime(false),
        ConfigEdit::Channels(mask),
        ConfigEdit::SampleRate(1_000_000),
        ConfigEdit::Length(length),
        ConfigEdit::Seed(Some(seed)),
        ConfigEdit::AcceptBudgetRisk(true),
    ] {
        shell.apply(Command::EditConfig(edit));
    }
    let outcome = shell.apply(Command::StartCapture);
    assert!(matches!(outcome, Outcome::Done(_)), "{outcome:?}");
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(60);
    while shell.state().activity != Activity::Idle {
        shell.poll(std::time::Instant::now());
        assert!(
            std::time::Instant::now() < deadline,
            "the capture did not end"
        );
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
    shell.apply(Command::SetViewSize(ViewSize {
        pixels: 800,
        scale: 1.0,
    }));
    shell
}
