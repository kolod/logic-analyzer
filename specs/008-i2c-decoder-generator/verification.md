# Verification: I2C Decoder and Generator

Written when the implementation was complete (all tasks of `tasks.md`). It records what was checked,
by which test, and what is left to a person.

## Automated checks

| Check | Result |
|-------|--------|
| `cargo fmt --check` | clean |
| `cargo clippy --workspace --all-targets -- -D warnings` | clean |
| `cargo +1.85 check -p la-decoders -p la-host` (MSRV) | clean |
| `cargo test --workspace` | all pass (see "Known issues" for one flaky test that is not from this feature) |
| `cargo test --release -p la-decoders --test i2c_round_trip -- --include-ignored` (10,000 seeds) | pass |
| `LA_ROUND_TRIP_SEEDS=10000 … i2c_round_trip` (10,000 seeds for every round-trip test) | pass |
| `cargo deny check` | not installed on the development machine; runs in CI. No dependency was added. |
| `cargo bench -p la-decoders --bench decode_i2c` | 30 M samples of dense traffic in 0.77 s (about 39 M samples/s, 1.46 M annotations); baseline 25,798 µs per million samples recorded in `crates/decoders/benches/baseline.json` |

## Traceability

| Requirement | Tests |
|-------------|-------|
| FR-001 the decoder, SCL and SDA | `i2c.rs`; host `the_i2c_decoder_is_in_the_catalogue_and_reads_a_capture_of_a_bus` |
| FR-002 start, repeated start, stop, bits, acknowledge | `i2c.rs` (write, read, repeated start, same-sample edges), `i2c_recorded.rs` (fixtures) |
| FR-003 lane: address, bytes, acknowledgements, transaction, events | `i2c.rs`, `i2c_recorded.rs` |
| FR-004 overlays on SDA and SCL, on the exact sample, no colour named | `i2c.rs` (`assert_marks_name_no_colour`, 27 arrows, positions) |
| FR-005 NAK classification | `i2c.rs` (NAK of a read is not an error), `i2c_errors.rs`, fixtures `nak_on_address`, `nak_on_data` |
| FR-006 7-bit and 10-bit addresses | `i2c_options.rs` (10-bit write, read part, orphan header), `i2c_round_trip.rs::ten_bit_addresses_round_trip_with_and_without_errors` |
| FR-007 options and their defaults | `i2c_options.rs`, host `changing_the_address_notation_runs_the_decoder_again_and_is_saved_with_the_session` |
| FR-008 errors, incomplete transactions | `i2c_errors.rs`, fixtures `byte_cut_by_stop`, `start_without_clock`, `ends_mid_transaction` |
| FR-009 independent of chunking and growth | `i2c_chunking.rs` (chunk sizes, growing capture in pieces of 1 to 100,000 samples), `i2c_sync.rs` (growing capture), `i2c_errors.rs` (continuing transaction) |
| FR-010 no panic on any input | `i2c_chunking.rs::random_signals_never_break_the_decoder` (more than a million chunks, gaps included), `i2c_period.rs::awkward_positions_do_not_panic_or_overflow`; the lints of the workspace deny unchecked arithmetic and indexing in the decoder |
| FR-011 six languages | `la-i18n` parity and formatting tests (`cargo test -p la-i18n`) |
| FR-012 items for a stacked decoder | `i2c_stacking.rs`, `i2c_options.rs` (10-bit items), host stacking test |
| FR-013 the generator makes valid traffic | `i2c_round_trip.rs`, host `a_virtual_capture_decodes_to_the_traffic_that_was_generated` |
| FR-014 planted errors and their rate | `i2c_round_trip.rs::planted_errors_are_reported_where_they_were_planted_and_nowhere_else` (more than 100 planted), host `errors_are_planted_when_the_generator_option_is_on_and_not_otherwise` |
| FR-015 reproducible, independent of chunking, instances differ | `i2c_round_trip.rs` (`the_signal_does_not_depend_on_how_it_is_asked_for`, `two_instances_with_the_same_seed_differ`), host (same seed twice) |
| FR-016 `expected()` and `expected_errors()` | `i2c_round_trip.rs` |
| FR-017 a stacked decoder finds valid input | `i2c_stacking.rs::the_decoder_above_receives_exactly_the_transactions_the_generator_made` |
| FR-018 other channels unchanged | `composite.rs`, host `channels_without_a_decoder_keep_the_ordinary_random_signal` |
| FR-019 round trip over many seeds | `i2c_round_trip.rs` (1,000 seeds by default, 10,000 in release) |
| FR-020 recorded signals, malformed and truncated, random | `i2c_recorded.rs` (11 fixtures, `i2c_fixtures.rs` for the loader), `i2c_errors.rs`, `i2c_chunking.rs` |
| FR-021 documentation | `docs/decoders/i2c.md`, `docs/user-guide.md` ("Decoding I2C"), `docs/decoder-api.md` |
| FR-022 added by registration | SC-008 below; the one change to the contract is recorded in the plan (Complexity Tracking) and `contracts/decoder-api-1.1.md` |
| FR-023 synchronisation on an idle bus | `i2c_sync.rs`, fixtures `begins_mid_byte`, `preinit_low_then_noise` |
| FR-024 the bus period | `i2c_period.rs`, `i2c_sync.rs` (`while_the_period_is_unknown…`, the long noisy start-up), `i2c_stacking.rs` (the watermark), `final_through.rs` |
| SC-001 zero differences over 1,000 seeds | `i2c_round_trip.rs` |
| SC-003 marks on the exact sample | `i2c.rs`, `i2c_recorded.rs` (positions are stated by hand) |
| SC-004 no panic over more than a million chunks | `i2c_chunking.rs` |
| SC-005 same result for any chunking | `i2c_chunking.rs` |
| SC-006 ten million samples without an interface pause above 100 ms | host `ten_million_samples_decode_without_holding_the_interface` |
| SC-007 six languages, parity | `cargo test -p la-i18n` |
| SC-008 the shell is not touched | see below |

## Mutation checks

Tests that passed the first time they ran were checked by breaking the line they guard, seeing the test
fail, and restoring the line. All seven mutations were caught:

| Mutation | Caught by |
|----------|-----------|
| the idle rule of the first start ignored | `i2c_sync.rs` |
| the error of a gap not reported | `i2c_errors.rs` |
| the 8-bit address notation ignored | `i2c_options.rs` |
| the generator's stop one sample late | `i2c_round_trip.rs` |
| the controller's NAK of a read counted as an error | `i2c.rs` |
| the calibration needing 17 intervals instead of 16 | `i2c_period.rs` |
| the coverage ignoring `final_through` | `final_through.rs` |

## SC-008: the shell

`git diff main --stat -- crates/app crates/host/src crates/session crates/storage crates/devices
crates/core` shows one file, `crates/app/src/app.rs`, and its changes are in its test module only (the
catalogue now has two built-in decoders: two helpers add the sample decoder by name instead of through
the menu, and two counts went from 1 and 2 to 2 and 3). No behaviour of the shell changed.

## Decisions made while implementing, differing from the plan

- **The calibration condition** is "three neighbouring bins hold 16 of the *last 32* intervals", not
  "half of all intervals so far": with the second rule a long noisy start-up could keep the clock from
  ever being recognised (found by the test of a start-up of 300,000 edges). `research.md` R1 says so.
- **A bit counts when its clock pulse ends.** A rising edge of SCL is only a candidate; an SDA edge while
  SCL is high is a start or a stop and the pulse before it was its set-up. This is how a repeated start
  and a stop are told from a data bit, and it is the rule the generator's `expected()` follows (a byte
  counts when the falling edge of its acknowledge pulse is in the capture).
- **Items were emitted from the start** (task T035 was done with US2) because the round trip compares
  items.

## Known issues

- **The menu entry Decoders → Add decoder and the key D do nothing now.** With two built-in decoders the
  menu cannot choose which to add (`Message::AddDecoderMenu` adds a decoder only when there is exactly
  one, a rule of feature 007). The list **Add decoder** in the Decoders panel works, and the user guide
  says so. Making the menu offer one entry per decoder is a change to `crates/app` (and so outside
  SC-008): it is left for the owner to decide.
- **`app::tests::the_interface_scale_steps_and_follows_the_setting` failed once in a parallel run** and
  passes alone and on rerun. It names its settings directory after the process id, so two tests that
  write settings can meet. It is not touched by this feature.

## Left to a person

- Look at the lane and the overlays on **Windows 10/11 and Debian 13** (quickstart, section 4): add I2C
  to a simulated capture, zoom to one byte (nine arrows, dots at the start and the stop), turn errors
  on, switch the address notation.
- **SC-002**: someone who has not read the guide goes from an empty application to a decoded I2C
  transaction with the virtual device in under a minute. (The menu issue above affects this: the list in
  the panel is the way.)
- **Translations** of the 27 new keys (machine drafts), including the Japanese and the long German texts
  in the lane (`Byte nach { $value } Bits abgebrochen`, `アドレス { $value }（書き込み）`).
- A real I2C capture, when hardware exists (feature 011 onward).
