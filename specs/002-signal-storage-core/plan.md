# Implementation Plan: Signal Storage Core

**Branch**: `002-signal-storage-core` | **Date**: 2026-11-14 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `/specs/002-signal-storage-core/spec.md`

## Summary

Add the crate `la-storage`: a UI-free store for captured digital signals. Each channel
is kept as its transitions in immutable blocks of up to 1,024 `u32` offsets, indexed by
block (4 bytes per transition, cost independent of duration). A viewport query resolves
pixel boundaries to ranks with a finger search and derives per-pixel summaries
(start/end level, none/one/many transitions, missing-data flag) from rank differences,
so cost is polylogarithmic in the capture size and never proportional to it. Appends
are chunk-atomic and memory-bounded; readers use immutable snapshots published by
swapping one `Arc`, so they never block the producer. Lost samples are recorded as gaps
and flag the capture incomplete. All input is validated into typed errors. Complexity is
verified by a deterministic probe-count test (merge-blocking) plus recorded timing
benchmarks. Details and rationale: [research.md](research.md).

## Technical Context

**Language/Version**: Rust, edition 2024; MSRV 1.85; toolchain pinned in
`rust-toolchain.toml` (as in item 001)

**Primary Dependencies**: `la-core` (typed errors), `tracing` (events with metadata
only). No new third-party dependencies; `std` only for everything else.

**Storage**: in-memory only (file format is roadmap item 003)

**Testing**: `cargo test` — reference-model comparison tests, generated and
property-style tests with a deterministic xorshift generator (no extra dependency),
a deterministic complexity test based on probe counts, a threaded concurrency test;
timing benchmarks with a dependency-free `harness = false` binary

**Target Platform**: Windows 10/11 and Debian 13, 64-bit x86

**Project Type**: library crate in the desktop-app workspace (UI-independent core)

**Performance Goals**: 2,000-pixel × 16-channel viewport query on a 10^9-transition
capture in under 20 ms (SC-002); 1,000× growth in N costs under 3× query time
(SC-003); sustained 100 MHz × 32-channel ingestion (SC-006) — on the maintainer
machine recorded in `baseline.json` (research R9)

**Constraints**: no panic on external input; no `unsafe`; memory bounded by an explicit
budget (default 1 GiB) with atomic chunk acceptance; worst case about 4.03 bytes per
channel-sample; temporary memory per chunk at most `max_chunk_samples × channels × 4`
bytes (8 MiB at defaults); at most 32 channels

**Scale/Scope**: up to 10^9 transitions per channel (synthetic for tests), 32
channels, windows of up to 16,384 pixels; about 1,500–2,000 lines of Rust including
tests

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

Verified against `.specify/memory/constitution.md` v1.4.0. Result: **PASS with one
justified deviation** (Principle VIII, recorded in Complexity Tracking).

- [x] **I. Stability & Data Integrity**: all inputs validated to typed errors
  (FR-025); atomic chunk acceptance; rejected operations never change stored data
  (FR-026); the memory budget stops appends without losing data; no `unsafe`;
  lint policy from item 001 applies (checked/saturating arithmetic, `get` instead of
  indexing). A panic in a reader thread cannot corrupt the store (immutable snapshots).
- [x] **II. Hardware**: not affected (no device code); designed for the highest rates
  among the supported devices (100 MHz × 32 channels).
- [x] **III. Test-First**: tests precede implementation per user story; property-style
  generated tests and a 10,000-input hostile corpus; fmt + clippy + test run in CI on
  both OSes.
- [x] **IV. Architecture**: logic in a UI-independent crate; no OS-specific code; no
  host API contract change (the Rust API is documented in contracts/); long work is
  bounded by W, not by capture size; memory bounded and predictable (R6).
- [x] **V. Simplicity**: no new dependencies; one data structure (transition blocks);
  complexity justified below.
- [x] **VI. Observability**: lifecycle events logged with metadata only (R10); stable
  error codes with technical `detail`.
- [x] **VII. Localization**: no UI strings; errors carry stable codes for later
  localization; documentation and comments in English.
- [x] **VIII. Rendering**: bounded as analysed in research R2 — **deviation**: the
  achieved bound is `O(log N + W·(1 + log(1 + N_w/W)))`, which is the literal
  `O(log N + W)` when zoomed in and polylogarithmic otherwise; never O(N). Indexed
  storage, multi-resolution behaviour through rank differences, dense activity visible
  (`Many`), benchmark with recorded baseline and a merge-blocking complexity test.
- [x] **IX. Decoding**: not applicable here; the API provides exact `transitions()`
  iteration and `level_at()` that decoders will use.
- [x] **X. Export**: not applicable.
- [x] **XI. Virtual device**: not applicable yet; the producer API is what the virtual
  device (item 004) will call; the `test-support` synthetic capture is a test helper only.
- [x] **XII. Original design & third-party code**: original design; no code reused from
  PulseView, sigrok or Saleae; no new third-party crates.
- [x] **Platforms**: pure Rust on `std`; behaves identically on Windows 10/11 and
  Debian 13; no OS-specific code, no driver impact.

Post-design re-check (after Phase 1): unchanged — PASS with the same single deviation.

## Project Structure

### Documentation (this feature)

```text
specs/002-signal-storage-core/
├── plan.md              # This file (/speckit-plan command output)
├── research.md          # Phase 0 output (/speckit-plan command)
├── data-model.md        # Phase 1 output (/speckit-plan command)
├── quickstart.md        # Phase 1 output (/speckit-plan command)
├── contracts/           # Phase 1 output (/speckit-plan command)
│   ├── storage-api.md
│   └── viewport-semantics.md
├── checklists/
│   └── requirements.md
└── tasks.md             # Phase 2 output (/speckit-tasks command - NOT created by /speckit-plan)
```

### Source Code (repository root)

```text
Cargo.toml                         # workspace (unchanged; members = crates/*)

crates/storage/                    # package la-storage
├── Cargo.toml                     # deps: la-core, tracing; feature test-support;
│                                  #   [[bench]] viewport, ingest with harness = false
├── src/
│   ├── lib.rs                     # public API re-exports, crate docs
│   ├── config.rs                  # CaptureConfig, Limits, validation
│   ├── error.rs                   # error code constants and constructors (la-core Error)
│   ├── math.rs                    # checked position/rank arithmetic, u128 boundary math
│   ├── block.rs                   # Block, block builder (sealing at 1,024 / u32 overflow)
│   ├── segment.rs                 # append-only segments of OnceLock<Arc<Block>> slots
│   ├── channel.rs                 # ChannelData, rank(), level lookup, finger search
│   ├── ingest.rs                  # chunk -> per-channel transitions (xor + trailing_zeros)
│   ├── capture.rs                 # Capture producer: append, gaps, close, accounting
│   ├── published.rs               # Published, CaptureReader, Snapshot, status
│   ├── query.rs                   # viewport(), transitions(), level_at(), PixelSummary
│   ├── types.rs                   # LevelState, Edge, Activity, Gap, CaptureStatus, stats
│   └── testing.rs                 # (feature test-support) synthetic_capture()
├── tests/
│   ├── common/mod.rs              # reference model, xorshift generator, helpers
│   ├── exact_readback.rs          # US1
│   ├── viewport.rs                # US2 semantics
│   ├── glitches.rs                # US3
│   ├── complexity.rs              # US2 merge-blocking probe-count test
│   ├── limits.rs                  # US4
│   ├── concurrency.rs             # US5
│   ├── gaps.rs                    # US6
│   └── hostile_input.rs           # US7
└── benches/
    ├── viewport.rs                # timing harness (informational)
    ├── ingest.rs                  # ingestion throughput harness
    └── baseline.json              # recorded reference measurements
```

**Structure Decision**: one new library crate `crates/storage` (package `la-storage`)
following the workspace layout in the plan template and roadmap (`crates/core`,
`crates/diagnostics`). The crate name follows the `la-` prefix convention. No other crate
changes; `la-storage` depends on `la-core` and `tracing`.

## Complexity Tracking

| Violation | Why Needed | Simpler Alternative Rejected Because |
|-----------|------------|-------------------------------------|
| Achieved viewport bound `O(log N + W·(1 + log(1 + N_w/W)))` instead of the literal `O(log N + W)` of Principle VIII | W pixel boundaries on arbitrary sorted positions need W predecessor searches; the comparison-model optimum is Θ(W·log(N_w/W)) and finger search achieves it. The literal bound holds when zoomed in (N_w ≤ c·W) and the extra factor is the log of the average number of blocks per pixel (about 3–10 probes) | A dense time-bucket pyramid would make queries O(1) per pixel but needs memory proportional to duration (violates FR-005); van Emde Boas style structures give O(log log N) per pixel with large constants and far greater complexity for no measurable gain. **Decision for the project owner**: amend Principle VIII to "O(log N + W·log(1 + N_w/W)), never O(N)" (a PATCH clarification), or accept this documented reading |
| `testing.rs` synthetic capture referencing one block payload many times (feature `test-support`) | Verifies SC-002/SC-003 at 10^9 transitions without gigabytes of memory | Building a real 10^9-transition capture needs about 4 GB per channel, impossible on CI runners; scaled-down runs alone cannot show the growth at the real size |

## Assumptions Recorded During Planning

- Chunks are `&[u32]` words (bit `i` = channel `i`), at most 32 channels (research R5).
- Window queries may extend past the capture length; those parts are `Missing` and
  flagged `beyond_end` (contracts/viewport-semantics.md).
- Absolute timing targets are verified on the maintainer machine recorded in
  `baseline.json`; the merge gate uses the deterministic probe-count test, because
  shared CI runners are too noisy to gate on wall-clock time.
- The default memory budget (1 GiB), chunk size (65,536) and pixel limit (16,384) are
  starting values chosen here; they are configurable limits.
