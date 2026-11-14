# Verification Record: Session File Format

**Feature**: [spec.md](../spec.md) | **Recorded**: 2026-11-14 on the maintainer machine
(Windows 11, x86_64, 32 logical cores; toolchain 1.100.0; MSRV 1.85 locally; Debian 13 through CI)

## Test evidence

`cargo test --workspace --all-features`: 354 tests pass, 0 fail (also under `cargo +1.85 test
--workspace`). `cargo fmt --all --check`, `cargo clippy --workspace --all-targets --all-features -- -D
warnings` (and without `--all-features`) and `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps
--all-features` are clean. `cargo deny` is not installed on the maintainer machine; CI runs it.

| Success criterion | Evidence |
|-------------------|----------|
| SC-001 round trip | `round_trip`: 1,000 seeded random captures (1 to 32 channels, gaps, interrupted, open) equal on every query, probe counts and description field; the edges (empty, one sample, 32 channels, leading gap, one big gap, stopped by limit) too |
| SC-002 determinism | 100 saves byte-identical; a loaded and re-saved file identical; two committed reference files and four version 1.0 fixtures byte-identical to the writer's output (integer-only generators and a fixed description, so the same on Debian and Windows in CI) |
| SC-003 atomic save | `atomic_save`: every call of a save failed in turn (create, each write, flush, sync, rename; 3 errors each, plus a half-written disk-full case): at least 20 injection points (the test asserts it), old file intact and no temporary file every time; `kill_save`: 1,000 killed child processes (release build, 25 s) and 150 in the normal debug run, each leaving a target equal to one of the two complete versions, at least 90% of the kills landing inside a save |
| SC-004 truncation and flips | every truncation length and every single-bit flip of three files rejected with the documented code families; appended bytes rejected |
| SC-005 fuzzing | 20,000 mutated inputs in the normal suite (1 s), 1,000,000 in a release run (4 s, 974,150 rejected, 25,850 accepted and consistent); no panic, no hang (watchdog), allocation within budget; weekly in CI |
| SC-006 lying sizes | huge counts, lengths and texts rejected without allocating more than three times the file; a capture over the budget is refused after reading one buffer (64 KiB) of a 330 KB file |
| SC-007 throughput | see below |
| SC-008 latency | start of a save or load under 10 ms (50 ms debug limit), cancel ending within 100 ms (500 ms debug limit) with a deliberately slow file system |
| SC-009 running capture | longest stall of the recording during six saves: 2.5 ms (debug), none observed (release); every file a valid, closed, interrupted session holding exactly a prefix of the final capture; the recording identical to an unsaved run with the same seed |
| SC-010 marks | virtual captures stay simulated with their seed through repeated cycles, incomplete captures keep gaps, lost positions and reasons, every bit of every mark byte rejected when edited without fixing the checksums |
| SC-011 compatibility | four version 1.0 fixtures load to a hand-reviewed table of facts; unknown optional sections survive byte for byte |
| SC-012 independent reader | `independent_reader`: a second reader and writer from the contract alone (own checksum and varint code) agrees on 6 fixtures and 200 generated files and its writer reproduces the fixtures byte for byte |

Mutation checks done while writing the tests (each break was caught, then reverted): a fixed initial
level, absolute gap starts, a dropped interrupted flag, a temporary file left behind, ignored write
errors, an ignored rename error, a skipped section checksum, an accepted non-canonical varint (found
uncaught, a test was added), a dropped simulated bit and seed bit. A skipped footer signature check is
not observable because the footer checksum covers the signature (redundant defence).

## SC-007: save and load throughput (`benches/session_io.rs`)

Capture recorded by the virtual device through the real acquisition, saved and loaded through the real
file system. Baseline in `crates/session/benches/baseline.json`:

| Case | File | Save | Load |
|------|------|------|------|
| 1 channel, typical (flip every ~50 samples), 100 M samples | 1.9 MiB | 0.02 s | 0.03 s |
| **16 channels, typical, 100 M samples (gated)** | 30.5 MiB | **0.22 s** | **0.35 s** |
| 16 channels, sparse (every ~1,000), 100 M samples | 3.0 MiB | 0.03 s | 0.03 s |
| 32 channels, typical, 40 M samples | 24.4 MiB | 0.19 s | 0.28 s |

**SC-007 is met** with a wide margin (limit 10 s each). The file takes about 1 to 1.3 bytes per transition.

## Data-safety review (Principle I)

Search of `crates/session/src` and the new storage and devices code for `unwrap(`, `expect(`, `panic!`,
`todo!`, `unimplemented!`, `unreachable!`, `unsafe`, `#[allow`, `#![allow`, `#[expect`:

- no `unsafe`, `todo!`, `unreachable!`, `unimplemented!`, `panic!` or `expect(` in production code;
- `unwrap(` appears only inside `#[cfg(test)]` modules;
- `#[allow(clippy::indexing_slicing, ...)]` in `crc.rs` on the compile-time table construction (constant,
  in-range indexes) and on the table lookup (row and byte index masked into range); both justified
  in their `reason`;
- `#[allow(clippy::arithmetic_side_effects)]` on `#[cfg(test)]` modules only;
- the only `unsafe` in the feature is the `GlobalAlloc` implementation of the test-only counting allocator
  (`tests/common/support.rs`, each block with a `// SAFETY:` comment);
- searched for `with_capacity`, `vec![0`, `reserve` and `as` casts: every allocation in the loader is sized
  by a constant (the 64 KiB buffer, 16 KiB chunks) or grows with bytes that were really read; the only
  `as` casts are in the compile-time table construction and the masked table lookup.

## Remaining constitution gates

- **Localization (Principle VII)**: no UI strings; stable `session.*` codes; English details and docs.
- **Third-party code (Principles V and XII)**: `Cargo.lock` gains only the workspace package
  `la-session`; no new dependency; checksum, varints and the container are written in the crate;
  nothing reused from PulseView, sigrok or Saleae.
- **Principle I**: atomic write (unique temporary file, flush, fsync, rename, directory fsync on Unix),
  validated loader, fuzzing, kill test. The standard library cannot fsync a directory on Windows: the
  rename is atomic (old or new, never mixed) but its durability across a power failure is not forced
  there; the save report says `directory_synced`.
- **Principle X**: `write.rs` is written so the decoded log exporter (item 009) can reuse the
  temporary-file, sync and rename routine.
- **Platforms**: tests run locally on Windows 11 (and MSRV 1.85); Debian 13 and the MSRV job run in CI.
  The format is byte-order independent (explicit little-endian everywhere).
