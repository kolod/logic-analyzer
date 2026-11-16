---

description: "Task list for I2C Decoder and Generator"
---

# Tasks: I2C Decoder and Generator

**Input**: Design documents from `/specs/008-i2c-decoder-generator/`

**Prerequisites**: plan.md, spec.md, research.md (R1–R14), data-model.md, contracts/
(`decoder-api-1.1.md`, `i2c-items.md`, `i2c-generator.md`), quickstart.md

**Tests**: REQUIRED (constitution Principle III). In every phase the test tasks come first and MUST fail
(usually they do not compile yet) before the implementation tasks start. If a test passes at once because
earlier work already covers it, run a mutation check (temporarily break the code it guards, see the test
fail, restore) before relying on it. Expected values (annotation sets, item payloads, positions) are
written **by hand** from the contracts and from the bus builder's own arithmetic, never computed by the
code under test. The generator and the decoder are each other's test only in the round-trip tests; every
other test has a hand-written expectation.

**Organization**: tasks are grouped by user story (US1–US5, numbered as in spec.md), in this order: Setup,
Foundational (the one framework change), **US1** (read transactions, P1), **US2** (generator and round
trip, P1), **US3** (errors, P2), **US4** (options and 10-bit addresses, P2), **US5** (stacking output,
P3), Polish. Items for stacking are emitted in US5 and not before; everything else a story needs is in
the story.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: can run in parallel (different files, no dependency on an incomplete task)
- **[Story]**: user story from spec.md
- All paths are relative to the repository root. No new crate; everything is in `la-decoders` except
  host tests.

## Path Conventions

Decoder in `crates/decoders/src/decoders/i2c/`, generator in `crates/decoders/src/generators/i2c.rs`,
tests in `crates/decoders/tests/`, fixtures in `crates/decoders/tests/fixtures/i2c/`, benchmark in
`crates/decoders/benches/`, host tests in `crates/host/tests/`, strings in
`resources/i18n/<lang>/ui.ftl`, documents in `docs/`. "The items contract" is
`specs/008-i2c-decoder-generator/contracts/i2c-items.md`, "the generator contract" is
`contracts/i2c-generator.md`, "the API change" is `contracts/decoder-api-1.1.md`; decisions are
`research.md` R1–R14.

## Lint reminder (applies to every Rust task)

The workspace denies `unwrap`, `expect`, indexing (`v[i]`), unchecked arithmetic, `panic!` and `unsafe`
in production code of `la-decoders`. Use `get`, `checked_*`, `saturating_*`, `u128` intermediates and
`?`. The generator's per-sample function may carry the same file-level `allow(clippy::arithmetic_side_effects)`
as `generators/clocked_bits.rs`, with the same justification (parameters clamped when made). Test code
may relax lints with the file-level `#![allow(...)]` used by the other tests. Keep the code at **MSRV
1.85**: no `let` chains (check with `cargo +1.85 check -p la-decoders`). Never commit while clippy fails:
a helper that only a later task uses gets `#[expect(dead_code, reason = "used by T0xx")]`, removed by
that task. Never log signal data, annotation text or instance names. Every UI string comes from `ui.ftl`
(English first, then the five translations as drafts, **in the same task that first uses the key**, so
the parity check "no missing, no unused key" is green at every commit); message keys are string
**literals** in the code. Run `cargo fmt` before the next edit. Windows scripts that patch files: write
them to a file, not a shell heredoc.

---

## Phase 1: Setup

- [X] T001 Create the empty module skeletons with their doc comments and declarations:
  `crates/decoders/src/decoders/i2c/{mod,period,bus,text}.rs` (`mod.rs` declares the other three),
  `crates/decoders/src/generators/i2c.rs`; add `pub mod i2c;` to `crates/decoders/src/decoders/mod.rs`
  and `crates/decoders/src/generators/mod.rs`. Nothing is registered yet; the crate builds and clippy is
  clean (`#![deny(missing_docs)]` applies: every public item has a doc comment).
- [X] T002 [P] Test helpers in `crates/decoders/tests/common/i2c.rs` (declare in
  `crates/decoders/tests/common/mod.rs`): a **bus builder** that turns a list of bus events into
  `(sda, scl)` samples with a fixed 16 samples per SCL period, exactly as `contracts/i2c-generator.md`
  describes the bus (SCL high on samples 4..12 of a slot, SDA changing on sample 0): events `Idle(n)`,
  `Start`, `RepeatedStart`, `Bit(bool)`, `Byte(u8, ack)`, `Stop`, `Raw(sda, scl, n)`; it returns the
  samples and, for every event, the positions the tests need (the SDA edge of a start, the SCL rising
  edge of each bit). Add a **fixture loader** `load_fixture(name) -> Fixture` for the text format of
  T010 (documented in the module comment: one line per run `<sda><scl> <samples>`, `#` comments, and a
  `--- expect` section of `lane|overlay|item` lines). The loader returns a typed error for a malformed
  file. Test it in `crates/decoders/tests/i2c_fixtures.rs` with malformed inputs (bad level, negative or
  huge count, missing section, a run that overflows the length) and one good file.

---

## Phase 2: Foundational (the one change to the framework contract)

**Purpose**: `Run::final_through` (decoder API 1.1, research R3). BLOCKS US1.

**⚠️ CRITICAL**: the I2C decoder must hold work back honestly; no story can be finished without this.

- [X] T003 [P] Write `crates/decoders/tests/final_through.rs` first (it does not compile yet): with a
  test-only decoder (in the test file; it implements `Run` and holds back a configurable amount) check
  that the coverage of the results follows `min(chunk end, final_through)`; never exceeds the chunk end;
  never decreases even if `final_through` goes backwards; with the default (`None`) equals the chunk end
  exactly as before; after `finish(End::Complete)` equals the end of the pass; after `finish(End::Cut)`
  the results are marked incomplete; and a stacked decoder (`stack.rs` stream) is handed no item whose
  end is above the watermark, with items emitted late still delivered in order.
- [X] T004 Implement the API change: add `fn final_through(&self) -> Option<u64> { None }` with its
  doc comment to the `Run` trait in `crates/decoders/src/run/mod.rs`; in `Pass::advance`
  (`crates/decoders/src/run/job.rs`) report the coverage after each chunk as
  `min(end, run.final_through().unwrap_or(end))` clamped to be non-decreasing (`set_coverage` already
  keeps the maximum for the stream); `finish` keeps its present behaviour. The sample decoder and all
  existing tests are untouched and pass; T003 passes.
- [X] T005 Version and documents: `DECODER_API_VERSION = "1.1"` in `crates/decoders/src/lib.rs`;
  `docs/decoder-api.md`: version line `Version: 1.1`, the method under "What a decoder provides", a
  sentence under "What the framework guarantees", the generator options `errors` and `error_rate` as
  options every generator may use (they are what the shell edits), and the versioning example. The test
  `decoder_api_doc.rs` passes unchanged.

**Checkpoint**: framework ready; `cargo test -p la-decoders --features test-support` green.

---

## Phase 3: User Story 1 - Read the transactions on an I2C bus (Priority: P1) 🎯 MVP

**Goal**: add I2C with SCL and SDA, see transactions in the lane and the dots and arrows on the signals;
synchronise on the bus; start-up and truncated captures are handled silently and correctly.

**Independent Test**: decode a hand-built capture of a write of two acknowledged bytes to 0x50 and check
the lane, the overlays and the positions; decode captures that begin mid-byte or before initialisation
and check nothing is reported before the first valid start.

### Tests for User Story 1 (write first; they must fail)

- [X] T006 [P] [US1] `crates/decoders/tests/i2c_period.rs`: the period estimator of research R1 as pure
  functions on a list of SCL rising-edge positions (the API is `Period::new()`, `Period::edge(position)
  -> Option<u64>` returning `Some(T)` at the first edge after which the condition holds, `None` before).
  Cases, expected values by hand: a steady clock of 16 gives `T = 16` exactly after the 17th edge (16
  intervals, all in one bin); fewer than 16 intervals gives `None`; 100 random-looking junk intervals then
  steady traffic calibrates only when the steady bin holds half of all intervals, at an index computed
  by hand; a clock with every ninth interval stretched to 40 still gives `T = 16`; intervals of 15, 16, 17
  fall in the same or neighbouring bins and give their mean; a break (a gap) between two edges adds no
  interval; the result for a list is the same however it is split across calls (feed it one edge at a
  time and in batches) and never changes after it was found; huge positions (`u64::MAX - 1`) and equal
  positions do not panic or overflow.
- [X] T007 [P] [US1] `crates/decoders/tests/i2c.rs` (decoding, positions from the bus builder): a write to
  0x50 with bytes 0x12 and 0x34, all ACK, `Idle(64)` before and after, decoded with the decoder created
  by `decoders::i2c::create` over a closed capture, with the whole `Chunk` loop (`run_pass`), expects
  exactly: lane points for start and stop (keys `ann-i2c-start`, `ann-i2c-stop`), lane ranges for the
  address byte (`ann-i2c-address-write`, argument `0x50`) and the two data bytes (`ann-i2c-data`, `0x12`,
  `0x34`) each from the first bit's SCL rising edge to the acknowledge edge, lane points `ann-i2c-ack` at
  each acknowledge edge, one transaction range (`ann-i2c-transaction-write`, `0x50`) from the start edge
  to the stop edge + 1, a dot (`Shape::Dot`, no colour) on the SDA channel at the start and at the stop,
  and 27 arrows (`Shape::Arrow`, `rising: true`, no colour) on the SCL channel on every rising edge
  including the acknowledge bits; no error annotation. Further cases in the same file: a read of two
  bytes where the controller answers NAK on the last (no error; `ann-i2c-address-read`); a write then a
  repeated start then a read (one transaction range, a lane point `ann-i2c-repeated-start`, a dot on SDA
  at the repeated start); a NAK on the address byte and a NAK on a written data byte are errors
  (`error = true`) at the acknowledge point (detail messages come in US3, here only the flag and the
  position); SDA and SCL changing on the same sample as a clock edge (SCL is processed first, the bit is
  the level SDA held before); SDA and SCL assigned to swapped channels give an empty or wrong but valid
  result and no panic (scenario 5).
- [X] T008 [P] [US1] `crates/decoders/tests/i2c.rs` (synchronisation and start-up, research R2): nothing is
  drawn or reported before the first start that follows an idle of at least `T` with both lines high:
  (a) a capture that begins in the middle of a byte (SCL pulses, SDA changing, then an ordinary
  transaction after an idle) decodes only the ordinary transaction; (b) lines held low for thousands of
  samples then released: the same; (c) SDA falling while SCL is high with an idle shorter than `T` before
  it is ignored, and the next real start is accepted; (d) a stop-like edge before the first start is
  ignored; (e) an idle run that begins at position 0 counts; (f) a gap in the capture breaks an idle run;
  (g) a capture with fewer SCL pulses than the calibration needs gives an empty result and no error and
  `finish` returns `Ok`; (h) a capture fed in pieces (`append_parts`): while the period is unknown
  nothing is stored and the coverage reported by `Pass` stays at 0 (`final_through`), once enough pulses
  arrive the whole capture is decoded and equals the result for the same signal decoded closed in one
  pass; (i) a start-up of 3,000,000 edges before the first idle decodes the same transaction (and each
  call stays within the work bound: assert the number of `process` calls is more than 1 and that no
  single call reports more than 262,144 edges processed through a counter the decoder exposes under
  `#[cfg(test)]`-free means, namely the observable `final_through` steps).
- [X] T009 [P] [US1] Recorded signals: create `crates/decoders/tests/fixtures/i2c/` with
  `write_two_bytes.txt`, `read_two_bytes.txt`, `repeated_start.txt`, `begins_mid_byte.txt`,
  `preinit_low_then_noise.txt`, `clock_stretching.txt`, each in the format of T002 with a hand-written
  `--- expect` section, and `crates/decoders/tests/i2c_recorded.rs` that decodes each and compares every
  line. (Fixtures for errors are added in US3.)

### Implementation for User Story 1

- [X] T010 [US1] `crates/decoders/src/decoders/i2c/period.rs`: `Period` per research R1 (256 quarter-octave
  bins of `{count: u32, sum: u64}`, the previous rising-edge position, the interval count, `found:
  Option<u64>`), integer arithmetic only, `bin = 4·(63 - leading_zeros) + next two bits`, clamped to 255;
  `edge(position) -> Option<u64>` as T006 specifies; an explicit `break_at_gap()` that forgets the previous
  edge. T006 passes.
- [X] T011 [US1] `crates/decoders/src/decoders/i2c/text.rs`: the annotation texts of this story as functions
  returning `Text::Message { key, args }` with **literal** keys (`ann-i2c-start`, `-repeated-start`,
  `-stop`, `-transaction-write`, `-transaction-read`, `-incomplete`, `-address-write`, `-address-read`,
  `-data`, `-ack`, `-nak`) and the value formatting for the default options only (7-bit address and
  hexadecimal as `0x50`, a byte as `0x12`); and the styles (colour indexes for transaction, address,
  data; marks on signals name no colour). Add the keys with their argument documentation to
  `resources/i18n/en/ui.ftl` and the drafts to de, fr, es, ja, uk.
- [X] T012 [US1] `crates/decoders/src/decoders/i2c/bus.rs`: the bus state machine of research R4 as a type
  that is fed edges (`scl`/`sda`, rising or falling, position) in the order the reader delivers them and
  writes to a `&mut Sink`: levels of both lines (initial levels from the reader at position 0; unknown
  after a gap), the idle-run tracking and the synchronisation rule of R2 (first start only after both
  lines high for at least `T`), start, repeated start and stop from SDA edges while SCL is high, bit
  sampling on SCL rising edges (data bit = SDA before a same-position SDA edge), nine bits to a byte and
  its acknowledge, the address byte and direction, data bytes, transaction ranges, the lane points, the
  overlay dots and arrows, ACK or NAK (error flag per FR-005: NAK on an address and on a written byte is
  an error, never a NAK of the controller on a read), all with checked arithmetic and no indexing. Errors
  other than the NAKs (cut short, empty, gap, end of capture) are US3; until then those situations end
  the transaction silently. T007, T008 (a–g) and T009 pass for the cases that do not need US3.
- [X] T013 [US1] `crates/decoders/src/decoders/i2c/mod.rs`: the descriptor (`id` `i2c`, version 1, signals
  `scl` then `sda`, both required and not shareable, no options yet, `outputs: &["i2c"]`, `generator:
  true`), `create(&Setup)`, and the `Run` implementation in two states: **calibrating** (reads SCL rising
  edges from position 0 with `reader.edges`, at most 262,144 edges per call, feeds `Period`, outputs
  nothing) and **running** (the bus machine over the part not yet processed, in slices of at most
  262,144 edges per call, advancing as far as the chunk end), with `final_through` returning the
  position processed, and `finish` draining what is left and flushing a pending construct (End::Complete
  and End::Cut alike). Keys `decoder-i2c-name`, `-description`, `-signal-scl`, `-signal-sda` in six
  languages. T008 (h, i) passes.
- [X] T014 [US1] Register the decoder: in `Registry::builtin()` (`crates/decoders/src/registry.rs`) register
  `decoders::i2c::DESCRIPTOR` with `create`; the generator comes in US2. Run the whole workspace tests and
  fix every test that assumed a single built-in decoder (search for `catalogue.len()`, `decoders.len()` and
  `instances[0]` in `crates/host/tests/` and `crates/app/src/app.rs` tests); such a change is only to the
  expectation, never to the shell. Add `crates/host/tests/decoder_i2c.rs`: with a hand-built capture
  (`common` helpers of the host tests) add the `i2c` decoder with `Command::AddDecoder`, assign SCL and
  SDA with `SetDecoderChannel`, wait for the pass, and check `frame_plan` has one lane with the address
  and bytes, overlays on both channels, and that swapping the two channels (assign again) re-runs the
  decoder and gives the other result without failing.

**Checkpoint**: US1 works with hand-built signals; `cargo test -p la-decoders -p la-host` green.

---

## Phase 4: User Story 2 - Test against traffic the virtual device made (Priority: P1)

**Goal**: the virtual device produces valid I2C traffic for a configured instance; the round trip passes.

**Independent Test**: for many seeds, generate, decode, compare transactions (errors off).

### Tests for User Story 2 (write first; they must fail)

- [X] T015 [P] [US2] `crates/decoders/tests/i2c_round_trip.rs`: for seeds `0..seeds()` (env
  `LA_ROUND_TRIP_SEEDS`, default 1,000) with `max_bytes` varied 1..=6, generate with
  `generators::i2c::source`, decode (same `decode` helper as `round_trip.rs`), and require the decoded
  items to equal `expected(...)` exactly and no error annotation; also: the same seed twice gives the same
  signal and another seed another one; the signal is the same for fills of 1, 7, 4,096 and the whole
  length; two instances with the same seed differ (instance number); only the two channels carry the
  signal; the first 64 samples of a frame are both lines high; a 10,000-seed `#[ignore]` test for
  release mode. (These need items, which US5 emits; until then the test compares through a helper that
  rebuilds the transactions from the lane annotations — write it so it uses items once T033 lands, and
  note this in a comment so the helper is removed then.)
- [X] T016 [P] [US2] `crates/decoders/tests/i2c_chunking.rs` and `crates/decoders/tests/i2c_random.rs`:
  chunking: for 50 seeds, decode with `ChunkLimits { max_edges, max_span }` from (1, 1) up to the default
  and with a capture fed in pieces of 1, 7, 1,000 and 100,000 samples (`append_parts`), and require the
  complete set of annotations to be identical (SC-005). Random: `random.rs` feeds one million chunks of
  random levels (including one-sample pulses, long constant runs, and gaps made with the helpers) in many
  captures and requires: no panic, every annotation valid for the bounds, `finish` returns `Ok`, and with
  the property that the decoder never returns a `DecodeError` (SC-004).
- [X] T017 [P] [US2] In `crates/decoders/tests/composite.rs`: a registry claim for an `i2c` setup fills only
  its two channels (`claimed_channels`), leaves the others to the random signals, and an instance without
  both channels assigned gives no claim (FR-018).

### Implementation for User Story 2

- [X] T018 [US2] `crates/decoders/src/generators/i2c.rs` per `contracts/i2c-generator.md` for the part
  without errors, start-up and 10-bit: `Params::from_setup`, the pure `sample(position)`, the frame layout
  and sizing from `max_bytes`, 16 samples per period, the lead-in of 64 samples, random content from
  `mix(seed, k)` (reuse the `mix` of `generators/clocked_bits.rs` by making it `pub(super)`),
  `source`, `claim`, `claimed_channels`, and `expected` (items per `contracts/i2c-items.md`; write the
  item builder here once and share it with T033). Unknown keys and out-of-range values are clamped.
  `#![allow(clippy::arithmetic_side_effects)]` for the per-sample code only, with the justification.
- [X] T019 [US2] Register the generator: in `Registry::builtin()` push `(decoders::i2c::ID,
  generators::i2c::claim)` after the sample decoder's. T015 (through lanes), T016, T017 pass.
- [X] T020 [US2] Host end to end in `crates/host/tests/decoder_i2c.rs`: a virtual device capture with an
  `i2c` instance (the channels assigned before the capture starts) decodes to the transactions that
  `generators::i2c::expected` returns for the seed and length; the same seed twice gives identical
  captures; with no instance the channels are the ordinary random signals; a capture of 10,000,000
  samples never lets one `Shell::poll` take more than 100 ms while decoding (the measurement of
  `opening_a_file_with_many_saved_results_does_not_hold_the_interface` is the model) (SC-006).
- [X] T021 [P] [US2] Benchmark: add an I2C case to `crates/decoders/benches/decode_throughput.rs`
  (generated dense traffic, `max_bytes` 4, edges per second) and record its baseline in
  `crates/decoders/benches/baseline.json` in the format the file already uses; the case must be
  deterministic (fixed seed).

**Checkpoint**: US2 works; the generator and the decoder agree on 1,000 seeds.

---

## Phase 5: User Story 3 - See the errors on purpose (Priority: P2)

**Goal**: protocol violations and planted errors are reported where they happen with a message; the
decoder carries on; truncated and start-up captures round trip.

**Independent Test**: plant errors with known positions; the decoder reports exactly those.

### Tests for User Story 3 (write first; they must fail)

- [X] T022 [P] [US3] In `crates/decoders/tests/i2c.rs`, hand-built signals with hand-written expected
  errors: the address NAK (message `ann-i2c-error-address-nak` with the address, error flag, the
  transaction ends at its stop); a data NAK on a written byte (`ann-i2c-error-data-nak`); no error for the
  controller's NAK of the last read byte; a stop in the middle of a byte after 3 bits
  (`ann-i2c-error-byte-cut` with the bit count, the stop still marked, the next transaction decodes); a
  start in the middle of a byte (the byte reported cut short, the transaction restarted as a start of a
  new one, no item); a start followed directly by a stop with no clock (`ann-i2c-error-empty`); a gap
  in the middle of a byte (`ann-i2c-error-gap`, the transaction is incomplete, decoding resumes at the
  next accepted start); a capture that ends in the middle of a byte (`finish(Complete)`: the
  `ann-i2c-error-byte-cut` error, a transaction range to the end with `ann-i2c-incomplete`, not an
  error); a capture that ends between bytes (the incomplete range, no byte error); and a growing capture
  where the transaction continues when more samples arrive (the result equals the closed capture).
- [X] T023 [P] [US3] In `crates/decoders/tests/i2c_round_trip.rs`: with `errors = 1` and `error_rate` 2..12,
  for 400 seeds, the decoded items equal `expected` and the error annotations (kind, position) equal
  `expected_errors` exactly; with `errors = 0` no error; the same seed and rate plant the same errors;
  lengths chosen to cut inside a transaction (every phase of one frame) give the items completed before
  the cut and no spurious error; `startup = 1` for 200 seeds decodes the same items as `startup = 0`
  shifted by the start-up length, and nothing is reported for the start-up. Assert that more than 100
  errors were planted across the seeds so the test cannot pass vacuously.
- [X] T024 [P] [US3] Fixtures `nak_on_address.txt`, `nak_on_data.txt`, `byte_cut_by_stop.txt`,
  `start_without_clock.txt`, `ends_mid_transaction.txt` in `crates/decoders/tests/fixtures/i2c/` and their
  cases in `crates/decoders/tests/i2c_recorded.rs`.

### Implementation for User Story 3

- [X] T025 [US3] Error paths in `bus.rs` and `text.rs`: the cut-short byte on a stop, on a start and on the
  end of the pass (`finish`), the empty transaction, the gap, the incomplete transaction range, with the
  messages and arguments of `research.md` R9 (`ann-i2c-error-address-nak`, `-data-nak`, `-byte-cut`,
  `-empty`, `-gap`, `ann-i2c-incomplete`) and the error colour; the keys with their drafts in six
  languages. T022 and T024 pass.
- [X] T026 [US3] Generator errors and start-up in `generators/i2c.rs`: planted errors (address NAK, data
  NAK, cut short) chosen per frame from `mix`, with `error_rate` clamped to at least 2; the
  `startup` prefix (lines low, then pulses with highs shorter than `T`, a multiple of the bit period, at
  most 8 periods, no valid start); `expected` returns items with the error flag where an acknowledge is
  required and missing; `expected_errors` returns `PlantedError::{AddressNak, DataNak, ByteCut}` with
  positions; truncated lengths handled. T023 passes. Document the options in the module comment.
- [X] T027 [US3] Host: in `crates/host/tests/decoder_i2c.rs` set `Setup::generator` `errors` and
  `error_rate` through the command the shell uses for them (`Command::ConfigureDecoder { id, setup }` with the key
  inserted into `setup.generator`, as `Message::DecoderGenerator` does in `crates/app/src/app.rs`), run a virtual capture and check that the lane holds error
  annotations at the planted positions; with the option off none. No shell code changes.

**Checkpoint**: US3 works; errors are where they were planted and nowhere else.

---

## Phase 6: User Story 4 - Choose how the bus is read (Priority: P2)

**Goal**: address notation and number base options; 10-bit addresses shown as one address.

**Independent Test**: decode one capture with each setting; only the text changes.

### Tests for User Story 4 (write first; they must fail)

- [ ] T028 [P] [US4] In `crates/decoders/tests/i2c.rs`: option `address` = `addr8` shows `0xA0` for a write
  and `0xA1` for a read of the device at 0x50 and decodes everything else identically (compare all
  annotations except the text arguments); option `radix` = `dec` and `bin` format address and data (`80`,
  `0b01010000`); a 10-bit write (header `11110 01 0`, low byte `0x23`) is shown as one range
  `ann-i2c-address-write` with the argument `0x123` and a single ACK pair for each byte; the read part
  after a repeated start with header `11110 01 1` shows the same address; a header `11110xx1` with no
  preceding write header is an ordinary address; invalid option values are refused by the framework's
  validation (test through `Setup::validate`).
- [ ] T029 [P] [US4] Host in `crates/host/tests/decoder_i2c.rs`: `SetDecoderOption` for `address` re-runs
  the decoder, the old results stay flagged outdated until the new ones replace them (the rule of
  feature 007, test model: `a_setup_change_keeps_the_old_results_flagged_outdated...`); after saving and
  reopening the session the options and channels are restored (`DECO`), and results too when saved
  (`DANN`); a session saved with an `i2c` instance opens with the instance kept raw (`Kept`) by a
  registry that lacks the decoder.
- [ ] T030 [P] [US4] In `crates/decoders/tests/i2c_round_trip.rs`: `ten_bit = 1` for 300 seeds round trips
  (generator draws 10-bit addresses with the write header and low byte, and a split transaction with the
  read header); items carry the 10-bit flag and value.

### Implementation for User Story 4

- [ ] T031 [US4] Options in the descriptor and `text.rs`: `address` (`addr7` default, `addr8`), `radix`
  (`hex` default, `dec`, `bin`) as `OptionSpec`s with labels; `create` reads them; the formatting
  functions of T011 take them; keys `decoder-i2c-option-address`, `-choice-address-addr7`,
  `-choice-address-addr8`, `-option-radix`, `-choice-radix-hex`, `-choice-radix-dec`,
  `-choice-radix-bin` in six languages. T028 (options) and T029 pass.
- [ ] T032 [US4] 10-bit addresses in `bus.rs` per research R4 (header test `0b11110xxD`, the second byte,
  the read part after a repeated start, the state held in the transaction) and in `generators/i2c.rs`
  (`ten_bit`, frame sizing for the longer address). T028 (10-bit) and T030 pass.

**Checkpoint**: US4 works; every story so far is green.

---

## Phase 7: User Story 5 - Build on the result (Priority: P3)

**Goal**: a stacked decoder receives the `i2c` items exactly as the contract says.

**Independent Test**: a test-only decoder stacked on I2C receives the transactions the lane shows.

### Tests for User Story 5 (write first; they must fail)

- [ ] T033 [P] [US5] `crates/decoders/tests/i2c_stacking.rs`: with the test-only decoder of T034 stacked on
  an I2C instance (`Input::Items("i2c")`), over generated traffic (200 seeds, errors on and off), the
  items it receives equal `generators::i2c::expected` exactly (layout per the items contract: byte 0 type,
  1 direction, 2 acknowledgement, 3 flags, 4–5 value; spans; the `error` flag); items arrive in order and
  once; a byte cut short yields no item; an unfinished transaction yields the completed ones; while the
  period is unknown the watermark does not advance and the upper decoder receives nothing, then receives
  everything in order (the watermark never passes an item not yet delivered); removing or changing the
  I2C instance stops or re-runs the upper one as feature 007 specifies and notifies the user (test
  through the host: `Guarded::RemoveDecoder`).
- [ ] T034 [P] [US5] In `crates/decoders/src/testing.rs` (feature `test-support`): a test-only decoder
  `test.i2c_reader` (`Input::Items("i2c")`) that stores each item it receives as an annotation (so the
  test reads them back) and registers with `testing::fixtures()` like the others.

### Implementation for User Story 5

- [ ] T035 [US5] Emit the items in `bus.rs` per the items contract: at each start, repeated start and stop,
  and at the acknowledge edge of an address and of a data byte (with direction, acknowledgement, 10-bit
  flag, value; `end` never decreases; the `error` flag where an acknowledge was required and missing; no
  item for a cut byte). T033 passes; the helper in T015 that rebuilt transactions from lane annotations
  is removed and the round trip compares items only.

**Checkpoint**: all five stories pass independently and together.

---

## Phase 8: Polish & Cross-Cutting Concerns

- [ ] T036 [P] Documents (English): `docs/decoders/i2c.md` (what the decoder reads; the synchronisation rule
  and the automatic bus period; start-up and truncated captures; every annotation and overlay; the
  options; the items contract copied from `contracts/i2c-items.md`; the generator, its options and the
  start-up it can produce); a section "Decoding I2C" in `docs/user-guide.md` (address notation, reading
  arrows and dots, why nothing is shown before the first start, NAK semantics); a one-line mention in
  `docs/decoder-api.md` that I2C is the reference decoder; check links.
- [ ] T037 [P] Localization review: every new key exists in six languages and the parity check passes
  (`cargo test -p la-i18n`); the drafts are marked as machine drafts the way the earlier features mark
  them; add the Japanese and long German lane texts (`ann-i2c-error-address-nak`, `ann-i2c-transaction-read`)
  to the existing lane layout test if one exists, otherwise to `docs/` as a manual check item.
- [ ] T038 Principle I review of `crates/decoders/src/decoders/i2c/` and `generators/i2c.rs`: no `unwrap`,
  `expect`, indexing, unchecked arithmetic or `unsafe`; every loop bounded by the data it reads or by the
  per-call work bound; histogram index clamped; run `cargo clippy --workspace --all-targets -- -D warnings`
  and `cargo +1.85 check --workspace`.
- [ ] T039 Mutation checks for the tests that passed on first run (T008 b/c/e, T022 gap, T028 option
  tests): break the guarded line, see the test fail, restore; record in `verification.md`.
- [ ] T040 Prove SC-008: `git diff main --stat -- crates/app crates/host/src crates/session` shows no
  change (tests of the host and the changed expectation in `app.rs` tests of T014 excepted, which is a test
  module); record the result in `verification.md`.
- [ ] T041 `specs/008-i2c-decoder-generator/verification.md`: traceability of every FR and SC to the tests
  that prove it (as 007's), the results of T039 and T040, the benchmark numbers against the baseline, and
  the list of manual checks left to the owner: look at the lane and overlays on Windows and Debian
  (quickstart section 4), SC-002 (under one minute from an empty application, by someone who has not read
  the guide), translations review, one real I2C capture when hardware exists (feature 011 onward).
- [ ] T042 Run the whole quickstart (`specs/008-i2c-decoder-generator/quickstart.md` sections 1–3 and 5),
  `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo deny check`,
  `cargo test --workspace` (and `LA_ROUND_TRIP_SEEDS=10000` once in release mode); update `roadmap.md`
  (008 `(merged)` when the pull request is merged, not before).

---

## Dependencies & Execution Order

### Phase dependencies

- Setup (T001–T002) → Foundational (T003–T005, blocks US1) → US1 → US2 → US3 → US4 → US5 → Polish.
- US2 needs US1 (it decodes what it generates). US3 needs US1 and US2 (planted errors need the generator).
  US4 needs US1 (and extends the generator from US2 for 10-bit). US5 needs US1 and the item builder of
  T018; it is independent of US3 and US4 and can be done before them if wanted.

### Within each story

Tests first and failing, then period/text/bus/descriptor (US1: T010 → T011 → T012 → T013 → T014), then
the story's registration or host test. Files shared by tasks (`bus.rs`, `text.rs`, `i2c.rs` tests,
`decoder_i2c.rs`) are edited by tasks of different stories one after another, never in parallel.

### Parallel opportunities

- T002 with T001's follow-up; T003 with T002.
- Within US1: T006, T007, T008, T009 (different files or independent sections) together.
- Within US2: T015, T016, T017 together; T021 with T020.
- Within US3: T022, T023, T024 together. Within US4: T028, T029, T030. Within US5: T033, T034.
- T036, T037 in Polish.

## Implementation Strategy

### MVP first

Setup → Foundational → **US1**: a decoder that reads hand-built or recorded I2C and shows it with dots and
arrows, synchronising on the bus. Stop and validate with the quickstart section 4 on a hand-built capture.

### Incremental delivery

US2 makes it testable without hardware and without hand-built signals (the round trip); US3 adds the
errors; US4 the options and 10-bit; US5 the output for feature 010. Each is mergeable on its own, but
one pull request for the feature is the plan (as 007), with a commit per task or logical group.

## Notes

- [P] tasks = different files, no dependencies.
- Expected values are by hand, from the bus builder and the contracts.
- Never commit with failing clippy, fmt or tests; commits are signed (ask the owner to unlock GPG if
  signing fails; never bypass it).
- The shell (`crates/app`, `crates/host/src`) is not edited by any task (SC-008, T040).
