# Quickstart: Validate the Signal Storage Core

Prerequisite: the workspace builds (`cargo build --workspace`). All commands run from
the repository root on Windows 10/11 and Debian 13.

## 1. Exact readback (User Story 1)

```text
cargo test -p la-storage --test exact_readback
```

Expected: generated patterns (constant, alternating, random, long runs) appended in
many chunk sizes read back exactly: `level_at` and `transitions` match an independent
reference at every position, including chunk boundaries.

## 2. Viewport semantics and glitches (User Stories 2 and 3)

```text
cargo test -p la-storage --test viewport --test glitches
```

Expected: the pixel rules in
[contracts/viewport-semantics.md](contracts/viewport-semantics.md) hold for random
windows and pixel counts (end level equals next start level, parity identity, partition
of transitions); every isolated glitch is reported at every zoom level.

## 3. Complexity class (User Story 2, merge-blocking)

```text
cargo test -p la-storage --test complexity
```

Expected: probe counts for captures of 10^3 to 10^9 transitions per channel grow
logarithmically with N (probes(10^9) / probes(10^6) < 3) and within the formula bound
of research R9. The 10^9 capture is synthetic and uses tens of megabytes of memory.

## 4. Limits, gaps, hostile input (User Stories 4, 6, 7)

```text
cargo test -p la-storage --test limits --test gaps --test hostile_input
```

Expected: appends over the budget are rejected atomically with `storage.memory_limit`
and stored data stays exact; gaps flag the capture incomplete and mark pixel columns;
more than 10,000 generated invalid or extreme inputs produce typed errors, no panic, and
leave the data unchanged.

## 5. Concurrent readers (User Story 5)

```text
cargo test -p la-storage --test concurrency
```

Expected: while a producer appends continuously, every reader result equals a prefix
made of complete chunks; the producer's worst-case publish delay stays within the
documented maximum.

## 6. Timing benchmarks (informational, recorded baseline)

```text
cargo bench -p la-storage --bench viewport
cargo bench -p la-storage --bench ingest
```

Expected: JSON results are written to `target/la-storage-bench/`; compare with
`crates/storage/benches/baseline.json` (SC-002: 2,000 pixels x 16 channels on a
10^9-transition synthetic capture under 20 ms; SC-006: 100 MHz x 32 channels
ingestion). A run reports a failure if a measurement is more than 5x slower than the
baseline. These runs are manual or scheduled, not required checks.

## 7. Whole gate

```text
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace
cargo deny --all-features check
```

The same checks run in CI on both operating systems.
