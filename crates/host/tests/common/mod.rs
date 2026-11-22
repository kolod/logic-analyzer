//! Helpers shared by the host tests.
#![allow(
    dead_code,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects
)]

mod session_support;
#[allow(unused_imports)]
pub use session_support::*;

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
        ConfigEdit::Channels(la_devices::ChannelSet::from_u32(mask)),
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

/// A pointer over the signal area (no lane) that resolves to `sample` in the window now shown.
/// The pointer is in the middle of the pixel column that shows the sample, so that, at a zoom
/// where one column shows several samples, it is the first of them.
#[allow(dead_code)]
pub fn pointer_at(shell: &Shell, sample: u64) -> la_host::markers::Pointer {
    let state = shell.state();
    let window = state.window;
    let capture_length = state.capture.as_ref().map_or(0, |c| c.snapshot.length());
    let window = if state.auto_fit {
        (0, capture_length.max(1))
    } else {
        (window.start, window.length.max(1))
    };
    let pixels = f64::from(state.view_size.pixels);
    let offset = sample.saturating_sub(window.0) as f64;
    la_host::markers::Pointer {
        x: ((offset + 0.5) * pixels / window.1 as f64) as f32,
        lane: None,
    }
}

/// Places a single marker at `sample` with the commands a user's clicks send; the id.
#[allow(dead_code)]
pub fn place_single(shell: &mut Shell, sample: u64) -> u32 {
    let outcome = shell.apply(Command::BeginPlacement(la_host::markers::Kind::Single));
    assert!(matches!(outcome, Outcome::Done(_)), "{outcome:?}");
    let pointer = pointer_at(shell, sample);
    let outcome = shell.apply(Command::FixPlacement { pointer });
    assert!(matches!(outcome, Outcome::Done(_)), "{outcome:?}");
    shell.state().markers.selected.unwrap()
}

/// Places a pair marker between two samples with two clicks; the id.
#[allow(dead_code)]
pub fn place_pair(shell: &mut Shell, a: u64, b: u64) -> u32 {
    let outcome = shell.apply(Command::BeginPlacement(la_host::markers::Kind::Pair));
    assert!(matches!(outcome, Outcome::Done(_)), "{outcome:?}");
    for sample in [a, b] {
        let pointer = pointer_at(shell, sample);
        let outcome = shell.apply(Command::FixPlacement { pointer });
        assert!(matches!(outcome, Outcome::Done(_)), "{outcome:?}");
    }
    shell.state().markers.selected.unwrap()
}

/// Every stable error code of the product: constants of the form
/// `pub const NAME: &str = "domain.code";` in the `error.rs` files under `crates/*/src`.
pub fn all_error_codes() -> Vec<String> {
    let crates = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let mut codes = Vec::new();
    for entry in std::fs::read_dir(crates).unwrap().flatten() {
        let file = entry.path().join("src").join("error.rs");
        let Ok(text) = std::fs::read_to_string(&file) else {
            continue;
        };
        for line in text.lines() {
            let Some(rest) = line.trim().strip_prefix("pub const ") else {
                continue;
            };
            let Some((_, value)) = rest.split_once(": &str = \"") else {
                continue;
            };
            let Some(code) = value.strip_suffix("\";") else {
                continue;
            };
            if code.split_once('.').is_some_and(|(d, n)| {
                !d.is_empty() && d.chars().all(|c| c.is_ascii_lowercase()) && !n.is_empty()
            }) {
                codes.push(code.to_owned());
            }
        }
    }
    codes.sort();
    codes.dedup();
    codes
}

/// The text of every file in `dir` (the log files of one run).
pub fn read_logs(dir: &std::path::Path) -> String {
    let mut text = String::new();
    for entry in std::fs::read_dir(dir).unwrap().flatten() {
        if entry.path().is_file() {
            text.push_str(&std::fs::read_to_string(entry.path()).unwrap_or_default());
        }
    }
    text
}

/// A capture of `length` positions, 1 MHz, on the 32 channels of the virtual device, whose channel
/// `c` starts at `initial[c]` and flips at every position of `flips[c]` (sorted, below `length`,
/// above 0): the transitions are exactly those positions. The channels after the ones given are
/// low and never change.
pub fn snapshot_with_edges(initial: &[bool], flips: &[Vec<u64>], length: u64) -> Snapshot {
    let mut capture = Capture::new(CaptureConfig {
        channels: 32,
        sample_rate_hz: 1_000_000,
        limits: Limits::default(),
    })
    .unwrap();
    let mut levels = initial.to_vec();
    let mut next = vec![0_usize; initial.len()];
    let mut words: Vec<u32> = Vec::new();
    for position in 0..length {
        for c in 0..initial.len() {
            while flips[c].get(next[c]).is_some_and(|f| *f == position) {
                levels[c] = !levels[c];
                next[c] += 1;
            }
        }
        let mut word = 0_u32;
        for (c, high) in levels.iter().enumerate() {
            if *high {
                word |= 1 << c;
            }
        }
        words.push(word);
        if words.len() == 60_000 {
            capture.append(capture.length(), &words).unwrap();
            words.clear();
        }
    }
    if !words.is_empty() {
        capture.append(capture.length(), &words).unwrap();
    }
    capture.close().unwrap();
    capture.reader().snapshot()
}

/// A shell that shows `snapshot` (made by `snapshot_with_edges`; opened from a session file made
/// for it) on 800 pixels, whole, with the first `shown` channels recorded and shown.
pub fn shell_showing(dir: &TempDir, snapshot: Snapshot, shown: u8) -> Shell {
    let mask = (1_u32 << shown) - 1;
    let shell = finished_shell(dir, mask, snapshot.length(), 1);
    let description = shell.state().capture.as_ref().unwrap().description.clone();
    let data = la_session::SessionData::new(description, snapshot);
    let path = dir.path().join("shown.lasession");
    la_session::save_blocking(&path, &data, &la_session::Control::new()).unwrap();
    let other = dir.path().join("shown-shell");
    std::fs::create_dir_all(&other).unwrap();
    let mut shell = Shell::new(ShellConfig::for_tests(&other));
    shell.apply(Command::SetViewSize(ViewSize {
        pixels: 800,
        scale: 1.0,
    }));
    let outcome = shell.apply(Command::OpenSession(path));
    assert!(matches!(outcome, Outcome::Done(_)), "{outcome:?}");
    wait_operation(&mut shell);
    shell.apply(Command::SetViewSize(ViewSize {
        pixels: 800,
        scale: 1.0,
    }));
    shell
}
