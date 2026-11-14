# Verification Record: Device Abstraction and Virtual Device

**Feature**: [spec.md](../spec.md) | **Recorded**: 2026-11-14 on the maintainer machine
(Windows 11, x86_64, 32 logical cores; toolchain 1.100.0; MSRV 1.85 and Debian 13 through CI)

## Test evidence

`cargo test --workspace --all-features`: 231 tests pass, 0 fail. `cargo fmt --all --check`
and `cargo clippy --workspace --all-targets --all-features -- -D warnings` are clean.
`RUSTDOCFLAGS="-D warnings" cargo doc -p la-devices --no-deps --all-features` passes
(`#![deny(missing_docs)]` is on).

Mutation checks done while writing the tests (each break was caught by a test, then
reverted): close of an interrupted capture, the delivered-samples count, gap recording,
the simulated label, the stale-scan guard of the hot-plug monitor (1 of 4 runs failed
without the guard, 0 of 10 with it).

## SC-003 / SC-004: the UI thread never waits for a device (`tests/ui_latency.rs`)

Slowest of 25 calls each, with one provider whose `scan` hangs, one driver whose `open`
hangs and one driver that hangs inside `read`:

| Call | Debug build (limit 50 ms) | Release build (limit 10 ms) |
|------|---------------------------|-----------------------------|
| `list` | 9.9 µs | 7 µs |
| `describe` | 0.7 µs | 0.4 µs |
| `validate` | 0.6 µs | 0.2 µs |
| `progress` | 0.1 µs | 0.1 µs |
| `outcome` | 0.2 µs | 0.1 µs |
| `drain_events` | 0.9 µs | 1.3 µs |
| `start` (open hangs) | 47 µs | 79 µs |
| `stop` (read hangs) | 0.1 µs | 0.1 µs |
| `stop` (open hangs) | 1.2 µs | 0.1 µs |

Stop on the virtual device, from the request to a closed capture (worst of 5 runs on a
running 32-channel capture): 2.9 ms in a debug build (limit 500 ms), 0.45 ms in a release
build (limit 100 ms).

## SC-010: virtual device throughput (`benches/virtual_throughput.rs`)

Fast mode through the real acquisition (driver thread, bounded channel, ingest thread)
into a capture, no recorded loss in any case. Baseline in `crates/devices/benches/baseline.json`:

| Channels | Sparse (flip every ~1,000 samples) | Typical (flip every ~50 samples) |
|----------|------------------------------------|----------------------------------|
| 1 | 975 MS/s | 761 MS/s |
| 8 | 814 MS/s | 287 MS/s |
| 16 | 625 MS/s | 154 MS/s |
| 32 | **479 MS/s** | 80 MS/s |

**SC-010 is met**: 479 MS/s sparse on 32 channels (target 100 MS/s), 0 lost positions. The
typical 32-channel case (80 MS/s) is below 100 MS/s and is reported for information only,
as the same case is in the storage benchmark (item 002); the limit there is the store's
per-transition cost, not the generator or the supervisor. The benchmark runs weekly and on
demand in CI as an informational job (`bench.yml`) and is not a required check.

## Data-safety review (Principle I)

Search of `crates/devices/src` for `unwrap(`, `expect(`, `panic!`, `todo!`,
`unimplemented!`, `unreachable!`, `unsafe`, `#[allow`, `#![allow`, `#[expect`:

- no `unsafe`, `todo!`, `unreachable!`, `panic!` or `expect(` anywhere in production code;
- `unwrap(` appears only inside `#[cfg(test)]` modules (`harness/scripted.rs`,
  `harness/provider.rs`, `harness/transport.rs`);
- `#[allow(clippy::panic, reason = ...)]` on `ScriptedDevice::read`/`stop` and on
  `conformance::run_all`: deliberate test-only fault injection and the assertion that fails
  the calling test, both inside the `test-support` harness;
- `#[allow(clippy::arithmetic_side_effects)]` on one `#[cfg(test)]` module of the generator;
- `unimplemented!()` appears once, inside a `compile_fail` doctest of `CaptureDescription`.

## Remaining constitution gates

- **Localization (Principle VII)**: the crate has no UI strings; errors carry stable
  `device.*` codes and English `detail`; all docs and comments are English.
- **Third-party code (Principles V and XII)**: `Cargo.lock` gains only the workspace package
  `la-devices` (its dependencies, including `tracing`, were already locked); `deny.toml` is
  unchanged; nothing is taken from PulseView, sigrok or Saleae. `cargo deny` runs in CI (the
  tool is not installed on the maintainer machine).
- **Principle II (hardware independence)**: the shared conformance suite
  (`harness::conformance::run_all`) is what each driver of items 011-013 must pass, together
  with a `RecordedTransport` script of its protocol; `tests/transport_driver.rs` is the
  worked example.
- **Principle XI (virtual device)**: delivered, selectable like hardware, seedable, simulated
  mark on every capture; decoder round-trip tests come with items 007-008 through the
  `SignalSource` hook.
- **Platforms**: tests run locally on Windows 11; Debian 13 and the MSRV (1.85) job run in CI.
