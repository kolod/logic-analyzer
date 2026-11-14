# Contract: Application-Facing API of `la-session` (signatures only)

What the UI, the decoder items and the importers/exporters use. A non-final sketch: bodies,
private helpers and exact lifetimes are decided during implementation; changes follow semver.
Every fallible function returns `la_core::Result<T>`; no function panics on external input; the
non-blocking functions never wait for the disk.

```rust
// ---- values --------------------------------------------------------------------------
/// A section tag: exactly four ASCII letters or digits.
pub struct SectionTag([u8; 4]);
impl SectionTag {
    pub fn new(text: &str) -> la_core::Result<SectionTag>;
    pub fn as_str(&self) -> &str;
    pub fn bytes(&self) -> [u8; 4];
    pub fn is_reserved(&self) -> bool;   // META, CAPT
}

/// An optional section carried through a session. `META` and `CAPT` are reserved.
pub struct Section { pub tag: SectionTag, pub critical: bool, pub payload: Vec<u8> }

/// What to save. Cheap to build: it holds a snapshot, not a copy of the data.
pub struct SessionData { /* description, snapshot, extras (unique tags) */ }
impl SessionData {
    pub fn new(description: CaptureDescription, snapshot: Snapshot) -> SessionData;
    pub fn with_section(self, section: Section) -> la_core::Result<SessionData>; // rejects reserved/duplicate tags
    pub fn from_session(session: &Session) -> SessionData;                       // re-save a loaded session
    pub fn description(&self) -> &CaptureDescription;
    pub fn snapshot(&self) -> Snapshot;
    pub fn extras(&self) -> &[Section];                                            // ascending tags
}

/// A loaded session. Its capture is closed.
pub struct Session { /* description, capture, extras */ }
impl Session {
    pub fn description(&self) -> &CaptureDescription;
    pub fn reader(&self) -> la_storage::CaptureReader;
    pub fn snapshot(&self) -> la_storage::Snapshot;
    pub fn extras(&self) -> &[Section];
    pub fn memory_used(&self) -> u64;
}

pub struct LoadLimits {
    pub storage: la_storage::Limits,   // memory budget etc. (default: storage default)
    pub max_file_bytes: u64,           // default 64 GiB
}
impl Default for LoadLimits {}

// ---- blocking (tests, tools, worker bodies) ----------------------------------------------
pub struct Control { /* cancel flag + progress; cloneable */ }
impl Control {
    pub fn new() -> Control;
    pub fn cancel(&self);
    pub fn is_cancelled(&self) -> bool;
    pub fn progress(&self) -> Progress;
}

pub fn save_blocking(path: &Path, data: &SessionData, control: &Control) -> la_core::Result<SaveReport>;
pub fn load_blocking(path: &Path, limits: &LoadLimits, control: &Control) -> la_core::Result<Session>;

pub struct SaveReport {
    pub bytes_written: u64,
    pub sections: u32,
    /// `false` if the directory entry could not be forced to disk after the rename: the new file
    /// is complete and in place, only its durability across a power failure is not guaranteed
    /// (always `true` on Unix when it works; Windows cannot sync a directory and reports `true`).
    pub directory_synced: bool,
}

// ---- background --------------------------------------------------------------------------
/// Returns at once; the work runs on a supervised worker.
pub fn start_save(path: PathBuf, data: SessionData) -> SaveOperation;
pub fn start_load(path: PathBuf, limits: LoadLimits) -> LoadOperation;

impl SaveOperation {
    pub fn progress(&self) -> Progress;                 // lock-free, never blocks
    pub fn cancel(&self);                               // returns at once
    pub fn outcome(&self) -> Option<Result<SaveReport, Arc<la_core::Error>>>;
    pub fn wait_timeout(&self, timeout: Duration) -> Option<Result<SaveReport, Arc<la_core::Error>>>;
}
impl LoadOperation {
    pub fn progress(&self) -> Progress;
    pub fn cancel(&self);
    pub fn outcome(&self) -> Option<Result<Arc<Session>, Arc<la_core::Error>>>;  // set once
    pub fn wait_timeout(&self, timeout: Duration) -> Option<Result<Arc<Session>, Arc<la_core::Error>>>;
}

pub struct Progress { pub state: OperationState, pub done_bytes: u64, pub total_bytes: u64 }
pub enum OperationState { Running, Succeeded, Failed, Cancelled }
```

## Error codes (`session.*`)

`session.not_a_session`, `session.unsupported_version`, `session.truncated`, `session.damaged`,
`session.inconsistent`, `session.unknown_critical_section`, `session.too_large`,
`session.memory_budget`, `session.invalid_input` (bad path, reserved or duplicate section tag, text too
long, start time out of range), `session.io_read`, `session.io_write` (with the operating system error as
the source and an actionable detail: free space, permission, location), `session.cancelled`,
`session.worker_failed` (a panic contained on a worker), `session.internal`. Storage errors that cannot
occur from a well-formed file are mapped to `session.inconsistent`; none is passed through raw.

## Additions to other crates

```rust
// la-storage
impl Snapshot {
    pub fn initial_high(&self, channel: u8) -> la_core::Result<bool>;     // level of the first stored sample
    pub fn transition_count(&self, channel: u8) -> la_core::Result<u64>;  // O(1)
}
/// Builds a closed Capture from stored transitions; validates every rule of the data model
/// and enforces the memory budget with the recorder's own accounting.
pub struct CaptureRestorer { /* ... */ }
impl CaptureRestorer {
    pub fn new(config: CaptureConfig, length: u64, status: RestoredStatus, gaps: Vec<Gap>)
        -> la_core::Result<CaptureRestorer>;             // checks gaps, flags and the memory estimate
    pub fn begin_channel(&mut self, channel: u8, initial_high: bool, transitions: u64) -> la_core::Result<()>;
    pub fn push_transition(&mut self, position: u64) -> la_core::Result<()>;   // strictly increasing, validated
    /// Checks, before any transition is pushed, that channels with these transition counts fit the
    /// memory budget together with the gap list.
    pub fn check_budget(&self, transitions_per_channel: &[u64]) -> la_core::Result<()>;
    pub fn finish(self) -> la_core::Result<Capture>;     // closed, flags applied
}
pub struct RestoredStatus { pub stopped_by_limit: bool, pub interrupted: bool }

// la-devices
pub struct SavedDescription { /* every field of CaptureDescription as plain values */ }
impl CaptureDescription {
    pub fn to_saved(&self) -> SavedDescription;
    /// For loading a saved session only; validates the parts. There is still no setter.
    pub fn restore(saved: SavedDescription) -> la_core::Result<CaptureDescription>;
}
```

## Behaviour guarantees

- `save_blocking` / `start_save`: validates the inputs, writes a unique temporary file in the target's
  directory, flushes and syncs it, renames it over the target, syncs the directory where the platform
  allows; on any failure or cancellation the temporary file is removed and the target is untouched.
- A snapshot of a capture that is not closed is written with the interrupted flag.
- `load_blocking` / `start_load`: returns a correct `Session` or a typed error; never allocates from a size it
  has not checked against the file and the budget; the result is not returned unless every checksum and every
  consistency rule passed.
- Progress never decreases; `done_bytes == total_bytes` exactly when the state is `Succeeded`.
- Cancellation is honoured at least every 64 KiB; a cancelled operation reports `session.cancelled`.
- A worker panic never reaches the caller as a panic: it becomes `session.worker_failed`.
- Logging contains sizes, counts, codes and durations only.

## Test support (feature `test-support`, not part of the stable API)

`la_session::harness` re-exports the file-system seam (`FileSystem`, `WriteSink`, `RealFileSystem`,
`PathKind`, `DirEntryInfo`), the fault-injecting `FaultFs` / `FaultPlan` / `Op`, and the entry points
that take a file system: `save_with`, `load_with`, `start_save_with`, `start_load_with`.

## Notes fixed during implementation

- A failing directory sync after the rename is not an error: the save succeeded and the report says
  `directory_synced: false`; a failing scan for stale temporary files never fails a save either.
- A damaged section is reported as `session.damaged` when its checksum is wrong even if decoding
  failed first, but only when little of the section remains (the loader does not read a huge file just
  to improve an error message).
- Violations of the capture rules in well-encoded data are `session.inconsistent`; violations of the
  encoding or framing are `session.damaged` (see `file-format.md`).
