# Contract: Public Rust API of `la-storage` (signatures only)

A non-final sketch of the public surface other crates may rely on. Bodies, private
helpers and exact lifetimes are decided during implementation; changes follow semver.
All fallible functions return `la_core::Result<T>`; no function panics on external
input.

```rust
// ---- configuration -------------------------------------------------------------
pub const MAX_CHANNELS: u8 = 32;

pub struct Limits { pub memory_budget_bytes: u64, pub max_chunk_samples: u32, pub max_pixels: u32 }
impl Default for Limits {}

pub struct CaptureConfig { pub channels: u8, pub sample_rate_hz: u64, pub limits: Limits }

// ---- producer ------------------------------------------------------------------
pub struct Capture { /* single owner */ }
impl Capture {
    pub fn new(config: CaptureConfig) -> Result<Capture>;
    /// Handle for readers; cloneable, Send + Sync.
    pub fn reader(&self) -> CaptureReader;
    /// `start` must equal the current length. Bit `i` of every word is channel `i`.
    /// Atomic: all of the chunk is accepted or none of it.
    pub fn append(&mut self, start: u64, samples: &[u32]) -> Result<()>;
    /// Records `length` lost positions at the current end; flags the capture incomplete.
    pub fn record_gap(&mut self, length: u64) -> Result<()>;
    /// Ends the capture; further appends and gaps fail with `storage.closed`.
    pub fn close(&mut self) -> Result<CaptureStatus>;
    pub fn length(&self) -> u64;
    pub fn memory_used(&self) -> u64;
}

// ---- readers -------------------------------------------------------------------
#[derive(Clone)] pub struct CaptureReader { /* ... */ }
impl CaptureReader { pub fn snapshot(&self) -> Snapshot; }

pub struct Snapshot { /* immutable, cheap to clone */ }
impl Snapshot {
    pub fn length(&self) -> u64;
    pub fn sample_rate_hz(&self) -> u64;
    pub fn channels(&self) -> u8;
    pub fn status(&self) -> CaptureStatus;
    pub fn gaps(&self) -> &[Gap];

    pub fn level_at(&self, channel: u8, position: u64) -> Result<LevelState>;
    pub fn transitions(&self, channel: u8, start: u64, end: u64) -> Result<TransitionIter<'_>>;
    pub fn viewport(&self, channel: u8, start: u64, end: u64, pixels: u32) -> Result<ViewportResult>;
}

// ---- values --------------------------------------------------------------------
pub enum LevelState { Low, High, Missing }
pub struct Edge { pub position: u64, pub rising: bool }
pub enum Activity { None, One(Edge), Many { count: u32 } }
pub struct PixelSummary { pub start_level: LevelState, pub end_level: LevelState,
                          pub activity: Activity, pub missing_data: bool, pub beyond_end: bool }
pub struct ViewportResult { pub pixels: Vec<PixelSummary>, pub stats: QueryStats }
pub struct QueryStats { pub probes: u64 }
pub struct Gap { pub start: u64, pub length: u64 }
pub struct CaptureStatus { pub closed: bool, pub complete: bool,
                           pub lost_positions: u64, pub stopped_by_limit: bool }
```

## Test support (feature `test-support`, not part of the stable API)

```rust
/// Builds a capture whose channels reference one block payload many times, so a
/// 10^9-transition capture needs only kilobytes (used by complexity tests/benches).
pub fn synthetic_capture(channels: u8, transitions_per_channel: u64) -> Result<Snapshot>;
```

## Error codes

`storage.invalid_channel_count`, `storage.invalid_sample_rate`,
`storage.invalid_limits`, `storage.chunk_out_of_order`, `storage.chunk_too_large`,
`storage.chunk_empty`, `storage.position_overflow`, `storage.memory_limit`,
`storage.closed`, `storage.invalid_window`, `storage.invalid_pixels`,
`storage.channel_out_of_range`, `storage.invalid_gap`.
