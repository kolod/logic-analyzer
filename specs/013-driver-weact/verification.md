# Verification: 013 driver for the WeAct LogicAnalyzer V1

Status at the time of writing: everything that can be checked without the board is checked and passes;
the items that need the real board (T038) are **left to the owner** and listed at the end.

All tests are in `crates/driver-weact/tests/` unless a crate is named.

## Requirements and the tests that prove them

| Requirement | Proof |
|-------------|-------|
| FR-001 listing, FR-002 stable identity | `provider.rs` (listing, identity, numbering); `preparation.rs` (one identity across the renumeration, never twice) |
| FR-003 listing does not block | `preparation.rs` (hung download, scan under 200 ms); `provider.rs::scanning_is_quick`; `failures.rs` (hang) |
| FR-004 only the listed identity | `provider.rs::a_device_with_another_identity_is_never_listed_or_opened`; `hostile.rs` (strange descriptors) |
| FR-005 capabilities, FR-006 refused configurations, FR-007 exact rates | `provider.rs` (capabilities); `acquisition.rs::a_configuration_the_core_rejects_never_reaches_the_board`, `every_listed_rate_starts_a_capture_with_its_own_payload`; `protocol.rs` |
| FR-008 preparation without user action, FR-009 no download for a running board, FR-010 image checks, FR-011 not renumerated | `preparation.rs`; `firmware.rs`; unit tests of `src/firmware.rs` |
| FR-012 start returns at once, FR-013 stop in time, FR-014 finishes by itself, FR-015 one capture per board | `acquisition.rs` (including the prompt stop of an open-ended capture and the truncated last chunk); `conformance.rs` (shared suite) |
| FR-016 books balance, FR-017 stream stops, FR-018 silence limits | `failures.rs` (exact balance for several received counts, pauses inside the limit, no data); `random_scripts.rs` |
| FR-019 not simulated | `acquisition.rs::a_capture_is_not_simulated_and_records_the_device_and_the_rate` |
| FR-020 failures end the capture and keep the data, FR-021 containment | `failures.rs` (unplug at start, mid-stream and during stop; transport errors; 210 panics at seven points; a hang); `random_scripts.rs` (10,000 scripts with panics, hangs and errors) |
| FR-022 typed errors, FR-023 distinguished causes | `src/error.rs` unit tests; `preparation.rs` (access, driver, in use); `failures.rs`; host error-code test (`cargo test -p la-host`) |
| FR-024 hardware-free tests and recordings | `recordings.rs`, `replay.rs` (six hand-written recordings), `src/testing.rs` and `src/recording.rs` |
| FR-025 manual procedure | `docs/drivers/weact-verification.md`; `docs.rs` |
| FR-026 documentation | `docs/drivers/weact.md`, `packaging/linux/60-logic-analyzer-weact.rules`, `packaging/windows/README.md`, `docs/user-guide.md`; `docs.rs` |
| FR-027 localization | the nine codes in six languages (`resources/i18n/*/errors.ftl`, checked by the parity tests of `la-i18n` and `la-host`); the device name is a proper name and is not translated |
| FR-028 nothing else changes | the whole workspace test run; the virtual device suites are untouched |
| FR-029 third-party material | `docs/third-party-notices.md`, `crates/driver-weact/firmware/SOURCE.md` (checksum checked by `firmware.rs`), `docs.rs` |
| FR-030 the rig, FR-031 opt-in | `rig/arduino-nano/`; `rig.rs` (ignored by default, prints `SKIPPED` without `LA_RIG_PORT`); the pattern checkers' unit tests run always |
| SC-002 sample-for-sample | `acquisition.rs::a_capture_of_a_known_signal_is_stored_sample_for_sample`, `channels_that_are_not_enabled_are_zero`; `replay.rs` (counter signal) |
| SC-003 failures are typed and keep data | `failures.rs` |
| SC-004 books balance | `failures.rs`, `random_scripts.rs`, `replay.rs` |
| SC-005 stop in time | `acquisition.rs::an_open_ended_capture_runs_until_stopped_and_stops_promptly` (250 ms margin on the 100 ms of the contract) |
| SC-006 containment | `failures.rs` (210 injected panics and a hang), `random_scripts.rs` (10,000 scripts, about one in three injecting a panic, hang or error at each of eight points). The count of panics alone is below the 1,000 of the criterion in `failures.rs`; the random scripts cover it |
| SC-007 no hardware needed | the suite runs without the board; the shared conformance suite passes (`conformance.rs`) |
| SC-010 hot plug | `preparation.rs::a_hundred_rapid_plugs_and_unplugs_leave_the_list_equal_to_what_is_present` |

## Mutation checks (T035)

A line was broken, the named tests run, and the line restored (the working tree was clean afterwards).

| Mutation | Result |
|----------|--------|
| the delay of a rate is one too small | killed (`the_start_payload_of_every_rate_is_the_table_of_the_contract`, `the_clock_rule_prefers_48_mhz_…`) |
| the boundary of the delay `> 1,536` made `>= 1,536` | **survived, and is equivalent**: no rate of the table gives a delay of exactly 1,536 (`48 MHz / 1,537` and `30 MHz / 1,537` are not whole numbers), so no input can tell the two apart |
| the silence floor of 2 s lowered to 100 ms | survived at first (the tests only had silences longer than the limit); the test `a_pause_shorter_than_the_silence_limit_is_not_the_end_of_the_stream` was added and kills it |
| the truncation of the last chunk removed | killed (`a_set_length_shorter_than_the_data_truncates_the_last_chunk_and_finishes`) |
| `Lost(requested - received)` made `Lost(requested)` | killed (`a_stream_that_stops_in_a_capture_of_a_set_length_is_one_gap_and_the_books_balance`) |
| the 5 s renumeration limit made 9 s | killed (`a_board_that_never_returns_fails_after_five_seconds_…`) |
| the drain of stale data removed | killed (`two_captures_in_a_row_work_and_old_bytes_in_the_buffer_never_enter_the_second`) |
| a board that left the bus is kept in the table | killed (`a_board_unplugged_and_plugged_again_is_listed_again_and_captures_again`) |
| the separator between ports removed from the identity | killed (`usb::tests::…`) |

## Localization review (T036)

The nine `weact.*` codes exist in all six languages with advice (the parity tests pass). For a reader of
Japanese and German, the messages most likely to be long or awkward are `error-weact-stream_stopped`
(three sentences) and `error-weact-driver_missing` (it points to the documentation for a procedure).
Both are acceptable, but a native reader should look at them first.

## Principle I and licence review (T037)

- `cargo clippy --workspace --all-targets -- -D warnings` is clean; `cargo +1.85 check -p
  la-driver-weact --all-targets` is clean; the new code has no `unwrap`, `expect`, indexing, unchecked
  arithmetic or `unsafe` outside tests (the lints are denied in the crate).
- Every parser of device data has a hostile-input test: the version answer, the strings and
  identifiers (`hostile.rs`), the stream bytes and sizes (`hostile.rs`, `protocol.rs`) and the
  recording format (`recordings.rs`).
- `cargo deny` runs in CI; it passed on the draft pull request with `nusb`.
- The third-party table in `plan.md`, `docs/third-party-notices.md` and `firmware/SOURCE.md` agree
  (nusb; the firmware image under GPL-2.0-or-later, shipped as data; `serialport` and `sha2` for tests only).
- The `nusb` call that names the bound driver exists on Windows only; on other systems the driver name
  is simply absent (found when the Debian jobs of the draft pull request failed, fixed).

## Deviations from the plan

- The preparation uses one worker per blank board rather than one at a time, so a board that hangs
  cannot hold up another (a requirement of FR-003 read strictly).
- The revision request (`0xB2`) is not read by the driver.
- The recordings are in a new module `src/recording.rs` (feature `test-support`) rather than inside
  `testing.rs`.
- The loop of injected panics in `failures.rs` has 210 rounds; the 1,000 of SC-006 are covered by the
  10,000 random scripts.
- The engine reports an error code that is not `device.*` as `device.driver_error` with the driver's
  code in its detail; the tests read the driver's code from there. Showing `weact.*` messages to the
  user relies on the application using that detail, which is a matter for the host and the interface.

## Known issues

- The Windows timing tests of other features (storage, decoder sessions) are flaky under load and are
  re-run in CI; they are not part of this feature.

## Verification on the real board (T038, 2026-11-17, Windows 11)

Board: WeAct LogicAnalyzer V1 on WinUSB (installed with Zadig), wired to the Arduino Nano rig on COM7,
the sketch of `rig/arduino-nano/`. What was found:

- The blank board is listed with the driver `WinUSB`, no strings; the driver prepares it (hold, 4,096-byte
  chunks from address 0, release) and it comes back running the firmware: the download chunk size and
  the CPUCS values of the contract are right. It reports firmware version 1.5 with the shipped image.
- `cargo test -p la-driver-weact --test rig -- --ignored` passed six of six in three consecutive runs:
  `HOLD` on each of the eight channels, `WALK` at 1 MS/s, `COUNTER` for 300 ms (no value skipped),
  `SQUARE` at 500 kS/s, every rate from 100 kS/s to 1 MS/s, and two captures in a row with the same
  structure. A real recording of the walk is in `tests/recordings/rig-walk-1mhz.rec`.
- **The stock firmware 1.4 hangs.** Sending `CMD_START` while a capture was being read (my experiments
  with two STARTs in a row) left the board deaf to every request until it was unplugged: the
  `while (!(GPIFTRIG & 0x80))` of `gpif_acquisition_prepare` never ends while the host reads. Before the
  fix the first samples of a capture were also stale samples of the previous one, which looked like a
  capture running at the previous capture's rate. Both are fixed by the patched firmware 1.5 (a bounded
  START, a FIFO reset at START, a STOP command) and by the driver starting with nothing in flight; see
  `crates/driver-weact/firmware/SOURCE.md`.
- **The sequences that hung the stock firmware were then tried on purpose** with the patched 1.5
  (`firmware_survives_the_start_sequences_that_hung_the_stock_firmware`, through the raw link): START
  while the previous capture is still being read; two STARTs in a row with nothing in flight; a 20 kHz
  capture cancelled without STOP and restarted at once, three times; START followed at once by STOP,
  three times; STOP with nothing running, twice. After each the board still answered the version
  request, and a normal capture afterwards was still right. Passed in four consecutive runs together
  with the six other rig tests.
- The rig itself needed fixes found here: the sketch read the serial port by the register while the
  library owned the interrupt (no answers), kept interrupts off too long (lost bytes) and wrote the two
  ports with an interrupt possible between them (a mixed value for a few microseconds); the pattern
  checkers also ignore the 0.1 µs skew between the two ports.

Not yet done on the board: the manual procedure of `docs/drivers/weact-verification.md` (steps 7 to 14:
unplug during a capture, a hub or dock at 24 MHz, another program holding the board, a clean Windows 11
and Debian 13) and the product string and manufacturer string Windows reports for a running board (the
listing worked with the rule of the contract, so the board is recognised).

## Left to a person

- Run `docs/drivers/weact-verification.md` steps 7 to 14 and attach the record.
- SC-001 (time from start to first capture), SC-008 (a 60-second capture at 24 MHz on a direct port),
  SC-009 (a new user on clean Windows 11 and Debian 13); the rig covers SC-011 up to 1 MS/s (the Nano
  cannot drive faster).
