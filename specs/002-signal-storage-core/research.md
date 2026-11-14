# Research: Signal Storage Core

All Technical Context unknowns are resolved below.

## R1. Signal representation

- **Decision**: store each channel as its **transitions**: the sorted positions at
  which the level changes, plus the level of the first sample. The level at a position
  is the first level flipped once per transition at or before it (parity of the
  transition rank). Positions are grouped into immutable **blocks** of at most 1,024
  transitions; a block stores a 64-bit base position and 32-bit offsets from it (a
  block is closed early if an offset would overflow 32 bits, so very long flat
  stretches simply start a new block). Each block also records the rank (number of
  transitions before it) of its first transition.
- **Rationale**: cost depends on the number of changes, not the duration (FR-005);
  4 bytes per transition keeps the worst case (a change at every sample) at about
  4.02 bytes per channel-sample including headers; offsets inside a block can be
  binary-searched, so one rank lookup costs O(log N).
- **Alternatives considered**: raw bit-packed samples (1 bit per channel-sample: cheap
  for dense signals but proportional to duration, and range counts would need
  popcount scans, O(N)); run-length lists of `(position, level)` pairs of 16 bytes
  (4x the memory); delta encoding with variable-length integers (smaller but loses
  binary search inside a block).

## R2. Viewport query algorithm and the achievable bound

- **Decision**: a viewport query resolves the W+1 pixel boundaries to **ranks**
  (`rank(p)` = number of transitions at positions `<= p` boundary rules in
  contracts/viewport-semantics.md). The first boundary is found by a binary search
  over the block index and then over the block's offsets: O(log N). Each following
  boundary is found with a **finger (galloping) search starting from the previous
  boundary's block**, so a boundary in the same or the next block costs O(1), and a
  boundary many blocks further costs O(log of the blocks skipped). Pixel summaries
  (transition counts, single-transition detail, levels) are derived from rank
  differences and parity, never by scanning the transitions inside a pixel.
- **Achieved bound**: O(log N + W · (1 + log(1 + N_w / W))) where N_w is the number of
  transitions inside the window. It is independent of N_w when the window is dense
  per pixel only through the logarithm, and it is **linear in W and polylogarithmic in
  N** — never O(N). When N_w ≤ c·W (zoomed in) it is exactly O(log N + W).
- **Why not literally O(log N + W) for every window**: W independent boundaries on
  arbitrary sorted positions amount to W predecessor searches; with comparison-based
  search the optimum for W boundaries over N_w elements is Θ(W · log(N_w / W)), which
  is what finger search achieves. Reaching a strict O(1) per pixel would need
  structures such as van Emde Boas trees (O(log log N) per pixel, large constants,
  complexity far beyond the benefit). In practice the extra factor is `log` of the
  average number of blocks between two pixel boundaries (about 3–10 probes), which
  is invisible next to memory latency.
- **Consequence for the constitution**: Principle VIII states `O(log N + W)`. The
  plan meets its intent (never O(N); smooth at any size) and the literal bound for
  zoomed-in windows, and records the difference in Complexity Tracking with a
  proposed clarifying amendment. The merge-blocking benchmark (R9) is defined on the
  achieved bound.
- **Alternatives considered**: a dense time-bucket pyramid (memory proportional to
  duration, violates FR-005); a segment tree over ranks (same Θ(W log(N/W)) with more
  memory); precomputed zoom-level caches (first query at a new zoom costs O(N_w)).

## R3. Pixel boundary arithmetic

- **Decision**: for a window `[start, end)` of length `L = end - start` and `W`
  pixels, column `i` covers positions `[a_i, a_{i+1})` with
  `a_i = start + floor(i · L / W)`, computed in `u128` so it cannot overflow.
  Transitions are assigned by `a_i < p <= a_{i+1}` (a transition at `p` is the change
  between sample `p-1` and `p`), so the columns partition `(a_0, a_W]`;
  `start_level` is the level at sample `a_i`, `end_level` the level at sample
  `a_{i+1}` (clamped to the last sample), hence `end_level(i) == start_level(i+1)` by
  construction and `start_level XOR (count mod 2) == end_level` always holds. Full
  rules in contracts/viewport-semantics.md.
- **Rationale**: exact, overflow-free, makes FR-012 true by construction, and works
  when `L < W` (zero-width columns hold no transitions and repeat the level).
- **Alternatives considered**: floating-point boundaries (rounding drift, glitches at
  column borders); per-column "min/max sample" (a sampled representation, loses the
  count).

## R4. Producer/reader concurrency (snapshot isolation)

- **Decision**: blocks are immutable once sealed and are stored in append-only
  segments of 256 block slots (each slot is a `OnceLock<Arc<Block>>`, written once).
  The producer builds new blocks outside any lock and then **publishes** a chunk by
  swapping one `Arc<Published>` behind a `RwLock` that is held only to clone or swap
  the pointer (nanoseconds). `Published` is an immutable value: per channel the number
  of sealed blocks visible, a copy of the unsealed tail (at most 1,024 offsets =
  4 KiB), the visible length, the gap list and the status. A reader's
  `Snapshot` is a clone of that `Arc`; it therefore sees exactly the chunks published
  before it was taken and nothing after, and it stays valid while the producer
  continues (FR-019, FR-021).
- **Rationale**: readers never take a lock while querying, so they cannot delay the
  producer (FR-020); the producer's publish cost is O(channels) plus the tail copies
  (about 32 × 4 KiB per chunk, negligible at chunk sizes of tens of thousands of
  samples). Safe Rust only (`OnceLock`, `Arc`, `RwLock`), no `unsafe`.
- **Alternatives considered**: one `RwLock` around all data held for the whole query
  (producer waits for the slowest reader; writer starvation behavior differs between
  Windows and Linux locks); lock-free structures needing `unsafe` (against the
  project's lint policy); copy-on-write of the whole block list (O(blocks) per
  publish).

## R5. Chunk format and channel limit

- **Decision**: a chunk is `&[u32]`, one word per sample, bit `i` = level of channel
  `i`; at most 32 channels (the configured channel count selects the valid bits;
  higher bits must be zero or are masked, documented). Transition detection is
  `word ^ previous_word`, skipping zero words and iterating set bits with
  `trailing_zeros`. The producer states the chunk's start position; it must equal the
  current length.
- **Rationale**: matches how the supported devices deliver data (parallel sample
  words, at most 32 lines), makes ingestion branch-light for sparse signals, and
  needs no per-channel allocation to validate input.
- **Alternatives considered**: per-channel bit vectors (extra transposition step on
  the hot path); `u8`/`u16` words (would need generic code or per-width copies).

## R6. Limits and memory accounting

- **Decision**: `Limits` has `memory_budget_bytes` (default 1 GiB), `max_chunk_samples`
  (default 65,536), `max_pixels` (default 16,384). The channel count is capped at
  32. Memory used is the exact sum of sealed block bytes (4 B per transition plus a
  32-byte header per block), tails, gap list and index segments. A chunk is
  converted into its new blocks **before** publishing (peak temporary memory
  bounded by `max_chunk_samples × channels × 4` bytes = 8 MiB at the defaults); if
  `used + new > budget` the whole chunk is discarded (atomic, FR-016) and the
  capture is marked "stopped by limit"; no allocation is sized from an untrusted field
  before it is validated (FR-018).
- **Worst case documented**: a level change on every channel at every sample costs
  `channels × 4.03` bytes per sample (about 129 bytes for 32 channels).
- **Alternatives considered**: estimating (reject a chunk that *could* exceed the
  budget in the worst case: rejects valid data early); spilling to disk (out of scope,
  see spec Assumptions).

## R7. Gaps and levels around lost data

- **Decision**: a gap is `(start_position, length)`; recording it advances the
  capture length by `length` without adding samples and marks the capture incomplete.
  The first sample after a gap is **not** compared with the sample before it (the
  real transition time is unknown); if its level differs, a *resync transition* is
  stored at the gap's end position so the parity formula stays valid. Level queries
  inside a gap return `Missing`; any pixel whose closed range touches a gap is flagged
  `missing`, and its levels at positions inside the gap are `Missing`.
- **Rationale**: never invents a transition time or a level inside lost data (FR-023)
  while keeping the O(log N) machinery unchanged.
- **Alternatives considered**: a separate "unknown" run per channel (doubles the index
  logic); interpolation (invents data).

## R8. Error model use

- **Decision**: all fallible calls return `la_core::Result`. Error codes (stable):
  `storage.invalid_channel_count`, `storage.invalid_sample_rate`,
  `storage.invalid_limits`, `storage.chunk_out_of_order`, `storage.chunk_too_large`,
  `storage.chunk_empty`, `storage.position_overflow`, `storage.memory_limit`,
  `storage.closed`, `storage.invalid_window`, `storage.invalid_pixels`,
  `storage.channel_out_of_range`, `storage.invalid_gap`. Kinds: `InvalidInput` for
  caller mistakes, `Resource` for the memory limit, `Internal` only for impossible
  states (never reachable from external input).
- **Rationale**: consistent with item 001; stable codes feed later localization.

## R9. Verifying the complexity class (merge-blocking) and performance

- **Decision**: three layers.
  1. **Deterministic complexity test (blocking, in `cargo test`)**: every query
     returns a `QueryStats` with the number of block/offset probes. The test builds
     captures of 10^3 … 10^9 transitions per channel and asserts that probes grow at
     most logarithmically with N (and linearly with W): probes(10^9)/probes(10^6)
     must be below 3 (matches SC-003), and probes never exceed a formula-based upper
     bound `c · (log2 N + W · (1 + log2(1 + N_w/W)))`. Probes are machine-independent,
     so the test is not flaky on shared CI runners.
  2. **Large synthetic captures without large memory**: blocks are immutable and
     reference their base position separately, so a test-only feature
     (`test-support`) builds a 10^9-transition capture by referencing one block
     payload many times (tens of megabytes: one table of about 10^6 small block descriptors shared by all channels). It verifies SC-002/SC-003 at the
     real size. Caveat (documented): it is cache-friendly, so timings from it are
     optimistic; real-data timings at 10^7–10^8 are measured too.
  3. **Timing benchmarks (informational, recorded baseline)**: a dependency-free
     harness (`harness = false`, `std::time::Instant`) in `benches/` measures viewport
     queries (SC-002), ingestion throughput (SC-006) and writes JSON; the committed
     `benches/baseline.json` records the maintainer-machine reference and a run
     fails if a measurement is more than 5x slower than the baseline or if the growth
     ratio breaks R9.1. Timing runs are manual and scheduled (shared CI runners are too
     noisy to gate on absolute time), not part of the required checks.
- **Reference machine**: "the maintainer's development machine, recorded in
  `baseline.json` (CPU model, cores, RAM, OS) at the time the baseline is
  generated"; the absolute targets in the spec (SC-002, SC-006) are verified there.
- **Alternatives considered**: `criterion` (heavy dependency tree, statistics we do
  not need, and still noisy on shared runners); asserting wall-clock thresholds in
  `cargo test` (flaky).

## R10. Logging

- **Decision**: use `tracing` events with metadata only: `capture created`
  (channels, rate, limits), `memory limit reached` (used, budget, rejected samples),
  `gap recorded` (position, length), `capture closed` (status, lengths). No signal
  data (FR-027). Chunk appends are not logged individually (hot path).
- **Rationale**: Principle VI and the item-001 rule never to log payloads.

## R11. Dependencies

- **Decision**: no new third-party dependencies. `la-storage` depends on `la-core`
  (error model) and `tracing` (already in the workspace). Tests and benches use only
  the standard library.
- **Rationale**: Principle V; all needed primitives (`OnceLock`, `Arc`, `RwLock`,
  `u128`, `leading_zeros`, `trailing_zeros`) are in `std`.

## R12. Panic safety with arithmetic-heavy code

- **Decision**: the workspace denies `arithmetic_side_effects` and `indexing_slicing`.
  Position and rank math uses `checked_*` / `saturating_*` helpers and `u128`
  intermediates in a small `math` module with unit tests; slices are accessed with
  `get` and iterators. Each helper that can fail returns a typed error
  (`storage.position_overflow`). No `#[allow]` for these lints is planned; any
  exception would need a written reason per the contributor guide.
- **Rationale**: the store is the component that holds the user's data; the lint
  policy from item 001 is the enforcement.
