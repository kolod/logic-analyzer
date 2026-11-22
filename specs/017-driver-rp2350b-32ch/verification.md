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
