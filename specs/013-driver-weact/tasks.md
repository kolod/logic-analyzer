---

description: "Task list for the WeAct LogicAnalyzer Driver"
---

# Tasks: WeAct LogicAnalyzer Driver

**Input**: Design documents from `/specs/013-driver-weact/`

**Prerequisites**: plan.md, spec.md, research.md (R1–R14 and the open items), data-model.md,
contracts/ (`usb-protocol.md`, `driver-behaviour.md`, `rig.md`), quickstart.md

**Tests**: REQUIRED (constitution Principle III). In every phase the test tasks come first and MUST fail
(usually they do not compile yet) before the implementation tasks start. If a test passes at once
because earlier work already covers it, run a mutation check (temporarily break the line it guards, see
the test fail, restore) before relying on it. Expected values (the start payloads, the bytes of a
download, the identities, the sample sequences) are written **by hand** from the contracts, never
computed by the code under test.

**Organization**: tasks are grouped by user story (US1–US5, numbered as in spec.md). They run in this
order: Setup, Foundational, **US1** (capture from a ready board, P1), **US2** (first connection: the
board prepares itself, P1), **US3** (failures, P2), **US4** (recordings, the rig and the manual
procedure, P2), **US5** (documentation, P3), Polish. US1 is testable with a board that already runs the
firmware; US2 adds the blank board; US3 hardens both.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: can run in parallel (different files, no dependency on an incomplete task)
- **[Story]**: user story from spec.md
- All paths are relative to the repository root.

## Path Conventions

The driver is the new crate `crates/driver-weact/` (package `la-driver-weact`): sources in `src/`, tests
in `tests/`, the firmware image in `firmware/`, replayable exchanges in `tests/recordings/`. The rig is
in `rig/arduino-nano/`. Documents: `docs/drivers/`. Packaging: `packaging/`. Strings in
`resources/i18n/<lang>/errors.ftl`. "The USB contract" is `specs/013-driver-weact/contracts/usb-protocol.md`,
"the behaviour contract" `contracts/driver-behaviour.md`, "the rig contract" `contracts/rig.md`;
decisions are `research.md` R1–R14.

## Lint reminder (applies to every Rust task)

The workspace denies `unwrap`, `expect`, indexing (`v[i]`), unchecked arithmetic, `panic!` and `unsafe`
in production code; use `get`, `checked_*`, `saturating_*`, `u128` intermediates and `?`. Test code may
relax lints with the file-level `#![allow(...)]` the other tests use. The crate keeps **MSRV 1.85** (no
`let` chains: check with `cargo +1.85 check -p la-driver-weact --all-targets`). Never commit while
clippy fails: a helper that only a later task uses gets `#[expect(dead_code, reason = "used by T0xx")]`,
removed by that task. Every public item has a doc comment (`#![deny(missing_docs)]`). Every error is a
`la_core::Error` with one of the codes of research R10 or an existing `device.*` code, and the text a
person reads comes from `errors.ftl` in six languages (English first, then the five translations, **in
the task that first uses the code**, so the parity check is green at every commit); codes are string
**literals** in `error.rs`. No sample data in any error or log. Run `cargo fmt` before the next edit.
Windows scripts that patch files: write them to a file, not a shell heredoc (apostrophes break it).
Commits are signed; if signing fails, ask the owner to unlock GPG, never bypass it. Commit subjects are
`type(scope): …` with a scope (`scripts/check-commits.sh`).

---

## Phase 1: Setup

- [X] T001 Create the crate `crates/driver-weact/`: `Cargo.toml` (package `la-driver-weact`, edition and
  MSRV from the workspace, license GPL-3.0-only, `publish = false`; dependencies `la-core`, `la-devices`,
  `la-diagnostics`, `tracing`, and `nusb = "0.2"`; feature `test-support = []`; dev-dependencies
  `la-devices` with `test-support`, `la-driver-weact` with `test-support`, `serialport = { version = "4",
  default-features = false }` and `sha2`; `[lints] workspace = true`), `src/lib.rs` with
  `#![deny(missing_docs)]`, a crate doc comment naming the board and the contracts, and the empty modules
  of the plan declared (`error`, `protocol`, `usb`, `nusb_backend`, `firmware`, `boards`, `device`,
  `testing` behind `test-support`), each with a one-line doc comment. Nothing uses them yet: unused items
  get `#[expect(dead_code, reason = "used by T0xx")]`. `cargo build -p la-driver-weact` and
  `cargo clippy -p la-driver-weact --all-targets -- -D warnings` pass.
- [X] T002 Spike the USB crate (research R3 risk, open items 3 and 4): write `crates/driver-weact/examples/probe.rs`
  that lists every USB device with `nusb::list_devices()` and prints for each the vendor and product ID,
  the manufacturer and product strings, the bus id, the port chain, the device address and, if the API
  has it, the bound driver name. Run it on the maintainer's machine (the board is attached: expect
  `1d50:608c`) and record in `research.md` R3/R4: the fields that exist, whether the Windows driver name
  is available, what the strings of the blank board are. Check that the crate builds with
  `cargo +1.85 check -p la-driver-weact --all-targets` on Windows and that its dependencies pass the
  licence list (`deny.toml`; `cargo deny` runs in CI). If the MSRV build or `cargo deny` fails, say so in
  `plan.md` Complexity Tracking and stop to ask before choosing another way.
- [ ] T003 Fetch the firmware image (research R5, open item 1): download
  `https://sigrok.org/download/binary/sigrok-firmware-fx2lafw/sigrok-firmware-fx2lafw-bin-0.1.7.tar.gz` into
  a new empty directory outside the repository, read its `COPYING`, `COPYING.LESSER`, `README` and `NEWS`,
  extract `fx2lafw-sigrok-fx2-8ch.fw` and check that it is at most 7,168 bytes. Copy into
  `crates/driver-weact/firmware/`: the image, `COPYING` (GPL-2.0), `COPYING.LESSER` (LGPL-2.1), and write
  `SOURCE.md` with the release, its URL, the date of the file, the SHA-256 of the **image** and of the
  tarball, the git tag or commit it was built from (find it in the repository
  `https://github.com/sigrokproject/sigrok-firmware-fx2lafw`), the statement of the licences (GPL-2.0-or-later
  for the firmware, LGPL-2.1-or-later for fx2lib inside it) and an offer to supply the source. Add the entry
  to `docs/third-party-notices.md`. Update `research.md` R5 open item 1 with the values.

**Checkpoint**: the crate builds, the USB crate is shown to work under the project's rules, the image is in
the repository with its licence.

---

## Phase 2: Foundational (blocks every story)

### Tests first

- [ ] T004 [P] `crates/driver-weact/tests/protocol.rs`: the start payload of every one of the 16 rates equals
  the table of the USB contract, written by hand (for example 1,000,000 → `[0x40, 0x00, 0x2F]`, 20,000 →
  `[0x00, 0x05, 0xDB]`, 24,000,000 → `[0x40, 0x00, 0x01]`); every rate not in the list (0, 1, 19,999, 3,000,001,
  24,000,001, 48,000,000, `u64::MAX`) is an error and never a panic; the clock rule at its boundary (the
  48 MHz delay of 1,536 is accepted, 1,537 falls back to 30 MHz); `parse_version` accepts exactly two bytes
  and reads `[major, minor]`, rejects lengths 0, 1, 3 and 64; the buffer plan (size a multiple of 512, at
  least 512, count 4 to 32) for every rate, with the hand-worked values of the USB contract (20 kHz → 512
  bytes × 19, 1 MHz → 10,240 × 32); the capability list is strictly increasing and has 16 entries.
- [ ] T005 [P] Error codes (research R10): `crates/driver-weact/src/error.rs` with the nine codes as
  `pub const` string literals (`weact.access_denied`, `weact.driver_missing`, `weact.in_use`,
  `weact.firmware_rejected`, `weact.not_renumerated`, `weact.preparing`, `weact.no_data`,
  `weact.stream_stopped`, `weact.firmware_invalid`) and one constructor each returning
  `la_core::Error` of kind Device with an English `detail`; messages with advice (what to do, per research
  R10) in `resources/i18n/{en,de,fr,es,ja,uk}/errors.ftl`; a unit test in the crate that the codes are unique
  and in the `weact.` domain. The host test that scans every `error.rs` for codes and checks the six
  languages (`crates/host/tests/`, `all_error_codes`) and `cargo test -p la-i18n` must pass; add the new
  crate to the places that scan (`crates/*/src/error.rs` is already scanned).

### Implementation

- [ ] T006 `crates/driver-weact/src/protocol.rs`: the constants of the USB contract (vendor requests `0xA0`,
  `0xB0`, `0xB1`, `0xB2`, endpoint `0x82`, CPUCS `0xE600`, identity `1d50:608c`, the strings `sigrok` and
  `fx2lafw`, the required major version 1, timeouts, the silence limits and the renumeration times of the
  behaviour contract), `SAMPLE_RATES: [u64; 16]`, `start_payload(rate) -> Result<[u8; 3]>` by the rule
  of research R2 (integer arithmetic only, `checked_*`), `parse_version(&[u8]) -> Result<(u8, u8)>`,
  `buffer_plan(rate) -> BufferPlan { size, count }` and a pure function that turns a byte slice and the
  enabled mask into sample words (bit `i` of the byte is channel `i`, disabled channels zero). T004 passes.
- [ ] T007 `crates/driver-weact/src/usb.rs`: the seam of research R11, documented: `BoardKey` (bus id, port
  chain; `Display` as `<bus>-<p1>.<p2>…`; `DeviceId` as `weact:<that>`; a parser for the id with tests for
  hostile text), `UsbInfo` (key, vendor, product, manufacturer, product string, address, driver name),
  `UsbBackend` (`list() -> Result<Vec<UsbInfo>>`, `open(&BoardKey) -> Result<Box<dyn UsbLink>>`), `UsbLink`
  (`control_in(request, value, index, len, timeout) -> Result<Vec<u8>>`, `control_out(request, value, index,
  data, timeout) -> Result<()>`, `open_stream(buffers) -> Result<Box<dyn BulkStream>>`, `strings()`), and
  `BulkStream` (`submit_all()`, `wait(timeout) -> StreamEvent { Data(Vec<u8>), Empty, Timeout, Error(Error) }`,
  `cancel_all()`, `release()`). All methods are `Send`; none of them panics by contract.
- [ ] T008 `crates/driver-weact/src/testing.rs` (feature `test-support`): `RecordedUsb`, a `UsbBackend`/`UsbLink`
  double driven by a script of steps (a board appears, disappears, renumerates after a given number of
  calls, answers a control request with given bytes or an error kind, delivers a bulk chunk, delivers an
  empty or short chunk, times out, disconnects, fails with a given typed error, hangs until released, panics),
  that logs every request the driver sent (`written()` as a list of typed entries for assertions) and has a
  controllable clock. Its own unit tests in the file: steps are consumed in order, a hang is released by the
  handle, the log is exact, an unexpected request is reported (not ignored).
- [ ] T009 `crates/driver-weact/src/nusb_backend.rs`: the only code that uses `nusb`: a `UsbBackend` and `UsbLink`
  over `list_devices`, `open`, `claim_interface(0)`, vendor `control_in`/`control_out` with the timeout, the
  bulk `Endpoint` `0x82` with `submit`, `wait_next_complete(timeout)` and `cancel_all`; a pure function
  `map_usb_error(kind, context) -> Error` (Disconnected → `device.disconnected`, PermissionDenied →
  `weact.access_denied`, Busy → `weact.in_use`, on Windows an open that fails because the board is not on
  WinUSB, or `driver()` names something else, → `weact.driver_missing` naming the driver found, anything else →
  `device.transport` with the kind in the detail), tested as a table in the file; the module is
  `#[cfg]`-free (both systems) and has no `unsafe`. Where hardware is needed the check is T002's probe and the
  rig; here only the mapping is tested.

**Checkpoint**: protocol, seam, double and the real backend exist; nothing yet lists or captures.

---

## Phase 3: User Story 1 - Capture from a WeAct LogicAnalyzer (Priority: P1)

**Goal**: a board that runs the firmware is listed, configured within its capabilities and captured like any
other device, with a stored capture equal to the signal.

**Independent Test**: with a scripted ready board, list, pick, configure 8 channels at 1 MHz, capture
100,000 samples; the stored capture equals the input sample for sample.

### Tests for User Story 1 (write first; they must fail)

- [ ] T010 [P] [US1] `crates/driver-weact/tests/provider.rs`: with `RecordedUsb` and a running board: it is listed once
  with the identity `weact:<bus>-<chain>` written by hand for the scripted position, the name "WeAct
  LogicAnalyzer V1", `simulated = false` and the capabilities of the data model (8 channels, the 16 rates as a
  list, no triggers, no buffer depth) and `DeviceDescriptor::validate` passes; two boards on two ports have two
  identities and the names "WeAct LogicAnalyzer V1 (1)" and "(2)" (a single board has no number); a device
  with another vendor or product ID is never opened (the log shows no `open`); the blank Cypress identity
  `04b4:8613` is ignored; a bus that cannot be read gives `Ok` with the boards already known, and a typed
  `device.transport` error when none is known; `scan` completes with the script in under 200 ms of wall time;
  `open` of an unknown id is `device.not_found`.
- [ ] T011 [P] [US1] `crates/driver-weact/tests/acquisition.rs`, through `DeviceRegistry` and the real acquisition engine
  of `la-devices` with the scripted board: a capture of a known signal (a hand-built pattern of 100,000
  bytes, 8 channels, 1 MHz) is stored equal sample for sample; with channels 1 and 3 enabled all other bits
  are zero; a set length shorter than the data truncates the last chunk and ends with `Finished`; an
  open-ended capture runs until stop; a stop closes the capture within 100 ms and leaves the board ready; two
  captures in a row work and the second contains no byte of the first (stale bytes scripted before the start are
  drained); the log shows the order (strings/version checks at open, then the transfers submitted, **then**
  `0xB1` with the payload of the rate); a configuration the core rejects (rate not in the list such as 48 MHz,
  channel 8, no channel) is refused and the log shows nothing sent to the board; the capture is not marked
  simulated and its description records the device id, name and rate.
- [ ] T012 [P] [US1] `crates/driver-weact/tests/conformance.rs`: `la_devices::harness::conformance::run_all` against the
  provider with a scripted ready board and `Expectations` with a finite capture of 10,000 samples; the
  deliberately broken devices of the harness still fail (the suite is the one of feature 004, not a copy).

### Implementation for User Story 1

- [ ] T013 [US1] `crates/driver-weact/src/boards.rs` (ready part) and `src/lib.rs`: the table of boards (key, state
  `Ready`, `last_seen`), reconciliation from `UsbBackend::list` (only `1d50:608c`; the strings `sigrok`/`fx2lafw`
  mean the firmware runs), the descriptors (names numbered when several), `WeActProvider` implementing
  `DeviceProvider` (`name` = `"weact"`, `scan`, `open` for a `Ready` board: open the link, check
  `CMD_GET_FW_VERSION` major 1, return the device), and `pub fn provider() -> Arc<WeActProvider>` over the
  `nusb` backend plus `WeActProvider::with_backend` for tests. T010 passes.
- [ ] T014 [US1] `crates/driver-weact/src/device.rs`: `WeActDevice` implementing `Device`: `start` (drain stale data
  with short waits up to 500 ms, create the buffers by `buffer_plan`, submit them, send `CMD_START`, return
  `AppliedConfig::new(rate)`), `read` (wait up to 40 ms, bytes to words by the mask, count, truncate at the set
  length and return `Finished` after the last sample, `Idle` when nothing came), `stop` (cancel, wait up to
  500 ms, release, no command). No failure handling beyond mapping a link error to its typed error yet (T021).
  T011 and T012 pass.
- [ ] T015 [US1] Composition: `crates/host/src/shell.rs` makes `Shell::add_provider` an ordinary public method (remove the
  `#[cfg(feature = "test-support")]` on it; document it) and keeps the doc line in `docs/host-api.md`
  ("`Shell::add_provider`: registers a device provider; the application does this at start-up"); in
  `crates/app/src/main.rs` (or `app.rs` where the shell is created) add `fn device_providers() ->
  Vec<Arc<dyn DeviceProvider>>` returning `la_driver_weact::provider()` and register each after the shell is
  built; `crates/app/Cargo.toml` depends on `la-driver-weact`. A test in `crates/app` that `device_providers()`
  has one provider named `weact`; a host test that a provider added to a `Shell` without the `test-support`
  feature is listed (`cargo test -p la-host` with default features). `cargo +1.85 check -p la-host` passes.

**Checkpoint**: a ready board can be listed and captured through the whole application path (US1 works with
the scripted board; the blank board and the failures come next).

---

## Phase 4: User Story 2 - First connection: the board prepares itself (Priority: P1)

**Goal**: a blank board is made ready without user action; what is missing is said in the user's language.

**Independent Test**: scripted exchanges of a blank board, a ready board, a board that never returns, refused
access, no WinUSB, a busy board, a wrong firmware: each gives its outcome.

### Tests for User Story 2 (write first; they must fail)

- [ ] T016 [P] [US2] `crates/driver-weact/tests/preparation.rs` with `RecordedUsb`: a blank board is prepared and becomes
  ready: the log shows, in order, the write of `0x01` to CPUCS `0xE600` (`0xA0`, `wValue 0xE600`), the image
  written from address 0 in chunks of at most 4,096 bytes whose concatenation equals the image (compare with
  the file in the repository, bytes written by the test's own copy of the expected chunk boundaries), the write
  of `0x00` to `0xE600`, then polls every 100 ms from 300 ms after the release, then the version request (major
  1 accepted, minor logged); a running board is used with **no** download; a board that never returns gives
  `weact.not_renumerated` once 5 s of the controllable clock have passed (not before, and `scan` stays under
  200 ms throughout); refused access, a board not on WinUSB (with a driver name `libusb0`) and a held board
  give `weact.access_denied`, `weact.driver_missing` (the detail names `libusb0`) and `weact.in_use` from
  `open`; a version answer with major 2 gives `weact.firmware_rejected`; an empty image and an image of 7,169
  bytes (injected with `with_image`) give `weact.firmware_invalid` and **nothing** is sent to the board; the
  board stays listed, with one identity, while it is off the bus during the download, and is never listed
  twice; a selection by id stays valid across the renumeration; `open` during preparation is
  `weact.preparing`; a failed board is still listed while on the bus, `open` returns its error once and the
  next scan prepares it again; two blank boards are prepared one after the other; 100 rapid plug and unplug
  events leave the list equal to what is present (SC-010); a hung download does not stop `scan` or the other
  board.

### Implementation for User Story 2

- [ ] T017 [US2] `crates/driver-weact/src/firmware.rs`: the embedded image (`include_bytes!` of the file of T003), the
  run-time check (non-empty, at most 7,168 bytes) as `Image::checked() -> Result<Image>` with
  `weact.firmware_invalid`, a constructor for a test image, `upload(link, &Image)` per the USB contract (hold,
  chunks, release), tests of the chunking in the file (an image of 0, 1, 4,096, 4,097 and 7,168 bytes).
  `crates/driver-weact/tests/firmware.rs` (written in this task, first): the SHA-256 of the embedded image
  equals the value in `firmware/SOURCE.md` (the test reads the file, so the record cannot drift) and its size is
  within 7,168 bytes.
- [ ] T018 [US2] `crates/driver-weact/src/boards.rs` (full): the states and transitions of the data model (`Blank`,
  `Preparing`, `Ready`, `Failed`, `Absent`), the preparation worker (a contained worker of `la-diagnostics`,
  one board at a time, a `Clock` seam for the allowed times), the listing rules (a preparing board stays listed
  while off the bus up to 5 s; a failed board stays listed while on the bus and for 30 s after it vanished
  during preparation), and the typed results of `open` in each state. T016 and T010 pass; log the state changes
  and the elapsed times at `info`.

**Checkpoint**: US1 and US2 together: a blank board becomes ready and is captured, all in tests.

---

## Phase 5: User Story 3 - Failures never lose data or stop the application (Priority: P2)

**Goal**: unplug, a stream that stops, nothing arriving, a transfer error, a panic or a hang are contained, typed,
and leave the data.

**Independent Test**: inject each failure at each step of a capture; the books balance and another capture works.

### Tests for User Story 3 (write first; they must fail)

- [ ] T019 [P] [US3] `crates/driver-weact/tests/failures.rs`, through the registry and acquisition engine: an unplug at
  `start`, before the first data, in the middle of a stream and during `stop` gives `device.disconnected` (the
  capture keeps the samples already stored, is closed interrupted and marked incomplete); a bulk error status gives
  `device.transport`; short and empty chunks and one timeout are not errors; the stream stops in the middle of a
  capture of a set length (the script goes silent): after the silence limit of the behaviour contract (the larger
  of 2 s and four transfer periods, on the controllable clock) `read` returns `Lost(requested - received)` once and
  then `weact.stream_stopped`, the capture holds `received`, records one gap at the end and the sum of stored and
  lost positions equals the requested length (checked exactly, for several `received`); an open-ended capture whose
  stream stops gives only the error; nothing at all within 2 s of the start gives `weact.no_data`; the board
  unplugged and plugged again is listed again and a new capture works without restarting; a link that panics and a
  link that hangs, at each of `scan`, `open`, `start`, `read`, `stop` and during preparation, are contained
  (the registry reports `device.driver_panic` or `device.unresponsive`, other devices and the list are not
  affected) — a loop of 1,000 injected panics and hangs over varied points never crashes or freezes the test
  (SC-006); `weact.in_use` at start is reported without a capture.
- [ ] T020 [P] [US3] `crates/driver-weact/tests/hostile.rs`: data from the device that is wrong never panics and is typed: a
  control answer of the wrong length for `0xB0` and `0xB2`, strings that are not valid UTF-8 or are enormous, a
  port chain of 255 hops, a vendor ID of `u16::MAX`, a bulk completion of length 0, one longer than its buffer,
  one of an odd length, a stream of random bytes of random chunk lengths for 10,000 cases from a fixed seed (an
  LCG written in the test): the words produced are exactly the bytes masked; nothing is indexed out of range.

### Implementation for User Story 3

- [ ] T021 [US3] `crates/driver-weact/src/device.rs`: silence tracking with the clock seam (first data 2 s; later the larger of
  2 s and four transfer periods), the `Lost`-then-error sequence for a set length and the error alone otherwise,
  mapping of every link error to its typed error, `stop` safe after a failed `read` and when called twice,
  `Drop` cancels and releases; fix whatever T020 finds (checked lengths, no indexing). T019 and T020 pass.

**Checkpoint**: failures are contained and typed (US1–US3).

---

## Phase 6: User Story 4 - Hardware-free tests, the rig and the manual procedure (Priority: P2)

**Goal**: recorded exchanges can be replayed and made from a real board; the Nano rig and its opt-in tests exist;
a tester can follow a written procedure.

**Independent Test**: the suite passes with no device; with `LA_RIG_PORT` the rig tests pass on the maintainer's
machine; without it they are skipped with a reason.

### Tests for User Story 4 (write first; they must fail)

- [ ] T022 [P] [US4] `crates/driver-weact/tests/recordings.rs`: the text format of a recording (header line with a version and
  an origin, one step per line: control answers, bulk chunks as hex, `short`, `empty`, `timeout`, `disconnect`,
  `error <code>`, `appear`, `disappear`, `renumerate`) round-trips (parse then write gives the same text); the
  parser returns typed errors, never panics, for an unknown step, bad hex, an odd number of hex digits, a missing
  header, a version it does not know, a line of 1 MB, 100,000 steps (a limit with a typed error), and files that
  are not UTF-8.
- [ ] T023 [P] [US4] `crates/driver-weact/tests/replay.rs`: each file of `tests/recordings/` replays against the driver and
  the outcome is the one named in its header (a ready 1 MHz capture of 100,000 samples equals the signal described
  in the header; a blank board is prepared; a stream that stops at 50 kHz ends with `weact.stream_stopped` and
  balanced books; an unplug in mid-stream; a board without WinUSB; a held board).

### Implementation for User Story 4

- [ ] T024 [US4] `crates/driver-weact/src/testing.rs`: the recording format of T022 (parser and writer) and `RecordingLink`, a wrapper
  around any `UsbLink` that writes what passes through to a recording (used by the rig tests when
  `LA_RIG_RECORD=<directory>` is set); script construction from a recording. T022 passes.
- [ ] T025 [US4] The recordings `crates/driver-weact/tests/recordings/*.rec` for the six scenarios of T023, **written by hand
  from the USB contract**; each header says "hand-written from the protocol facts, not recorded from a board" and
  names the signal and the expected outcome in words. When the rig run of T038 produces real recordings they are
  added next to these after review, with their header saying where they came from. T023 passes.
- [ ] T026 [P] [US4] The sketch `rig/arduino-nano/la_rig/la_rig.ino` and `rig/arduino-nano/README.md` per the rig contract: D2 to D9 as outputs,
  the value written as `PORTD` bits 2 to 7 and `PORTB` bits 0 and 1, the serial protocol at 115200 baud
  (`HELLO`, `STOP`, `HOLD <hex>`, `COUNTER <step_us>`, `WALK <step_us>`, `SQUARE <step_us>`, `ERR` for anything
  else), the greeting `LA-RIG 1` after a reset, the serial port polled by registers between steps; the README
  (English) has the wiring table, the common ground, how to load the sketch (Arduino IDE or `arduino-cli`,
  board "Arduino Nano", "ATmega328P (Old Bootloader)" for clones) and the protocol. The sketch cannot be tested by
  `cargo`; its behaviour is checked by T029 on the rig, and the reviewer checks the pin mapping against the
  contract.
- [ ] T027 [US4] `crates/driver-weact/tests/rig/mod.rs`, `tests/rig/patterns.rs`: the support of the rig tests: reading `LA_RIG_PORT`,
  opening the port at 115200 with `serialport`, waiting for the greeting (up to 5 s, accepting the reset caused by
  opening the port), sending a command and reading its answer line with a timeout, a guard value whose `Drop`
  sends `STOP` and idles the analyzer, the skip rule (prints `SKIPPED: <reason>` and returns when the variable, the
  board or the greeting is missing), and the **pure checkers** of the patterns (`check_walk`, `check_counter`,
  `check_hold`, `check_square`) as functions over a captured byte sequence, a rate and a step, with the tolerance of
  the rig contract (-20 % to +25 % of `step * rate`). Unit tests, always run, with synthetic captures: correct
  captures pass; captures with a missing channel, a skipped counter value, a stuck bit, run lengths 2× too long and
  an empty capture fail with a message naming the defect; the parsing of answer lines and the skip rule need no
  hardware.
- [ ] T028 [US4] `crates/driver-weact/tests/rig.rs`: the opt-in tests, all `#[ignore = "needs the rig: set LA_RIG_PORT and run with --ignored"]`:
  `HOLD` on each of the eight channels; `WALK` at 1 MS/s with a step of 100 µs (every channel seen, one-hot, cycle
  order); `COUNTER` at 1 MS/s, step 100 µs, over 300 ms; `SQUARE` at 500 kS/s; the rates of the table whose period
  divides the step (at least 100 kS/s up to 1 MS/s with the steps the Nano can keep: 20 µs and up); two captures in a
  row alike; each capture opened through `provider()` and the real engine (so preparation of a blank board is
  exercised when the board is blank); every test ends with `STOP` and the analyzer idle; `LA_RIG_RECORD=<dir>` also
  writes the exchange of the test with `RecordingLink`. Without `LA_RIG_PORT`, `cargo test -p la-driver-weact --test rig -- --ignored`
  prints `SKIPPED` for each and passes; without `--ignored` they are listed as ignored.
- [ ] T029 [US4] `docs/drivers/weact-verification.md`: the manual hardware verification procedure (English): the setup (the rig
  wiring table, or any known source), one numbered step per check with the **expected result** beside it — list; the
  blank board becoming ready within a few seconds; configure and capture at 20 kHz, 1 MHz, 12 MHz and 24 MHz (24 MHz
  with a source that can reach it); all eight channels with the walk; a known signal compared with the capture; stop;
  unplug during a capture (typed error, data kept, incomplete); replug and capture without restarting; a hub or docking
  station at 24 MHz (the capture ends early with `weact.stream_stopped`, flagged incomplete, never looking clean);
  another program (PulseView) holding the board (`weact.in_use`); the Windows driver not WinUSB
  (`weact.driver_missing` naming it); a Debian session without the udev rule (`weact.access_denied`); a clean Windows 11
  and a clean Debian 13 following only `docs/drivers/weact.md` (time taken); and a **record template** (date, operating
  system, board, USB port, application version, result per step, notes) to attach to the pull request.

**Checkpoint**: the suite runs without hardware; the rig and the manual procedure exist for the maintainer.

---

## Phase 7: User Story 5 - Documentation: access and limits (Priority: P3)

**Goal**: one page gets a new user to a working device on both systems and states what the device cannot do.

**Independent Test**: a person follows the page on a clean Windows 11 and a clean Debian 13 and gets a capture.

### Tests for User Story 5 (write first; they must fail)

- [ ] T030 [P] [US5] `crates/driver-weact/tests/docs.rs`: `docs/drivers/weact.md` exists and mentions every `weact.*` code of
  `error.rs` and every `device.*` code the driver returns (`device.disconnected`, `device.transport`,
  `device.not_found`), the words WinUSB and Zadig, `udev`, `24 MHz`, `5.5 V`, `no trigger`, the firmware licence
  (`GPL`) and its origin (`sigrok`); `packaging/linux/60-logic-analyzer-weact.rules` exists and contains `1d50`, `608c`
  and `uaccess`; `packaging/windows/README.md` mentions the board and WinUSB; `docs/drivers/weact-verification.md`
  has the record template; the SHA-256 in `firmware/SOURCE.md` has 64 hex digits.

### Implementation for User Story 5

- [ ] T031 [US5] `docs/drivers/weact.md` (English): the device and its limits (8 channels, at most 24 MHz and the exact list of
  rates, input levels 0 to 5.5 V with the fixed thresholds above 2 V and below 0.8 V, no trigger, no memory so a slow
  or shared USB path stops the capture early, why a docking station or a hub can hurt and 48 MHz is not offered, what
  the lights mean); **access on Windows 10/11** (install WinUSB with Zadig, step by step, replacing libusb-win32 or
  libusbK, checking the result, why the vendor's instruction to name the device `fx2LAFw` is optional); **access on
  Debian 13** (install the udev rule, reload the rules, replug, no administrator rights); how the first connection
  looks (the board is prepared automatically, a second or two); the errors of the application and what to do for each
  (the nine `weact.*` codes and the `device.*` codes the driver returns); the firmware (what it is, where it comes
  from, its licence GPL-2.0-or-later, that it ships with the application and nothing is downloaded or read from
  elsewhere); a link to the verification procedure. T030 passes for this file.
- [ ] T032 [US5] `packaging/linux/60-logic-analyzer-weact.rules` (`SUBSYSTEM=="usb", ATTRS{idVendor}=="1d50",
  ATTRS{idProduct}=="608c", TAG+="uaccess", MODE="0660", GROUP="plugdev"`, with a comment) and the WeAct section of
  `packaging/windows/README.md` (the board needs WinUSB; the installer of a later feature may ship an INF, nothing is
  installed by the application). T030 passes.
- [ ] T033 [P] [US5] The other documents: `docs/user-guide.md` (a short "WeAct LogicAnalyzer" subsection under Devices and
  capturing: it appears in the Device list, first connection, a pointer to `docs/drivers/weact.md`; what an
  early-ending capture means), `docs/third-party-notices.md` (`nusb`; the firmware was added by T003; `serialport` and
  `sha2` as test-only), `README.md` (the supported device list, if it has one), `docs/host-api.md` (checked in T015).
  `roadmap.md` is updated (013 merged) only when the pull request is merged, not before.

**Checkpoint**: all five stories are done; polish remains.

---

## Phase 8: Polish & Cross-Cutting Concerns

- [ ] T034 [P] Robustness against random input (Principle I): `crates/driver-weact/tests/random_scripts.rs`: 10,000 scripts
  from a fixed seed, each a random sequence of steps (appearing, disappearing, renumerating boards, answers of random
  lengths, chunks of random bytes and lengths, errors of every kind, hangs, panics) driven through `scan`, `open`,
  `start`, `read` and `stop`: nothing panics outside the contained workers, every capture's books balance (stored plus
  lost equals what was claimed), the list never shows a board twice, and every run ends with the board released.
- [ ] T035 Mutation checks for tests that passed the first time they ran (the payload of one rate, the 1,536 boundary of the delay,
  the silence limit, the truncation of the last chunk, the `Lost(requested - received)` arithmetic, the 5 s renumeration
  limit, the stale-data drain, the removal of a vanished board, the identity from the port chain): break the guarded line,
  see a test fail, restore; record in `specs/013-driver-weact/verification.md`.
- [ ] T036 Localization review: the nine codes exist in six languages with advice in each (`cargo test -p la-i18n`, the host
  error-code test); list for a person who reads Japanese and German the messages most likely to be long or odd in
  `verification.md`; the device name is not translated.
- [ ] T037 Principle I and licence review of the new code: no `unwrap`, `expect`, indexing, unchecked arithmetic, `unsafe`
  (`cargo clippy --workspace --all-targets -- -D warnings`), `cargo +1.85 check -p la-driver-weact --all-targets`;
  every parser of device data has its hostile-input test (T020, T022); `cargo deny check` (runs in CI) accepts `nusb`
  and its tree; the third-party table of `plan.md` matches `docs/third-party-notices.md` and `firmware/SOURCE.md`.
- [ ] T038 Verification on the real board (**needs the owner**): install WinUSB (quickstart section 2; the board is now on
  libusb-win32), wire the rig (the Nano on COM7 with the sketch of T026), then run quickstart sections 3 and 4. Settle the
  **[verify]** items of the USB contract and open items 2 and 3 of the research (the download chunk size and registers, the
  meaning of the lowest delays, the bound-driver name), correct the contracts if the board disagrees, add the real
  recordings of the rig run (`LA_RIG_RECORD`) next to the hand-written ones, and run the manual procedure of T029 on both
  operating systems as far as the owner's machines allow; record every run with the template.
- [ ] T039 `specs/013-driver-weact/verification.md`: traceability of every FR and SC to the tests that prove it; SC-001, SC-008,
  SC-009 and SC-011 measured or marked as left to a person with the record of T038; the mutation checks of T035;
  the deviations from the plan; the known issues (the flaky Windows timing tests of other features are not this
  feature's); what is left to a person.
- [ ] T040 Run the whole quickstart sections 1 and 5 (and 2 to 4 where T038 was done), `cargo fmt --check`,
  `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`, and the CI of the pull request on
  both systems; merge only when it is green, without `--admin`.

---

## Dependencies & Execution Order

### Phase dependencies

- Setup (T001–T003) → Foundational (T004–T009) → US1 → US2 → US3 → US4 → US5 → Polish.
- T002 gates T009 and everything after it (the USB crate must work under the project's rules); T003 gates T017.
- US2 builds on US1's table and provider (T013) and device (T014); US3 hardens T014/T018; US4's recordings use the
  seam of Foundational; the rig tests (T028) need T014, T018 and T027; US5's documentation can be written once the
  error codes exist (T005) and the behaviour is settled (end of US3). T038 needs everything before it and the owner.

### Within each story

Tests first and failing, then the implementation. Files shared by several tasks (`src/boards.rs`: T013, T018;
`src/device.rs`: T014, T021; `src/testing.rs`: T008, T024; `resources/i18n/*/errors.ftl`: T005 only) are edited one
task after another, never in parallel.

### Parallel opportunities

- T004 and T005 together; T010–T012 together; T019 and T020 together; T022 and T023 together; T026 alongside T027;
  T030 and T033 together; T034 with T036 and T037.
- Everything marked [P] in a phase touches different files; run those together only when no other task of the phase is
  in progress on the same files.

### Parallel example: User Story 1 tests

```text
T010  tests/provider.rs      (listing, identity, ignored devices)
T011  tests/acquisition.rs   (capture equals the signal, order of requests, stop, two in a row)
T012  tests/conformance.rs   (the shared conformance suite)
```

## Implementation Strategy

1. **MVP = Setup + Foundational + US1 + US2**: a blank or ready board is listed, prepared and captured through the whole
   application, proven with scripted exchanges. That is the smallest slice that delivers the device; it is proven
   against the real board by T038.
2. **US3** makes it safe (the failure matrix of the behaviour contract), **US4** adds replayable recordings, the rig and
   the manual procedure, **US5** the documentation people need for access.
3. The two things that can still surprise are on the real board and are named: the **[verify]** items of the USB contract
   (T038) and the transitive dependencies of `nusb` under MSRV 1.85 (T002). Do T002 first and T038 as soon as US1 and US2
   run, not at the end.
4. Commit after each task or logical group (signed, with a scope); keep `clippy` green at every commit.
