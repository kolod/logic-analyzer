---

description: "Task list for I2C Lane as Blocks"
---

# Tasks: I2C Lane as Blocks

**Input**: Design documents from `/specs/016-i2c-lane-blocks/`

**Prerequisites**: plan.md, spec.md, research.md (R1–R7), data-model.md, contracts/
(`i2c-lane.md`, `decoder-api-1.2.md`), quickstart.md

**Tests**: REQUIRED (constitution Principle III). In every phase the test tasks come first and MUST fail
(usually they do not compile yet) before the implementation tasks start. If a test passes at once because
earlier work already covers it, run a mutation check (temporarily break the code it guards, see the test
fail, restore) before relying on it. Expected block positions are written **by hand** from research R2 and
the contract (`[from, to)` in samples for a known period), never computed by the code under test; only the
round-trip test derives them, from the generator's `expected()` items and `T = 16`.

**Organization**: grouped by user story as in spec.md: Foundational (the framework addition, blocking),
**US1** (read a transaction at a glance, P1), **US2** (see that a packet went wrong, P1), **US3** (blocks
line up with the signals, P2), Polish. The decoder's blocks and their drawing are in US1; errors in US2;
the places in awkward cases (overlaps, soon stops, unfinished packets) in US3.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: can run in parallel (different files, no dependency on an incomplete task)
- **[Story]**: user story from spec.md
- All paths are relative to the repository root. No new crate and no new dependency.

## Path Conventions

Decoder in `crates/decoders/src/decoders/i2c/` (`bus.rs`, `text.rs`), framework in
`crates/decoders/src/` (`model.rs`, `descriptor.rs`, `lanes.rs`, `persist/dann.rs`), host in
`crates/host/src/decoders.rs`, drawing in `crates/app/src/decoder_view.rs`, tests in
`crates/decoders/tests/`, documents in `docs/`. "The lane contract" is
`specs/016-i2c-lane-blocks/contracts/i2c-lane.md`, "R2" is research R2 of this feature.

---

## Phase 1: Setup

- [ ] T001 Record the baseline in `specs/016-i2c-lane-blocks/verification.md`: `cargo test -p la-decoders`
  passes before any change (the one failure of `la-host`, `decoder_growth`, is a timing check that fails on
  some machines without this feature; note it, do not fix it here). Branch `016-i2c-lane-blocks` is off `main`
  after the merge of PR #32 (rebase if it is not merged yet, the generator signal of 008 is what the
  round-trip test here uses).

---

## Phase 2: Foundational (blocking: decoder API 1.2)

**Purpose**: the two shapes, the row count of a lane and the band routing. Nothing of the I2C lane can be
written or drawn before this.

### Tests first

- [ ] T002 [P] In `crates/decoders/tests/model.rs`: `Shape::Part` and `Shape::Whole` are valid on a
  `Kind::Range` and refused (`Refusal::Style`) on a point and on an edge; the other shapes are as before.
- [ ] T003 [P] In `crates/decoders/tests/lanes.rs`: with `rows = 2` a `Part` is in row 0 and a `Whole` in row
  1 whatever the order of insertion, also when the `Whole` starts first and covers the parts; two `Part`s that
  overlap make the later a dense cell and not a second row; with `rows = 3` and no new shapes the plan is
  exactly as before (the present tests keep passing); a window of a growing and one of the finished store
  give the same rows; dense traffic of 100,000 blocks keeps the cell bound of the plan.
- [ ] T004 [P] In `crates/decoders/tests/dann_section.rs`: annotations with `Part` and `Whole` shapes and the
  error flag survive save and load unchanged (the style byte keeps its layout, codes 5 and 6); a style byte
  with shape code 7 is still refused.
- [ ] T005 [P] In `crates/decoders/tests/descriptors.rs`: `lane_rows` outside 1 to 3 is refused when a
  registry is built, 1, 2 and 3 are accepted; `decoder_api_doc.rs` compares `docs/decoder-api.md` with the
  new names (`Part`, `Whole`, `lane_rows`, version `1.2`) and fails until the document says them.

### Implementation

- [ ] T006 Add `Shape::Part` and `Shape::Whole` to `crates/decoders/src/model.rs`; `Annotation::validate`
  refuses them on a point or an edge (T002).
- [ ] T007 Add `lane_rows: u8` to `Descriptor` in `crates/decoders/src/descriptor.rs` with validation (T005);
  set `lane_rows: LANE_ROWS` in every existing descriptor (about 40 sites in `src` and `tests`, mechanical);
  bump `DECODER_API_VERSION` to `1.2`.
- [ ] T008 In `crates/decoders/src/lanes.rs`: route `Part` to row 0 and `Whole` to row `min(1, rows - 1)`
  without packing; a cell that finds its place taken becomes a dense cell with its error count; keep the
  packing of every other shape and the bound of the plan (T003).
- [ ] T009 In `crates/decoders/src/persist/dann.rs`: shape codes 5 and 6 (T004); version stays 1.
- [ ] T010 In `crates/host/src/decoders.rs`: build the lane plan with the descriptor's `lane_rows` instead of
  `LANE_ROWS` (`crates/host/src/decoders.rs:1201`); a host test in `crates/host/tests/decoder_plan.rs`
  checks that a decoder with `lane_rows = 2` gets a plan of two rows.
- [ ] T011 `docs/decoder-api.md`: the shapes in "Sink", `lane_rows` in "Descriptor", the rows in "Results,
  lanes and overlays", "Versioning" 1.2; run T005's document test.

**Checkpoint**: the framework can place and save band blocks; nothing draws or produces them yet. Existing
tests pass.

---

## Phase 3: User Story 1 - Read a transaction at a glance (Priority: P1) 🎯 MVP

**Goal**: a good transaction is two bands of blocks in the right colours, drawn as solid rectangles whose
text is whole or absent.

**Independent Test**: decode a hand-built good write and a read of one byte and check the blocks (band,
`[from, to)`, colour, text) and the picture helpers; then look at it with the virtual device (quickstart 2).

### Tests first

- [ ] T012 [P] [US1] `crates/decoders/tests/i2c_blocks.rs` (new): hand-built signal of a good write of one
  byte with `T = 16` (the builder of `common/i2c`): the upper band holds, in order and by hand-written
  `[from, to)` from R2, start (blue, `ann-i2c-start`), address (yellow, `ann-i2c-address-write`), ack (green),
  data (yellow, `ann-i2c-data`), ack, stop (blue); the lower band holds one `Whole` from the start's `from` to
  the stop's `to`, green, `ann-i2c-transaction-write`; consecutive upper blocks touch; no lane annotation has
  any other shape; the arrows and dots are as in 008.
- [ ] T013 [P] [US1] Same file: a read of two bytes with the controller's NAK on the last: address and data
  green, the last nak block red **without** the error flag, the packet green; the first byte's ack is green.
- [ ] T014 [P] [US1] Same file: a repeated start between a write part and a read part: a blue `Part` for it
  (`ann-i2c-repeated-start`) at `[s - 8, s + 8)`, the two parts' colours follow their own direction, one
  packet block across both; a 10-bit address: two address blocks with an ack block each.
- [ ] T015 [P] [US1] In `crates/app/src/decoder_view.rs` (tests module): `block_label` returns the whole text or
  `None`, never an ellipsis, for widths below, at and above the width of the text; `contrast_text` picks
  black or white by the luminance of the fill and gives at least the contrast ratio 4.5 for each of the five
  roles (blue 4, yellow 1, green 2, red 0, grey 7) on each of the three variants; the five fills are pairwise
  distinguishable on every variant (hue or luminance difference above a stated threshold).
- [ ] T016 [P] [US1] Rewrite the lane assertions (ranges and points) of `crates/decoders/tests/i2c.rs`,
  `i2c_fixtures.rs`, `i2c_options.rs` and `i2c_sync.rs` to blocks; what they check about the items, the
  arrows, the dots, the options and the synchronisation does not change.

### Implementation

- [ ] T017 [US1] `crates/decoders/src/decoders/i2c/text.rs`: styles for the blocks (`Part` / `Whole`, colour
  by role as in the contract, constants for the five palette indices); the lane functions return block
  styles; `lane_rows: 2` in the descriptor (`crates/decoders/src/decoders/i2c/mod.rs`).
- [ ] T018 [US1] `crates/decoders/src/decoders/i2c/bus.rs`: the geometry of R2 (`before`, `after` from the
  period); write the start/repeated start block at the edge, the address and data blocks and the ack/nak block
  when a byte completes, the stop block and the packet block at the stop; keep the state of the data-model
  (end of the last block, start of the packet, cell of the current byte); no point annotations in the lane any
  more; saturating arithmetic only.
- [ ] T019 [US1] `crates/app/src/decoder_view.rs`: draw `Part` and `Whole` as solid blocks (R4) with
  `block_label` and `contrast_text`; the fill is not replaced by the danger colour for these shapes; heavier
  outline for an error, the selection outline as now; the two rows share the lane height.
- [ ] T020 [US1] Run T012–T016; run the whole I2C suite of 008 (`round trip`, chunking, stacking, options): the
  items and the overlays compare as before.

**Checkpoint**: US1 works: two bands, colours, texts, solid blocks. MVP.

---

## Phase 4: User Story 2 - See at once that a packet went wrong (Priority: P1)

**Goal**: errors show in the colours of the packet and nak blocks, and their texts stay reachable.

**Independent Test**: decode traffic with planted errors; exactly the packets with an error are red.

### Tests first

- [ ] T021 [P] [US2] `crates/decoders/tests/i2c_blocks.rs`: an address not acknowledged: nak block red with
  the error flag, packet red; a written byte not acknowledged: the same; both followed by a stop block; the
  arrow of that acknowledge bit carries the text `ann-i2c-error-address-nak` / `-data-nak` with the error flag
  and is still an edge at the same position (the one change to the overlays).
- [ ] T022 [P] [US2] Same file: a byte cut short by a stop, by a start and by the end of the capture: a `Part`
  from the first bit's cell start to the cell end of the last bit read, in the colour its data block would
  have, error flag, text `ann-i2c-error-byte-cut`, no ack block; packet red (cut by the end: grey and no
  error); an empty transaction (a start and a stop, no clock pulse): start, stop and a red `Whole` with
  `ann-i2c-error-empty`; lost samples: a red `Part` with `ann-i2c-error-gap`, the packet in progress red.
- [ ] T023 [P] [US2] `crates/decoders/tests/i2c_round_trip.rs`: for the seeds of the generator, with and
  without planted errors, the blocks derived from `expected()` and `T = 16` equal the lane (band, `from`, `to`,
  colour index, text key); the packets that are red are exactly those that have a planted error and every
  other packet is green, whatever the direction and the NAK of the end of a read (SC-001).
- [ ] T024 [P] [US2] Rewrite the lane assertions of `crates/decoders/tests/i2c_errors.rs` to blocks (the
  error positions and counts they check do not change).

### Implementation

- [ ] T025 [US2] `bus.rs` / `text.rs`: nak block red with the error flag only for an error; the explanation text
  and the error flag on the SCL arrow of that bit; the cut byte, empty transaction and lost-samples blocks per
  the data model; the packet block red when any part was an error, grey when unfinished; the packet colour
  comes from the state of the data model (an error seen).
- [ ] T026 [US2] Run T021–T024 and the random-signal test of 008 (no panic, no refusal by the sink on any
  input); mutation check of T023: make the packet green for an error and see the test fail.

**Checkpoint**: US1 and US2 work independently of US3.

---

## Phase 5: User Story 3 - Blocks that line up with the signals (Priority: P2)

**Goal**: blocks start and end at the places of R2 in the awkward cases too, never overlapping.

**Independent Test**: hand-built signals with a known period and the expected `[from, to)` by hand.

### Tests first

- [ ] T027 [P] [US3] `crates/decoders/tests/i2c_blocks.rs`: a stop less than half a period after the last
  ack's rising edge (the stop block starts at the end of the ack block or is cut so that nothing overlaps);
  a stop and the next start less than a period apart (the earlier block is cut at the start of the later);
  a start whose clock falls much later than half a period (the start block is still the period around the SDA
  edge, a gap to the first bit is allowed); a block that would be empty or inverted is not written.
- [ ] T028 [P] [US3] Same file: no two blocks of the upper band overlap and every `Part` lies inside the packet
  block, over 1,000 generated frames and over the random signals of 008; an unfinished packet (the capture
  ends mid-byte, after a complete byte, after a start) is drawn up to the end of its last block, grey, no error;
  odd periods (`T` = 15, 17): `before` is `T / 2` rounded down and the blocks still touch.
- [ ] T029 [P] [US3] `crates/decoders/tests/i2c_chunking.rs`: the blocks are the same for every way of
  splitting the capture into chunks and for a growing capture (the result of the whole capture equals that of
  the pieces), including a split inside a byte and between the ack and the stop.

### Implementation

- [ ] T030 [US3] `bus.rs`: the shortening rule (the earlier block is cut at the start of the later), the
  stop block's `from = max(last block end, p - T)`, omission of empty blocks, the unfinished packet at `flush`
  and at a gap; keep the work bounded per call.
- [ ] T031 [US3] Run T027–T029; run `LA_ROUND_TRIP_SEEDS=10000` once in release mode.

**Checkpoint**: all three stories work.

---

## Phase 6: Polish & Cross-Cutting Concerns

- [ ] T032 [P] `docs/decoders/i2c.md`: the table "What it produces" and the colours replaced by the lane
  contract; the one change of the arrow of an error bit; the colours table; keep the document tests green
  (`crates/decoders/tests/i2c_doc.rs` if present).
- [ ] T033 [P] `docs/user-guide.md`: how the I2C row reads now (two bands, colours, texts that hide), if the
  guide describes the lane; `roadmap.md`: item of this feature.
- [ ] T034 [P] Localization review: no key was added or removed (`ann-i2c-*` are as before); the parity,
  budget and font checks pass; the long German and Japanese texts still hide whole in narrow blocks.
- [ ] T035 Principle I review of `crates/decoders/src/decoders/i2c/` and `lanes.rs`: no `unwrap`, `expect`,
  indexing on external data or unchecked arithmetic outside the allowed generator modules.
- [ ] T036 Benchmarks: `cargo bench -p la-decoders` for the lane plan and the I2C decode: within the recorded
  baseline (the number of lane annotations per byte is about the same).
- [ ] T037 `specs/016-i2c-lane-blocks/verification.md`: traceability of every FR and SC to the tests, the
  mutation checks of T026 and T020, the baseline of T001, and the manual checks left to the owner: look at
  the lane on Windows and Debian (quickstart 2) in the dark, light and high-contrast themes, the colour of an
  unfinished packet (grey), the text of an error on the SCL arrow.
- [ ] T038 Run `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo deny
  check`, `cargo test --workspace --no-fail-fast`, the quickstart; fix what fails.

---

## Dependencies & Execution Order

- Phase 1 first. Phase 2 blocks everything: T002–T005 together, then T006 → T007 → T008, T009, T010, T011.
- US1 needs Phase 2. US2 needs US1 (same functions of `bus.rs`). US3 needs US1 and US2 for the same reason;
  its tests (T027–T029) can be written while US2 is implemented.
- Within a phase the tests come first and fail; `bus.rs` and `text.rs` are edited by tasks of different
  stories one after another, never in parallel; so are `lanes.rs` and `decoder_view.rs`.

### Parallel opportunities

- T002–T005 together; T012–T016 together; T021–T024 together; T027–T029 together; T032–T034 together.

## Implementation Strategy

### MVP first

Setup → Foundational → **US1**: a good transaction as two bands of solid blocks. Stop and look at it with
the virtual device (quickstart 2, steps 1–4 and 6).

### Incremental delivery

US2 adds the error colours and texts, US3 the awkward places. One pull request for the feature (as 008),
with a commit per task or logical group.

## Notes

- [P] tasks = different files, no dependencies.
- Expected values are by hand, from R2 and the lane contract.
- Never commit with failing clippy, fmt or tests; commits are signed (ask the owner to unlock GPG if signing
  fails; never bypass it); commit subjects are Conventional Commits with a scope.
- The generator, the items for stacking and the arrows' places are not edited by any task (FR-013); the one
  change to an arrow is its text on an error bit (T025).
