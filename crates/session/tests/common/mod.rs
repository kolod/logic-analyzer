//! Helpers shared by the integration tests (not a test target itself).
#![allow(
    dead_code,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::panic
)]

mod support;
#[allow(unused_imports)]
pub use support::*;

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime};

use la_devices::{CaptureDescription, SavedDescription, Trigger, TriggerKind};
use la_session::SessionData;
use la_storage::{Capture, CaptureConfig, LevelState, Limits, Snapshot};

/// A scratch directory removed on drop.
pub struct TempDir(PathBuf);

impl TempDir {
    pub fn new(name: &str) -> Self {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let unique = COUNTER.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "la-session-it-{name}-{}-{unique}",
            std::process::id()
        ));
        std::fs::create_dir_all(&path).unwrap();
        Self(path)
    }

    pub fn path(&self) -> &Path {
        &self.0
    }

    pub fn join(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }

    /// Names of all entries in the directory, sorted.
    pub fn names(&self) -> Vec<String> {
        let mut names: Vec<String> = std::fs::read_dir(&self.0)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        names
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Deterministic xorshift generator.
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        Self(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1)
    }

    pub fn next_u64(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    pub fn below(&mut self, n: u64) -> u64 {
        if n == 0 { 0 } else { self.next_u64() % n }
    }
}

/// Random words: every channel flips with probability `per_mille / 1000` per sample.
pub fn random_words(len: usize, channels: u8, per_mille: u64, rng: &mut Rng) -> Vec<u32> {
    let mask = if channels >= 32 {
        u32::MAX
    } else {
        (1_u32 << channels) - 1
    };
    let mut word = rng.next_u64() as u32 & mask;
    (0..len)
        .map(|_| {
            for ch in 0..channels {
                if rng.below(1000) < per_mille {
                    word ^= 1 << ch;
                }
            }
            word
        })
        .collect()
}

/// How a recorded capture ends.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum End {
    Closed,
    Interrupted,
    /// Left open, as a capture that is still being recorded.
    Open,
}

/// Records a capture from random data: `parts` are `(gap before, samples)` pairs.
pub fn record(
    channels: u8,
    per_mille: u64,
    parts: &[(Option<u64>, usize)],
    end: End,
    seed: u64,
    limits: Limits,
) -> Capture {
    let mut capture = Capture::new(CaptureConfig {
        channels,
        sample_rate_hz: 12_500_000,
        limits,
    })
    .unwrap();
    let mut rng = Rng::new(seed);
    for (gap, len) in parts {
        if let Some(gap) = gap {
            capture.record_gap(*gap).unwrap();
        }
        let mut left = *len;
        while left > 0 {
            let n = left.min(60_000);
            let words = random_words(n, channels, per_mille, &mut rng);
            capture.append(capture.length(), &words).unwrap();
            left -= n;
        }
    }
    match end {
        End::Closed => {
            capture.close().unwrap();
        }
        End::Interrupted => {
            capture.close_interrupted().unwrap();
        }
        End::Open => {}
    }
    capture
}

/// A description built from plain values (no clock involved, so files are reproducible).
pub fn description(channels: u8, simulated: bool, seed: Option<u64>) -> CaptureDescription {
    let mask = if channels >= 32 {
        u32::MAX
    } else {
        (1_u32 << channels) - 1
    };
    CaptureDescription::restore(SavedDescription {
        device_id: if simulated {
            "virtual:0"
        } else {
            "usb:logic16-7"
        }
        .to_owned(),
        device_name: if simulated {
            "Virtual device"
        } else {
            "Logic16 Клон"
        }
        .to_owned(),
        simulated,
        seed,
        requested_sample_rate_hz: 12_500_000,
        sample_rate_hz: 12_500_000,
        channels,
        enabled_mask: mask,
        trigger: Some(Trigger {
            channel: 0,
            kind: TriggerKind::RisingEdge,
        }),
        samples_before_trigger: 3,
        started_at: SystemTime::UNIX_EPOCH + Duration::new(1_700_000_000, 123_456_700),
    })
    .unwrap()
}

/// A session of a closed capture and a matching description.
pub fn data(capture: &Capture, simulated: bool, seed: Option<u64>) -> SessionData {
    let snapshot = capture.reader().snapshot();
    SessionData::new(description(snapshot.channels(), simulated, seed), snapshot)
}

/// Asserts that two snapshots answer every query identically.
pub fn assert_same_capture(a: &Snapshot, b: &Snapshot) {
    assert_eq!(a.length(), b.length(), "length");
    assert_eq!(a.channels(), b.channels(), "channels");
    assert_eq!(a.sample_rate_hz(), b.sample_rate_hz(), "sample rate");
    assert_eq!(a.gaps(), b.gaps(), "gaps");
    let (sa, sb) = (a.status(), b.status());
    assert_eq!(sa.lost_positions, sb.lost_positions);
    assert_eq!(sa.stopped_by_limit, sb.stopped_by_limit);
    assert_eq!(sa.complete, sb.complete);
    let mut rng = Rng::new(4242);
    for ch in 0..a.channels() {
        let count = a.transition_count(ch).unwrap();
        assert_eq!(
            count,
            b.transition_count(ch).unwrap(),
            "count of channel {ch}"
        );
        let len = a.length();
        if count > 0 {
            assert_eq!(a.initial_high(ch).unwrap(), b.initial_high(ch).unwrap());
            let ea: Vec<_> = a.transitions(ch, 0, len).unwrap().collect();
            let eb: Vec<_> = b.transitions(ch, 0, len).unwrap().collect();
            assert_eq!(ea, eb, "transitions of channel {ch}");
        }
        for _ in 0..100 {
            let p = rng.below(len + 3);
            assert_eq!(a.level_at(ch, p).unwrap(), b.level_at(ch, p).unwrap());
        }
        if len >= 8 {
            for (start, end, px) in [(0, len, 61_u32), (len / 4, len - 1, 17)] {
                let va = a.viewport(ch, start, end, px).unwrap();
                let vb = b.viewport(ch, start, end, px).unwrap();
                assert_eq!(va.pixels, vb.pixels, "viewport of channel {ch}");
                assert_eq!(va.stats, vb.stats, "probe counts of channel {ch}");
            }
        }
    }
}

pub fn level(snapshot: &Snapshot, ch: u8, p: u64) -> LevelState {
    snapshot.level_at(ch, p).unwrap()
}
