# Verification: 32-Channel RP2350B Logic Analyzer

Results of the checks of each stage, filled in as the work is done. Expected values come from the
contracts, never from the code under test.

## Baseline (T001), before any change of stage 1

Machine: Windows 11, `cargo test --workspace --no-fail-fast` on `main` (ddd134c).

- All tests pass except `la-host --test decoder_growth`, which fails on this machine without any change
  (CI is green); `hostile_input` and `four_stuck_workers_reach_the_refusal_threshold` are flaky under load.
- Storage ingest (`cargo bench -p la-storage --bench ingest`): within noise of `crates/storage/benches/baseline.json`.
- Virtual device throughput: within noise of `crates/devices/benches/baseline.json`.

## Stage 1: wide channels (T006 to T021)

- `la-storage`: a sample takes `ceil(channels / 32)` words, `MAX_CHANNELS` is 160. Tests:
  `crates/storage/tests/wide_ingest.rs` (any chunking, bits above the channel count, a chunk that is not
  whole samples, the limit 160/161, a gap, memory that grows with the channels that change, queries and
  `crop` above channel 31).
- Narrow ingest speed: the first wide implementation made 1 channel sparse 3.9 times slower than the
  baseline; the narrow path (one word per sample) was given its own loop and is again at the baseline
  (0.9 to 1.0 times). SC-006 still holds (32 channels typical 138 MS/s, target 100).
- Wide ingest, for information (`cargo bench -p la-storage --bench ingest`, typical signals, 2 % flips):
  64 channels 63 MS/s, 160 channels 28 MS/s.
- Virtual device: unchanged speed (0.87 to 1.06 times the baseline).
- `la-devices`: `ChannelSet` (`tests/channel_set.rs`, against a `BTreeSet` model), the virtual device up to
  160 channels (`tests/wide_virtual.rs`: every channel against a formula written in the test, any chunking,
  a trigger on channel 140, validation), `WideVirtualProvider` (`virtual-wide:<n>`).
- `la-session`: the enabled channels are written as `ceil(channels / 32)` words; for 32 channels or fewer the
  layout is the old one (pinned by the offsets in the `meta` unit test). `tests/wide_description.rs`.
- `la-host`: `tests/wide_channels.rs` (160 channels captured, named, coloured, hidden, moved, saved and
  opened again; a decoder on channels 150 and 151). Host API 2.2.
- `la-app`: a capture of 160 channels renders and its channel buttons work
  (`a_capture_of_160_channels_fits_the_panel_and_renders`).

Not done in stage 1 (left for the owner or for later stages): the application does not offer the 160-channel
virtual device in its device list (it is available to tests through `WideVirtualProvider`); a reader of the
previous file format cannot be run here, so "an older reader refuses more than 32 channels" rests on the
previous reader's own check of the channel count.

## Stage 2: the abstraction describes the original's functions (T022 to T027)

- `la-devices`: `TriggerKind::{Pattern, FastPattern}`, `Pattern`, `TriggerLimits`, `ModeLimit`, `DepthBand` and the
  capabilities `trigger_limits`, `pre_post`, `buffer_depths`, `bursts`, `max_rates`; the configuration gains
  `pattern`, `pre_trigger_samples` and `bursts`. `validate` checks each against the capabilities and names the
  limit in the detail of `device.unsupported_trigger`, `device.invalid_capture_length`,
  `device.unsupported_sample_rate` and the new `device.unsupported_bursts` (text in the six languages).
  Tests: `crates/devices/tests/rich_capabilities.rs` (16: a value at the limit passes, one past it is refused,
  patterns complete and consistent, a trigger in a chain, the mode by channel count, the depth by rate band, the
  highest rate by channel count, bursts, the descriptor checks, and a device that declares none of it behaves as
  before). Every other test of `la-devices`, `la-driver-weact`, `la-session` and `la-host` passes unchanged apart
  from the new fields in their literals.
- `la-session`: the description keeps the samples kept before the trigger and the pattern behind a new flag of
  the `META` section; a file that needs neither does not set it and its description is byte for byte the old one
  (the `meta` layout test is unchanged). A reader that predates the flag refuses the file as damaged; this was
  not run against a previous reader (none can be built here).
- `la-host`: `ConfigEdit::{Pattern, PreTrigger, Bursts}`, `Setting::Bursts`, the defaults of a device respect
  its mode and depth, and selecting a device keeps the trigger, the samples before it and the bursts only where
  the new device accepts them (`crates/host/tests/capture_settings_rich.rs`, 6). Host API 2.3.
- `la-app`: the capture panel shows the trigger type, channel and pattern, the samples before (and after) the
  trigger and the bursts, each only when the selected device offers it; the virtual device, which has edge
  triggers, now shows the trigger. Texts in the six languages (parity test passes). Tests: the pattern as typed
  levels and the choice of a trigger.
- No driver uses these functions yet; the virtual device declares none of the new limits.
- `cargo fmt`, `cargo clippy --workspace --all-targets` and `cargo +1.85 check --workspace --exclude la-app
  --all-targets` are clean.

## Stage 3, part 1: `la-proto` (T028 to T030, T034)

- `crates/la-proto`: `no_std` without the `std` feature, no allocation. `frame` (framing, escapes, a parser that
  gives the same events in any chunking, drops a damaged or oversize frame without losing the next), `request`
  (the 58 bytes of capture request 2 by offset, the reading with its structural checks, the limits of the device
  checked by `DeviceInfo::check_request` with the first wrong field named), `info` (the original's lines and the
  new ones, unknown lines ignored), `samples` (raw packing of 1 to 32 selected channels and its reverse, the
  compact coding and a decoder that fills only the buffer it is given).
- Tests: `tests/frames.rs` (11, including a fuzz run, `LA_PROTO_FUZZ_ITERATIONS`), `tests/request.rs` (11),
  `tests/samples.rs` (16). `cargo test -p la-proto` also passes with `--no-default-features`, clippy is clean, MSRV
  1.85 builds.
- Not done here: the build for an embedded target (no such target is installed on this machine); the firmware
  logic crate (T035), the driver (T036, T037) and the firmware itself (T038) are the next parts of stage 3.
- The frame limit is read as 256 bytes for the whole frame (four marker bytes and a body of at most 252), since
  the device information request is 5 bytes long; the contract says "at most 256 bytes after unescaping".

## Stage 3, part 2: the driver (T036, T037; T031 in part)

- `crates/driver-rp2350b` (`la-driver-rp2350b`): the serial seam (`link`: `PortBackend`, `SerialLink`), the
  `serialport` backend, the provider (lists the ports with the USB ids of the firmware, asks each new board for
  its information once, keeps the answer, never waits more than 500 ms on a silent port, keeps a known board
  listed without asking it again so a running capture is not disturbed), the capabilities from the
  information reply, the capture request from a validated configuration, the device (start returns after the
  answer to the request, `read` returns in pieces and `Idle` while the board waits for its trigger, stop sends
  the abort byte and drains the port), and the typed errors of `data-model.md` with texts in six languages.
  The application registers it next to the WeAct driver.
- Tests (`tests/recorded.rs` 9, `tests/failures.rs` 7, unit 2) run against recordings in `tests/fixtures/`.
  **The recordings are synthetic**: written by hand from `contracts/protocol.md`, since no board runs the
  firmware yet; the real ones replace them in T039. They cover the listing (and a silent port, a port with other
  ids, no serial number, a board that goes away), a capture of 32 channels and one of channels 24 to 31 (exact
  words), the request the driver sends, a refused request, no answer, an unplug in the middle, a stopped stream,
  and replies that break the contract.
- Not done: the lost-sample mark of FR-013 (the protocol of `contracts/protocol.md` has no such mark yet: it
  must be added there, in `la-proto` and in the firmware first), reading with the compact coding and the options
  (T046, T047), the restart commands in the driver (T054), the unsupported-firmware text for a board that is
  listed with a message (T050: today a board with an unsupported firmware is not listed and the reason is in the
  log), and the end of the information reply is taken as 100 ms of silence (the contract does not name a
  terminator; to be fixed in the contract with the firmware).
- The USB ids are the default of the Pico SDK for development (`0x2E8A:0x000A`); a pair of our own is to be
  registered before the first release (`research.md`, R13), and the firmware must present the same pair.
- Without a trigger the driver sends trigger type 3 (blast), and declares as highest rate the lower of `FREQ` and
  `BLASTFREQ`; the limits before and after the trigger are the tightest of the four modes, because the board's
  mode follows the highest channel and the core's follows the number enabled.

## Stage 3, part 3: the loss marks (FR-013, T031 closed)

- `contracts/protocol.md`: the head of the data of a capture is now 7 bytes (encoding, samples stored, number of
  loss marks) followed by the marks (position in the stored samples, samples lost; strictly ascending, at most
  1024), then the samples. The marks come first because the board knows them when it starts to send. Stored plus
  lost is never more than the request asked for. No firmware exists yet, so nothing had to change there.
- `la-proto` (`data`): `DataHeader`, `LossMark`, `check_marks` (`tests/loss.rs`, 6: offsets, the rules, and a
  property run of 500 random mark lists that stored plus lost equals the positions covered).
- The driver reports each loss where it happened (`ReadOutcome::Lost`) before the stored sample at the mark's
  position, including a loss after the last sample and a capture that lost everything (`tests/losses.rs`, 5).
  Marks that break the rules are `rp2350b.protocol` and nothing of that capture is delivered.
- Mutation checks: a loss reported as 0 and a read that ignores the next mark are both caught by `losses.rs`.
- Not run: a whole capture with a loss through the acquisition and the store (the supervisor already records a
  loss as a gap; this driver was checked only up to `Device::read`). The recordings are still synthetic.

## Stage 3, part 4: the firmware's logic (T032, T035)

- `firmware/rp2350b` is its own workspace (own `Cargo.lock`, apart from the application's); `la-fw-logic` is `no_std`
  and allocates nothing, over `la-proto` (built without `std`). CI has a `firmware (debian)` job (fmt, clippy with
  `-D warnings`, test) that is not a required check of `main`.
- `la-fw-logic`: `Tables` and `device_info` (the information the board reports; `la-proto` gained
  `DeviceInfo::write_text`, tested to read back as written), `mode_for_mask`, `Dispatcher` (the answer to every
  frame, error and command: busy, unsupported, bad request, the restarts accepted at any time, a refused request
  naming its field), `CompactEncoder` (streaming; the same bytes as `la_proto::encode_compact` in any size of
  input and output pieces) and `LossTracker` (marks that pass `check_marks`, adjacent runs merged, the 1024-mark
  limit ends the capture).
- Tests: `requests.rs` 8, `packing.rs` 4, `losses.rs` 5. Mutation checks (the busy flag never set, a restart refused
  while busy, a distance counted wrong in the compact coding, adjacent losses not merged) are each caught.
- The raw packing of the board is `la_proto::pack_raw`, tested in `la-proto`; the original's capture request
  (command 1) is answered `ERR_UNSUPPORTED` for now (the original's structure is not in `la-proto` yet), a gap
  against FR-015 to close in stage 3 (T044). The values of `Tables` that depend on the hardware are measured on the
  board (T048).

## Stage 3, part 5: restarting the board (T054)

- `la-devices`: `DeviceAction` (`Restart`, `RestartIntoBootloader`), `Capabilities::actions`, `DeviceProvider::perform`
  (the default refuses), and `DeviceRegistry::perform`, which returns at once with an `ActionHandle` and runs the provider
  on a contained worker (an answer later than 2 s is `device.unresponsive`, a panic is `device.driver_panic`;
  a device that is capturing is `device.busy`, one that does not declare the action `device.invalid_state`).
- `la-proto`: the `restart` feature (`FEATURES:` line); `la-fw-logic` announces it.
- Driver: the board declares `RestartIntoBootloader` always and `Restart` when it announces `restart`; `perform` sends
  command 8 or 4, reads `RESTARTING` or `RESTARTING_BOOTLOADER`, forgets the board so it is asked again when it
  comes back (`tests/actions.rs`, 6: the frames, the answers, the board listed again, a refusal, a silent board,
  an unknown board).
- Host (API 2.4): `Command::DeviceAction`, `State::device_action`; only for an idle host, an action the selected device
  declares and none in progress; the answer arrives with `poll`, a failure is a notice (`tests/device_actions.rs`, 4).
- Application: two entries of the Capture menu, enabled by the same rule (`EnabledWhen::DeviceOffers`), in six languages
  (within the menu width budget).
- Not done: a real restart of a board (no firmware yet: T055/T060), and a message that offers the bootloader
  when the board has a firmware the driver does not support (T050).

## Stage 3, part 6: a board with a firmware the driver cannot use (T050)

- A board that answers as an analyzer but whose firmware is older (no capture request 2) or not known to the driver
  is now **listed**, with the capabilities of what it reported and only the action `RestartIntoBootloader`, so it can
  be updated from the Capture menu. A capture on it is refused when started with `rp2350b.firmware_unsupported`; the
  detail names what it reported ("an older firmware without capture request 2 (...)" or "firmware 9.0.0, protocol
  level 3") and what the driver supports. Before, such a board was left out of the list and the cause was only in
  the log (a warning with the same text is still written).
- The text of the error (six languages) now says how to update: the Capture menu, Restart into bootloader, copy
  the new firmware file to the drive that appears.
- Tests (`tests/version.rs`, 6): an unknown version, an older firmware (its own 24 channels kept), the restart into
  the bootloader of such a board and its being asked again when it returns, the normal restart not offered for it,
  something that does not answer as an analyzer not listed, a supported board unaffected.
- Not run: with a real board and a real UF2 (T055).
