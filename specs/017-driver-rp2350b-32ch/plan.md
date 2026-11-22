# Implementation Plan: 32-Channel RP2350B Logic Analyzer (Driver and Rust Firmware)

**Branch**: `017-driver-rp2350b-32ch` | **Date**: 2026-11-22 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `specs/017-driver-rp2350b-32ch/spec.md`

## Summary

Four pieces of work, delivered in this order because each needs the one before it (research R12):

1. **A wider application** (FR-025, FR-026): captures of up to 160 channels. The sample stays a 32-bit
   word, a capture of more than 32 channels delivers several words per sample (`words_per_sample`), and the
   channel mask becomes a `ChannelSet`, a type used only through its methods whose size follows one constant. Nothing changes for 32 channels or fewer, in
   behaviour or in the files.
2. **A richer device abstraction** (FR-003, FR-009, FR-010): the pattern and fast pattern triggers, a count of
   samples before and after the trigger, bursts with their times, and a buffer depth that follows the rate,
   declared by capabilities and enforced by the core, as the single-channel triggers are now.
3. **The firmware and the driver** (FR-001 to FR-021): a `no_std` Rust firmware for the RP2350B in a workspace of
   its own, a protocol crate shared by the firmware and the driver (the original gusmanb protocol extended for
   32 channels), the driver `driver-rp2350b` with recorded-transport tests, and the tools for building and
   flashing.
4. **Chaining** (FR-022 to FR-024): up to five boards presented as one device, merged by the driver.

The pieces are separate pull requests (or more); each leaves the application working, and only the third
needs the board.

## Technical Context

**Language/Version**: Rust, stable. The host crates keep MSRV 1.85 where they have it. The firmware is a
separate workspace with its own pinned toolchain (the `thumbv8m.main-none-eabihf` target), not bound by the
MSRV of the host (R4).

**Primary Dependencies**: host: none new (`serialport` and `nusb` are already used by `driver-weact`; this
driver talks to a serial port). Firmware: an embedded runtime for the RP2350 (research R4 names the choice and
its alternatives), the `pio` assembler crate, a panic and logging layer; each is checked by `cargo deny` for
licence and advisories, and recorded with its reason (Principle V).

**Storage**: results of the work on sessions: the description of a capture (`enabled_mask`) and the `VIEW` and
`LAYT` sections gain a wide form (R9); files of 32 channels or fewer are written as before.

**Testing**: `cargo test` for the wide channels (unit, property and recorded sessions), for the abstraction
(validation tests of every new capability), for the protocol crate (round trip of every frame, fuzzing of the
parser, which builds for the host and for the target), for the driver (a recorded transport and a simulated
device in the tests), for the firmware's logic crate (on the host); a build of the firmware in CI; hardware
tests are opt-in and run a rig (R10).

**Target Platform**: host: Windows 10/11 and Debian 13. Device: RP2350B (WeAct RP2350B Core Board).

**Project Type**: desktop-app plus an embedded firmware (a second Cargo workspace in the same repository).

**Performance Goals**: sampling 100 MHz for up to 24 channels with no lost sample; the highest rate for 32
channels measured and declared (aim 100 MHz); the transfer of a capture as short as the link allows, with the
compact coding (R7); a capture of 160 channels as usable as one of 32 (no cost for channels not shown).

**Constraints**: the firmware does not panic and does not hang on any input (a panic handler restarts the
board into a state the driver finds again); the host never indexes with external data; no new dependency
without a reason; the pins take 3.3 V.

**Scale/Scope**: five boards of 32 channels (160); up to about 2 million samples in the external memory at
the lower rates and about 120 thousand in the internal memory; one new host crate, one new protocol crate, one
firmware workspace, changes in `la-storage`, `la-devices`, `la-session`, `la-host`, `la-app`.

## Constitution Check

*GATE: passed before Phase 0 research and re-checked after Phase 1 design.*

- [x] **I. Stability & Data Integrity**: the protocol parser is total (any bytes give a frame, an error or a
  request for more, never a panic), tested by fuzzing for the host and the target; the firmware's panic
  handler restarts the board; the wide storage keeps the checked arithmetic of the narrow one; sessions of the
  old width read and write as before. Nothing writes files other than the existing session path.
- [x] **II. Hardware**: a dedicated driver behind the common abstraction; it declares channels, rates,
  triggers and buffer depth and the core enforces them (the abstraction grows to describe them, R8); a
  recorded-transport test (no hardware) and a manual procedure with a rig (`quickstart.md`, `verification.md`);
  no change to the behaviour of another driver (their tests are part of the gate).
- [x] **III. Test-First**: the tasks put the failing test before each change; the wide-channel work starts
  from tests that fail at 33 channels.
- [x] **IV. Architecture**: device logic stays in the driver crate; the abstraction is extended, not bypassed;
  work is off the interface thread (the supervisor of `la-devices`); the host API grows additively (a minor
  version) and the decoder API is untouched.
- [x] **V. Simplicity**: no new host dependency; the firmware's dependencies are those the target needs and
  are justified in R4; one shared protocol crate instead of two copies of the frames; the optional compact
  coding is in because it is what makes a large capture usable (R7).
- [x] **VI. Observability**: the firmware reports errors by named codes in its replies (never signal data in
  logs); the driver logs codes and counts.
- [x] **VII. Localization**: the device name, the driver's errors and the notices of the chain are in the six
  languages; the parity, budget and font checks cover them.
- [x] **VIII. Rendering**: the lane layout, the planner and the legend only touch the channels that are shown;
  a benchmark with 160 channels is added; the complexity of drawing is unchanged.
- [x] **IX. Decoding**: decoders read channels by number (`u8`, already enough for 160) and need no change;
  a test runs one on channel 150.
- [x] **X. Export**: not affected beyond the channel count (the export of 009 will take it from the capture).
- [x] **XI. Virtual device**: the virtual device is extended to the wide channel counts so that the interface
  and the storage can be tested at 160 channels without hardware; its existing behaviour at 32 or fewer is
  unchanged. Generated data stays labelled as simulated.
- [x] **XII. Original design, careful inspiration**: the firmware and the protocol crate are written from the
  behaviour of the original and from the facts of its protocol, not from its code; what was read is recorded
  in `research.md`; the original (GPL-3.0) and this project (GPL-3.0-only) have compatible licences, and the
  plan still copies none of it.
- [x] **Platforms**: nothing in the host code is platform-specific; the serial port is opened by the
  platform's own layer; the access instructions cover both systems.

## Project Structure

### Documentation (this feature)

```text
specs/017-driver-rp2350b-32ch/
├── spec.md
├── plan.md                    # this file
├── research.md                # R1 to R12
├── data-model.md              # ChannelSet, words per sample, requests, info reply, chain, errors
├── quickstart.md              # how to build, test, flash and verify
├── contracts/
│   ├── protocol.md            # the original protocol and its extension, byte by byte
│   ├── device-abstraction.md  # what la-devices gains
│   └── firmware.md            # layout, build and flash of the firmware, panics, limits
├── checklists/requirements.md
└── tasks.md                   # by /speckit-tasks
```

### Source Code (repository root)

```text
crates/
├── storage/        # words_per_sample, wide ingest, MAX_CHANNELS 160            (stage 1)
├── devices/        # ChannelSet, triggers, pre/post samples, bursts, virtual 160 (stages 1, 2)
├── session/        # wide enabled mask in the description section              (stage 1)
├── host/           # config, presentation, VIEW and LAYT sections, chain settings (stages 1, 4)
├── app/            # channel buttons and legend for up to 160 channels           (stage 1)
├── la-proto/       # NEW no_std: frames, requests, replies, limits, compact coding (stage 3)
└── driver-rp2350b/ # NEW: serial transport, recorded transport, driver, chain      (stages 3, 4)
firmware/rp2350b/   # NEW workspace (own Cargo.lock and toolchain): la-fw (the board), la-fw-logic
                    # (no_std logic tested on the host), la-fw-rig (the pattern generator of the rig)
resources/i18n/     # new texts in six languages
docs/drivers/rp2350b.md, docs/drivers/rp2350b-verification.md
```

**Structure Decision**: the host side follows `driver-weact`; the protocol lives in one `no_std` crate that
both sides depend on, so the frames cannot drift apart; the firmware is a separate workspace because it builds
for another target with another toolchain and must not make `cargo test --workspace` need it.

## Complexity Tracking

| Violation | Why needed | Simpler alternative rejected because |
|-----------|------------|--------------------------------------|
| A second Cargo workspace and a toolchain pinned for the firmware | The firmware is `no_std` for a Cortex-M33 target; its dependencies and its toolchain differ from the host's | One workspace would make every host build and CI job need the target and the embedded crates, and would tie the host's MSRV to them |
| Changing the sample word contract of storage and devices (`words_per_sample`) although 32 bits were enough for every device so far | 160 channels do not fit in 32 bits; the spec asks for them (FR-025) | A wider single integer (`u128`) stops at 128; a capture per group of 32 channels would split one device into several and break the alignment of channels |
| Extending the device abstraction (patterns, pre/post samples, bursts) | The original's functions, which the spec requires (FR-008 to FR-010), are not expressible as one channel and one kind | Putting them in `DeviceOptions` only would hide them from the core's validation, against Principle II |
