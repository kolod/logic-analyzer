# Research: Device Abstraction and Virtual Device

All Technical Context unknowns are resolved below.

## R1. Crate and ownership model

- **Decision**: one new library crate `crates/devices` (package `la-devices`) holding the
  device contract, discovery, acquisition supervision, the virtual device and the test
  harness. Hardware drivers (items 011–013) become modules or crates that depend on it.
  A driver instance is created by a factory **per acquisition** (`DeviceProvider::open`)
  and moved into the acquisition's driver thread; it is dropped when the acquisition
  ends. The registry only tracks which device identities are busy.
- **Rationale**: no shared mutable driver state, nothing to hand back after a hang (a hung
  instance is simply abandoned), one acquisition per device is a simple busy flag, and
  independent devices cannot interfere (FR-014, FR-021).
- **Alternatives considered**: long-lived device objects shared between UI and worker
  (needs locking around every driver call; a hung driver would hold the lock and block
  the UI); one crate per driver from the start (premature, the contract is what must be
  stable first).

## R2. The device contract: a blocking pull model with a bounded read time

- **Decision**: the `Device` trait is small and synchronous:
  `descriptor()`, `start(&ResolvedConfig) -> Result<AppliedConfig>`,
  `read(&mut self, max_samples: usize, out: &mut Vec<u32>) -> Result<ReadOutcome>` and
  `stop(&mut self) -> Result<()>`. `read` must return within a documented bound (the
  *read timeout*, 50 ms by default); `ReadOutcome` is `Samples`, `Lost(n)`, `Idle` or
  `Finished`. Words follow the storage layout: bit `i` is channel `i`, disabled channels
  are zero. A `Transport` trait (`read`/`write` with timeout, scripted by the harness)
  gives driver authors a ready seam for USB or serial adapters.
- **Rationale**: a bounded `read` makes stop and watchdog logic trivial and portable
  (no async runtime, no cross-thread interruption); `Lost(n)` is the one place where a
  driver reports overruns; `Idle` lets a device that waits for a trigger prove it is alive.
- **Alternatives considered**: push model with callbacks from the driver (callbacks
  re-enter our code from driver threads, harder to contain); `async` (a runtime
  dependency and a colouring problem for drivers; the project avoids new dependencies).

## R3. Two threads per acquisition: driver thread and ingest thread

- **Decision**: an acquisition runs two supervised workers (`la_diagnostics::spawn_worker`):
  - the **driver thread** is the only code that calls the driver. It runs
    `open` (done by the provider) → `start` → loop `read` → `stop`, and sends messages
    through a bounded `std::sync::mpsc::sync_channel`: `Samples(Vec<u32>)`, `Lost(u64)`,
    `Finished`, `Failed(Error)`;
  - the **ingest thread** never calls the driver. It owns the `la_storage::Capture`, appends
    chunks, records gaps, updates progress, closes the capture and publishes the outcome.
- **Containment**: a driver panic ends only the driver thread (caught by `spawn_worker`,
  reported as `device.driver_panic`); a driver hang leaves only the driver thread stuck:
  the ingest thread has a watchdog (R4), closes the capture with what it has, reports
  `device.unresponsive`, and abandons the driver thread. The caller is never blocked.
- **Backpressure and accounting**: the driver thread uses `try_send`. If the channel is
  full (the store is slower than the device) the chunk is *not* dropped silently: its
  length is added to a pending-loss counter and sent as `Lost(n)` before the next chunk,
  so the order is preserved and `samples stored + positions lost == positions the device
  accounted for` (FR-019). The device is never made to wait for the store.
- **Rationale**: puts the untrusted code (driver) on a thread that can be abandoned and
  keeps the user's data (the capture) on a thread that cannot hang because of a driver.
- **Alternatives considered**: a single worker doing both (a hung driver would also hold
  the capture, which then could never be closed); an unbounded channel (a slow store would
  grow memory without limit, against Principle IV).

## R4. Stop, timeouts and the watchdog

- **Decision**: `stop()` only sets an atomic flag and returns at once. The driver loop
  checks it between reads, so a stop takes effect within one read timeout (50 ms) plus
  the time of `Device::stop`. The ingest thread keeps a heartbeat: the driver thread
  stores a timestamp after every loop iteration (including `Idle`). If no heartbeat is
  seen for the *stall timeout* (default 2 s, configurable) the ingest thread declares the
  driver unresponsive. `Acquisition::wait_timeout(d)` lets the UI wait without blocking
  forever. All UI-facing calls (list, configure, start, stop, progress) only touch
  atomics, caches or short locks and therefore return within milliseconds (SC-003).
- **Rationale**: a heartbeat distinguishes a legitimately idle device (waiting for a
  trigger) from a hung one without any cross-thread interruption.
- **Alternatives considered**: killing the thread (not possible in safe Rust and unsafe on
  every OS); a fixed total acquisition timeout (breaks long captures).

## R5. Capabilities, validation and the memory estimate

- **Decision**: `Capabilities` = channel count (1..=32), supported sample rates (an
  explicit list or a range), supported trigger kinds (none, level, rising, falling,
  any edge) and an optional buffer depth in samples (`None` for streaming devices).
  `validate(&Capabilities, &DeviceConfig, &Limits) -> Result<ResolvedConfig>` is pure
  (no device call) and checks: enabled channels exist and at least one is enabled, the
  sample rate is supported (never rounded silently; the error lists the supported rates
  or the nearest one), the trigger kind is supported and its channel is enabled, a
  finite capture length is required and within the buffer depth for buffered devices,
  and the worst-case storage cost of a finite capture
  (`length × channels × 4.03` bytes, see item 002) fits the memory budget unless the
  caller explicitly accepts the risk (`accept_budget_risk`), in which case the storage's
  own clean stop (item 002) applies. A device description with zero channels, no sample
  rate or an inconsistent range is rejected when it is listed (`device.invalid_capabilities`).
  `ResolvedConfig` is what the device will use; for devices that cannot round, it equals
  the request.
- **Rationale**: pure validation makes "no device call for invalid configuration"
  (FR-006) true by construction and trivially testable with a counting device.
- **Alternatives considered**: letting the driver validate (hardware could be touched
  before rejection, and every driver would re-implement the rules); estimating memory
  optimistically (would admit captures the store then rejects mid-run).

## R6. Storage integration and the "incomplete" flag

- **Decision**: the capture is created with `channels = capabilities.channels` and the
  resolved sample rate; disabled channels read as constant zero and the description lists
  the enabled mask. Chunks larger than the store's maximum are split. `Lost(n)` calls
  `Capture::record_gap(n)`. Finishing normally or by user stop closes the capture
  (`complete` if nothing was lost). **Failures** (disconnect, transport error, driver
  panic, hang, storage limit) close the capture as *interrupted*.
- **Cross-crate change**: `la-storage` gets one additive, backwards-compatible item:
  `Capture::close_interrupted()` and `CaptureStatus::interrupted`, with
  `complete = closed && lost_positions == 0 && !stopped_by_limit && !interrupted`.
  Without it a disconnect that loses no recorded position would leave the capture
  reading as complete, which the constitution forbids. The item 002 contract and tests
  are updated in the same change.
- **Rationale**: the completeness flag must live where every consumer reads it (the
  capture status), not only in the acquisition outcome.
- **Alternatives considered**: recording an invented terminal gap of length 1 (invents
  lost positions); keeping the flag only in the acquisition outcome (a consumer reading
  the capture alone would be misled).

## R7. Discovery, registry and hot-plug events

- **Decision**: `DeviceProvider` is the seam for a family of devices: `scan() ->
  Result<Vec<DeviceDescriptor>>`, `open(&DeviceId) -> Result<Box<dyn Device>>`, and an
  optional `watch(sink)` through which a provider pushes `Appeared`/`Disappeared` events.
  `DeviceRegistry` aggregates providers. A **monitor thread** scans every provider (each
  scan runs on its own contained worker with a timeout), reconciles the result with the
  known set, and publishes `DeviceEvent`s through a channel; pushed events and scan
  differences are deduplicated against the known set, so none is lost (pushed events carry
  rapid sequences, scans catch silent changes), none is duplicated and order is preserved
  per source. `list()` returns the monitor's cached list, so it is instant and never
  waits for a device (FR-003, SC-003). The virtual device is provided by a built-in
  provider that always reports it and never reports it disappearing.
- **Stable identity**: `DeviceId` is a provider prefix plus a provider-defined unique
  string (for USB: vendor/product plus serial where present), equal across listings and
  reconnects (FR-002).
- **Rationale**: real OS hot-plug notification differs per platform and per bus; the
  registry defines the contract and the reconciliation, drivers add the OS-specific
  source later (Principle IV: OS-specific code stays in providers).
- **Alternatives considered**: polling only (cannot guarantee that 100 rapid events are
  all delivered); push only (a missed notification would leave the list stale).

## R8. The virtual device

- **Decision**: `VirtualDevice` implements `Device`; its provider reports one device
  (`virtual:0`, 32 channels, sample rates 1 Hz to 500 MHz as a range, all trigger kinds,
  streaming). Settings: `seed`, `signal source`, `failure script`, `pacing`.
  - **Generator** (default): per channel the level flips after a pseudo-random number of
    samples, `1 + rand % (2 × mean_gap − 1)` with `mean_gap` configurable (default 50).
    All math is integer (SplitMix64 to derive per-channel streams from the seed, xorshift
    for draws), so it is identical on every platform and, because state advances per
    channel, identical for any chunk size. Chunks are filled by run (constant stretches)
    plus one XOR per flip, so generation costs O(samples + flips) and is fast enough for
    SC-010.
  - **Signal-source hook** (`SignalSource` trait with a factory): `fill(start_position,
    out)`; decoders and their generators (items 007–008) implement it; the default
    random generator is one implementation, so the acquisition path never changes.
  - **Failure script**: a list of `(position, failure)` with `Loss{n}`, `Overrun{n}` (a loss
    reported as an overrun), `TransportError`, `Disconnect`; nothing fails unless
    scripted (FR-028).
  - **Triggers**: level or edge on one channel; the device generates and discards samples
    until the condition holds (bounded by a search limit, then `device.trigger_timeout`),
    capturing starts at the trigger sample, and the number of samples skipped is reported.
  - **Pacing**: `Fast` (as fast as the store accepts) or `Realtime` (paced to the sample
    rate in slices of at most 10 ms so stop stays responsive).
- **Simulated mark**: the acquisition builds the `CaptureDescription` from the device
  descriptor; its fields are private and have no setters, so a consumer cannot clear
  `simulated`; real devices carry `false` (FR-027).
- **Alternatives considered**: floating-point geometric gaps (platform-dependent `ln`,
  would break cross-platform reproducibility); per-sample per-channel random draws (about
  32 draws per sample, far too slow for 100 MS/s).

## R9. Test harness and conformance suite

- **Decision**: behind the feature `test-support`:
  - `Transport` trait (`read`/`write` with timeout) and `RecordedTransport` that replays
    a scripted sequence of byte chunks, short reads, timeouts and disconnects, so a
    driver's protocol code is testable without hardware;
  - `ScriptedDevice` (a `Device`) that replays a script of `Samples`, `Lost`, `Idle`,
    `Error`, `Panic` and `Hang` steps, plus a `CountingDevice` wrapper that records every
    call (used to prove invalid configurations never reach a device);
  - `TestProvider` with scripted appear/disappear sequences;
  - `conformance`: functions that take a device factory and run the shared checks
    (descriptor sanity, capability enforcement with zero device calls, start/stop,
    completion, failure handling, panic containment), used for the virtual device and the
    scripted devices here and by every driver crate later.
- **Rationale**: makes every later driver cheap to test (Principle II) and gives one place
  that defines "behaves like a device".
- **Alternatives considered**: per-driver ad-hoc mocks (inconsistent coverage); recording
  real traffic as the only test source (needs hardware to create new cases).

## R10. Progress, events and the outcome

- **Decision**: progress is a pair of atomics (state, samples captured) readable at any
  time without locks. Events (`Started`, `GapRecorded`, `Finished`, `Failed`) go through a
  small bounded channel with `try_send`: best effort, never blocking the acquisition
  (FR-016); the authoritative result is the `AcquisitionOutcome`, stored once and
  available through `outcome()` and `wait_timeout`, so a slow or absent event consumer
  can never make the user miss the result.
- **Alternatives considered**: an unbounded event channel (a vanished consumer would grow
  memory); events as the only result channel (could be lost).

## R11. Dependencies, logging and the reuse of item 001/002

- **Decision**: no new third-party dependency: `std` threads, `mpsc`, atomics, `Mutex`,
  `Condvar`; `la-core` (errors), `la-diagnostics` (`spawn_worker`, `lock_recover`,
  logging), `la-storage`, `tracing`. Log events (device found/lost, acquisition started,
  gap, failure, finished) carry metadata only (ids, counts, positions, codes), never
  samples.
- **Rationale**: Principle V; the workers and logging already exist.

## R12. Error codes

- **Decision**: stable codes: `device.invalid_capabilities`, `device.invalid_channel`,
  `device.unsupported_sample_rate`, `device.unsupported_trigger`,
  `device.invalid_capture_length`, `device.memory_budget`, `device.not_found`,
  `device.busy`, `device.invalid_state`, `device.disconnected`, `device.transport`,
  `device.driver_error`, `device.driver_panic`, `device.unresponsive`,
  `device.trigger_timeout`, plus storage codes passed through unchanged. Kinds:
  `InvalidInput` (configuration), `Device` (disconnect, transport, driver), `Resource`
  (memory budget), `Internal` (never reachable from external input). The `detail` text is
  English and actionable (what is wrong, what is allowed); user-facing text is derived from
  the code by the localization layer later.

## R13. Verification of timing and containment

- **Decision**: SC-003 (UI calls under 10 ms) and SC-004 (stop within 100 ms) are asserted
  in tests with generous margins in debug builds and measured precisely in a benchmark;
  SC-007 (1,000 injected panics) is a loop test over `ScriptedDevice` panic steps at
  varied positions; SC-010 (virtual device at 100 MS/s sparse on 32 channels) is measured
  by `benches/virtual_throughput.rs` with a recorded baseline in the existing benchmark
  style (informational, not a required check), and the deterministic parts (accounting,
  reproducibility, simulated mark) are ordinary tests.
