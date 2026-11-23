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
