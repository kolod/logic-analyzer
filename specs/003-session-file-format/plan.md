# Implementation Plan: Session File Format

**Branch**: `003-session-file-format` | **Date**: 2026-11-14 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `/specs/003-session-file-format/spec.md`

## Summary

A new UI-free crate, `la-session`, saves one capture plus its description into a versioned
container of individually checksummed sections and loads it back, treating every byte as
untrusted. The capture is stored as what the storage core already keeps — per channel the
initial level and the strictly increasing transition positions, plus the gaps, the length
and the status flags — encoded as delta varints, so the file grows with the number of
level changes. Writing is atomic (unique temporary file next to the target, flush, fsync,
rename, directory fsync where the platform allows) behind a small file-system seam that
tests use to inject a failure at every step; reading is a bounded, streaming decoder that
checks sizes against the file and the memory budget before allocating and validates the
capture's consistency while it builds it. Save and load run as cancellable background
operations on supervised workers with progress and exactly one outcome. Two small
additions outside the new crate make this possible without exposing storage internals:
`la-storage` gains read accessors and a validating restorer that builds a closed capture
from transitions, and `la-devices` gains explicit read/restore functions for
`CaptureDescription`. Fuzzing is a dependency-free mutation fuzzer in the test suite
(short run in CI, one-million-input run on a schedule), backed by exhaustive truncation
and bit-flip tests.

## Technical Context

**Language/Version**: Rust 2024 edition, MSRV 1.85 (workspace setting), toolchain pinned by `rust-toolchain.toml`

**Primary Dependencies**: `la-core` (errors), `la-storage` (capture, snapshot, limits), `la-devices` (capture description), `la-diagnostics` (supervised workers), `tracing` (metadata-only logs). No new third-party crate: CRC-32C, varints and the container are implemented in the crate (small, fully tested, no supply-chain surface).

**Storage**: files on the local file system (the session file); in-memory capture of the storage core.

**Testing**: `cargo test` — unit tests per module, integration tests (round trip, atomic save with injected failures, process-kill test, corrupt/hostile loads, compatibility fixtures, background operations, independent reader), exhaustive truncation and bit-flip tests, deterministic mutation fuzzer; benchmark `crates/session/benches/session_io.rs` with recorded baseline.

**Target Platform**: Windows 10/11 and Debian 13 (x86_64; the format is byte-order independent and tested identically on both)

**Project Type**: desktop-app workspace; this feature adds the UI-independent crate `crates/session`

**Performance Goals**: 100 M samples on 16 channels, typical signal: save and load each under 10 s (SC-007); starting or cancelling an operation under 10 ms / 100 ms (SC-008); saving a running capture pauses the recording for under 10 ms (SC-009)

**Constraints**: loading never allocates beyond the memory budget nor more than 1 MiB beyond the file size for a lying file (SC-006); no panic, hang or unbounded recursion on any input; saving never damages an existing file

**Scale/Scope**: captures up to the storage core's maxima (32 channels, memory budget 1 GiB by default); files from a few bytes to several GiB; up to 64 sections

## Constitution Check

*GATE: passes before Phase 0 research; re-checked after Phase 1 design (still passes).*

- [x] **I. Stability & Data Integrity**: this feature exists to uphold it. Atomic writes
  (temp, flush, fsync, rename), loaders that validate everything and return typed errors,
  checksums on every section, no `unwrap`/`expect`/indexing/unchecked arithmetic (workspace
  lints), workers contained, a failed save never touches the old file. The audit grep from
  item 004 is repeated for the new crate.
- [x] **II. Hardware**: not affected; no driver is touched. The capture description is read
  and restored, not changed.
- [x] **III. Test-First**: tests precede implementation per story; malformed and boundary
  inputs (every truncation length, a bit flip at every position, hostile counts), a
  deterministic fuzzer plus a scheduled one-million-input run; fmt, clippy and tests run in
  CI on both platforms.
- [x] **IV. Architecture**: logic lives in `la-session` (no UI dependency); storage changes are
  minimal and keep the crate UI- and device-free; background operations are cancellable;
  memory is bounded by the storage core's budget; no host API is changed.
- [x] **V. Simplicity**: zero new dependencies. CRC-32C (about 60 lines, table-driven) and
  LEB128 varints are implemented and tested in the crate; a checksum crate or a serialization
  framework would add surface and would not give the control over allocation the loader needs.
- [x] **VI. Observability**: `tracing` events with metadata only (sizes, counts, codes,
  durations); never signal data or text fields (FR-035); errors carry a stable code and an
  actionable English detail.
- [x] **VII. Localization**: no UI text. Stable `session.*` codes map to localized messages
  later; text stored in files is user data, stored as UTF-8 and never interpreted.
- [x] **VIII. Rendering**: not affected; loading builds the same blocks the recorder builds, so
  viewport query cost of a loaded capture is identical (tested by comparing probe counts).
- [x] **IX. Decoding**: not affected; the optional-section mechanism lets item 007 add decoder
  settings and annotations without a format change.
- [x] **X. Export**: not affected (a different, later item); the atomic-write routine is written
  so the log exporter can reuse it.
- [x] **XI. Virtual device**: the simulated mark and the seed are stored in the required
  description section, cannot be cleared by a consumer, and an edit is caught by the checksum;
  captures from the virtual device are the main test data.
- [x] **XII. Original design & third-party code**: no code, layout or text from PulseView, sigrok
  or Saleae; the format is the project's own. PulseView/sigrok session files are not read.
- [x] **Platforms**: Windows 10/11 and Debian 13. OS-specific code is limited to the directory
  fsync (Unix only; Windows has no equivalent in the standard library, documented) and is
  isolated in the file-system seam.

## Project Structure

### Documentation (this feature)

```text
specs/003-session-file-format/
├── plan.md              # This file
├── research.md          # Phase 0: decisions and alternatives
├── data-model.md        # Phase 1: entities, validation rules, state
├── quickstart.md        # Phase 1: how to validate the feature
├── contracts/
│   ├── file-format.md   # the byte-level format (what an independent reader implements)
│   └── session-api.md   # Rust API of la-session and the additions to storage and devices
├── checklists/
│   └── requirements.md
└── tasks.md             # Phase 2 (/speckit-tasks, not created here)
```

### Source Code (repository root)

```text
crates/
├── storage/             # additions: Snapshot read accessors, CaptureRestorer (validating bulk build)
│   ├── src/restore.rs
│   └── tests/restore.rs
├── devices/             # additions: CaptureDescription::parts / restore
├── session/             # NEW crate la-session
│   ├── Cargo.toml
│   ├── src/
│   │   ├── lib.rs           # public API, `#![deny(missing_docs)]`
│   │   ├── error.rs         # session.* codes and constructors
│   │   ├── crc.rs           # CRC-32C
│   │   ├── varint.rs        # LEB128 encode, size, bounded decode
│   │   ├── limits.rs        # LoadLimits and format constants
│   │   ├── format.rs        # header, footer, section framing (pure, no I/O)
│   │   ├── meta.rs          # META section: description encode/decode
│   │   ├── capture_codec.rs # CAPT section: encode from Snapshot, decode into CaptureRestorer
│   │   ├── session.rs       # Session, SessionSource, Section
│   │   ├── fs.rs            # FileSystem seam, real implementation
│   │   ├── write.rs         # atomic save
│   │   ├── read.rs          # bounded streaming load
│   │   ├── operation.rs     # background SaveOperation / LoadOperation
│   │   └── fault.rs         # fault-injecting file system (feature `test-support`)
│   ├── tests/
│   │   ├── round_trip.rs  atomic_save.rs  kill_save.rs  corrupt_files.rs
│   │   ├── truncation_and_flips.rs  fuzz.rs  compatibility.rs  marks.rs
│   │   ├── background.rs  running_capture.rs  independent_reader.rs
│   │   └── fixtures/v1/      # files written by version 1.0, always loaded by tests
│   └── benches/
│       ├── session_io.rs     # save/load throughput (harness = false)
│       └── baseline.json
.github/workflows/fuzz.yml   # scheduled/manual one-million-input run (informational)
```

**Structure Decision**: a separate crate keeps file I/O, checksums and format evolution away
from the storage core (which stays free of I/O) and from the UI; `la-session` depends on
`la-devices` only for the description type. The storage and devices additions are small,
documented and tested in their own crates.

## Complexity Tracking

No constitution violations. The only deliberate additions to other crates are the read
accessors and the validating restorer in `la-storage` (needed so a loaded capture is built by
the same invariants-keeping code as a recorded one rather than by a second implementation of
the block layout) and two functions on `CaptureDescription`.
