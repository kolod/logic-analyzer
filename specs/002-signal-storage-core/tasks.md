---

description: "Task list for Signal Storage Core"
---

# Tasks: Signal Storage Core

**Input**: Design documents from `/specs/002-signal-storage-core/`

**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/, quickstart.md

**Tests**: REQUIRED. Constitution Principle III (test-first) applies: in every story phase
the test tasks come first and MUST fail (typically they do not compile yet) before the
implementation tasks start. If a test passes immediately because earlier work already
covers it, run the mutation check described in that phase before relying on it.

**Organization**: tasks are grouped by user story. Stories are numbered as in spec.md
(US1–US7); phases are ordered by priority (all P1 stories first, then the P2 ones).

## Format: `[ID] [P?] [Story] Description`

- **[P]**: can run in parallel (different files, no dependency on an incomplete task)
- **[Story]**: user story from spec.md
- All paths are relative to the repository root; the new crate is `crates/storage`
  (package `la-storage`)

## Path Conventions

Source in `crates/storage/src/`, integration tests in `crates/storage/tests/`,
benchmarks in `crates/storage/benches/`. Semantics are defined in
`specs/002-signal-storage-core/contracts/viewport-semantics.md` (referred to below as
"the semantics contract") and the API in `contracts/storage-api.md`.

## Lint reminder (applies to every Rust task)

The workspace denies `unwrap`, `expect`, indexing (`v[i]`), unchecked arithmetic,
`panic!` and `unsafe` in production code. Use `get`, `checked_*`, `saturating_*`, `u128`
intermediates and `?`. Test code may relax this with the file-level
`#![allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing,
clippy::arithmetic_side_effects, clippy::panic)]` used by the item-001 tests. Run
`cargo clippy --workspace --all-targets --all-features -- -D warnings` at the end of
every phase.

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: the new crate and its wiring

- [X] T001 Create `crates/storage/Cargo.toml` (package `la-storage`, `edition/rust-version/license/publish` inherited from the workspace, `[lints] workspace = true`, dependencies `la-core = { path = "../core" }` and `tracing` with the same version as `crates/diagnostics/Cargo.toml`, feature `test-support = []`, dev-dependency `la-storage = { path = ".", features = ["test-support"] }`, and `[[bench]]` entries `viewport` and `ingest` with `harness = false`) and `crates/storage/src/lib.rs` with a crate-level English doc comment describing the crate and the "never log signal data" rule
- [X] T002 Run `cargo build --workspace`, `cargo clippy --workspace --all-targets --all-features -- -D warnings` and `cargo deny --all-features check` to confirm the empty crate adds no dependency problems; `Cargo.lock` gains only the new workspace member

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: shared types, validation, arithmetic and the test reference model

**⚠️ CRITICAL**: no user story work can begin until this phase is complete

### Tests first

- [X] T003 [P] Write unit tests (in a `#[cfg(test)]` module with the arithmetic/unwrap relaxation) in `crates/storage/src/math.rs` for: `checked_position_add(a, b)` (overflow near `u64::MAX` returns `storage.position_overflow`), `pixel_boundary(start, len, i, w)` (`a_i = start + floor(i*len/w)` computed in `u128`, exact for `len`/`w` up to `u64::MAX`/65,536, `a_0 == start`, `a_w == start + len`, monotonic non-decreasing, zero-width columns when `len < w`), and `chunk_bytes(len, channels)` (no overflow)
- [X] T004 [P] Write tests in `crates/storage/tests/config.rs` for `CaptureConfig::validate` / `Capture::new` rejection: 0 and 33 channels (`storage.invalid_channel_count`), sample rate 0 (`storage.invalid_sample_rate`), `Limits` out of range — budget < 1 MiB, `max_chunk_samples` 0 or > 1,048,576, `max_pixels` 0 or > 65,536 (`storage.invalid_limits`) — and that `Limits::default()` equals 1 GiB / 65,536 / 16,384
- [X] T005 [P] Create the shared test helpers `crates/storage/tests/common/mod.rs` (with the lint relaxation and `dead_code` allowed): a deterministic xorshift generator, a naive `ReferenceCapture` that stores every sample word in a `Vec<u32>` and answers `level(channel, pos)`, `transitions(channel, start, end)` and brute-force pixel summaries per the semantics contract, pattern generators (constant, alternating, random with a given density, long runs, isolated single-sample glitches, bursts), and a helper that splits a sample stream into chunks of given sizes

### Implementation

- [X] T006 [P] Implement `crates/storage/src/types.rs`: `LevelState {Low, High, Missing}`, `Edge {position, rising}`, `Activity {None, One(Edge), Many{count: u32}}`, `PixelSummary`, `ViewportResult`, `QueryStats {probes: u64}`, `Gap {start, length}`, `CaptureStatus {closed, complete, lost_positions, stopped_by_limit}` exactly as in data-model.md, deriving `Debug, Clone, Copy/PartialEq/Eq` where possible, with English rustdoc
- [X] T007 [P] Implement `crates/storage/src/error.rs`: the stable error code constants from research R8 (`storage.invalid_channel_count`, …, `storage.invalid_gap`) as `pub const` `&str`s and small constructor functions returning `la_core::Error` with the right `ErrorKind` (`InvalidInput`, `Resource` for `storage.memory_limit`) and an English `detail`
- [X] T008 Implement `crates/storage/src/config.rs`: `MAX_CHANNELS = 32`, `Limits` (+ `Default`), `CaptureConfig`, and `CaptureConfig::validate(&self) -> la_core::Result<()>` per data-model.md (depends on T007)
- [X] T009 Implement `crates/storage/src/math.rs` to satisfy T003 (depends on T007): checked helpers, `u128` pixel boundary function, worst-case chunk byte estimate
- [X] T010 Export the public surface from `crates/storage/src/lib.rs` (`mod` declarations for types, error, config, math; `pub use` for the types, `CaptureConfig`, `Limits`, `MAX_CHANNELS`) and make T003 and T004 compile; run `cargo test -p la-storage` for the unit tests that exist so far (T004 still fails until `Capture::new` exists in Phase 3)

**Checkpoint**: foundation ready — user story work can begin

---

## Phase 3: User Story 1 - Store a capture and read it back exactly (Priority: P1) 🎯 MVP

**Goal**: append chunks for many channels and read back exact levels and transitions.

**Independent Test**: `cargo test -p la-storage --test exact_readback --test config` (quickstart step 1).

### Tests for User Story 1 (write FIRST, ensure they FAIL)

- [X] T011 [P] [US1] Write `crates/storage/tests/exact_readback.rs`: for each generated pattern (constant, alternating, random at 1%/10%/50% density, long runs of up to 5 million samples, runs separated by single transitions) and several chunk-size splits (1, 7, 1,024, 4,096, 65,536 and random sizes), append through the public API, close, then compare against `ReferenceCapture`: `level_at` at every position (or a dense random sample for long patterns), `transitions(channel, start, end)` for random ranges (exact positions and directions, in order, none missing or duplicated), transitions that fall exactly on chunk boundaries and on block boundaries (1,024-transition blocks), 1 and 32 channels, several channels changing at the same position, an empty capture (`length() == 0`, queries give defined empty results) and a one-sample capture
- [X] T012 [P] [US1] Add to `crates/storage/tests/exact_readback.rs` a memory-proportionality test: a capture of 100 million constant samples followed by one transition uses memory (`Capture::memory_used`) below 4 KiB per channel, independent of duration (FR-005), appended in maximum-size chunks of constant words

### Implementation for User Story 1

- [X] T013 [P] [US1] Implement `crates/storage/src/block.rs`: immutable `Block {base: u64, first_rank: u64, offsets: Box<[u32]>}` with `len`, `position(i)`, `rank_of(position) -> usize` (binary search over offsets, counting probes), byte size (`4 × len + 32`); and `BlockBuilder` that collects strictly increasing positions, closes the block at 1,024 transitions or when the next offset would not fit `u32` (starting a new block with a new base), returning sealed blocks
- [X] T014 [P] [US1] Implement `crates/storage/src/segment.rs`: `Segment` = fixed array of 256 `OnceLock<Arc<Block>>` slots; a `SegmentList` that appends sealed blocks (creating a new segment when full) and gives random access by block index; blocks are written once before they become visible
- [X] T015 [US1] Implement `crates/storage/src/channel.rs`: `ChannelData` (initial level, `Arc` segment list, visible sealed block count, copy of the open tail) with `rank(position) -> u64` (number of transitions with position `<= x`: binary search over blocks then offsets, then tail), `level_at`, and a transition iterator over a range; probe counting hooks stubbed to return the count (depends on T013, T014)
- [X] T016 [P] [US1] Implement `crates/storage/src/ingest.rs`: turn a `&[u32]` chunk plus the previous word into per-channel transition position lists using `word ^ previous` (skip zero words, iterate set bits with `trailing_zeros`, mask to the configured channels), returning the new transitions without touching shared state (so a failed chunk can be discarded atomically)
- [X] T017 [US1] Implement `crates/storage/src/published.rs`: `Published` (immutable: length, per-channel `ChannelData`, gaps, status, used bytes), `CaptureReader` (`Clone + Send + Sync`, holds `Arc<RwLock<Arc<Published>>>`, lock held only to clone/swap the `Arc`, poisoned locks are recovered with `PoisonError::into_inner`), and `Snapshot` with `length`, `sample_rate_hz`, `channels`, `status` (depends on T015)
- [X] T018 [US1] Implement `crates/storage/src/capture.rs`: `Capture::new` (uses `CaptureConfig::validate`, logs a `capture created` `tracing` event with metadata only), `reader()`, `append(start, samples)` (checks `start == length` → `storage.chunk_out_of_order`, empty → `storage.chunk_empty`, over `max_chunk_samples` → `storage.chunk_too_large`, runs ingest, seals blocks, publishes a new `Published` by swapping the `Arc`), `close()` returning `CaptureStatus`, `length()`, `memory_used()` (exact sum per R6; the limit check itself is Phase 6) (depends on T008, T016, T017)
- [X] T019 [US1] Implement `Snapshot::level_at` and `Snapshot::transitions` (iterator, O(log N + k)) in `crates/storage/src/query.rs` per the semantics contract (channel out of range → `storage.channel_out_of_range`; windows with `start >= end` → `storage.invalid_window`) and export `Capture`, `CaptureReader`, `Snapshot`, `TransitionIter` from `crates/storage/src/lib.rs`
- [X] T020 [US1] Make T004, T011 and T012 pass; run `cargo clippy --workspace --all-targets --all-features -- -D warnings` and fix without weakening lints

**Checkpoint**: User Story 1 works and is verifiable on its own (exact storage and readback)

---

## Phase 4: User Story 2 - Fast viewport queries on very large captures (Priority: P1)

**Goal**: viewport queries whose cost is polylogarithmic in the capture size and
independent of the number of transitions inside the window.

**Independent Test**: `cargo test -p la-storage --test viewport --test complexity` (quickstart steps 2–3) plus the benchmark runs of quickstart step 6.

### Tests for User Story 2 (write FIRST, ensure they FAIL)

- [X] T021 [P] [US2] Write `crates/storage/tests/viewport.rs`: for random captures, windows and pixel counts (including `W = 1`, `W = max_pixels`, `L_w < W`, windows starting at 0, ending at the capture length and extending beyond it), compare every `PixelSummary` field with the brute-force result of `ReferenceCapture` per the semantics contract, and assert the invariants: `end_level(i) == start_level(i+1)`, `start XOR (count mod 2) == end` without gaps inside the data, columns partition the transitions of `(a_0, a_W]` (sum of counts equals the transitions in that range), zero-width columns have `None` and equal levels, a transition exactly at `a_0` is not counted and one exactly at `a_W` is counted in the last column, `beyond_end` for columns starting at or after the length
- [X] T022 [P] [US2] Write `crates/storage/tests/complexity.rs` (merge-blocking, deterministic): using `la_storage::testing::synthetic_capture` (feature `test-support`), build captures of 10^3, 10^4, …, 10^9 transitions per channel and run the same 2,000-pixel window over the whole capture; assert (a) `probes(10^9) / probes(10^6) < 3.0`, (b) probes never exceed `c · (log2(N) + W · (1 + log2(1 + N_w / W)))` with a documented constant `c`, (c) doubling `W` roughly doubles the probes (ratio between 1.5 and 2.5), (d) a window covering 1 million versus 1 billion transitions at the same pixel count costs the same order of probes (ratio < 3), (e) the number of probes never grows linearly: `probes(N) < N / 1000` for N ≥ 10^8 (below that the fixed per-pixel cost dominates)
- [X] T023 [P] [US2] Add to `crates/storage/tests/complexity.rs` an alternating-signal worst case (a transition at every sample for 10^6 samples, real data not synthetic) asserting the same bound, and a test that a viewport query over a real 10^7-transition capture returns the same pixel results as the brute-force reference on a sampled subset of columns

### Implementation for User Story 2

- [X] T024 [US2] Add the finger-search cursor to `crates/storage/src/channel.rs`: `RankCursor` that resolves a non-decreasing sequence of boundaries to ranks, starting with a binary search over the block index for the first boundary (O(log N)) and then galloping from the previous block for each next boundary (O(1) in the same/next block, O(log skipped blocks) otherwise); every comparison increments a probe counter (replace the stubbed hooks from T015)
- [X] T025 [US2] Implement `Snapshot::viewport` in `crates/storage/src/query.rs`: validate window (`start < end` else `storage.invalid_window`), pixels in `1..=max_pixels` (else `storage.invalid_pixels`) and channel; compute the `W + 1` boundaries with `math::pixel_boundary`; resolve ranks with `RankCursor`; build each `PixelSummary` (levels from parity of the rank at `a_i` and `a_{i+1}`, `count` from the rank difference, `Activity` None/One/Many with `One` carrying the exact edge found by one offset lookup, `Many` saturating at `u32::MAX`, `beyond_end`, `missing_data` left `false` until Phase 9 adds gaps); return `ViewportResult` with `QueryStats` (depends on T024)
- [X] T026 [P] [US2] Implement `crates/storage/src/testing.rs` (compiled only with feature `test-support`): `synthetic_capture(channels, transitions_per_channel) -> la_core::Result<Snapshot>` that builds channels whose segments reference one shared block payload many times with different `base` and `first_rank` values (so 10^9 transitions need tens of megabytes), plus a helper that builds a capture from a closure for tests; document in rustdoc that it is a test helper whose timings are cache-friendly and optimistic
- [X] T027 [US2] Export `synthetic_capture` from `crates/storage/src/lib.rs` behind `#[cfg(feature = "test-support")]`; make T021–T023 pass; run clippy with `-D warnings`
- [X] T028 [US2] Create the timing harness `crates/storage/benches/viewport.rs` (`harness = false`, `std::time::Instant`, `std::hint::black_box`): measures a 2,000-pixel × 16-channel viewport on synthetic captures of 10^6 … 10^9 transitions per channel and on real captures of 10^6 … 10^7 transitions, prints and writes JSON to `target/la-storage-bench/viewport.json` (machine description, per-size median and p95 in microseconds, probes), and compares against `crates/storage/benches/baseline.json` if present (fail when more than 5× slower than baseline or when the growth ratio of T022 is violated)
- [X] T029 [US2] Run `cargo bench -p la-storage --bench viewport` on the maintainer machine and commit the first `crates/storage/benches/baseline.json` (CPU model, cores, RAM, OS, date, the measured numbers); check SC-002 (< 20 ms for 2,000 px × 16 channels at 10^9 synthetic transitions) and SC-003 and note the result in `specs/002-signal-storage-core/checklists/verification.md`

**Checkpoint**: User Stories 1 and 2 work independently; the complexity class is gated by `cargo test`

---

## Phase 5: User Story 3 - Zoomed-out views never hide glitches (Priority: P1)

**Goal**: guarantee that every pixel column containing transitions is distinguishable from an empty one at every zoom level.

**Independent Test**: `cargo test -p la-storage --test glitches` (quickstart step 2).

### Tests for User Story 3

- [ ] T030 [P] [US3] Write `crates/storage/tests/glitches.rs`: (a) a long flat signal (up to 10 million samples) with one single-sample glitch at a random position: at pixel counts 1, 2, 3, 100, 2,000 and at zoom levels from 1 sample per pixel to the whole capture, exactly the column(s) whose range `(a_i, a_{i+1}]` contains the glitch's two transitions report activity (`One` if the two transitions fall in different columns, `Many { count: 2 }` if in the same one) and every other column reports `None`; (b) bursts of 3, 10 and 1,000 closely spaced transitions report `Many` with the exact count; (c) random sparse and dense signals: for every column `Activity::None` iff the brute-force count is 0, `One` iff 1 (with the exact edge), `Many` iff ≥ 2 (with the exact count); (d) column-boundary continuity from the semantics contract on every generated window
- [ ] T031 [P] [US3] Add to `crates/storage/tests/glitches.rs` a saturation test using `synthetic_capture`: a single column covering more than `u32::MAX` transitions (for example 5 billion) reports `Activity::Many { count: u32::MAX }` and never wraps to a small number; the start and end levels of that column still come from the exact rank parity (they are not derived from the saturated count), so the test asserts `end_level` against the true parity of the transition count and states in a comment that the parity identity of the semantics contract applies to the true count, not to the saturated one
- [ ] T032 [US3] Mutation check (do not commit the mutations): temporarily change `Snapshot::viewport` so a column's count ignores the last transition, then so that `Many` is returned for count 1, then so that `None` is returned for count 1; confirm `glitches.rs` fails each time; restore the code

### Implementation for User Story 3

- [ ] T033 [US3] Fix any defect found by T030–T031 in `crates/storage/src/query.rs` and `crates/storage/src/channel.rs`; add the documented maximum (`u32::MAX`) as a public constant `MAX_REPORTED_TRANSITIONS` in `crates/storage/src/types.rs` with rustdoc explaining saturation
- [ ] T034 [US3] Document in rustdoc of `Activity` and `PixelSummary` how a consumer draws each variant (flat line, single clean edge, dense-activity mark) without implementing any drawing; run clippy with `-D warnings`

**Checkpoint**: User Stories 1–3 work independently

---

## Phase 6: User Story 4 - Bounded, predictable memory (Priority: P1)

**Goal**: an explicit memory budget that rejects appends atomically and keeps stored data intact.

**Independent Test**: `cargo test -p la-storage --test limits` (quickstart step 4).

### Tests for User Story 4 (write FIRST, ensure they FAIL)

- [ ] T035 [P] [US4] Write `crates/storage/tests/limits.rs`: with a 1 MiB budget append an alternating signal until `append` fails; assert the error code is `storage.memory_limit` and kind `Resource`; the rejected chunk left no partial data (`length()` and every readback equal the state before the failed call, verified against `ReferenceCapture` of the accepted chunks); `memory_used()` never exceeds the budget at any point; a chunk that exactly fills the remaining budget is accepted and one that exceeds it by a single transition is rejected; after the limit all stored data reads back exactly and viewport queries still work; `status().stopped_by_limit` is true and `complete` is false after `close()`; further appends keep failing with `storage.memory_limit` (not `storage.closed`) until `close()`
- [ ] T036 [P] [US4] Add to `crates/storage/tests/limits.rs`: the worst case (a level change on all 32 channels at every sample) consumes at most `32 × 4.03` bytes per sample as documented (assert `memory_used() / samples <= 130`), sparse signals consume far less, and temporary memory per chunk is bounded (a maximum-size alternating chunk of 65,536 samples × 32 channels is processed and accounted without exceeding `max_chunk_samples × channels × 4` bytes of temporary allocation — verify through an allocation counter or by checking the documented bound on the ingest output size)

### Implementation for User Story 4

- [ ] T037 [US4] Implement exact memory accounting and the atomic limit check in `crates/storage/src/capture.rs`: compute the bytes of the new blocks and tail changes produced by ingest before publishing, reject with `storage.memory_limit` when `used + new > budget` (discarding the prepared blocks, publishing nothing), set the `stopped_by_limit` flag in the next published status, and keep rejecting later appends with the same error until `close()`
- [ ] T038 [US4] Log a `memory limit reached` `tracing` warning (used bytes, budget, rejected sample count; no signal data) once when the limit is first hit, and a `capture closed` event with the final status in `close()`, in `crates/storage/src/capture.rs`
- [ ] T039 [US4] Document the worst-case memory per sample (4 bytes per channel-sample plus 32 bytes per 1,024-transition block) and the temporary per-chunk bound in the rustdoc of `Limits` and `Capture::append` in `crates/storage/src/config.rs` and `crates/storage/src/capture.rs`
- [ ] T040 [US4] Make T035–T036 pass; run clippy with `-D warnings`

**Checkpoint**: User Stories 1–4 work independently

---

## Phase 7: User Story 7 - Hostile or malformed input never crashes the store (Priority: P1)

**Goal**: every invalid or extreme call returns a typed error or a defined result and changes nothing.

**Independent Test**: `cargo test -p la-storage --test hostile_input` (quickstart step 4).

### Tests for User Story 7 (write FIRST)

- [ ] T041 [P] [US7] Write `crates/storage/tests/hostile_input.rs`: a table-driven set of named cases with the expected error code — out-of-order start (earlier, later, equal-length mismatch), overlapping chunk, empty chunk, chunk of `max_chunk_samples + 1`, chunk of `u32::MAX as usize`-claimed size is not constructible so use a slice of the limit + 1, words with bits above the configured channel count (masked and documented, not an error), channel index equal to and above the channel count, window with `start == end` and `start > end`, `pixels == 0` and `pixels == max_pixels + 1` and `u32::MAX`, a position at `u64::MAX` and `u64::MAX - 1` in `level_at`, windows ending at `u64::MAX`, append when the capture length is near `u64::MAX` (use the test-support to set a huge starting length or a gap of `u64::MAX - 5`), creation with all invalid configurations of T004
- [ ] T042 [P] [US7] Add to `crates/storage/tests/hostile_input.rs` a generated corpus of at least 10,000 random calls (random operation, random parameters including extreme values) executed against a live capture under `std::panic::catch_unwind`; assert that no call panics, that every failing call returns a `la_core::Error` with a `storage.*` code, and that after every failing call a fingerprint of the capture (length, status, a hash of level and transition readbacks for all channels over a fixed set of windows) equals the fingerprint before the call (FR-026); keep the corpus deterministic (fixed seed) and print the seed on failure

### Implementation for User Story 7

- [ ] T043 [US7] Fix every panic or state change found by T041–T042 in `crates/storage/src/{capture,query,channel,math,ingest}.rs`, using checked arithmetic and early validation before any allocation sized from caller input (FR-018); no `#[allow]` for the denied lints, and no `unwrap`/`expect` in production code
- [ ] T044 [US7] Verify FR-018 explicitly: in `crates/storage/src/capture.rs` and `query.rs`, confirm that no `Vec::with_capacity` or similar is sized from `samples.len()`, `pixels` or other caller-controlled values before validation against `Limits`, and add comments at the validation points; run clippy with `-D warnings`

**Checkpoint**: all P1 stories (US1–US4, US7) work independently

---

## Phase 8: User Story 5 - Read while a capture is in progress (Priority: P2)

**Goal**: readers see consistent snapshots while the producer appends; neither blocks the other.

**Independent Test**: `cargo test -p la-storage --test concurrency` (quickstart step 5).

### Tests for User Story 5 (write FIRST)

- [ ] T045 [P] [US5] Write `crates/storage/tests/concurrency.rs`: a producer thread appends a deterministic stream in chunks of random sizes while 4 reader threads loop taking snapshots and running `length`, `level_at`, `transitions` and `viewport`; every result must equal the `ReferenceCapture` prefix of exactly `snapshot.length()` positions where that length is a sum of complete chunk sizes (record chunk boundaries in a shared log), 1,000,000 sampled queries in total in release-like settings (scale down under `debug_assertions` with a documented constant); results already returned stay unchanged after more appends (compare a held `Snapshot` before and after further appends)
- [ ] T046 [P] [US5] Add to `crates/storage/tests/concurrency.rs`: a measurement that the producer's worst-case `append` duration with 4 busy readers is within a documented maximum (assert a generous bound relative to the same run without readers, for example worst case ≤ 20× the no-reader median plus 5 ms, recorded in the test as a named constant); a reader thread that panics while holding a `Snapshot` (using `la_diagnostics::spawn_worker` from item 001, add `la-diagnostics` as a dev-dependency) does not prevent further appends or corrupt later readbacks; a lock poisoned by a panicking thread is recovered

### Implementation for User Story 5

- [ ] T047 [US5] Ensure in `crates/storage/src/published.rs` and `capture.rs` that the publish step holds the `RwLock` only to swap the `Arc<Published>`, all block building happens before the swap, `Snapshot` clones are O(1), and every lock acquisition recovers from poisoning with `PoisonError::into_inner`; fix any defect found by T045–T046
- [ ] T048 [US5] Document the concurrency model and the documented producer-delay bound in the rustdoc of `Capture`, `CaptureReader` and `Snapshot`; make T045–T046 pass; run clippy with `-D warnings`

**Checkpoint**: live reading works independently

---

## Phase 9: User Story 6 - Incomplete captures are flagged (Priority: P2)

**Goal**: lost samples are recorded as gaps; the capture is never reported complete.

**Independent Test**: `cargo test -p la-storage --test gaps` (quickstart step 4).

### Tests for User Story 6 (write FIRST, ensure they FAIL)

- [ ] T049 [P] [US6] Write `crates/storage/tests/gaps.rs`: `record_gap(n)` advances `length()` by `n`, flags the capture incomplete, `gaps()` reports exact `start` and `length`, adjacent gaps are merged, a gap at the very start (before any sample), at the very end and next to another gap behave as specified; `level_at` inside a gap returns `Missing`; the first sample after a gap is not compared with the sample before it and a resync transition is stored at the gap end if the levels differ (parity formula still holds on both sides); a viewport covering a gap sets `missing_data` on exactly the columns satisfying the semantics contract and reports `Missing` levels at positions inside the gap; `close()` after a gap gives `complete == false`, `lost_positions` equal to the sum of gaps; a capture without gaps closed normally gives `complete == true`; a capture that is not closed is never `complete`; `record_gap` and `append` after `close()` return `storage.closed`; `record_gap(0)` and a gap that would overflow `u64` return `storage.invalid_gap` / `storage.position_overflow`
- [ ] T050 [P] [US6] Add to `crates/storage/tests/gaps.rs`: random interleavings of chunks and gaps compared with `ReferenceCapture` extended with `Option<u32>` samples (missing positions) for `level_at`, `transitions` and every viewport field; a capture stopped by the memory limit that also had gaps reports both reasons in its status

### Implementation for User Story 6

- [ ] T051 [US6] Implement `Capture::record_gap` in `crates/storage/src/capture.rs`: validate (`length >= 1`, no overflow), advance the length, merge with an adjacent gap, clear `previous_word` and set `pending_resync` so ingest emits the resync transitions on the next chunk, publish a new `Published` with the gap list and status, and log a `gap recorded` event (position and length only)
- [ ] T052 [US6] Update `crates/storage/src/ingest.rs` to emit resync transitions at the gap end position when the first sample after a gap differs from the last level before it (the last level per channel is remembered across the gap), and `crates/storage/src/channel.rs` / `query.rs` so `level_at` returns `Missing` inside gaps and `viewport` computes `missing_data` and `Missing` levels per the semantics contract
- [ ] T053 [US6] Update `Capture::close` and the published status in `crates/storage/src/capture.rs` so `complete = closed && lost_positions == 0 && !stopped_by_limit`; make T049–T050 pass; run clippy with `-D warnings`

**Checkpoint**: all user stories (US1–US7) are independently functional

---

## Phase 10: Polish & Cross-Cutting Concerns

**Purpose**: ingestion benchmark, scheduled benchmark workflow, documentation, constitution gates

- [ ] T054 [P] Create `crates/storage/benches/ingest.rs` (`harness = false`): ingestion throughput for 1, 8, 16 and 32 channels with sparse, medium and worst-case (alternating) signals, in chunks of 65,536 samples; reports million samples per second and writes `target/la-storage-bench/ingest.json`; extend `crates/storage/benches/baseline.json` with the maintainer-machine numbers and verify SC-006 (100 MHz × 32 channels sustained) for realistic (sparse to medium) signals, noting honestly that the alternating worst case is memory-bound and expected to be slower
- [ ] T055 [P] Add `.github/workflows/bench.yml` (triggers `workflow_dispatch` and a weekly `schedule`; Debian container and Windows as in `ci.yml`, actions pinned by SHA, the same `setup-rust` composite action) running both benchmarks and uploading the JSON files as artifacts; it is NOT added to `scripts/protection.json` (informational, research R9); document it in `specs/001-workspace-ci-foundation/contracts/ci-checks.md` under non-blocking checks
- [ ] T056 [P] Review the public API against `contracts/storage-api.md` and update the contract where the implementation differs (names, signatures, extra error codes), and add rustdoc for every public item in `crates/storage/src/`; run `cargo doc -p la-storage --no-deps` with `RUSTDOCFLAGS="-D warnings"`
- [ ] T057 [P] Data-safety review (constitution Principle I): search `crates/storage/src` for `unwrap(`, `expect(`, `panic!`, `todo!`, `unimplemented!`, `unreachable!`, `unsafe` and `#[allow`; none may remain in production code; record the result in `specs/002-signal-storage-core/checklists/verification.md`
- [ ] T058 [P] Record the remaining constitution gates in the same checklist: localization (no UI strings; stable error codes; English docs), third-party code (no new dependencies — show `cargo deny list` before/after; nothing reused from PulseView/sigrok/Saleae), Principle VIII (the achieved bound and the pending project-owner decision), platforms (tests pass on Windows and in the Debian container, MSRV 1.85 job passes)
- [ ] T059 Run the whole quickstart (`specs/002-signal-storage-core/quickstart.md`, steps 1–7) on Windows 10/11 and rely on CI for Debian 13 and the MSRV job; fix any gap and update the quickstart where reality differs
- [ ] T060 Update `roadmap.md` (mark item 002 implemented) and `README.md` status line; if the project owner approves the clarification of Principle VIII, amend `.specify/memory/constitution.md` to a PATCH version (1.4.1) with the wording "O(log N + W·log(1 + N_w/W)), never O(N)" and update the Principle VIII gate text in `.specify/templates/plan-template.md`; otherwise leave the Complexity Tracking entry as the record
- [ ] T061 Final local gate: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets --all-features -- -D warnings`, `cargo test --workspace --all-features`, `cargo deny --all-features check`, `cargo +1.85 test --workspace`, then push the branch, open a pull request and wait for all 10 required checks on both operating systems

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: none
- **Foundational (Phase 2)**: depends on Setup — blocks all stories
- **US1 (Phase 3)**: depends on Foundational; it provides the store, publication and readback that every later story builds on
- **US2 (Phase 4)**: depends on US1 (needs `ChannelData`, `Snapshot`); T026 `synthetic_capture` needs the segment/block types from US1
- **US3 (Phase 5)**: depends on US2 (it tests and hardens `viewport`)
- **US4 (Phase 6)**: depends on US1 (extends `Capture::append`); independent of US2/US3
- **US7 (Phase 7)**: depends on US1; best after US4 and US2 so it covers limits and viewports; needs `record_gap` only for the gap-overflow case (that single case is moved to US6 tests T049 if US6 is not yet done)
- **US5 (Phase 8)**: depends on US1 (publication) and US2 (queries to run while appending)
- **US6 (Phase 9)**: depends on US1 and US2 (it changes ingest and `viewport`)
- **Polish (Phase 10)**: depends on all stories

### Within Each User Story

- Tests first and failing before implementation
- Data types/blocks before channel logic, channel logic before capture/query
- Run clippy with `-D warnings` at the end of every phase

### Parallel Opportunities

- Phase 2 tests T003, T004, T005 in parallel; T006, T007 in parallel
- US1 tests T011, T012; implementation T013, T014, T016 in parallel
- US2 tests T021, T022, T023 in parallel; T026 in parallel with T024
- After US1: US4 and US2 can proceed in parallel (different files, except both touch `capture.rs`/`query.rs` only at different functions — coordinate)
- Polish T054–T058 in parallel

---

## Parallel Example: User Story 2

```text
# Launch all tests for User Story 2 together:
Task: "viewport semantics tests in crates/storage/tests/viewport.rs"
Task: "complexity probe-count test in crates/storage/tests/complexity.rs"
Task: "alternating worst-case and real-capture tests in crates/storage/tests/complexity.rs"

# Then independent implementation pieces:
Task: "RankCursor finger search in crates/storage/src/channel.rs"
Task: "synthetic_capture in crates/storage/src/testing.rs"
```

---

## Implementation Strategy

### MVP First (US1)

1. Phase 1 Setup and Phase 2 Foundational
2. Phase 3 US1: exact storage and readback
3. **STOP and VALIDATE**: `cargo test -p la-storage --test exact_readback`

### Incremental Delivery (P1 stories first)

1. US1 → exact store (MVP)
2. US2 → fast viewport and the merge-blocking complexity test
3. US3 → glitch guarantee
4. US4 → memory budget
5. US7 → hostile input corpus
6. US5 → live readers; US6 → gaps (P2)
7. Polish → ingestion benchmark, scheduled benchmark workflow, gates

### Notes

- [P] tasks = different files, no dependencies
- Commit after each task or logical group with Conventional Commits and the scope
  `storage` (for example `feat(storage): add transition blocks`)
- The Principle VIII wording decision (T060) belongs to the project owner; the plan's
  Complexity Tracking records it
- Avoid weakening a lint to make a test pass; `unsafe` is not needed anywhere in this feature
