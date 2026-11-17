# Implementation Plan: WeAct LogicAnalyzer Driver

**Branch**: `013-driver-weact` | **Date**: 2026-11-17 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `/specs/013-driver-weact/spec.md`

## Summary

A new crate `la-driver-weact` implements the device contract of feature 004 for the WeAct
LogicAnalyzer V1: an 8-channel, 24 MHz, streaming logic analyzer on a Cypress FX2 USB controller with
no firmware of its own. The driver lists boards by their position on the USB bus (stable across the
re-enumeration that loading firmware causes), prepares a blank board on a background worker by
uploading an open-source firmware image that ships with the application, and then streams samples
from one bulk endpoint into the acquisition engine. Failures (unplug, a stream that stops, a board
that does not come back, no access, no driver, another program holding the board) become typed
errors with advice in six languages. All USB access goes through a small seam of the driver's own,
so the driver is tested without hardware by replaying scripted exchanges, and by opt-in tests on the
maintainer's rig (an Arduino Nano that produces known patterns on the eight inputs and is controlled
over a serial port). USB access uses the pure-Rust `nusb` crate (WinUSB on Windows, usbfs on Linux),
so no C library is added; the Windows driver for the board is therefore WinUSB.

Findings of the study that shaped the design (details in [research.md](research.md)):

- The firmware **stops the capture** when the computer cannot keep up (it has no lost-sample
  counter and sends no sign), so there is no gap in the middle of a capture; spec FR-017 and
  User Story 3 were amended accordingly.
- `scan()` must return in about a second and `start`/`stop` in 2 s, while preparing a board takes
  seconds: preparation runs on a worker of the provider, never in `scan`.
- The maintainer's board currently has the driver **libusb-win32**, which neither `nusb` nor
  libusb-1.0 supports; the documentation and the "no driver" error say how to install WinUSB.

## Technical Context

**Language/Version**: Rust, stable; MSRV 1.85 for the new crate (as `la-devices` and `la-host`).

**Primary Dependencies**: `nusb` 0.2 (production; Apache-2.0 OR MIT; pure Rust, MSRV 1.85; the only
crate that touches USB, in one module). `serialport` 4 (**dev-dependency only**, for the rig tests;
MPL-2.0 is already allowed by `deny.toml`; built with `default-features = false` so Linux needs no
`libudev`). `sha2` (dev-dependency only, to check the checksum of the firmware image in a test).
Existing: `la-core`, `la-devices`, `la-diagnostics` (contained workers), `tracing`.

**Storage**: none new. The firmware image (about 7 KB) is a file of the repository embedded into the
crate with `include_bytes!`.

**Testing**: `cargo test` with no hardware: the driver's own tests replay scripted USB exchanges
(`RecordedUsb`), the shared conformance suite of `la-devices` runs against the driver, parsers of
device data have malformed/truncated/boundary tests, injected failures at every step of a capture.
Opt-in rig tests (`#[ignore]`, enabled by `LA_RIG_PORT`) run on the maintainer's machine only.

**Target Platform**: Windows 10/11 (WinUSB) and Debian 13 (usbfs with a udev rule); x86-64 both.

**Project Type**: desktop-app (Rust workspace: UI-independent core crates + UI shell). This feature
adds one driver crate and two lines of composition in the application.

**Performance Goals**: sustain 24 MS/s of 8-bit samples (24 MB/s, within USB 2.0 high-speed bulk)
without the computer being the cause of a stop on a directly connected port; a stop closes the
capture within the contract's limit (100 ms in replayed tests); the list call never waits for a board.

**Constraints**: no panic on any data from the device; no `unsafe` (the workspace denies it; `nusb`
is the safe wrapper); bounded memory (a fixed set of transfer buffers); `scan` under 1 s,
`start`/`stop` under 2 s, `read` under 50 ms; the application never blocks while a board is being
prepared or does not answer; no change to the device contract or to any other driver.

**Scale/Scope**: one device model, 8 channels, 16 sample rates, one firmware image, 9 new error
codes in six languages, two documents, one Arduino sketch.

## Constitution Check

*GATE: passed before Phase 0 research and re-checked after Phase 1 design.*

- [x] **I. Stability & Data Integrity**: every parser of data from the device returns typed errors
  and has malformed/truncated/boundary tests; no `unwrap`, indexing or unchecked arithmetic (the
  workspace lints apply); a stream that stops, an unplug or any driver fault ends the capture with
  what was captured kept and marked incomplete; the books balance (samples stored plus positions
  lost equal the positions claimed) because a stopped stream is closed with one gap of the
  never-captured positions; faults are contained by the registry's supervisor.
- [x] **II. Hardware**: the application depends only on the device abstraction; the driver declares
  its capabilities (8 channels, 16 exact rates up to 24 MHz, no trigger, streaming) and the core
  enforces them; hardware-free tests and a manual verification procedure are deliverables
  (FR-024, FR-025); no other driver or the contract changes (FR-028).
- [x] **III. Test-First**: the replay seam and the tests of each parser come before the driver code
  in `tasks.md`; the conformance suite and failure injection run on both operating systems in CI.
- [x] **IV. Architecture**: the logic is in a UI-independent crate; USB is behind a seam implemented
  once (`nusb`) and replayed in tests; preparation and streaming run on workers and are cancellable;
  memory is bounded; the host API does not change (the application registers the provider with a
  method that already exists).
- [x] **V. Simplicity**: one new production dependency (`nusb`: the only maintained pure-Rust USB
  crate, safe API, same MSRV, dual-licensed permissively; the alternative `rusb` needs the C
  library libusb and a system package or a C build on both systems). `serialport` and `sha2` are
  test-only. No HID/serial crate in production. Justification in [research.md](research.md) R3.
- [x] **VI. Observability**: the driver logs the board key, the state changes of preparation, the
  firmware upload and re-enumeration times, and every failure with its code and technical detail
  (never sample data); every user-visible failure has an actionable message in six languages.
- [x] **VII. Localization**: nine new error codes with messages in en/de/fr/es/ja/uk and a parity
  test; the device name is a proper name, not translated; documentation in English.
- [x] **VIII. Rendering**: not affected (no change to storage queries or the plan); samples enter
  the storage core exactly as for every other device.
- [x] **IX. Decoding**: not affected; decoders work on the capture like on any other (the
  acceptance of User Story 1 includes decoding a capture from this device on the rig).
- [x] **X. Export**: not affected.
- [x] **XI. Virtual device**: not affected; a capture from this driver is **not** simulated and the
  session records the device (FR-019). The rig patterns are test signal sources, not a second
  virtual device.
- [x] **XII. Original design & third-party code**: the driver is written from the protocol
  (reimplementation, not a port). Reused items are listed below with origin, version, reason and
  licence; no PulseView/libsigrok code is copied.
- [x] **Platforms**: Windows 10/11 and Debian 13; OS specifics are confined to the `nusb` module
  and to documentation/packaging (WinUSB via Zadig, a udev rule); device-access instructions are
  part of the deliverables.

### Third-party items (Principle XII)

| Item | Origin and version | Why reused | Licence | Handling |
|------|--------------------|------------|---------|----------|
| Firmware image `fx2lafw-sigrok-fx2-8ch.fw` | sigrok-firmware-fx2lafw 0.1.7 binary release (<https://sigrok.org/download/binary/sigrok-firmware-fx2lafw/>), built from <https://github.com/sigrokproject/sigrok-firmware-fx2lafw> | The board has no firmware; writing and building an FX2 firmware (8051 C, a GPIF waveform) is out of scope and error-prone; the vendor's own instructions use this firmware | GPL-2.0-or-later (the helper library fx2lib inside it LGPL-2.1-or-later); compatible with the project's GPL-3.0 | Embedded as data (loaded into another processor, not linked); the image, its SHA-256, the exact release and tag, the licence texts and an offer of the source are kept next to it in `crates/driver-weact/firmware/`; a test checks the checksum |
| Protocol of the firmware (vendor requests `0xB0`/`0xB1`/`0xB2`, the three-byte start payload, endpoint `0x82`, the delay rule) | Read from the firmware source and the libsigrok `fx2lafw` driver at the master of 2026-11-17 | The firmware defines it; there is no other description | GPL-2.0-or-later (facts, not copied code) | Reimplemented in safe Rust from the facts in `contracts/usb-protocol.md`; checked against the real board on the rig |
| `nusb` 0.2 | crates.io | USB access | Apache-2.0 OR MIT | `cargo deny` and the MSRV job must pass; one module uses it |
| `serialport` 4, `sha2` | crates.io | tests only | MPL-2.0; MIT OR Apache-2.0 | dev-dependencies |

## Project Structure

### Documentation (this feature)

```text
specs/013-driver-weact/
├── plan.md              # This file
├── research.md          # Phase 0: decisions with sources and the open items
├── data-model.md        # Phase 1: board, state machine, stream accounting, errors
├── quickstart.md        # Phase 1: how to run each kind of test and the rig
├── contracts/
│   ├── usb-protocol.md      # what is said to and read from the board
│   ├── driver-behaviour.md  # the device contract applied: timing, states, errors, accounting
│   └── rig.md               # the Arduino Nano rig: wiring, serial protocol, patterns, opt-in rules
├── checklists/requirements.md
├── spec.md
└── tasks.md             # Phase 2 (/speckit-tasks, not created here)
```

### Source Code (repository root)

```text
crates/
└── driver-weact/                      # new: package la-driver-weact
    ├── Cargo.toml
    ├── firmware/
    │   ├── fx2lafw-sigrok-fx2-8ch.fw  # the image (raw binary, < 7 KB)
    │   ├── SOURCE.md                  # release, tag, SHA-256, where the source is, offer of source
    │   ├── COPYING                    # GPL-2.0 text; COPYING.LESSER (LGPL-2.1) for fx2lib
    │   └── COPYING.LESSER
    ├── src/
    │   ├── lib.rs                     # `provider()`; `WeActProvider` (DeviceProvider)
    │   ├── error.rs                   # the nine codes and their constructors
    │   ├── protocol.rs                # constants; `start_payload(rate)`; version parse (pure, tested)
    │   ├── usb.rs                     # the seam: `UsbBackend`, `UsbLink`, `UsbInfo`, `BoardKey`
    │   ├── nusb_backend.rs            # the only code that uses `nusb`
    │   ├── firmware.rs                # the embedded image, its checks, `upload()`
    │   ├── boards.rs                  # table of boards and the preparation worker (state machine)
    │   ├── device.rs                  # `WeActDevice` (Device): start / read / stop, accounting
    │   └── testing.rs                 # feature `test-support`: `RecordedUsb` and its scripts
    └── tests/
        ├── protocol.rs                # delay table for every rate, flag bytes, hostile input
        ├── provider.rs                # scan / identity / renumeration / hot-plug with RecordedUsb
        ├── preparation.rs             # blank, prepared, no return, no access, in use, bad version
        ├── acquisition.rs             # replayed captures, finite length, stop, accounting
        ├── failures.rs                # unplug, short read, timeout, stall, stream stops, panic, hang
        ├── conformance.rs             # `la_devices::harness::conformance::run_all`
        ├── firmware.rs                # checksum and size of the image
        ├── recordings/                # replayable exchanges (text), also written by the rig tests
        └── rig.rs                     # opt-in, `#[ignore]`: the Nano rig (needs LA_RIG_PORT)
rig/
└── arduino-nano/
    ├── la_rig/la_rig.ino              # the sketch: patterns on D2..D9, serial control
    └── README.md                      # wiring table, loading the sketch, the serial protocol
docs/
└── drivers/
    ├── weact.md                       # access (Windows WinUSB, Debian udev), limits, errors, firmware
    └── weact-verification.md          # manual procedure, expected results, record template
packaging/linux/60-logic-analyzer-weact.rules   # udev rule: uaccess for 1d50:608c
crates/app/src/main.rs                 # registers the provider (one line)
crates/host/src/shell.rs               # `add_provider` is no longer test-only
resources/i18n/*/errors.ftl            # nine codes x six languages
```

**Structure Decision**: a new crate rather than a module of `la-devices`, so that the device crate
and its contract stay unchanged (spec FR-028), the USB dependency is confined to one crate, and the
two other drivers (011, 012) can follow the same shape. The seam is the driver's own (control
transfers and a bulk stream) because the `Transport` trait of the harness is a byte stream and
cannot express control transfers; the harness pieces that fit (`ScriptedDevice`, `TestProvider`,
the conformance suite) are used as they are.

## Complexity Tracking

| Item | Why needed | Simpler alternative rejected because |
|------|------------|--------------------------------------|
| A seam of the driver's own instead of the harness `Transport` | USB has control transfers and endpoints; the harness `Transport` is `read`/`write` bytes | Forcing control transfers into a byte stream hides the protocol and makes tests unreadable |
| A background preparation worker | Uploading firmware and waiting for re-enumeration takes seconds; `scan` has 1 s and `start` 2 s | Doing it in `scan` or `start` breaks the contract's timing and freezes the list |
| Listing a board before it is ready and keeping it listed during re-enumeration | The identity and the selection must survive the board leaving the bus for about a second | Listing only ready boards makes the board flicker and drops the user's selection |
| A serial-port crate (dev-only) | The rig tests control the Nano over its port (clarification of 2026-11-17) | A hand-written Win32/termios layer needs `unsafe`, which the workspace denies |
