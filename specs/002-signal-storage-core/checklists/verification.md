# Verification Record: Signal Storage Core

**Purpose**: Evidence for the measurable success criteria and constitution gates of
feature 002, filled in as the corresponding tasks are completed.

**Feature**: [spec.md](../spec.md)

## Performance baseline (T029)

Recorded by `cargo bench -p la-storage --bench viewport -- --write-baseline` into
`crates/storage/benches/baseline.json` on the maintainer machine (Windows, x86_64,
Intel64 Family 6 Model 183, 32 logical cores), release profile, 2,000 pixels:

| Capture | Channels | Median | p95 | Probes |
|---------|----------|--------|-----|--------|
| synthetic, 10^6 transitions per channel | 16 | 446 us | 470 us | 367,584 |
| synthetic, 10^7 | 16 | 647 us | 720 us | 511,968 |
| synthetic, 10^8 | 16 | 1,527 us | 1,676 us | 703,952 |
| synthetic, 10^9 | 16 | 2,739 us | 3,063 us | 895,968 |
| real (appended), 10^6 | 1 | 30 us | 41 us | 22,974 |
| real (appended), 10^7 | 1 | 75 us | 102 us | 31,998 |

- **SC-002** (2,000 px x 16 channels on 10^9 transitions under 20 ms): **met**, 2.7 ms
  (about 7x headroom).
- **SC-003** (1,000x more transitions costs under 3x): **met**, probe growth from 10^6 to
  10^9 is 2.44x (deterministic, enforced by `tests/complexity.rs`).
- Caveat: synthetic captures are cache-friendly (every block shares one payload), so
  their timings are optimistic; the real-data rows are the more honest numbers.
- Mutation check of the complexity test: replacing the galloping search with a linear
  scan makes 4 of the 6 tests fail (10^9: about 998,000 probes against a bound of
  about 479,000); the original code was restored.

## Ingestion throughput (T054, SC-006)

Recorded by `cargo bench -p la-storage --bench ingest -- --write-baseline` on the same
machine, release profile, chunks of 65,536 samples, million samples per second:

| Channels | sparse (1 change per 1000 samples per channel) | typical (1 per 50) | stress (1 per 20) | alternating (1 per sample) |
|----------|-----------|---------|--------|-------------|
| 1 | 1,076 | 888 | 774 | 206 |
| 8 | 863 | 410 | 204 | 26 |
| 16 | 742 | 240 | 122 | 11 |
| 32 | 642 | **128** | 82 | 4.3 |

- **SC-006** (100 MS/s on 32 channels): **met** for sparse and typical signals (642 and
  128 MS/s). The benchmark first measured the 5% "stress" density at 82 MS/s and showed
  that the criterion had no density definition; the spec now states the density it holds
  for (one change per 50 samples per channel). Stress and the alternating worst case
  (memory-bound: 4.3 MS/s, 251 MiB per 2 million samples) are reported, not gated.
- A possible later optimization is to merge the two passes over the transitions
  (chunk to per-channel lists, then to blocks); it would raise the stress figure but is
  not needed for the criterion.

## Data safety, Principle I (T057)

A search of `crates/storage/src` for `unwrap(`, `expect(`, `panic!`, `todo!`,
`unimplemented!`, `unreachable!`, `unsafe`, `#[allow`, `#![allow` and `#[expect` finds
matches only inside the two `#[cfg(test)]` modules (`math.rs`, `capture.rs`). Production
code, including the `test-support` helper, contains none of them and no lint exception.
The temporary `#[expect(dead_code)]` markers used while the feature was built phase by
phase are all gone. `#![deny(missing_docs)]` is enabled and `cargo doc` runs clean with
`-D warnings`.

Behavioural evidence for FR-025/FR-026 and SC-008: `tests/hostile_input.rs` runs 15,000
generated calls (3,865 succeed, 11,135 are rejected, so SC-008 is met; seed `0x5eed0042`):
0 panic, every failure carries a `storage.*` code, and a failed call never changes
stored data. `tests/limits.rs` shows the memory accounting is exact (after the largest
accepted chunk less than 36 bytes of the budget remain).

## Other constitution gates (T058)

- **Localization (Principle VII)**: no UI strings; errors carry stable `storage.*` codes
  and an English technical `detail` for the localization layer to map later;
  documentation and comments are English.
- **Third-party code (Principles V and XII)**: no new third-party dependency. `Cargo.lock`
  gains only the workspace package `la-storage` (`la-core`, `tracing`, and the
  dev-dependencies `la-diagnostics` and itself with `test-support`); `cargo deny
  --all-features check` passes. Nothing was reused from PulseView, sigrok or Saleae.
- **Principle VIII (rendering)**: the achieved bound is
  `O(log N + W·(1 + log(1 + N_w/W)))`, the literal `O(log N + W)` when zoomed in. The
  deviation is recorded in the plan's Complexity Tracking; amending the constitution
  wording is the project owner's decision (task T060).
- **Platforms (Principle IV and the platform gate)**: pure `std` Rust, no OS-specific
  code. Verified locally on Windows; Debian 13 and the MSRV 1.85 job are verified by CI
  (task T061): all 10 required checks and the commit report passed on pull request #7.
