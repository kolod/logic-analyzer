---

description: "Task list for the 32-Channel RP2350B Logic Analyzer (Driver and Rust Firmware)"
---

# Tasks: 32-Channel RP2350B Logic Analyzer (Driver and Rust Firmware)

**Input**: Design documents from `/specs/017-driver-rp2350b-32ch/`

**Prerequisites**: plan.md, spec.md, research.md (R1–R12), data-model.md, contracts/
(`protocol.md`, `device-abstraction.md`, `firmware.md`), quickstart.md

**Tests**: REQUIRED (constitution Principle III). In every phase the test tasks come first and MUST fail
(usually they do not compile yet) before the implementation tasks start. If a test passes at once because
earlier work already covers it, run a mutation check (temporarily break the code it guards, see the test
fail, restore) before relying on it. Expected values are written **by hand** from the contracts (a frame is
spelled out byte by byte, a packing is worked out on paper), never computed by the code under test; the
round-trip tests are the only ones where the encoder and the decoder check each other.

**Organization**: the plan delivers in four stages that are separate pull requests (research R12). The two
foundational phases (wide channels, a richer abstraction) block every story and are stages 1 and 2; the stories
then follow the order of the spec (the documents of US5, T066 to T069, ship with stage 3): **US1** (32 channels, P1), **US2** (at least what the original does, P1),
**US3** (the firmware in the repository and on the board, P2), **US4** (failures, P2), **US6** (chaining,
P2, stage 4), **US5** (documents, P3), Polish. US1 to US4 together are stage 3.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: can run in parallel (different files, no dependency on an incomplete task)
- **[Story]**: user story from spec.md
- All paths are relative to the repository root. Tasks marked **(board)** need the physical board; the rest
  run without hardware.

## Path Conventions

Host crates in `crates/` (`storage`, `devices`, `session`, `host`, `app`), new crates `crates/la-proto` and
`crates/driver-rp2350b`, firmware in `firmware/rp2350b/`, tests in each crate's `tests/`, documents in `docs/`,
strings in `resources/i18n/<lang>/ui.ftl`. "The protocol" is `contracts/protocol.md` of this feature.

---

## Phase 1: Setup

- [ ] T001 Record the baseline in `specs/017-driver-rp2350b-32ch/verification.md`: `cargo test --workspace
  --no-fail-fast` before any change (the known failures of this machine, `decoder_growth` and the load-dependent
  ones, are named) and the benchmark baselines of storage and rendering.
- [ ] T002 Read the schematic of the board (`RP2350B_SCH.pdf`: the owner supplies the pin list if the file
  cannot be read here) and decide the pin map: the 32 consecutive input pins and their window (base 0 or 16),
  the two trigger pins, the pins of the external memory, USB and the boot button, the indicator; write it in
  `firmware/rp2350b/PINS.md` and create `docs/drivers/rp2350b.md` with it (T067 completes that document) (research R5, contract `firmware.md`). Confirm in
  the same note the chip variant (the RP2350B) and the memory chip's select pin.
- [ ] T003 [P] Create the crate skeletons `crates/la-proto` (`no_std`, `std` feature for the host) and
  `crates/driver-rp2350b`, add them to the workspace, and check `cargo build --workspace` and `cargo deny check`
  (no new host dependency).
- [ ] T004 [P] Create the firmware workspace `firmware/rp2350b/` (`Cargo.toml`, `rust-toolchain.toml`,
  `.cargo/config.toml` with the target `thumbv8m.main-none-eabihf`, members `la-fw-logic`, `la-fw`, `la-fw-rig`,
  `README.md`, `THIRD-PARTY.md`, its own `deny.toml`); choose and pin the embedded dependencies after checking
  that the 48-pin chip feature exists, the versions and the licences (research R4, **to confirm**) and record
  each with its reason; the host `cargo test --workspace` must not need the target.
- [ ] T005 [P] Add a `firmware` job to `.github/workflows/ci.yml` that builds `la-fw` and `la-fw-rig` for the
  target and runs the tests of `la-fw-logic` and `la-proto` on the host, with `cargo deny` for the firmware
  workspace; run the recorded-transport tests on Windows and Debian (SC-007), keep the names of the required checks and the "descriptions only" rule of the `changes` job
  (a change under `firmware/` is code).

---

## Phase 2: Foundational A: wide channels (stage 1, blocks everything)

**Purpose**: captures of up to 160 channels, with no change for 32 channels or fewer (FR-025, FR-026).

### Tests first

- [ ] T006 [P] `crates/storage/tests/wide_ingest.rs`: for 33 to 160 channels, a capture ingested from words in
  any chunking (including chunks of one sample and chunks that cut a gap) equals the capture built one channel
  at a time; for 1 to 32 channels, with `words_per_sample` 1, every result equals the present ingest's; bits above
  the channel count are ignored.
- [ ] T007 [P] `crates/storage/tests/wide_channels.rs`: `MAX_CHANNELS` is 160 and 161 is refused with the typed
  error; queries, summaries, `restore` and `crop` work on channels above 31; the memory of a capture grows with
  the channels that have transitions, not with the ones that have none.
- [ ] T008 [P] `crates/devices/tests/channel_set.rs`: `ChannelSet` operations against a plain `BTreeSet<u8>`
  model over random sequences (insert, remove, contains, count, iter order, and, highest, `all(n)`,
  `from_u32`/`to_u32`), the bound at `MAX_CHANNELS`, `Copy` and equality.
- [ ] T009 [P] `crates/devices/tests/wide_virtual.rs`: the virtual device with 33 to 160 channels delivers
  `n × words_per_sample` words per `n` samples, the supervisor passes them to storage unchanged, validation
  refuses a channel at or above the device's count and a count above 160, and the capture it makes equals the
  generator's signal channel by channel.
- [ ] T010 [P] `crates/session/tests/wide_description.rs` and `crates/host/tests/sessions_compat.rs`: a session
  of 160 channels is saved and reopened identically; a session of 32 channels or fewer is **byte for byte** what
  the previous version wrote (the committed fixtures); a file of the previous version opens unchanged; a reader
  of the previous format refuses more than 32 channels with its typed error (a fixture written by hand).
- [ ] T011 [P] `crates/host/tests/wide_channels.rs`: configuration, presentation (names, colours, heights, order,
  visibility), the `VIEW` and `LAYT` sections and the markers with 160 channels; a decoder assigned to channel 150
  decodes it.
- [ ] T012 [P] `crates/app/src/app.rs` and `signal_view.rs` tests: 160 channel buttons in twenty rows of eight
  that fit the panel and scroll; the legend and the lanes of 160 channels scroll; at 32 channels the layout is
  the present one.

### Implementation

- [ ] T013 `crates/storage/src/config.rs`, `ingest.rs`, `capture.rs`: `MAX_CHANNELS` 160, `words_per_sample` from
  the channel count, `ingest_chunk` with the stride (the narrow path is the same code with a stride of 1).
- [ ] T014 `crates/storage/src/{restore,crop,published,query,block,channel}.rs` and the benches: every place that
  assumed a `u32` mask or 32 channels (`channel_mask`, `u32::MAX`) uses the channel count and the stride.
- [ ] T015 `crates/devices/src/channel_set.rs` (new) and `config.rs`, `description.rs`, `capabilities.rs`:
  `ChannelSet` (a type used only through its methods, a fixed bitset sized from `MAX_CHANNELS`), replacing the
  `u32` masks; the capability check allows 1 to 160 channels.
- [ ] T016 `crates/devices/src/{driver,supervisor,acquisition}.rs` and `virtual_device/`: `Driver::read` and the
  supervisor with the stride, the virtual device up to 160 channels.
- [ ] T017 `crates/session/src/meta.rs` and `crates/devices/src/description.rs`: the wide description (a minor
  version of the format: the words after the first only when the channels exceed 32), the reader's checks.
- [ ] T018 `crates/host/src/{shell,state,presentation,layout_section,view_section,sessions,decoders}.rs`: the
  configuration and the presentation with `ChannelSet`, `MAX_CHANNELS` 160 in the sections, every
  `enabled_channels` and `visible_mask` caller (about eighty places in source and tests).
- [ ] T019 `crates/app/src/app.rs`, `signal_view.rs`, `lane_layout.rs`: the channel buttons and the capture
  settings for any number of channels, scrolling panels.
- [ ] T020 A benchmark with 160 channels (ingest, summary, plan and drawing of 8 shown channels out of 160) added
  to the existing benches and a baseline recorded (Principle VIII); the drawing cost must not depend on the
  channels not shown.
- [ ] T021 Documents: `docs/host-api.md` (the minor version, the types), the storage and session notes, and
  `verification.md` for stage 1; run the whole of stage 1's tests, `fmt`, `clippy`.

**Checkpoint (stage 1)**: 160 channels work with the virtual device; 32 or fewer are unchanged. A pull request.

---

## Phase 3: Foundational B: the abstraction describes the original's functions (stage 2)

### Tests first

- [x] T022 [P] `crates/devices/tests/rich_capabilities.rs`: for each of `Pattern`, `FastPattern`, samples before the
  trigger by capture mode, buffer depth by rate band, bursts and the highest rate by channel count, a value at
  the limit is accepted and one past it is refused with the `device.*` error and the limit in its detail; a
  trigger in a chain is refused where the capabilities say; a device that declares none of the new things
  behaves exactly as before (the tests of every other driver pass unchanged).

### Implementation

- [x] T023 `crates/devices/src/{capabilities,config,error,description}.rs`: the types and the validation of
  `contracts/device-abstraction.md`, the error `device.unsupported_bursts`, `pre_trigger_samples` in the
  description (and its place in the session as a minor addition).
- [x] T024 [P] `crates/host/tests/capture_settings_rich.rs`: the host commands for the new settings (the trigger
  type, channel and pattern, the samples before and after, the number of bursts) validate against the capabilities
  of the selected device and report the typed error; settings are kept when the device changes only where they
  still fit.
- [x] T025 `crates/host/src/{command,shell,state,devices}.rs`: the configuration edits and the state for the new
  settings (the host API minor version), documented in `docs/host-api.md`.
- [x] T026 `crates/app/src/app.rs` and `resources/i18n/*/ui.ftl`: the controls for the trigger (type, channel,
  pattern), the samples before and after the trigger and the bursts in the capture settings, shown only for a device
  whose capabilities offer them; texts in the six languages (parity, budget, fonts).
- [x] T027 `verification.md` for stage 2; run the tests of the stage, `fmt`, `clippy`.

**Checkpoint (stage 2)**: the abstraction can describe the original's functions; no driver uses them yet except
the virtual device's declared limits. A pull request.

---

## Phase 4: User Story 1 - Capture all 32 channels (Priority: P1) 🎯 MVP

**Goal**: the board lists as a 32-channel analyzer and a capture of any of its channels shows the signals as driven.

**Independent Test**: the recorded-transport tests without the board; then, with the board and the rig, every
channel shows its own pattern (spec US1).

### Tests first

- [x] T028 [P] [US1] `crates/la-proto/tests/frames.rs`: frames spelled out byte by byte (the escapes of `AA` and `F0`,
  the start and end, the longest frame, one byte too many), the parser fed in chunks of every size gives the
  same frames, a damaged frame is dropped without losing the next, and a fuzz run of random bytes never panics
  (`LA_PROTO_FUZZ_ITERATIONS`, the host and the target build).
- [x] T029 [P] [US1] `crates/la-proto/tests/request.rs`: the 58 bytes of capture request 2 by offset (every
  field at its offset, the channel list of 32, a request one byte short or long refused), the information reply
  parsed from the original's lines and from the new ones, an unknown line ignored, every limit.
- [x] T030 [P] [US1] `crates/la-proto/tests/samples.rs`: the raw packing of 1 to 32 selected channels by hand (a
  gap in the numbers, all 32 bits, bit order), the compact coding round trip for constant, sparse, random and
  worst-case signals and its fall back to raw, a stream that is too long, too short or ends inside a record is
  refused, nothing allocates more than the declared number of samples.
- [x] T031 [P] [US1] `crates/driver-rp2350b/tests/recorded.rs` with the recordings in
  `crates/driver-rp2350b/tests/fixtures/`: the driver lists the device from the information reply (32 channels,
  rates, depth), a capture of 32 channels and one of channels 24 to 31 only return exactly the recorded words as
  `words_per_sample` 1, the device identity is stable and a capture is not marked simulated; a test of the
  32-channel packing fails if the eight extra channels are lost or mixed; a recording with a lost-sample mark
  gives stored samples plus lost positions equal to the positions the capture covers (FR-013); `start` returns at
  once and `read` returns in pieces without blocking (FR-012); listing does not block on a port that stays silent
  (FR-002).
- [ ] T032 [P] [US1] `firmware/rp2350b/la-fw-logic/tests/`: request checks against the limits (each field, the
  first failure named), the mode for every channel set, the packing and the compact coding of the same cases as
  T030 (the firmware's code, run on the computer), and the count of lost samples on an overrun (the stream's
  mark plus the samples sent equal the samples sampled).
- [ ] T033 [P] [US1] `crates/driver-rp2350b/tests/hardware.rs` (opt-in: `#[ignore]` and an environment variable):
  the capture of all 32 channels against the rig (steps of research R10).

### Implementation

- [x] T034 [US1] `crates/la-proto`: frames, the parser and builder, capture request 2, the information reply, the
  limits, the raw packing and the compact coding, `no_std` with an `std` feature (T028 to T030 pass on the host
  and the target).
- [ ] T035 [US1] `firmware/rp2350b/la-fw-logic`: the same checks, tables, packing and coding for the firmware, over
  `la-proto` (T032 passes).
- [x] T036 [US1] `crates/driver-rp2350b`: the serial transport trait with the `serialport` backend and a recorded
  transport for the tests, listing (VID/PID and the chip's serial number → `DeviceId`, never blocking on a
  port that does not answer), the information request and the capabilities, start, read (words, with the stride),
  stop, and the typed errors of `data-model.md` (T031 passes).
- [x] T037 [US1] Register the driver with the host's device registry, the device name and the error texts in
  `resources/i18n/*/ui.ftl` in six languages, the capture description marking it as not simulated
  (`crates/host/src/devices.rs`, `crates/devices/src/registry.rs`).
- [ ] T038 [US1] `firmware/rp2350b/la-fw` **(board)**: the start-up (clocks, the pins of T002), USB serial through the
  embedded runtime, the command loop (information, capture requests 1 and 7, abort by any byte, restart into
  the bootloader and the normal restart (command 8), the indicator), the capture engine for the internal memory (PIO sampling of the 32 pins, two
  chained DMA channels into a ring buffer) at the rates of one capture mode, a panic handler that records a code
  and resets.
- [ ] T052 [US1] `firmware/rp2350b/la-fw-rig` **(board)**: the pattern generator of the rig (channel `n` a square
  wave of a period that depends on `n`, a counter on all pins, the trigger patterns, started by a text line) and its
  wiring note in `docs/drivers/rp2350b-verification.md`.
- [ ] T039 [US1] **(board)** Flash the board, run T033 against the rig (`la-fw-rig`, T052) and the steps 1 to 3 of
  `quickstart.md` section 3; write the results in `verification.md`, with the time from plugging in to the first capture (SC-006).

**Checkpoint**: US1 works with the board; the recorded tests run without it. MVP.

---

## Phase 5: User Story 2 - At least what the original does, for 24 channels (Priority: P1)

**Goal**: every function of the original firmware for 24 channels is there, with limits not smaller and the same
accuracy.

**Independent Test**: the list of the original's functions, each with a test or a step of the procedure.

### Tests first

- [ ] T040 [US2] `docs/drivers/rp2350b-parity.md`: the list of the original's functions with 24 channels (rates 3.1
  kHz to 100 MHz, pre/post samples by mode and their limits, edge trigger, pattern trigger up to 16 channels,
  fast pattern up to 5, burst with times, blast, abort, the information fields), made from the original's
  documents and behaviour as recorded in `research.md`, each line naming its test or its step; a test fails if a
  line has neither (the list is parsed by `crates/driver-rp2350b/tests/parity_list.rs`).
- [ ] T041 [P] [US2] `firmware/rp2350b/la-fw-logic/tests/triggers.rs`: the parameters and the programs derived for
  the edge, pattern and fast pattern triggers by hand (which pins, which pattern bits, the length limits, the
  refusals of a pattern wider than 16 or a fast one wider than 5), the pre- and post-trigger positions in the
  ring buffer, the burst times across the wrap of the counter, the blast limits.
- [ ] T042 [P] [US2] `crates/driver-rp2350b/tests/recorded_triggers.rs`: recordings of each trigger type, a burst
  with its times, a blast, an abort and the refusals; the driver turns the abstraction's settings into the
  request and back, and the capabilities from the reply give the limits of T022 (the values of the original at 24
  channels are the least the capabilities declare).
- [ ] T043 [P] [US2] `crates/la-proto/tests/memory.rs` and `firmware/rp2350b/la-fw-logic/tests/memory.rs`: the
  choice of memory (auto, internal, external) by rate, mode and length against a `PSRAMRATE` threshold given as a parameter (the measured value comes from T048), the depth declared by
  rate band, a request the memory cannot take refused with `ERR_NO_MEMORY`.

### Implementation

- [ ] T044 [US2] `firmware/rp2350b/la-fw` **(board)**: the pattern trigger (a second PIO block), the fast pattern
  trigger, the edge trigger as a one-channel pattern, samples before and after the trigger, burst with times,
  blast, the rates and the clock of research R5 (the divider table and any raised core voltage as a documented,
  limited step).
- [ ] T045 [US2] `firmware/rp2350b/la-fw` **(board)**: the external memory (initialisation in quad mode, the
  window, the buffer for the lower rates) and the choice of memory of research R6; a board without the chip uses
  the internal memory and says so in `PSRAM:`.
- [ ] T046 [US2] `firmware/rp2350b/la-fw` and `la-fw-logic`: the compact coding on the board with the fall back to
  raw (T030, T032), announced in `FEATURES:` and used when requested.
- [ ] T047 [US2] `crates/driver-rp2350b`: the options (memory, compact transfer), the triggers, pre/post, bursts
  and blast through the abstraction, the use of the compact coding when the board announces it (the data the
  application gets is the same).
- [ ] T048 [US2] **(board)** Measure and write in `docs/drivers/rp2350b.md` and `verification.md`: the highest rate
  at 8, 16, 24 and 32 channels without a lost sample, the jitter at each offered rate, the DMA and bus load at 100
  MHz and 32 channels, the sustained write speed of the external memory and the `PSRAMRATE` it gives; declare the
  rates in the information reply from the result, and count the samples lost at each rate (none up to 24 channels
  at 100 MHz: SC-003).
- [ ] T049 [US2] **(board)** Steps 4 and 5 of `quickstart.md` section 3 against the list of T040: each function checked,
  the results recorded.

**Checkpoint**: US2 works; the parity list is closed.

---

## Phase 6: User Story 3 - The firmware is in the repository, in Rust, and can be put on the board (Priority: P2)

**Goal**: build from a clean checkout, flash with no other tool, and the application knows the firmware's version.

**Independent Test**: spec US3.

### Tests first

- [ ] T050 [P] [US3] `crates/driver-rp2350b/tests/version.rs`: a firmware or protocol level the driver does not
  support is refused with `rp2350b.firmware_unsupported` naming the version found; an older firmware (command 7
  unknown) is told apart from an unsupported one; a recorded reply for the restart into the bootloader and for the normal restart (command 8); the driver finds
  the board again after either.
- [ ] T051 [P] [US3] `firmware/rp2350b/tests/build_check.sh` (run by CI): the UF2 is produced from a clean checkout
  with the documented commands and the tool named in the README exists.

### Implementation

- [ ] T052 Moved to Phase 4 (the rig is needed by T033 and T039).
- [ ] T053 [US3] The commands and the tools in `firmware/rp2350b/README.md` and `contracts/firmware.md`
  (the UF2 step, flashing through the boot button, the restart into the bootloader); the version of the firmware
  from the workspace version; `THIRD-PARTY.md` complete.
- [ ] T054 [US3] The driver's `restart into the bootloader` and `restart` and a command and menu entry for each in
  the host and the application (`crates/host/src/command.rs`, `crates/app/src/app.rs`), enabled only for a device that offers it,
  with the text in six languages.
- [ ] T055 [US3] **(board)** Build from a clean checkout following only the README, flash by the drive (`D:\`),
  see the device with the version; flash a build with another version and see the refusal; record the time taken
  (SC-005).

---

## Phase 7: User Story 4 - Failures never lose data or stop the application (Priority: P2)

**Goal**: unplugging, errors, garbage and overruns end the capture cleanly, with the data kept and marked incomplete.

**Independent Test**: spec US4.

### Tests first

- [ ] T056 [P] [US4] `crates/driver-rp2350b/tests/failures.rs`: recorded exchanges for a disconnect in the middle of
  a capture (the samples so far are kept, incomplete, within one second), a board that stops answering
  (`rp2350b.timeout`), an `ERR_` line, a reply that does not follow the contract (`rp2350b.protocol`), a capture
  that ends early (`rp2350b.stream_stopped`), a port that is busy or denied, and a panic in the driver's thread
  contained.
- [ ] T057 [P] [US4] `firmware/rp2350b/la-fw-logic/tests/malformed.rs` and `crates/la-proto/tests/fuzz.rs`: every
  malformed request (unknown command, damaged or oversized frame, out of limits, wrong state) gets the named
  error and leaves the state ready; a fuzz run through the request handler of the logic crate never panics.

### Implementation

- [ ] T058 [US4] `crates/driver-rp2350b`: the error mapping and containment, the incomplete mark, the logging of error codes and counts (never signal data: Principle VI), the texts of
  the errors in six languages and the stable codes of `data-model.md`; the device list updates on unplugging.
- [ ] T059 [US4] `firmware/rp2350b/la-fw` **(board)**: the watchdog that restarts a board that stops serving the
  link, the panic handler's code area, the answers to every malformed input, `ERR_BUSY`.
- [ ] T060 [US4] **(board)** Unplug during a capture (20 trials: SC-008), send garbage, abort in every state; record.

---

## Phase 8: User Story 6 - Chain several boards (Priority: P2, stage 4)

**Goal**: up to five boards, defined as a chain, appear as one device and line up in time.

**Independent Test**: spec US6, with two simulated boards and then two real ones.

### Tests first

- [ ] T061 [P] [US6] `crates/driver-rp2350b/tests/chain.rs` with two to five simulated boards: the channels are those of
  the boards in order, the words of the boards are merged (`words_per_sample` the number of boards), the slaves
  are armed before the master, the shift of a board moves its samples, the allowed triggers are those of the
  master's pattern kinds and the edge trigger is refused, a missing or silent board gives
  `rp2350b.chain_incomplete` naming it, boards of different firmware versions give `rp2350b.chain_mismatch`,
  and nothing is captured as if the chain were whole.
- [ ] T062 [P] [US6] `crates/host/tests/chain_settings.rs`: a chain is defined, saved in the settings and read back, a
  board removed from the list removes the chain, two chains cannot use one board.

### Implementation

- [ ] T063 [US6] `crates/driver-rp2350b/src/chain.rs`: the chain device, the merge, the roles in capture request 2,
  the shifts, the combined capabilities (`contracts/device-abstraction.md`).
- [ ] T064 [US6] `crates/host/src/{settings,devices}.rs` and `crates/app/src/app.rs`: define and edit a chain and
  the shift of each board in the application, with the texts in six languages.
- [ ] T065 [US6] **(board)** Two boards wired as the document says to one rig pin: the signal on both within one
  sample, or after the correction (SC-010); record in `verification.md` and in
  `docs/drivers/rp2350b-verification.md`.

---

## Phase 9: User Story 5 - Documentation and a verification procedure (Priority: P3)

**Goal**: everything a user or a maintainer needs, in English, with a procedure for a real board.

### Tests first

- [ ] T066 [P] [US5] `crates/driver-rp2350b/tests/docs.rs`: the documents name every error code of `data-model.md`,
  the pin map in them equals the one the firmware and the driver use (parsed from `PINS.md`), and the parity list
  is closed.

### Implementation

- [ ] T067 [US5] `docs/drivers/rp2350b.md` (created by T002): complete it with the pin map, the warning that the pins take 3.3 V only, what the device
  can and cannot do (the measured rates, the memory, the chain), access on Windows 10 and 11 and on Debian 13 (the
  serial port, the group `dialout`), the errors with their remedies.
- [ ] T068 [US5] `docs/drivers/rp2350b-verification.md`: the manual procedure with the rig for every function of the
  parity list, a single board and a chain, the expected result for each step; opt-in automated hardware tests
  described.
- [ ] T069 [US5] `roadmap.md`: mark 017 and its stages; `docs/user-guide.md`: the device, the new settings, the
  chain and the limit of 160 channels.

---

## Phase 10: Polish & Cross-Cutting Concerns

- [ ] T070 [P] Localization review: every new key exists in six languages, parity, budget and fonts pass, long
  German and Japanese texts checked in the capture settings.
- [ ] T071 [P] Principle I review of `la-proto`, `driver-rp2350b` and the firmware crates: no `unwrap`, `expect`,
  indexing on external data or unchecked arithmetic outside the allowed hot loops; the firmware's `deny` lints.
- [ ] T072 [P] Principle XII record: what was read from the original and from the datasheets, in `research.md` and
  `THIRD-PARTY.md`; no copied code (a search for the original's identifiers in the new code).
- [ ] T073 Mutation checks for the tests that passed on first run (the packing of the eighth byte, the compact
  fall back, the chain's shift) and the results in `verification.md`.
- [ ] T074 `specs/017-driver-rp2350b-32ch/verification.md`: traceability of every FR and SC to the tests and the
  steps, the baseline of T001, the benchmarks, and the manual checks left to the owner (the board, the chain with
  two boards, Windows and Debian, the translations).
- [ ] T075 Run `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo deny check`,
  `cargo test --workspace --no-fail-fast`, the firmware job's commands and `quickstart.md`; fix what fails.
- [ ] T076 **(board)** The benchmark of R13: CDC against a vendor-specific bulk interface on the real firmware, on Windows
  and on Linux (throughput, the latency of the abort byte, the time from the end of a capture to the last byte);
  then decide the transport and register the USB pair of our own (pid.codes or Raspberry Pi), and write both in
  `research.md`. Until then the development pair is the SDK default.

---

## Dependencies & Execution Order

- Phase 1 first (T002 and T004 before any firmware task). Phase 2 (stage 1) blocks Phase 3 and all stories;
  Phase 3 (stage 2) blocks US2 and US6 and the new settings of US1's capture; T006 to T012 together, then T013
  to T021 in order.
- US1 needs Phases 2 and 3; US2 needs US1 (the same firmware and driver files); US3 and US4 need US1; US6 needs
  US1, US2 (the capabilities) and Phase 2; US5 after the behaviour is known.
- Within a phase the tests come first and fail. Tasks that edit the same file (`la-fw`'s main files, `driver-rp2350b`'s
  device file, `app.rs`) run one after another.
- **(board)** tasks are done by the owner with the physical board; their results go to `verification.md`.

### Parallel opportunities

- T003 with T004 with T005. T006 to T012. T022 with T024. T028 to T033. T041 to T043. T050 with T051. T056 with
  T057. T061 with T062. T070 to T072.

## Implementation Strategy

### MVP first

Setup → Phase 2 (wide channels) → Phase 3 (abstraction) → US1: the board lists as a 32-channel analyzer and a
capture shows the signals driven by the rig. Stop and validate with `quickstart.md` section 3, steps 1 to 3.

### Incremental delivery

Stage 1 and stage 2 are mergeable alone and need no hardware. Stage 3 (US1 to US4) is the device; US2 closes the
parity list. Stage 4 (US6) adds the chain. Each stage is its own pull request, with a commit per task or logical
group.

## Notes

- [P] tasks = different files, no dependencies.
- Expected values are by hand, from the protocol contract and the datasheets, never computed by the code under test.
- Never commit with failing clippy, fmt or tests; commits are signed (ask the owner to unlock GPG if signing
  fails; never bypass it); subjects are Conventional Commits with a scope.
- The host crates keep MSRV 1.85; the firmware workspace has its own toolchain and does not affect it.
