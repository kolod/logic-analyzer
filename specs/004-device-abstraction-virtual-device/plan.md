# Implementation Plan: Device Abstraction and Virtual Device

**Branch**: `004-device-abstraction-virtual-device` | **Date**: 2026-11-14 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `/specs/004-device-abstraction-virtual-device/spec.md`

## Summary

Add the crate `la-devices`: the common device contract (`DeviceProvider`, `Device`,
`Transport`), capability declaration with pure up-front validation, a registry with a
monitor thread for discovery and hot-plug events, and an acquisition engine that runs each
acquisition on two supervised threads: a **driver thread** (the only code that touches the
driver, abandonable) and an **ingest thread** (owns the `la_storage::Capture`, records
gaps, closes the capture, publishes the outcome). Losses are always accounted
(`try_send` backpressure turns an overloaded store into recorded gaps), driver panics and
hangs are contained, and every failure leaves the capture closed as *interrupted*, which
needs one small additive change in `la-storage`. The always-available virtual device
generates deterministic, seedable signals (integer-only generator, chunking-independent),
can script failures and triggers, labels its data as simulated, and exposes the
signal-source hook that decoders will use. A feature-gated harness (`RecordedTransport`,
`ScriptedDevice`, `CountingDevice`, `TestProvider`, shared conformance suite) makes the
three mandatory drivers cheap to test without hardware. Details and rationale:
[research.md](research.md).

## Technical Context

**Language/Version**: Rust, edition 2024; MSRV 1.85; toolchain pinned in
`rust-toolchain.toml` (as in items 001 and 002)

**Primary Dependencies**: `la-core` (typed errors), `la-diagnostics` (`spawn_worker`,
`lock_recover`, logging), `la-storage` (the capture), `tracing`. No new third-party
dependencies: `std` threads, `mpsc`, atomics, `Mutex`, `Condvar`.

**Storage**: none of its own; samples go to `la_storage::Capture` (in memory)

**Testing**: `cargo test` — conformance suite, counting-device validation tests,
scripted-fault tests (including a 1,000-panic loop), determinism tests, a hot-plug
sequence test and latency tests; a dependency-free `harness = false` benchmark for the
virtual device's throughput

**Target Platform**: Windows 10/11 and Debian 13, 64-bit x86; the virtual device and the
registry are pure `std` and behave identically on both; OS-specific hot-plug sources will
live in the hardware providers of items 011–013

**Project Type**: library crate in the desktop-app workspace (UI-independent core)

**Performance Goals**: every UI-facing call under 10 ms even with a hanging driver (SC-003);
stop within 100 ms on the virtual device (SC-004); virtual device at least 100 MS/s for a
sparse 32-channel signal in fast mode (SC-010), on the maintainer machine recorded in the
benchmark baseline

**Constraints**: no panic on external input; no `unsafe`; the driver is never called from
the UI thread; the acquisition is never slowed by event or progress consumers; bounded
memory (channel capacity and chunk size are limits; the capture has the storage budget);
all time bounds are explicit constants/options with documented defaults

**Scale/Scope**: up to 32 channels per device, several simultaneous acquisitions (one per
device), chunks up to the storage maximum (65,536 samples by default); about 3,000 lines of
Rust including tests and the harness

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

Verified against `.specify/memory/constitution.md` v1.4.1. Result: **PASS** (two justified
items in Complexity Tracking, no principle is deviated from).

- [x] **I. Stability & Data Integrity**: failures are first-class: disconnect, transport
  errors, overruns, panics, hangs and storage limits all leave the delivered data intact,
  record exact losses and close the capture as interrupted; positions stored plus lost
  equal positions accounted for (invariant tested); driver panic and hang contained on a
  separate abandonable thread; no `unsafe`; lint policy applies.
- [x] **II. Hardware**: this is the common device abstraction; UI and core depend only on
  it; capabilities are declared and enforced before hardware is touched; the harness and
  the shared conformance suite give each future driver hardware-free tests; adding a driver
  changes no other driver. (The three drivers themselves are items 011–013.)
- [x] **III. Test-First**: tests precede implementation per user story; conformance,
  fault-injection (including 1,000 panics), determinism and hot-plug sequence tests; fmt +
  clippy + test in CI on both OSes.
- [x] **IV. Architecture**: logic in a UI-independent crate; device I/O never on the UI
  thread and always cancellable; bounded channels and bounded chunk memory; OS-specific
  code stays in providers (none needed here); no host API contract change (the Rust API is
  documented in contracts/).
- [x] **V. Simplicity**: no new dependencies; one acquisition model (two threads and a
  bounded channel) reused by every device; complexity justified below.
- [x] **VI. Observability**: lifecycle and failure events logged with metadata only;
  stable error codes with actionable English `detail`.
- [x] **VII. Localization**: no UI strings; errors carry stable `device.*` and `storage.*`
  codes for the localization layer; docs and comments in English.
- [x] **VIII. Rendering**: not applicable here; live viewing during an acquisition uses the
  item 002 snapshots through `Acquisition::capture()`.
- [x] **IX. Decoding**: not applicable here; the signal-source hook is the seam decoders'
  generators will plug into.
- [x] **X. Export**: not applicable.
- [x] **XI. Virtual device**: delivered here: always available, selectable like hardware,
  seedable and reproducible, scripted failures, labelled simulated in the capture
  description, and a generator hook for decoders (the decoder round-trip tests come with
  items 007–008).
- [x] **XII. Original design & third-party code**: original design; no code reused from
  PulseView, sigrok or Saleae; no new third-party crates.
- [x] **Platforms**: pure `std`; behaves identically on Windows 10/11 and Debian 13; no
  OS-specific code in this feature; device-access documentation will come with each driver.

Post-design re-check (after Phase 1): unchanged — PASS.

## Project Structure

### Documentation (this feature)

```text
specs/004-device-abstraction-virtual-device/
├── plan.md              # This file (/speckit-plan command output)
├── research.md          # Phase 0 output (/speckit-plan command)
├── data-model.md        # Phase 1 output (/speckit-plan command)
├── quickstart.md        # Phase 1 output (/speckit-plan command)
├── contracts/           # Phase 1 output (/speckit-plan command)
│   ├── device-contract.md
│   ├── acquisition-api.md
│   └── virtual-device-and-harness.md
├── checklists/
│   └── requirements.md
└── tasks.md             # Phase 2 output (/speckit-tasks command - NOT created by /speckit-plan)
```

### Source Code (repository root)

```text
Cargo.toml                          # workspace (members = crates/*, unchanged)

crates/storage/                     # la-storage: one additive change (research R6)
└── src/{capture.rs,types.rs}       #   Capture::close_interrupted(), CaptureStatus::interrupted
    tests/interrupted.rs            #   new test for the flag

crates/devices/                     # package la-devices
├── Cargo.toml                      # deps: la-core, la-diagnostics, la-storage, tracing;
│                                   #   feature test-support; [[bench]] virtual_throughput
├── src/
│   ├── lib.rs                      # public API re-exports, crate docs
│   ├── error.rs                    # device.* error codes and constructors
│   ├── capabilities.rs             # Capabilities, SampleRates, TriggerKind, validation of a descriptor
│   ├── config.rs                   # DeviceConfig, Trigger, ResolvedConfig, validate()
│   ├── driver.rs                   # Device, DeviceProvider, ReadOutcome, Transport, AppliedConfig
│   ├── registry.rs                 # DeviceRegistry, monitor thread, DeviceEvent, cached list
│   ├── acquisition.rs              # Acquisition handle, state, progress, events, outcome
│   ├── supervisor.rs               # driver thread loop, ingest thread, watchdog, accounting
│   ├── description.rs              # CaptureDescription (private fields, no setters)
│   ├── virtual_device/
│   │   ├── mod.rs                  # VirtualProvider, VirtualDevice, VirtualOptions
│   │   ├── generator.rs            # integer random generator (SplitMix64, run-filling)
│   │   ├── source.rs               # SignalSource and SignalSourceFactory hook
│   │   └── script.rs               # ScriptedFailure, trigger search, pacing
│   └── harness/                    # (feature test-support)
│       ├── mod.rs
│       ├── transport.rs            # RecordedTransport, TransportStep
│       ├── scripted.rs             # ScriptedDevice, Step, CountingDevice
│       ├── provider.rs             # TestProvider
│       └── conformance.rs          # shared conformance checks
├── tests/
│   ├── common/mod.rs               # helpers (waiting for outcomes, standard configs)
│   ├── discovery.rs                # US1
│   ├── config_validation.rs        # US2
│   ├── acquisition.rs              # US3
│   ├── failures.rs                 # US4
│   ├── panic_injection.rs          # US4, SC-007
│   ├── virtual_device.rs           # US5, US6
│   ├── hotplug.rs                  # US7
│   ├── conformance.rs              # US8
│   └── ui_latency.rs               # SC-003, SC-004
└── benches/
    ├── virtual_throughput.rs       # SC-010 (harness = false)
    └── baseline.json               # recorded reference measurements
```

**Structure Decision**: one new library crate `crates/devices` (package `la-devices`)
following the workspace layout in the plan template and roadmap; hardware drivers will be
added as modules under `crates/devices/src/drivers/` or as separate crates depending on
`la-devices` (decided by items 011–013). The only change outside the new crate is the small
additive interrupted-flag change in `la-storage`.

## Complexity Tracking

| Violation | Why Needed | Simpler Alternative Rejected Because |
|-----------|------------|-------------------------------------|
| Two threads per acquisition (driver thread and ingest thread) joined by a bounded channel | A hung driver must never hold the capture hostage: the ingest thread owns the capture and can close it when the driver thread stops responding; `try_send` turns store backpressure into recorded gaps instead of making the device wait | A single worker doing both would leave a capture that can never be closed after a driver hang, and a slow store would stall the device and cause silent overruns |
| Additive change to `la-storage` (`close_interrupted`, `CaptureStatus::interrupted`) in a feature about devices | The constitution requires a capture that lost data to a failure to be flagged incomplete; a disconnect that loses no *recorded* position would otherwise read as complete to every consumer of the capture status | Recording an invented terminal gap would invent lost positions; keeping the flag only in the acquisition outcome would mislead any consumer that reads the capture alone |

## Assumptions Recorded During Planning

- A driver instance is created per acquisition by `DeviceProvider::open`; a hung instance is
  abandoned and the next start opens a fresh one.
- Validation is pure and uses the worst-case storage estimate (`length × channels × 4.03`
  bytes) for finite captures unless the caller accepts the budget risk.
- Defaults: read timeout 50 ms, stall timeout 2 s, scan interval 500 ms, scan timeout 1 s;
  all are options so tests can shorten them.
- The capture description is kept alongside the capture (private fields, no setters) until
  the session file format (item 003) persists it.
- Hot-plug is delivered as pushed events plus scan reconciliation; the virtual device never
  disappears.
