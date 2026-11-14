---

description: "Task list for Device Abstraction and Virtual Device"
---

# Tasks: Device Abstraction and Virtual Device

**Input**: Design documents from `/specs/004-device-abstraction-virtual-device/`

**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/, quickstart.md

**Tests**: REQUIRED. Constitution Principle III (test-first): in every phase the test tasks
come first and MUST fail (usually they do not compile yet) before the implementation tasks
start. If a test passes immediately because earlier work already covers it, run the
mutation check described in that phase before relying on it.

**Organization**: tasks are grouped by user story (US1–US8, numbered as in spec.md).
Phases are ordered by priority and by dependency: the virtual device core and the harness
are Foundational because several stories need them.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: can run in parallel (different files, no dependency on an incomplete task)
- **[Story]**: user story from spec.md
- All paths are relative to the repository root; the new crate is `crates/devices`
  (package `la-devices`)

## Path Conventions

Source in `crates/devices/src/`, tests in `crates/devices/tests/`, benchmarks in
`crates/devices/benches/`; one small change in `crates/storage/`. The contracts are
`specs/004-device-abstraction-virtual-device/contracts/device-contract.md`
("the device contract"), `acquisition-api.md` ("the API contract") and
`virtual-device-and-harness.md` ("the virtual device contract").

## Lint reminder (applies to every Rust task)

The workspace denies `unwrap`, `expect`, indexing (`v[i]`), unchecked arithmetic, `panic!`
and `unsafe` in production code. Use `get`, `checked_*`, `saturating_*`, `u128`
intermediates and `?`. Test code may relax this with the file-level
`#![allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing,
clippy::arithmetic_side_effects, clippy::panic)]` used by the item-001/002 tests. Deliberate
panics in harness code (the `Panic` step) use `std::panic::panic_any` inside the
`test-support` harness with a comment, never in production paths. Run
`cargo clippy --workspace --all-targets --all-features -- -D warnings` at the end of every
phase and do not commit while it fails (use `#[expect(dead_code, reason = "...")]` for code
that a later phase of this feature uses; remove it when it becomes used).

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: the new crate and its wiring

- [X] T001 Create `crates/devices/Cargo.toml` (package `la-devices`, `edition/rust-version/license/publish` inherited from the workspace, `[lints] workspace = true`, dependencies `la-core`, `la-diagnostics`, `la-storage` as path dependencies and `tracing` with the version used by `crates/storage/Cargo.toml`, feature `test-support = []`, dev-dependency `la-devices = { path = ".", features = ["test-support"] }`, and `[[bench]] name = "virtual_throughput" harness = false`) and `crates/devices/src/lib.rs` with a crate-level English doc comment and `#![deny(missing_docs)]`; add the stub `crates/devices/benches/virtual_throughput.rs` (`fn main() {}`)
- [X] T002 Run `cargo build --workspace`, `cargo clippy --workspace --all-targets --all-features -- -D warnings` and `cargo deny --all-features check`; `Cargo.lock` must gain only the new workspace member (no third-party crate)

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: the storage flag, the shared types, the device contract, the harness basics and
the virtual device core that US1–US8 build on

**⚠️ CRITICAL**: no user story work can begin until this phase is complete

### Tests first

- [X] T003 [P] Write `crates/storage/tests/interrupted.rs` (research R6): `Capture::close_interrupted()` closes the capture and returns a status with `closed == true`, `interrupted == true` and `complete == false` even when no position was lost; all data appended before stays readable and exact (compare with the `ReferenceCapture` from `tests/common/mod.rs`); `append` and `record_gap` afterwards fail with `storage.closed`; `close()` after `close_interrupted()` returns the same interrupted status; `close_interrupted()` on an already closed capture changes nothing; a normal `close()` keeps `interrupted == false`; the snapshot status shows the flag; a capture that is interrupted and also stopped by the limit reports both
- [X] T004 [P] Write unit tests in `crates/devices/src/capabilities.rs` (`#[cfg(test)]`) for `DeviceDescriptor::validate`: zero channels, 33 channels, an empty rate list, an unsorted or duplicated rate list, a zero rate, a range with `min == 0` or `min > max`, an empty name, `buffer_depth = Some(0)` each fail with code `device.invalid_capabilities`; a valid descriptor (1 and 32 channels, list and range, with and without triggers, streaming and buffered) passes
- [X] T005 [P] Write unit tests in `crates/devices/src/virtual_device/generator.rs` (`#[cfg(test)]`): disabled channels are always zero; with `mean_gap = 1` every enabled channel flips at every sample; gaps between flips always lie in `1 ..= 2 * mean_gap - 1` and their average is close to `mean_gap`; the same seed gives identical output and different seeds differ; filling a stream in different chunk splits (1, 7, 4096, one piece) gives identical samples; seeds `0` and `u64::MAX` work

### Implementation

- [X] T006 Implement in `crates/storage/src/{types.rs,capture.rs}`: `CaptureStatus::interrupted: bool` (rustdoc), `Capture::close_interrupted(&mut self) -> Result<CaptureStatus>` (marks the capture interrupted, closes it, publishes, logs a `capture interrupted` event with metadata only; no-op returning the current status if already closed) and `complete = closed && lost_positions == 0 && !stopped_by_limit && !interrupted`; update `specs/002-signal-storage-core/contracts/storage-api.md` and the data model note in `specs/002-signal-storage-core/data-model.md`; make T003 pass and confirm every existing storage test still passes
- [X] T007 [P] Implement `crates/devices/src/error.rs`: the stable `pub const` error codes from research R12 (`device.invalid_capabilities`, `device.invalid_channel`, `device.unsupported_sample_rate`, `device.unsupported_trigger`, `device.invalid_capture_length`, `device.memory_budget`, `device.not_found`, `device.busy`, `device.invalid_state`, `device.disconnected`, `device.transport`, `device.driver_error`, `device.driver_panic`, `device.unresponsive`, `device.trigger_timeout`) and `pub(crate)` constructors returning `la_core::Error` with the right `ErrorKind` (`InvalidInput`, `Device`, `Resource`, `Internal`) and an English, actionable `detail` that says what is wrong and what is allowed
- [X] T008 [P] Implement `crates/devices/src/capabilities.rs`: `DeviceId` (opaque string newtype, `new(provider, unique)`, `Display`, `Hash`, `Eq`), `Capabilities`, `SampleRates { List(Vec<u64>), Range { min, max } }` with `contains(rate)` and `nearest(rate)`, `TriggerKind { Level(bool), RisingEdge, FallingEdge, AnyEdge }`, `DeviceDescriptor { id, name, simulated, capabilities }` and `DeviceDescriptor::validate(&self) -> Result<()>` per data-model.md (makes T004 pass)
- [X] T009 [P] Implement the virtual device option types in `crates/devices/src/virtual_device/mod.rs`: `VirtualOptions` (`seed`, `source: Option<Arc<dyn SignalSourceFactory>>`, `mean_gap`, `failures: Vec<ScriptedFailure>`, `pacing`, `trigger_search_limit`) with `Default` per data-model.md and a `validate()` (`mean_gap` in `1..=1_000_000`), `Pacing { Fast, Realtime }`, `ScriptedFailure { position, kind }` with kinds `Loss(n)`, `Overrun(n)`, `TransportError`, `Disconnect`, and the `SignalSource` / `SignalSourceFactory` traits in `crates/devices/src/virtual_device/source.rs` exactly as in the virtual device contract
- [X] T010 Implement `crates/devices/src/config.rs` types (no validation yet): `DeviceConfig` (`enabled_channels: u32`, `sample_rate_hz`, `trigger: Option<Trigger>`, `capture_length: Option<u64>`, `options: DeviceOptions`, `accept_budget_risk: bool`), `Trigger { channel: u8, kind: TriggerKind }`, `DeviceOptions` (`#[non_exhaustive]`: `None`, `Virtual(VirtualOptions)`), `ResolvedConfig` (private fields, getters, a `pub(crate)` constructor) and `AppliedConfig { sample_rate_hz }` (depends on T008, T009)
- [X] T011 Implement `crates/devices/src/driver.rs`: the `Device`, `DeviceProvider` (`name`, `scan`, `open`, default `watch`) and `Transport` traits and `ReadOutcome { Samples, Lost(u64), Idle, Finished }` exactly as in the device contract, the constants `READ_TIMEOUT` (50 ms) and `START_STOP_TIMEOUT` (2 s), `DeviceEvent { Appeared(DeviceDescriptor), Disappeared(DeviceId) }` and `DeviceEventSink` (a cloneable wrapper around `std::sync::mpsc::Sender<DeviceEvent>` whose `send` never panics and returns a bool) (depends on T008, T010)
- [X] T012 Implement the harness basics behind the `test-support` feature in `crates/devices/src/harness/{mod.rs,scripted.rs,provider.rs}`: `ScriptedDevice::new(descriptor, steps)` with `Step { Samples(Vec<u32>), Lost(u64), Idle, Error(la_core::Error), Panic, Hang(Duration), Finish }` (a `Hang` sleeps in slices of 10 ms so abandoned threads end once a shared "release" flag is set, and `Panic` uses `std::panic::panic_any` with a comment that this is deliberate test-only fault injection), `CountingDevice` (wraps a device and counts `start`/`read`/`stop` calls through `CallCounts`), and `TestProvider` (`plug`, `unplug`, scripted descriptors, `set_scan_hang(bool)`, `set_scan_error(bool)`, counts of `scan` and `open` calls, pushes events through the `watch` sink); add small self-tests for each
- [X] T013 Implement the virtual device core: `crates/devices/src/virtual_device/generator.rs` (SplitMix64 to derive a stream per channel from the seed, xorshift64 for draws, per-channel level and next-flip position, chunks filled by constant runs plus one XOR per flip, gaps `1 + draw % (2 * mean_gap - 1)`, disabled channels zero; makes T005 pass) and, in `virtual_device/mod.rs`, `VirtualProvider` (`name() == "virtual"`, `scan` returns the one descriptor `virtual:0`, name "Virtual device", `simulated = true`, 32 channels, rates `Range { min: 1, max: 500_000_000 }`, all trigger kinds, streaming; `open` returns a `VirtualDevice`) and `VirtualDevice` implementing `Device` in fast mode (`start` builds the generator from the resolved config and seed and returns the applied rate, `read` fills up to `max_samples` and returns `Samples`, a finite `capture_length` ends with `Finished`, `stop` is a no-op); no failures, triggers, pacing or source hook yet (they arrive in US5/US6)
- [X] T014 Export the public surface from `crates/devices/src/lib.rs` (modules `error`, `capabilities`, `config`, `driver`, `virtual_device`, and `harness` behind `#[cfg(feature = "test-support")]`, with `pub use` for the main types), then make T004, T005 and the harness self-tests compile and pass with `cargo test -p la-devices --lib`; run clippy with `-D warnings`

**Checkpoint**: foundation ready — user story work can begin

---

## Phase 3: User Story 1 - Discover and select devices (Priority: P1) 🎯 MVP

**Goal**: list devices (always including the virtual one) with stable identity and
capabilities, without ever waiting for a misbehaving device.

**Independent Test**: `cargo test -p la-devices --test discovery` (quickstart step 1).

### Tests for User Story 1 (write FIRST, ensure they FAIL)

- [X] T015 [P] [US1] Create `crates/devices/tests/common/mod.rs` (with the lint relaxation and `dead_code` allowed): helpers `registry_fast()` (a `RegistryConfig` with `scan_interval` 10 ms and `scan_timeout` 100 ms), `wait_until(timeout, || condition)`, `descriptor(id, channels)` building a valid `DeviceDescriptor`, `std_config(channels_mask, rate)` building a valid `DeviceConfig`, and `wait_outcome(&Acquisition)` that waits up to 10 s and fails the test on timeout
- [X] T016 [P] [US1] Write `crates/devices/tests/discovery.rs`: with no provider added `list()` is exactly the virtual device (`virtual:0`, simulated, 32 channels, full capabilities); a `TestProvider` with two devices adds them in the same shape with `simulated == false`; identities are equal across two listings and across `unplug`/`plug` of the same device; `describe(&id)` returns the descriptor and `None` for an unknown id; a provider whose descriptor fails `validate` (zero channels) is omitted while the healthy ones are listed; a provider whose `scan` hangs and one whose `scan` returns an error are omitted, the other devices are still listed, and `list()` called while a scan hangs returns in under 10 ms; the list is sorted deterministically (virtual device first, then by id)

### Implementation for User Story 1

- [X] T017 [US1] Implement `crates/devices/src/registry.rs`: `RegistryConfig { scan_interval, scan_timeout }` (defaults 500 ms and 1 s), `DeviceRegistry::new` (registers `VirtualProvider`, starts the monitor thread with `la_diagnostics::spawn_worker`), `add_provider`, `list` (returns a clone of the cached list from an `RwLock`, poisoned locks recovered), `describe`, and the monitor loop: every `scan_interval` each provider's `scan` runs on its own `spawn_worker` and is waited for with `recv_timeout(scan_timeout)`; a late provider is marked unresponsive, omitted, logged once and its worker abandoned; descriptors that fail `validate` are dropped and logged; the cache is replaced atomically; the monitor stops when the registry is dropped (no thread leak in tests)
- [X] T018 [US1] Export `DeviceRegistry` and `RegistryConfig` from `crates/devices/src/lib.rs`, make T016 pass and run clippy with `-D warnings`

**Checkpoint**: User Story 1 works and is verifiable on its own

---

## Phase 4: User Story 2 - Configure within declared capabilities (Priority: P1)

**Goal**: validate a configuration against the capabilities before any device call.

**Independent Test**: `cargo test -p la-devices --test config_validation` (quickstart step 2).

### Tests for User Story 2 (write FIRST, ensure they FAIL)

- [ ] T019 [P] [US2] Write `crates/devices/tests/config_validation.rs`: a table of at least 30 invalid configurations against a device with 8 channels, rates `[1_000, 1_000_000, 100_000_000]`, triggers `{RisingEdge, FallingEdge}` and a buffer depth of 1,000,000 samples, each with its expected code: no channel enabled, channel 8 enabled (ninth), mask bit 31 on an 8-channel device, rate 0, rate not in the list (also a rate between two listed ones, and `u64::MAX`), trigger kind `AnyEdge` or `Level(true)` not supported, trigger on a disabled channel, trigger channel 8, capture length 0, length 1,000,001 (above the depth), `None` length on the buffered device, and over-budget finite lengths on a streaming device with a 1 MiB budget (the error carries `device.memory_budget`); each error's `detail` names the offending value and lists the allowed ones (assert the text contains the supported rates or the limit); the same device accepts all valid combinations, including `accept_budget_risk = true` for an over-budget length, and the resolved configuration equals the request
- [ ] T020 [P] [US2] Add to `crates/devices/tests/config_validation.rs` a registry-level test: starting with an invalid configuration through `registry.start(...)` (the function arrives in US3, so use the `validate` function directly for now and extend the test in T029) fails with the same codes and a `TestProvider` shows `open` was never called; a range-capability device accepts any rate in the range and rejects `min - 1` and `max + 1`

### Implementation for User Story 2

- [ ] T021 [US2] Implement `validate(caps: &Capabilities, config: &DeviceConfig, limits: &la_storage::Limits) -> la_core::Result<ResolvedConfig>` in `crates/devices/src/config.rs` per research R5 and data-model.md: channel mask checks, supported rate (never rounded; `detail` lists the supported rates or the nearest one), trigger kind and channel, capture length rules for buffered and streaming devices, the worst-case memory estimate (`length × channels × 4.03` bytes, computed with checked arithmetic) against `limits.memory_budget_bytes` unless `accept_budget_risk`, and the `DeviceOptions::Virtual` options validated through `VirtualOptions::validate`; the function makes no device call and cannot panic on any input
- [ ] T022 [US2] Export `validate` from `crates/devices/src/lib.rs`, make T019–T020 pass and run clippy with `-D warnings`

**Checkpoint**: User Stories 1 and 2 work independently

---

## Phase 5: User Story 3 - Run a capture into the signal store, in the background (Priority: P1)

**Goal**: start, observe, stop and finish an acquisition whose samples flow into a
`la_storage::Capture`, with no call blocking the caller.

**Independent Test**: `cargo test -p la-devices --test acquisition` (quickstart step 3).

### Tests for User Story 3 (write FIRST, ensure they FAIL)

- [ ] T023 [P] [US3] Write `crates/devices/tests/acquisition.rs` using the virtual device: `registry.start(...)` returns in under 10 ms and the state becomes `Running`; `progress().samples_captured` grows while another thread reads `acquisition.capture().snapshot()` and every snapshot is consistent (length equals a chunk boundary sum, levels match the deterministic generator for the same seed); `stop()` returns at once, the capture is closed within 100 ms and the outcome is `Stopped` with `complete == true`; a finite `capture_length` finishes by itself with outcome `Completed`, exactly that many positions and `complete == true`; the `CaptureDescription` reports the device id, name, requested and actual rate, channel count and enabled mask; a second `start` on the same device while one runs fails with `device.busy`, and after the outcome the device can be started again; unknown device id fails with `device.not_found`; two devices (virtual plus a `ScriptedDevice` from a `TestProvider`) run at the same time and independently; chunks larger than the storage maximum are split (use a scripted device that returns 200,000 samples per read with the default 65,536 limit)
- [ ] T024 [P] [US3] Add to `crates/devices/tests/acquisition.rs`: the storage budget ending an acquisition (a 1 MiB budget with `accept_budget_risk = true` and a very dense signal): the outcome is `Failed` with `storage.memory_limit`, the data stored before is intact and readable (verify the first 1,000 positions against the generator), and the capture status has `stopped_by_limit == true` and `complete == false`; `stop()` on an acquisition that already finished is harmless; `stop()` twice is harmless; events `Started` and `Finished` arrive on `events()`; a consumer that never reads `events()` does not slow or block the acquisition (run 50 short acquisitions without draining events)

### Implementation for User Story 3

- [ ] T025 [P] [US3] Implement `crates/devices/src/description.rs`: `CaptureDescription` with private fields and getters only (`device_id`, `device_name`, `simulated`, `seed`, `requested_sample_rate_hz`, `sample_rate_hz`, `channels`, `enabled_mask`, `trigger`, `samples_before_trigger`, `started_at`), a `pub(crate)` constructor used only by the acquisition, and a `compile_fail` doctest showing that a consumer cannot construct it or change `simulated`
- [ ] T026 [US3] Implement `crates/devices/src/acquisition.rs`: `AcquisitionOptions` (+ `Default`: `limits` default, `read_timeout` 50 ms, `stall_timeout` 2 s, `channel_capacity` 8, `chunk_samples` 16,384 clamped to `limits.max_chunk_samples`), `AcquisitionState`, `Progress`, `AcquisitionEvent`, `EndReason`, `AcquisitionOutcome`, and the `Acquisition` handle (`progress` from atomics, `description`, `capture` (the `CaptureReader`), `events` (a bounded `sync_channel(256)` filled with `try_send`, never blocking), `stop` (sets an atomic flag), `outcome` and `wait_timeout` over a `Mutex<Option<AcquisitionOutcome>>` plus `Condvar` with recovered poison) (depends on T025)
- [ ] T027 [US3] Implement `crates/devices/src/supervisor.rs` (normal path): the driver thread (`spawn_worker`) calls `Device::start`, loops `read` (checking the stop flag between reads, updating the heartbeat atomic after every iteration), sends `Samples` chunks through a bounded `sync_channel`, calls `Device::stop` and sends `Finished`; the ingest thread (`spawn_worker`) owns the `Capture` (created with `channels = capabilities.channels`, the applied rate and `options.limits`), appends chunks in order (splitting any chunk above `limits.max_chunk_samples`), updates progress, closes the capture on `Finished`/stop (`complete` if nothing lost), builds and publishes the `AcquisitionOutcome`, emits `Started`/`Finished` events and releases the device's busy flag; a storage error ends the acquisition cleanly (stop requested, capture closed, outcome `Failed` with the storage code)
- [ ] T028 [US3] Implement `DeviceRegistry::start(&self, id, config, options) -> Result<Acquisition>` in `crates/devices/src/registry.rs`: look up the descriptor (`device.not_found`), `validate` the configuration against its capabilities and `options.limits`, claim the device (`device.busy` if already claimed, atomic test-and-set so two simultaneous starts yield exactly one), call `DeviceProvider::open` on the driver thread (so a slow `open` cannot block the caller), and return the `Acquisition` immediately in state `Starting`; the busy flag is released when the outcome is set
- [ ] T029 [US3] Extend the registry-level invalid-configuration test of T020 to use `registry.start` (assert `provider.open` calls stay 0 for every invalid case of T019), export `Acquisition`, `AcquisitionOptions`, `AcquisitionState`, `AcquisitionEvent`, `AcquisitionOutcome`, `EndReason`, `Progress`, `CaptureDescription` from `crates/devices/src/lib.rs`, make T023–T024 pass and run clippy with `-D warnings`

**Checkpoint**: User Stories 1–3 work independently (the core flow with the virtual device)

---

## Phase 6: User Story 4 - Failures keep data and are never silent (Priority: P1)

**Goal**: any device failure keeps the delivered data, records exact losses, flags the capture
incomplete, reports an actionable error and never affects the application.

**Independent Test**: `cargo test -p la-devices --test failures --test panic_injection` (quickstart step 4).

### Tests for User Story 4 (write FIRST, ensure they FAIL)

- [ ] T030 [P] [US4] Write `crates/devices/tests/failures.rs` with `ScriptedDevice`s (the expected stream is the concatenation of the scripted `Samples` steps): for each of disconnect (`Error(device.disconnected)`), transport error, generic driver error, `Lost(n)` (overrun), `Panic`, `Hang`, and a failing `start` the outcome is `Failed` (except `Lost`, which continues and ends `Completed`), the stored samples before the failure equal the scripted prefix exactly (compare every position), the capture is closed, `complete == false`, `capture_status.interrupted == true` for failures and `lost_positions == n` for losses, the error has the stable code and an actionable English `detail`, and the application keeps running; `Lost(n)` is recorded as a gap of exactly `n` positions at exactly the right position and the acquisition continues; the accounting invariant `delivered_samples + lost_positions == positions == capture length` holds in every case; a hang during run: `stop()` returns at once, `wait_timeout` returns the `device.unresponsive` outcome within `stall_timeout` plus 500 ms (use a 200 ms stall timeout) and the capture is closed with what it had; a `Hang` inside `Device::stop` after a user stop yields a timeout outcome without blocking the caller; a failure right after a `Lost`, a disconnect during start and one immediately after the stop request each end with a defined outcome
- [ ] T031 [P] [US4] Add to `crates/devices/tests/failures.rs` the backpressure case: a scripted device producing chunks faster than a deliberately slow storage consumer (use `channel_capacity = 1` and a device that returns a chunk every call without sleeping, with `chunk_samples` small) never blocks the device, the excess is recorded as gaps, the capture is incomplete, and `delivered_samples + lost_positions == positions` still holds exactly; a storage budget hit during a run keeps the same invariant
- [ ] T032 [P] [US4] Write `crates/devices/tests/panic_injection.rs`: 1,000 acquisitions on `ScriptedDevice`s that panic at varied points (before the first read, after random counts of `Samples` steps, inside `stop`), run with `la_diagnostics::install_panic_hook()`; every one ends `Failed` with `device.driver_panic`, the samples before the panic read back exactly, none crashes the test process, other acquisitions running at the same time are unaffected, and the log contains one `ERROR` entry per panic; the test finishes within a documented time budget

### Implementation for User Story 4

- [ ] T033 [US4] In `crates/devices/src/supervisor.rs` map driver results to typed outcomes: `Err` from `start`/`read`/`stop` becomes `Failed` with the driver's own `device.*` code (any other error code is wrapped as `device.driver_error` keeping the original as the cause), the driver thread then calls `Device::stop` best effort, and the ingest thread closes the capture with `Capture::close_interrupted()` for every failure (storage-limit failures keep `stopped_by_limit` and close normally as in US3)
- [ ] T034 [US4] In `crates/devices/src/supervisor.rs` detect a driver panic: when the channel disconnects without `Finished`/`Failed`, the ingest thread joins the driver `WorkerHandle` (it has ended), turns the `WorkerFailure` into `device.driver_panic` (message and location in the `detail`) and closes the capture interrupted
- [ ] T035 [US4] In `crates/devices/src/supervisor.rs` implement the watchdog: the ingest thread waits on the channel with `recv_timeout` slices, compares the driver heartbeat age with `options.stall_timeout`, and on a stall abandons the driver thread (drops its handle without joining), reports `device.unresponsive`, closes the capture interrupted and releases the busy flag; the same applies after a user stop if the driver does not finish in time
- [ ] T036 [US4] In `crates/devices/src/supervisor.rs` implement lossless accounting: the driver thread uses `try_send`; on `Full` the chunk length is added to a pending-loss counter and sent as `Lost(n)` before the next chunk (also flushed before `Finished`); `Lost(n)` from the driver and pending losses call `Capture::record_gap` in order and emit `GapRecorded { position, length }`; the outcome fields `delivered_samples`, `lost_positions` and `positions` are filled and the invariant is asserted by `debug_assert!`-free explicit tests only (no panic in production code)
- [ ] T037 [US4] Make T030–T032 pass; run clippy with `-D warnings`; log every failure with metadata only (device id, code, counts, positions) through `tracing`

**Checkpoint**: all failure paths are verified; User Stories 1–4 work independently

---

## Phase 7: User Story 5 - A reproducible, clearly labelled virtual device (Priority: P1)

**Goal**: deterministic virtual device, labelled as simulated, with the signal-source hook and
both pacing modes.

**Independent Test**: `cargo test -p la-devices --test virtual_device` (quickstart step 5, determinism part).

### Tests for User Story 5 (write FIRST, ensure they FAIL)

- [ ] T038 [P] [US5] Write `crates/devices/tests/virtual_device.rs` (determinism part): 100 acquisitions with the same seed and configuration to a fixed length give identical capture contents (compare `transitions` of every enabled channel and `level_at` on a dense sample); different seeds differ; the result is identical for different `chunk_samples` (1, 7, 4,096, 65,536) and for both pacing modes (`Realtime` at 1 MHz for 20,000 samples takes at least 15 ms and at most 5 s, and gives the same samples as `Fast`); seeds `0` and `u64::MAX` work; one enabled channel and all 32 work; with no signal source the enabled channels carry random signals and the disabled ones stay zero
- [ ] T039 [P] [US5] Add to `crates/devices/tests/virtual_device.rs` the labelling part: every capture from the virtual device has `description().simulated == true`, its device id `virtual:0` and its seed; captures from `ScriptedDevice` and `CountingDevice` have `simulated == false` and `seed == None`; `CaptureDescription` has no way to be changed (covered by the `compile_fail` doctest of T025, assert it exists with `cargo test --doc`); the signal-source hook: a custom `SignalSourceFactory` producing a known counter pattern is used instead of the random generator and the capture equals the pattern exactly for chunk sizes 1, 100 and 65,536; a source supplied through `DeviceOptions::Virtual` is created with the enabled mask, sample rate and seed of the acquisition
- [ ] T040 [P] [US5] Add to `crates/devices/tests/virtual_device.rs`: `VirtualOptions::validate` rejects `mean_gap` 0 and 1,000,001 with a typed error and accepts the boundaries; `DeviceOptions::Virtual` on a non-virtual device is rejected with `device.invalid_state`-class input error (`ErrorKind::InvalidInput`)

### Implementation for User Story 5

- [ ] T041 [US5] Implement the signal-source hook in `crates/devices/src/virtual_device/mod.rs`: when `VirtualOptions::source` is `Some(factory)`, `VirtualDevice::start` creates the source with `(enabled_mask, sample_rate_hz, seed)` and `read` fills chunks through `SignalSource::fill(start, out)`; otherwise the generator of T013 is used
- [ ] T042 [US5] Implement `Pacing::Realtime` in `crates/devices/src/virtual_device/mod.rs`: samples are released at the configured rate (computed with integer arithmetic from elapsed time), waiting happens in slices of at most 10 ms and `read` returns `Idle` when nothing is due, so the driver loop keeps checking the stop flag
- [ ] T043 [US5] Set the simulated mark and seed in the `CaptureDescription` built by the acquisition from the `DeviceDescriptor` and the options (`simulated` from the descriptor, `seed` from `DeviceOptions::Virtual`), validate the options in `validate` (T021 already calls `VirtualOptions::validate`), make T038–T040 pass and run clippy with `-D warnings`

**Checkpoint**: virtual device behaviour is reproducible and labelled

---

## Phase 8: User Story 6 - Simulate failures and triggers on demand (Priority: P2)

**Goal**: scripted failures and simple triggers on the virtual device.

**Independent Test**: `cargo test -p la-devices --test virtual_device` (failure/trigger part).

### Tests for User Story 6 (write FIRST, ensure they FAIL)

- [ ] T044 [P] [US6] Add to `crates/devices/tests/virtual_device.rs` the failure script tests: `Loss(1000)` at position 50,000 yields a gap of exactly 1,000 positions starting exactly at 50,000 (positions count capture samples) and the capture is incomplete with `lost_positions == 1000`; `Overrun(n)` behaves like `Loss(n)` and is logged as an overrun; `TransportError` ends the acquisition with `device.transport` and `Disconnect` with `device.disconnected` after exactly the scripted number of samples, with the data before intact and the capture interrupted; several failures in one run (a loss, then a disconnect) apply in position order; with an empty script the device never fails in 100 runs
- [ ] T045 [P] [US6] Add trigger tests: a `RisingEdge` trigger on channel 3 starts capturing at a sample where channel 3 goes low to high (verify against an independent generation of the same seed: the first capture sample has channel 3 high and the previous skipped sample had it low), `FallingEdge` and `Level(true)`/`Level(false)` likewise, `AnyEdge` on any change; `samples_before_trigger` in the description equals the number of discarded samples (verified by regenerating the stream); a trigger that never occurs (`mean_gap = 1_000_000` and `trigger_search_limit = 1_000`) makes the acquisition fail with `device.trigger_timeout`; the trigger channel must be enabled (validated by `validate`)

### Implementation for User Story 6

- [ ] T046 [US6] Implement `crates/devices/src/virtual_device/script.rs`: the failure script applied as the device passes each position (sorted by position, positions counted in captured samples after the trigger): `Loss`/`Overrun` make the next `read` return `Lost(n)` and advance the generator or source by `n` samples, `TransportError` and `Disconnect` return the matching typed error; `VirtualDevice::read` consults the script before generating (depends on T013, T041)
- [ ] T047 [US6] Implement the trigger search in `crates/devices/src/virtual_device/script.rs` and `mod.rs`: before capturing the device generates and discards samples until the condition holds (level or edge per `TriggerKind`, on the one channel), up to `trigger_search_limit` (then `device.trigger_timeout` from `start`), keeps the number of discarded samples and hands it to the acquisition so it appears as `samples_before_trigger` in the description (via `AppliedConfig`, extended with `samples_before_trigger`), then capturing starts at the trigger sample
- [ ] T048 [US6] Make T044–T045 pass and run clippy with `-D warnings`; log scripted failures at `WARN` with metadata only

**Checkpoint**: failure and trigger paths are exercisable without hardware

---

## Phase 9: User Story 7 - Hot-plug awareness (Priority: P2)

**Goal**: ordered, lossless, duplicate-free device appear/disappear events.

**Independent Test**: `cargo test -p la-devices --test hotplug` (quickstart step 6).

### Tests for User Story 7 (write FIRST, ensure they FAIL)

- [ ] T049 [P] [US7] Write `crates/devices/tests/hotplug.rs`: `plug` of a device produces exactly one `Appeared` with the descriptor and `unplug` exactly one `Disappeared` with the id, and `list()` reflects both; 100 rapid plug/unplug pairs (200 events) pushed in a tight loop are delivered exactly once each, in order (compare with the pushed sequence); a provider that does not push events (scan-only) has its changes detected by the periodic scan and reported once; a pushed event that the scan also sees produces no duplicate; a pushed `Disappeared` for an unknown id and a pushed `Appeared` for a known id are ignored; the virtual device never produces `Disappeared` and is never reported twice; two providers pushing concurrently keep per-source order and lose nothing; an unplug during a running acquisition leaves the acquisition to fail through the device path (scripted `Error(device.disconnected)`) without affecting the registry

### Implementation for User Story 7

- [ ] T050 [US7] Implement hot-plug in `crates/devices/src/registry.rs`: `DeviceRegistry::events()` returns the receiver of the registry's event channel; `add_provider` calls `DeviceProvider::watch` with a sink that forwards into the monitor; the monitor keeps the known set, applies pushed events and scan results through one reconciliation function that emits `Appeared` only for an unknown id and `Disappeared` only for a known one (never for the virtual device), preserves per-source order and updates the cached list in the same step; sending into a dropped receiver never panics or blocks
- [ ] T051 [US7] Make T049 pass, run clippy with `-D warnings` and confirm the monitor shuts down cleanly when the registry is dropped (no leaked thread after 100 create/drop cycles in a test)

**Checkpoint**: discovery is live

---

## Phase 10: User Story 8 - Cheap to add a new driver, testable without hardware (Priority: P2)

**Goal**: a transport-level harness and a shared conformance suite any driver can run.

**Independent Test**: `cargo test -p la-devices --test conformance` (quickstart step 6).

### Tests for User Story 8 (write FIRST, ensure they FAIL)

- [ ] T052 [P] [US8] Write `crates/devices/tests/conformance.rs`: `conformance::run_all` passes for the virtual provider and for a `ScriptedDevice`-based provider; a deliberately broken device (a `ScriptedDevice` whose descriptor claims 0 channels, one that ignores the stop flag, one that reports `Lost` without a gap) makes the matching conformance check fail (assert the failure message names the broken rule), so the suite is not vacuous
- [ ] T053 [P] [US8] Write `crates/devices/tests/transport_driver.rs` with a tiny example driver defined in the test (`TransportDevice<T: Transport>` that speaks a trivial protocol: write a start command, read framed sample bytes, treat a short read as partial data, a timeout as `Idle`, a disconnect as `device.disconnected`): replaying a `RecordedTransport` script (bytes, short read, timeout, disconnect) reproduces exactly the same samples, gaps and errors on every one of 20 runs; the bytes the driver wrote are available through `written()` and equal the protocol's start command; the driver passes `conformance::run_all` built on a `RecordedTransport` factory

### Implementation for User Story 8

- [ ] T054 [US8] Implement `crates/devices/src/harness/transport.rs`: `RecordedTransport` and `TransportStep { Bytes(Vec<u8>), ShortRead(usize), Timeout, Disconnect, Error(String) }` implementing `Transport` per the virtual device contract (a `Bytes` step may be consumed over several reads, a `ShortRead(n)` limits the next read to `n` bytes, `Timeout` returns `device.transport` with a timeout detail or a zero-length read as defined in the rustdoc, `Disconnect` returns `device.disconnected`, writes are recorded), plus `written()`
- [ ] T055 [US8] Implement `crates/devices/src/harness/conformance.rs`: `Expectations` and `run_all(provider, id, expectations)` running the shared checks with the real registry and acquisition engine: descriptor sanity (`validate`), capability enforcement with zero `open` calls, start/stop within the documented bound, finite completion, loss accounting (`delivered + lost == positions`), failure handling for disconnect and driver error, panic containment and hang containment (when the expectations allow fault injection through a wrapper), each check returning a descriptive `Result<(), String>` and `run_all` panicking with all failures listed (it is test support, so panicking is its contract)
- [ ] T056 [US8] Document in rustdoc how a driver author uses `Transport`, `RecordedTransport` and `conformance::run_all` (a worked example), make T052–T053 pass and run clippy with `-D warnings`

**Checkpoint**: all user stories (US1–US8) are independently functional

---

## Phase 11: Polish & Cross-Cutting Concerns

**Purpose**: latency and throughput evidence, documentation, constitution gates

- [ ] T057 [P] Write `crates/devices/tests/ui_latency.rs` (SC-003, SC-004): with a `TestProvider` whose `scan` and `open` hang and a `ScriptedDevice` that hangs in `read`, every UI-facing call (`list`, `describe`, `validate`, `start`, `stop`, `progress`, `outcome`) returns in under 10 ms in release builds and under 50 ms in debug builds (two named constants); on the virtual device `stop()` to a closed capture takes under 100 ms in release and 500 ms in debug; record the observed numbers in `specs/004-device-abstraction-virtual-device/checklists/verification.md`
- [ ] T058 [P] Implement `crates/devices/benches/virtual_throughput.rs` (`harness = false`, same style as `crates/storage/benches/ingest.rs`): the virtual device in fast mode through the real acquisition into a capture with a sparse 32-channel signal (`mean_gap` 1,000) and a typical one (`mean_gap` 50), 1, 8, 16 and 32 channels, reporting million samples per second and the number of recorded losses, writing `target/la-devices-bench/virtual.json` and merging a `virtual` section into `crates/devices/benches/baseline.json` with `--write-baseline`; fails if SC-010 (at least 100 MS/s sparse on 32 channels without recorded losses) is not met or a result is more than 5x slower than the baseline
- [ ] T059 Run the benchmark on the maintainer machine, commit the first `crates/devices/benches/baseline.json` (machine description and numbers), and record the SC-010 result in `specs/004-device-abstraction-virtual-device/checklists/verification.md`; if SC-010 is not met, profile the generator and the supervisor and record what limits it before deciding whether to adjust the criterion honestly (as was done for SC-006 of item 002)
- [ ] T060 [P] Extend `.github/workflows/bench.yml` with a step `cargo bench -p la-devices --bench virtual_throughput` (non-blocking, artifacts include `target/la-devices-bench/*.json`) and document it in `specs/001-workspace-ci-foundation/contracts/ci-checks.md`; do not add it to `scripts/protection.json`
- [ ] T061 [P] Review the public API against `specs/004-device-abstraction-virtual-device/contracts/*.md` and update the contracts where the implementation differs (names, signatures, extra codes, behaviour fixed during implementation); ensure rustdoc exists for every public item (`#![deny(missing_docs)]`) and that `RUSTDOCFLAGS="-D warnings" cargo doc -p la-devices --no-deps --all-features` passes
- [ ] T062 [P] Data-safety review (constitution Principle I): search `crates/devices/src` for `unwrap(`, `expect(`, `panic!`, `todo!`, `unimplemented!`, `unreachable!`, `unsafe`, `#[allow`, `#![allow` and `#[expect`; matches are allowed only in `#[cfg(test)]` modules and in the clearly commented `harness` fault-injection code (`Step::Panic`, `conformance::run_all`); record the result in `specs/004-device-abstraction-virtual-device/checklists/verification.md`
- [ ] T063 [P] Record the remaining constitution gates in the same checklist: localization (no UI strings, stable `device.*` codes, English docs), third-party code (`Cargo.lock` gains only workspace packages; `cargo deny list` before and after; nothing reused from PulseView/sigrok/Saleae), Principle II (the shared conformance suite is what each driver in items 011–013 must pass), Principle XI (virtual device delivered; decoder round-trip tests come with items 007–008), platforms (Windows locally, Debian 13 and MSRV 1.85 through CI)
- [ ] T064 Run the whole quickstart (`specs/004-device-abstraction-virtual-device/quickstart.md`, steps 1–8) on Windows 10/11 and rely on CI for Debian 13 and the MSRV job; fix any gap and update the quickstart where reality differs
- [ ] T065 Update `roadmap.md` (mark item 004 implemented) and the `README.md` status line (devices crate, virtual device, no UI yet, no real drivers yet)
- [ ] T066 Final local gate: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets --all-features -- -D warnings`, `cargo test --workspace --all-features`, `cargo deny --all-features check`, `cargo +1.85 test --workspace`, then push the branch, open a pull request and wait for all 10 required checks on both operating systems

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: none
- **Foundational (Phase 2)**: depends on Setup — blocks all stories; the `la-storage` change (T003, T006) is a prerequisite of US4 and is done here so storage tests stay green from the start
- **US1 (Phase 3)**: depends on Foundational (needs `VirtualProvider`, `TestProvider`, `DeviceDescriptor::validate`)
- **US2 (Phase 4)**: depends on Foundational only (validation is pure); independent of US1
- **US3 (Phase 5)**: depends on US1 (registry) and US2 (`validate`) and on the virtual device core
- **US4 (Phase 6)**: depends on US3 (it hardens the supervisor) and on the `la-storage` change (T006)
- **US5 (Phase 7)**: depends on US3 (acquisitions to observe) and on Foundational (the generator); independent of US4
- **US6 (Phase 8)**: depends on US5 (it extends the virtual device) and on US4 (failure paths it triggers)
- **US7 (Phase 9)**: depends on US1 (extends the registry); independent of US2–US6
- **US8 (Phase 10)**: depends on US3 and US4 (the conformance suite runs acquisitions and fault injection)
- **Polish (Phase 11)**: depends on all stories

### Within Each User Story

- Tests first and failing before implementation
- Types and traits before behaviour, behaviour before the registry/supervisor integration
- Run clippy with `-D warnings` at the end of every phase and never commit a failing gate

### Parallel Opportunities

- Foundational tests T003, T004, T005 in parallel; implementation T007, T008, T009 in parallel
- US1 tests T015–T016; US2 tests T019–T020; US3 tests T023–T024 and implementation T025 in parallel
- After US1 and US2: US7 can proceed in parallel with US3/US4 (different parts of `registry.rs`: coordinate)
- After US3: US4 and US5 can proceed in parallel (different files: `supervisor.rs` versus `virtual_device`)
- Polish T057, T058, T060–T063 in parallel

---

## Parallel Example: User Story 4

```text
# Launch all tests for User Story 4 together:
Task: "failures table-driven tests in crates/devices/tests/failures.rs"
Task: "backpressure accounting test in crates/devices/tests/failures.rs"
Task: "1,000-panic loop in crates/devices/tests/panic_injection.rs"

# Then the supervisor changes (same file, sequential):
Task: "map driver results to typed outcomes in crates/devices/src/supervisor.rs"
Task: "detect a driver panic in crates/devices/src/supervisor.rs"
Task: "watchdog and abandonment in crates/devices/src/supervisor.rs"
Task: "lossless accounting in crates/devices/src/supervisor.rs"
```

---

## Implementation Strategy

### MVP First (US1 to US3)

1. Phase 1 Setup and Phase 2 Foundational (including the storage flag)
2. US1 discovery, US2 validation, US3 acquisition with the virtual device
3. **STOP and VALIDATE**: `cargo test -p la-devices --test discovery --test config_validation --test acquisition`: with no hardware a capture can be listed, configured, run and read.

### Incremental Delivery (all P1 stories first)

1. Foundational → virtual device core and harness basics
2. US1 → US2 → US3: the core flow
3. US4 → failure containment (the constitution's top priority)
4. US5 → reproducible, labelled virtual device
5. US6 → scripted failures and triggers; US7 → hot-plug; US8 → conformance suite for drivers
6. Polish → latency evidence, throughput benchmark, documentation, gates

### Notes

- [P] tasks = different files, no dependencies
- Commit after each task or logical group with Conventional Commits and the scope `devices`
  (or `storage` for T003/T006), for example `feat(devices): add the registry monitor`
- Do not commit while `cargo clippy ... -D warnings` fails; stage related tests and
  implementation together so every commit passes the gate
- Deliberate panics and hangs exist only in `harness` code; production paths never panic
- Avoid weakening a lint to make a test pass; `unsafe` is not needed anywhere in this feature
