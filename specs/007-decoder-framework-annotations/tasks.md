---

description: "Task list for Decoder Framework and Annotations"
---

# Tasks: Decoder Framework and Annotations

**Input**: Design documents from `/specs/007-decoder-framework-annotations/`

**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/, quickstart.md

**Tests**: REQUIRED. Constitution Principle III (test-first): in every phase the test tasks come
first and MUST fail (usually they do not compile yet) before the implementation tasks start. If a
test passes immediately because earlier work already covers it, run a mutation check (temporarily
break the code it guards, see the test fail, restore) before relying on it. Expected values (byte
vectors of `DECO` and `DANN`, annotation sets of the sample decoder for a hand-drawn signal, error
codes, text in six languages) are written by hand from the contracts, never computed by the code
under test. Window queries and lane plans are compared with a brute-force reference.

**Organization**: tasks are grouped by user story (US1–US7, numbered as in spec.md). They are
executed in this order: Setup, Foundational, **US1** (decode and see, P1), **US6** (the generator, P2,
done second so every later phase can be tried without hand-built signals), **US2** (faults, P1),
**US3** (large, background and growing, P1), **US4** (stacking, P2), **US5** (saving, P2), **US7**
(reading comfortably and in six languages, P3), Polish. The crate, the model, the index, the lane plan
and the chunk loop are Foundational because every story needs them.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: can run in parallel (different files, no dependency on an incomplete task)
- **[Story]**: user story from spec.md
- All paths are relative to the repository root. New crate: `crates/decoders` (package
  `la-decoders`); it is picked up by the workspace glob `crates/*`.

## Path Conventions

Decoder framework in `crates/decoders/src/`, its tests in `crates/decoders/tests/` and benchmarks in
`crates/decoders/benches/`; host in `crates/host/src/`, `crates/host/tests/`, `crates/host/benches/`;
application in `crates/app/src/`; session library in `crates/session/`; resources in
`resources/i18n/<lang>/ui.ftl` and `errors.ftl`; documents in `docs/`. The contracts are
`specs/007-decoder-framework-annotations/contracts/decoder-api.md` ("the decoder contract"),
`deco-section.md` ("the section contract"), `host-api-1.1.md` ("the API additions") and `decoder-ui.md`
("the UI contract"); entities, limits and states are in `data-model.md`; decisions in `research.md`
(R1–R17).

## Lint reminder (applies to every Rust task)

The workspace denies `unwrap`, `expect`, indexing (`v[i]`), unchecked arithmetic, `panic!` and
`unsafe` in production code, in `la-decoders`, `la-host` **and** `la-app`. Use `get`, `checked_*`,
`saturating_*`, `u128` intermediates and `?`. Test code may relax this with the file-level
`#![allow(clippy::unwrap_used, ...)]` used by the other crates' tests; the test decoders in
`crates/decoders/src/testing.rs` panic on purpose and are compiled only with `test-support`. Keep the
code at **MSRV 1.85**: no `let` chains (CI failed on them once; check with `cargo +1.85 check`).
Never commit while clippy fails: a helper that only a later task uses gets
`#[expect(dead_code, reason = "used by T0xx")]`, removed by that task. Never log signal data,
annotation text or instance names; log command names, decoder ids, codes, counts and positions. Every
UI string comes from `ui.ftl` or `errors.ftl` (English first, then the five translations as marked
drafts in the same phase) so the parity check stays green at every commit; message keys are string
**literals** in the code (the parity check finds them there). Windows scripts that patch files: write
them to a file, not a shell heredoc (apostrophes break it); run `cargo fmt` before the next edit.

---

## Phase 1: Setup

**Purpose**: the new crate, its place in the workspace and CI

- [X] T001 Create `crates/decoders/Cargo.toml` (package `la-decoders`, workspace edition, MSRV 1.85, license, `[lints] workspace = true`; dependencies `la-core`, `la-storage`, `la-devices`, `la-diagnostics`, `la-i18n`, `tracing`; `[features] test-support = []`; dev-dependency on itself with `test-support`) and `crates/decoders/src/lib.rs` (`#![deny(missing_docs)]`, `pub const DECODER_API_VERSION: &str = "1.0";`, empty module declarations for `error`, `text`, `model`, `descriptor`, `setup`, `registry`, `input`, `store`, `lanes`, `run`, `stack`, `persist`, `decoders`, `generators`, and `testing` behind `test-support`); `cargo build -p la-decoders` and `cargo clippy -p la-decoders -- -D warnings` pass
- [X] T002 [P] Add `la-decoders = { path = "../decoders" }` to `crates/host/Cargo.toml` and to its `[dev-dependencies]` with `features = ["test-support"]`; make the host's own `test-support` feature forward to `la-decoders/test-support`; `cargo build -p la-host` passes
- [X] T003 [P] Read `.github/workflows/ci.yml` and `deny.toml`: confirm the test, clippy and msrv jobs run `--workspace` (so `la-decoders` is covered on both systems and on 1.85) and that no new dependency needs a licence entry; change a job only if it lists crates explicitly
- [X] T004 [P] Create `crates/decoders/tests/common/mod.rs`: helpers shared by the tests (a `capture_from_edges(channels, sample_rate, &[(position, channel, high)], length)` builder over `la-storage` that can also add gaps and be appended to while a reader exists, a small deterministic `Lcg`, a brute-force `reference_window(&[Annotation], start, end)` and a random annotation generator with overlaps, nesting, equal positions and zero-length ranges); file-level `#![allow(...)]` as in other crates' tests

**Checkpoint**: the crate builds, lints and is covered by CI

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: error codes, the annotation model, descriptors and setups, the signal reader, the
indexed store, the lane plan and the chunk loop: what every story stands on

### Tests first (all fail before implementation)

- [X] T005 [P] Write `crates/decoders/tests/model.rs`: validation of annotations and items by tables written from `data-model.md` (accept and refuse: position beyond the capture length, end before start, unknown target channel, style outside the fixed sets, text over 256 bytes, more than 4 or too long message arguments, zero-length range accepted, control and bidirectional characters sanitized, item end going backwards refused, payload over 64 bytes), and that a refused record is counted, never stored
- [ ] T006 [P] Write `crates/decoders/tests/descriptors.rs`: descriptor validation (identifier form and uniqueness, duplicate roles or options, defaults inside ranges, counts within 16) and setup validation (unknown decoder, unknown or out-of-range channel, missing required role, shared channel where not allowed, option of the wrong type or outside its range, source that does not exist or does not match the input kind, cycle, depth over 4, instance limit 8) with the expected `decoder.*` code and the setting named for each
- [ ] T007 [P] Write `crates/decoders/tests/index.rs`: the store against the brute-force reference on random sets (overlap, nesting, equal positions, zero-length) for window, rank and neighbour queries after every kind of insertion order; invariants (at most 16 runs, tail at most 4,096, compaction gives one run); a snapshot taken before further inserts never changes; accounting of count and bytes and the refusal at each limit; a probe-count test (the store exposes a probe counter under `test-support`) showing the cost of a window query and of the per-column counts independent of the number of annotations outside the window
- [ ] T008 [P] Write `crates/decoders/tests/lanes.rs`: lane and overlay plans against a reference: at most 4 rows, no overlap in a row, `Dense` cells with the right count and error count, at most `4W` cells per lane and `2W` per channel, every annotation of the window represented exactly once (spans, marks and dense counts add up), an error never hidden inside a dense cell without `errors > 0`, overlays of a hidden channel dropped, and a zoom sweep from the whole capture to many pixels per sample over a store of 1,000,000 annotations with one planted one-sample annotation that is represented at every level (SC-003)
- [ ] T009 [P] Write `crates/decoders/tests/input.rs`: `SignalReader` against brute force over a storage snapshot: merged edges in position order for the assigned roles, `level` including `Missing` inside a gap, beyond the snapshot length and for an unassigned role, an edge exactly on a chunk boundary belonging to exactly one chunk, the gaps listed per chunk, and `ItemChunk` with its watermark
- [ ] T010 [P] Write `crates/decoders/tests/chunking.rs`: the chunk loop with a recording test decoder: chunks are contiguous from 0, bounded in edges and span, the same output for every chunk bound, validation applied to what the decoder emits, progress and covered position reported, `finish` called once at the real end (it fails until T017)
- [ ] T011 [P] Write `crates/decoders/tests/decoder_api_doc.rs`: the version line of `docs/decoder-api.md` equals `DECODER_API_VERSION`, and the document has the sections the contract names (it fails until T018)

### Implementation

- [X] T012 Implement `crates/decoders/src/error.rs`: the 15 codes of research R7 as `pub const NAME: &str = "decoder.x";` (so the existing error sweep finds them), constructors with kinds, `ALL_CODES`, unit tests for uniqueness and form; add the 15 messages to `resources/i18n/<lang>/errors.ftl` in all six languages (each says what failed and what the user can do; the message of a stack or setup refusal names the setting through an argument), drafts marked as in feature 005; run `cargo test -p la-i18n` (parity) and the host `error_sweep`
- [X] T013 Implement `crates/decoders/src/text.rs` (display sanitizing with the rules of `la-host`'s `sanitize_text`, the limits of `data-model.md`) and `crates/decoders/src/model.rs` (`Annotation`, `Kind`, `Target`, `Style`, `Text`, `Item`, `Payload`, validation against a capture length and channel count, refusal counters); add to `crates/host/tests/sanitize.rs` a cross-check that both sanitizers agree on a hostile corpus; T005 passes
- [ ] T014 Implement `crates/decoders/src/descriptor.rs`, `setup.rs` and `registry.rs` (descriptor and its validation, setup and its validation, the registry with `register`, `get`, `list`, `builtin()` empty for now, a way for tests to register extra decoders under `test-support`); T006 passes
- [ ] T015 Implement `crates/decoders/src/input.rs`: `Chunk`, `SignalReader` (k-way merge of the storage's `transitions` per assigned role, `level` via `level_at`, gaps), `ItemChunk`; no `unwrap`, no indexing; T009 passes
- [ ] T016 Implement `crates/decoders/src/store/` (`mod.rs`, `runs.rs`, `tree.rs`, `tail.rs`): immutable sorted runs with an implicit maximum-end tree, the unsorted tail, geometric merging, window, rank and neighbour queries, accounting and limits, `Arc` snapshots, `compact()`, the probe counter; T007 passes
- [ ] T017 Implement `crates/decoders/src/lanes.rs` (window query to lane and overlay cells with rows, dense cells and error counts, the cell bounds) and `crates/decoders/src/run/job.rs` (the synchronous chunk loop `run_pass`: a new snapshot per round, chunk cutting, the `Sink` that validates and counts, publishing to the store, progress and covered position, `finish`); T008 and T010 pass
- [ ] T018 Write `docs/decoder-api.md` from `contracts/decoder-api.md` (the final, authoritative text, `Version: 1.0` as its second paragraph line, any change made during implementation included) and point the design contract at it (a sentence at its top); T011 passes
- [ ] T019 Run `cargo fmt`, `cargo clippy --workspace --all-targets --all-features -- -D warnings`, `cargo test -p la-decoders -p la-i18n -p la-host` and `cargo +1.85 check -p la-decoders --all-targets`; commit the phase (split into logical commits: errors and messages, model and text, descriptors, reader, store, lanes and chunk loop, document)

**Checkpoint**: the framework core exists with a tested model, index, lane plan and chunk loop

---

## Phase 3: User Story 1 - Decode a signal and see what it says (Priority: P1) 🎯 MVP

**Goal**: add the sample decoder to a capture, give it channels, and see bytes in a decoder lane and
arrows and dots on the signals

**Independent Test**: with hand-built signals (and, after US6, the generator), add the sample decoder,
check that the decoded bytes, bit points and clock-edge arrows are exactly right, zoom in and out, click
an annotation, remove the decoder (spec US1, SC-001, SC-012)

### Tests first

- [ ] T020 [P] [US1] Write `crates/decoders/tests/clocked_bits.rs`: the sample decoder on hand-drawn captures: the exact annotation set for a known byte (one lane range per byte with the message key and the value as argument, one lane point per bit, one edge arrow on the clock channel at every sampled edge, one dot on the data channel at the first sampled edge), both bit orders, both clock edges, bits 4 to 16, the idle option; malformed and truncated input (a byte cut by the end of the capture, a byte cut by a gap, a one-sample glitch on the clock, no edges at all, data stuck high or low, an idle limit of 1); the same output for every chunk size from 1 edge up; `finish` flushing an incomplete byte as an error annotation
- [ ] T021 [P] [US1] Write `crates/host/tests/decoder_commands.rs`: `AddDecoder` (needs a capture on screen), `ConfigureDecoder`, `SetDecoderChannel`, `SetDecoderOption`, `RenameDecoder`, `ShowDecoderLane`, `RemoveDecoder`: each valid change, each refusal (the code and the setting named, `State.decoders.refusal` filled, nothing changed), the instance limit of 8, `Unsaved` becoming `Dirty` on a setup change, `SelectAnnotation`, `ClearAnnotationSelection`, and that a finished pass fills `decoders.instances` with state `Finished`, the counts and no fault
- [ ] T022 [P] [US1] Write `crates/host/tests/decoder_plan.rs`: `frame_plan` lanes and overlays for a finished decode equal a reference built from the stored annotations; the selection details (kind, formatted text, start, end, length in time in the current language); `GoToAnnotation` `Selected`, `Next` and `Previous` move the window so the annotation is shown whole and centred, and wrap or stop at the ends as documented; a hidden lane or channel removes its cells; the cell bounds hold
- [ ] T023 [P] [US1] Add to the tests of `crates/app/src/app.rs`: `a_registered_test_decoder_appears_without_changing_the_shell` (a test decoder registered under `test-support` shows in the add dialog, its configuration form lists its signals and options from the descriptor, and its lane is drawn; SC-012), and headless renders (written to `LA_SNAPSHOT_DIR`) of a lane with ranges and points plus overlays on the clock and data lanes for the sample decoder at two zoom levels

### Implementation

- [ ] T024 [US1] Resources first: `resources/i18n/<lang>/ui.ftl` gets, in English then the five translations as drafts, the keys of the sample decoder per the decoder contract (`decoder-sample-clocked-bits-name`, `-description`, `-signal-clock`, `-signal-data`, `-option-clock_edge`, `-option-bit_order`, `-option-bits`, `-option-idle`, the choice labels, and the standard annotation texts `ann-sample-clocked-bits-byte` (argument `value`), `-bit`, `-incomplete`, `-gap`), the menu **Decoders** and its entries (add, configure, run, cancel, remove, show lane, next annotation, previous annotation, go to selected annotation), the Decoders panel, the add dialog, the configuration form, the instance state names and the messages for the selected annotation; run `cargo test -p la-i18n` (the keys are unused until T025 and T029, so add them with the code that uses them or mark them `@reserved` and remove the mark in T029)
- [ ] T025 [US1] Implement `crates/decoders/src/decoders/clocked_bits.rs` (descriptor, the `Run` state machine, annotations and the `"byte"` item as in research R13) and register it in `registry::builtin()`; T020 passes
- [ ] T026 [US1] Implement `crates/decoders/src/run/worker.rs`: a pass on a supervised worker (`la_diagnostics::spawn_worker`) running `run_pass`, publishing store snapshots through an `Arc` swap, an atomic progress and covered position, a cancel flag, and a `JobHandle` the host polls (fault mapping, budgets and the watchdog come in US2 and US3)
- [ ] T027 [US1] Implement `crates/host/src/decoders.rs` and extend `command.rs`, `state.rs`, `shell.rs`: `DecoderState` (catalogue built in the current language from the descriptors, instances, selection, refusal, limits), the commands of the API additions for this story (`AddDecoder`, `ConfigureDecoder`, `SetDecoderChannel`, `SetDecoderOption`, `RenameDecoder`, `ShowDecoderLane`, `RemoveDecoder`, `SelectAnnotation`, `GoToAnnotation`, `ClearAnnotationSelection`) with names for the log, `Versions.decoders` and `Changes::DECODERS`, polling of the jobs, marking `Dirty` on setup changes, clearing instances when the capture is cleared or replaced; T021 passes
- [ ] T028 [US1] Extend `crates/host/src/plan.rs` and `shell.rs` (`frame_plan`) with `lanes` and `overlays` built through `la_decoders::lanes`; extend `crates/host/src/shortcuts.rs` with the **Decoders** menu rows and accelerators (unique, none reserved by the desktops) and add the new command names to the lists of `crates/host/tests/shortcut_coverage.rs`; T022 and the coverage test pass
- [ ] T029 [US1] Extend `crates/app/src/signal_view.rs` and `theme.rs`: decoder lanes below the channel lanes in the same canvas (label column with the name, rows, spans with text, marks, dense marks), overlays on the channel lanes (dots, edge arrows, dense marks), 8 annotation colours per theme with distinct shapes, hover readout, click to select, double-click to go to; the waveform cache is not invalidated by annotation changes
- [ ] T030 [US1] Create `crates/app/src/decoders.rs` and wire it in `app.rs` and `main.rs`: the Decoders panel (instances, state, counts, buttons, the selected annotation), the add dialog and the configuration form **generated from the catalogue** (a channel chooser per signal with the names from the Channels panel, a control per option by type, an instance name, refusal messages under the field), and the menu entries; T023 passes
- [ ] T031 [US1] Add a **Decoders** section (adding, the lane, the overlays, selecting) to `docs/user-guide.md`
- [ ] T032 [US1] Manual check from the quickstart (steps 8.1 to 8.3: capture, add the decoder, look at bits and arrows, next annotation, click); record platform, date and result in `specs/007-decoder-framework-annotations/verification.md`; run fmt, clippy, `cargo test --workspace`; commit the phase

**Checkpoint**: a user can decode a capture and see the result; the MVP

---

## Phase 4: User Story 6 - Try decoding without hardware, with a generator (Priority: P2)

**Goal**: the virtual device generates signals that are valid for the configured decoder, seeded and
reproducible, with deliberate errors on request

**Independent Test**: for many seeds, generate, encode, decode and compare; the same seed gives the same
signals; planted errors are reported where they were planted (spec US6, SC-006)

### Tests first

- [ ] T033 [P] [US6] Write `crates/decoders/tests/round_trip.rs`: for seeds 0 to `LA_ROUND_TRIP_SEEDS` (default 1,000; an `#[ignore]` variant with 10,000 for release) generate content, run the generator's `SignalSource` into a capture through chunk sizes 1, 7 and 4,096 (the result must be identical for each), decode with the sample decoder and compare with `expected()`; with deliberate errors every planted incomplete byte is reported at its position and the valid bytes around it are decoded; all option combinations of bit order, edge and bits; the same seed twice gives identical signals
- [ ] T034 [P] [US6] Write `crates/decoders/tests/composite.rs`: the composite factory fills only the channels claimed by generators, fills all others with the random source of the device (same seed gives the same signals), keeps disabled channels at zero, and reproduces for any chunking
- [ ] T035 [P] [US6] Write `crates/host/tests/decoder_generator.rs`: a capture on the virtual device with an instance whose decoder has a generator decodes to exactly `expected()` for the instance's channels; with no instance (or none with a generator) the capture is byte-identical to the one made before this feature for the same seed; the capture stays labelled simulated; a stop during generation takes effect at once; the deliberate-errors option of the setup is honoured

### Implementation

- [ ] T036 [US6] Implement `crates/decoders/src/generators/clocked_bits.rs` (the generator as a deterministic state machine over the position, `expected()`, options for deliberate errors and their rate, hooked to the descriptor's `generator` capability); T033 passes
- [ ] T037 [US6] Implement `crates/decoders/src/generators/composite.rs` (a `SignalSourceFactory` that combines generator sources with the random source for unclaimed channels); T034 passes
- [ ] T038 [US6] Extend `crates/host/src/devices.rs` (`ConfigDraft::to_device_config` takes an optional factory and sets `VirtualOptions::source`) and `shell.rs`/`decoders.rs` (when a capture starts on a simulated device, ask the instances with a generator for their sources and pass the composite); T035 passes
- [ ] T039 [US6] Resources and form: the generator's option labels and the "simulation" section texts in six languages (`ui.ftl`), and the configuration form in `crates/app/src/decoders.rs` shows the generator options when the selected device is simulated and the decoder has a generator; update the user guide
- [ ] T040 [US6] Manual check from the quickstart (step 8.4: capture with deliberate errors and find the error marks); record; run fmt, clippy, tests; commit the phase

**Checkpoint**: everything can be tried without hardware, reproducibly

---

## Phase 5: User Story 2 - A broken decoder never harms the session (Priority: P1)

**Goal**: every kind of decoder fault is contained, reported once with a stable code, partial results
kept, the capture and other decoders untouched

**Independent Test**: test decoders that fail in every way next to a healthy one on the same capture
(spec US2, SC-005)

### Tests first

- [ ] T041 [P] [US2] Create `crates/decoders/src/testing.rs` (behind `test-support`): test decoders registered under their own ids: `healthy` (counts edges, emits a range per chunk), `returns_error` (after N chunks), `panics` (after N chunks), `never_returns` (loops forever on chunk k and ignores cancellation), `grows_memory` (emits large literal text until stopped), `too_many_annotations`, `invalid_output` (end before start, beyond the capture, bad channel, over-long text, item going backwards); each is documented as a fixture, never a production decoder
- [ ] T042 [P] [US2] Write `crates/decoders/tests/fault_injection.rs`: each fault next to `healthy` on one capture: the capture's transitions, gaps and status are identical before and after, the healthy results are unchanged and complete, the faulty instance keeps what it produced before the fault flagged incomplete, exactly one fault with the right code per run, the instance is restartable, refused records are counted and reported once per pass, the late output of an abandoned worker is ignored, and after 4 abandoned workers a new pass is refused with `decoder.too_many_stuck`
- [ ] T043 [P] [US2] Write `crates/host/tests/decoder_faults.rs`: the notice for each code reads right in all six languages through `notice_text` (no code, no key, no technical detail shown), the log has the code and the decoder id and neither annotation text nor an instance name (as `logging.rs` does for channel names), repeats are grouped, `Unsaved` and the capture are unchanged, saving after a fault writes the same capture bytes as without a decoder, and `apply` and `poll` each return in under 100 ms while a never-returning decoder is stuck
- [ ] T044 [P] [US2] Write `crates/host/tests/decoder_settings.rs`: `SetDecoderMemoryLimit` is validated (16 MiB to 4 GiB), is remembered, applies to passes that start afterwards and never to a running one

### Implementation

- [ ] T045 [US2] Implement `crates/decoders/src/run/budget.rs`: the annotation count and memory budgets enforced by the `Sink` (`Err(Limit)` past a budget), the text and argument limits, the counters; the store reports its bytes to the shared results budget
- [ ] T046 [US2] Extend `crates/decoders/src/run/worker.rs` and create `run/supervisor.rs`: map every outcome to a code (error to `decoder.failed`, panic to `decoder.panicked` through the `WorkerFailure`, budgets to the limit codes, refusals to `decoder.invalid_output`), keep the partial results flagged incomplete, generation numbers so an abandoned worker's output is ignored, the 5 s chunk deadline and 1 s cancel deadline, the abandoned-worker count and `decoder.too_many_stuck`; T042 passes
- [ ] T047 [US2] Extend `crates/host/src/decoders.rs`, `command.rs`, `settings.rs`, `shell.rs`: the supervisor in `poll`, a fault becomes a grouped notice and the instance's `fault`, `RunDecoder` and `CancelDecoder`, the setting `decoder_memory_limit_bytes` with its validation and `SetDecoderMemoryLimit`, add the new command names to `shortcut_coverage` lists and shortcut rows (run, cancel); T043 and T044 pass
- [ ] T048 [US2] Resources and interface: the instance state and fault texts in `resources/i18n/<lang>/ui.ftl` in six languages; in `crates/app/src/decoders.rs` the panel shows the state, the fault message and the **Run** and **Cancel** buttons; the settings dialog in `crates/app/src/app.rs` gets the decoder memory limit field (validated like the capture limit); update `docs/user-guide.md`
- [ ] T049 [US2] Run fmt, clippy, `cargo test --workspace`; commit the phase

**Checkpoint**: decoder faults are contained and explained in six languages

---

## Phase 6: User Story 3 - A very large capture stays smooth, also while it grows (Priority: P1)

**Goal**: decoding is background, cancellable, progress-reporting and follows a growing capture; drawing
cost does not depend on the capture length

**Independent Test**: a capture of one hundred million samples on 16 channels decoded while zooming,
cancelling and growing (spec US3, SC-002, SC-004, SC-009)

### Tests first

- [ ] T050 [P] [US3] Write `crates/decoders/tests/execution.rs`: progress is monotonic; Cancel takes effect in under 1 s even with a slow decoder, keeps the results, and the pass can be started again; growth: a capture appended to while a pass runs yields no duplicate and no lost annotation, the covered position advances, `finish` comes only when the capture is closed, and a gap that appears is resumed; a setup change cancels the running pass and the results are swapped at once, never mixed, the old ones visible and flagged outdated until then; more instances than the thread cap run in turn
- [ ] T051 [P] [US3] Write `crates/host/tests/decoder_growth.rs`: with a capture running at the virtual device's highest rate and a decoder on it, annotations appear within 1 s of the data, the capture loses no samples and ends complete exactly as without a decoder, `wants_ticks` is `Fast` while a pass runs, a save while decoding writes a consistent state, and closing or starting a new capture cancels every pass cleanly
- [ ] T052 [P] [US3] Write the benchmarks (dependency-free harness like `frame_plan`): `crates/host/benches/annotation_plan.rs` (the lane and overlay plan for finished stores of 1 and 10 million annotations on capture lengths of 100 million and 1,000 million samples, whole capture, 1 % and 4 px per sample, at 2000 and 4000 px; fails if the ten times larger case is more than twice as slow or the plan exceeds the budget), `crates/decoders/benches/store_insert.rs` and `decode_throughput.rs` (the sample decoder over 100 M samples), each with a `--write-baseline` option and a `baseline.json`
- [ ] T053 [P] [US3] Add to the tests of `crates/app/src/app.rs` an ignored `measure_decoder_responsiveness` (a 100-million-sample, 16-channel capture with the sample decoder: the longest call of the interface thread during decoding, cancelling, zooming and a language change, and the frame plan time with the full annotation set), printing a table like `measure_responsiveness_on_a_hundred_million_samples`

### Implementation

- [ ] T054 [US3] Create `crates/decoders/src/run/scheduler.rs` and extend `worker.rs`: the queue and the cap of `max(1, cores / 2)` decode threads, yielding after every chunk (`yield_now` and a 1 ms sleep while a capture is running), the growth loop (a new snapshot per round, backoff when caught up, completion only when the capture is closed), the covered position; T050 partly passes
- [ ] T055 [US3] Extend `crates/decoders/src/store/`: progressive publishing per chunk through the `Arc` swap and `compact()` to a single run when a pass finishes (in the background); extend the host `poll` to bump `Versions.decoders` on growth at most 20 times a second; T050 passes
- [ ] T056 [US3] Extend `crates/host/src/decoders.rs` and the interface: a setup change cancels the pass and starts a new one with the old results kept and flagged outdated until the swap, `StartCapture`, `NewCapture` and `OpenSession` cancel every pass, `wants_ticks` is `Fast` while a pass runs; progress bars in the lane labels and in the panel, Cancel; T051 passes
- [ ] T057 [US3] Extend `crates/app/src/frame_log.rs` so `LA_FRAME_STATS=1` also times the lane and overlay plan; add the new benchmarks to `.github/workflows/bench.yml` (artifact paths) and their rows to `specs/001-workspace-ci-foundation/contracts/ci-checks.md`
- [ ] T058 [US3] Run the benchmarks and the measurement in release mode and record the baselines (`crates/host/benches/baseline.json`, `crates/decoders/benches/baseline.json`), the machine and the date, and the results in `verification.md`; run fmt, clippy, tests; commit the phase

**Checkpoint**: large and growing captures decode in the background and draw in constant time

---

## Phase 7: User Story 4 - Stack decoders (Priority: P2)

**Goal**: an upper decoder consumes what a lower one produced, with propagation and refusal rules

**Independent Test**: two test decoders, a lower one producing bytes and an upper one consuming them
(spec US4)

### Tests first

- [ ] T059 [P] [US4] Write `crates/decoders/tests/stacking.rs` with two test decoders added to `testing.rs` (`lower_bytes` produces `"byte"` items and `upper_words` accepts `Items("byte")` and annotates pairs): the upper sees exactly the lower's items; a changed, re-run or removed lower instance re-runs, marks outdated or asks for the removal of the upper; a fault below marks the one above incomplete and does not fail it; on a growing capture the upper follows the lower's watermark; refusals for a cycle, a source that produces the wrong kind and a chain of 5 with the right codes; the instance limit counts stacked instances
- [ ] T060 [P] [US4] Write `crates/host/tests/decoder_stack.rs`: `StackDecoder` valid and refused; `RemoveDecoder` of an instance with dependants returns `NeedsDecision(Guarded::RemoveDecoder)` and `Decide(Discard)` removes it and everything above while `Decide(Cancel)` keeps all (`Decide(Save)` is ignored for it); the lanes come in stack order; the upper instance shows `outdated` while its source is re-running

### Implementation

- [ ] T061 [US4] Implement `crates/decoders/src/stack.rs` (the instance graph, source and result versions, compatibility check, propagation, ordering of lanes) and extend `run/` so a job can read an item stream with a watermark (`ItemChunk`); T059 passes
- [ ] T062 [US4] Extend `crates/host`: the `StackDecoder` command, `Guarded::RemoveDecoder` in `command.rs` and its handling in `decide`, the list of possible sources in the state, new command names in `shortcut_coverage`; resources (the source chooser label and the remove-with-dependants texts) in six languages; the configuration form gets the source chooser and the interface a dialog for the removal question; edit `contracts/decoder-ui.md` so it no longer promises moving lanes up and down (the order is the stack order); T060 passes
- [ ] T063 [US4] Run fmt, clippy, `cargo test --workspace`; commit the phase (there is no manual step: the one shipped decoder has nothing to stack, the tests are the proof)

**Checkpoint**: stacking is complete and ready for features 008 and 010

---

## Phase 8: User Story 5 - Save and open a capture with its decoders (Priority: P2)

**Goal**: the setup (and, if chosen, the results) travel in the session file; old, damaged and newer files
open safely

**Independent Test**: configure a stack, save, reopen, compare; repeat with results; open old, damaged
and newer files (spec US5, SC-008)

### Tests first

- [ ] T064 [P] [US5] Write `crates/decoders/tests/deco_section.rs`: hand-written byte vectors from `deco-section.md` (empty, a typical stack of two instances with options of all three kinds, a maximal one of 8 instances) read by the reader independently of the writer, round trips, a failing fixture for every validation rule of the contract, unknown decoders and unknown options kept byte for byte through a load and a save, the deterministic writer, a newer version reported as `Newer`, and a mutation fuzzer (every single-byte change, truncation and extension of the small payloads, sampled for the large one; an `#[ignore]` randomized variant with `LA_DECO_FUZZ_ITERATIONS`, a million in the nightly run): nothing panics or allocates beyond the limits and any accepted payload re-encodes identically
- [ ] T065 [P] [US5] Write `crates/decoders/tests/dann_section.rs`: the same for `DANN`: hand-written vectors, round trips of annotations and items, the validation rules (beyond the capture, not in start order, string-table index out of range, counts checked before allocation, a block for an instance `DECO` does not define, the memory budget), the fuzzers
- [ ] T066 [P] [US5] Write `crates/session/tests/deco_sections_carried.rs`: a session with extra `DECO` and `DANN` sections round-trips unchanged through save and load and the writer now emits format minor 2; update the independent reader and writer test of feature 003 (`crates/session/tests/independent_reader.rs`) for the new minor and the new fixture directory; files of format 1.0 and 1.1 still load
- [ ] T067 [P] [US5] Write `crates/host/tests/decoder_sessions.rs`: the setup is restored (same decoders, channels, options, stack, lane visibility) and the passes start again; saved results are shown at once flagged `loaded`, identical to what was saved, until a new pass replaces them; a file from before this feature opens as it did; a damaged `DECO` opens with a warning and the capture; a damaged `DANN` keeps the setup and the decoders run again; an unknown decoder's record survives a save byte for byte; a newer version gives an information notice; `Unsaved` is `Dirty` when the setup changes and not when a pass finishes; a save with decoders interrupted at 100 or more fault points through `FaultFs` leaves the old file intact

### Implementation

- [ ] T068 [US5] Change `FORMAT_MINOR` to 2 in `crates/session/src/limits.rs`, regenerate the `reference` fixtures and add `crates/session/tests/fixtures/v1_2` (the writer-output check compares against it; the 1.0 and 1.1 fixtures stay as they are and keep loading), and update `specs/003-session-file-format/contracts/file-format.md` (the writer emits minor 2; `DECO` and `DANN` are defined by `specs/007-decoder-framework-annotations/contracts/deco-section.md`); T066 passes; run the whole `la-session` suite
- [ ] T069 [US5] Implement `crates/decoders/src/persist/` (`mod.rs`, `varint.rs`, `deco.rs`, `dann.rs`): the canonical varint code, readers, writers, validators and limits of the section contract; T064 and T065 pass
- [ ] T070 [US5] Extend `crates/host/src/sessions.rs`, `settings.rs`, `command.rs`, `state.rs`: build the `DECO` and `DANN` sections when saving (`DANN` only when the setting is on and results exist), apply them when opening (restore instances and start passes, or show the loaded results), keep unknown records for write-back, add the notices (`decoder.section_invalid`, `decoder.unknown_decoder`, `decoder.options_skipped`, an information notice for a newer version), the setting `save_decoder_results` and `SetSaveDecoderResults`, new names in `shortcut_coverage`; T067 passes
- [ ] T071 [US5] Resources and interface: the setting's label and the messages not yet covered in six languages; a "Save decoded results" check box in the settings dialog; update the user guide and `docs/host-api.md` draft rows
- [ ] T072 [US5] Manual check from the quickstart (step 8.5: save, close, reopen, with and without saved results); record; run fmt, clippy, `cargo test --workspace`; commit the phase

**Checkpoint**: nothing the user configured or decoded is lost, and any file opens

---

## Phase 9: User Story 7 - Read the annotations comfortably and in my language (Priority: P3)

**Goal**: legible annotations in all six languages and themes at the smallest window size

**Independent Test**: switch through all six languages with decoders on screen at the smallest window
(spec US7, SC-010)

### Tests first

- [ ] T073 [P] [US7] Write `crates/host/tests/decoder_language.rs`: after `SetLanguage` the decoder names, descriptions, signal and option labels, state names and the standard annotation texts of built-in decoders change at once (in the catalogue, the instances and the lane plan), literal annotation text and instance names are unchanged, and notices follow the language
- [ ] T074 [P] [US7] Extend `every_screen_renders_in_every_language_at_the_minimum_window_size` in `crates/app/src/app.rs` with the add dialog, the configuration form (including the generator options and an option with a long label), the Decoders panel with every state, and a lane with long annotation text and overlays; add unit tests of the drawing rules of `decoder-ui.md` (ellipsis, no text under three characters, no overlap with a neighbour, the later cell shortened); confirm that `crates/host/tests/string_budget.rs` still catches an over-long fixture and now covers the new keys

### Implementation

- [ ] T075 [US7] Implement the text rules of the UI contract in `crates/app/src/signal_view.rs`: ranges clipped with an ellipsis, a bar when too narrow, the readout with the full text and the length in time formatted for the language
- [ ] T076 [US7] Extend `crates/app/src/theme.rs`: the 8 annotation colours in the light, dark and high-contrast variants with a unit test of the contrast ratio against the background and the waveform colours, and distinct shapes so nothing depends on colour alone
- [ ] T077 [US7] Make everything reachable from the keyboard: the accelerators for next, previous and go to selected annotation in `crates/host/src/shortcuts.rs`, the tab order of the dialog and the panel in `crates/app/src/decoders.rs`; `crates/host/tests/shortcut_coverage.rs` must stay green
- [ ] T078 [US7] Manual check from the quickstart (steps 8.6 and 8.7: language switch with decoders, the smallest window, a Japanese IME entry of an instance name, high contrast); record; run fmt, clippy, tests; commit the phase

**Checkpoint**: the decoder features are usable in all six languages

---

## Phase 10: Polish & Cross-Cutting Concerns

**Purpose**: the long-run test, the documents, the measurements and the traceability

- [ ] T079 Extend `crates/host/tests/random_walk.rs` with decoder actions (add, configure, stack with the test decoders under `test-support`, cancel, re-run, remove, toggle save results, open files with damaged decoder sections, capture growth, language change) and the invariants: at most 8 instances, a consistent state per instance, no mixed old and new results, decoder memory within its limit, the capture's data safety unchanged, no thread left at the end; 100,000 actions in the normal run, the `#[ignore]` variant of two million for the nightly workflow
- [ ] T080 [P] Add the long variants to `.github/workflows/fuzz.yml` (the `DECO` and `DANN` mutation fuzzers at 1,000,000 iterations, the round trip with 10,000 seeds) and the rows to `specs/001-workspace-ci-foundation/contracts/ci-checks.md`
- [ ] T081 [P] Merge `contracts/host-api-1.1.md` into `docs/host-api.md` (version line `1.1`, `HOST_API_VERSION` to `"1.1"`, the command table with every new command, state, plan and polling text, the unsaved-data section for `RemoveDecoder`); the existing `crates/host/tests/host_api_doc.rs` must pass; point the design contract at the document
- [ ] T082 [P] Complete the **Decoders** chapter of `docs/user-guide.md` (what a decoder is, the lanes and overlays, selecting and navigating, the settings, saving results, faults and what to do), add "Adding a decoder" to `CONTRIBUTING.md` (the descriptor, message keys as literals, the generator and its round-trip test, registration, the rule that `la-app` needs no change), and update `README.md` and `roadmap.md` (007 implemented)
- [ ] T083 Run the long variants once locally in release mode (`cargo test -p la-decoders -p la-host --release -- --ignored`) and the benchmarks (`cargo bench -p la-decoders`, `cargo bench -p la-host`), and record results, machine and date in the baselines and in `verification.md`
- [ ] T084 Measure responsiveness and idle CPU on Windows with decoders on a 100 M-sample capture (`measure_decoder_responsiveness`, and the real window for one minute idle with decoders finished: SC-004, SC-009, SC-011) and record in `specs/007-decoder-framework-annotations/verification.md`; if the Debian measurement cannot be done from this machine, ask the user for the result and record it as pending rather than claiming it
- [ ] T085 Complete the traceability in `specs/007-decoder-framework-annotations/verification.md`: each FR-001 to FR-042 and SC-001 to SC-012 mapped to the test or manual check that proves it, each manual item with platform, date and result; any requirement not met stated as such with the reason
- [ ] T086 Final gate: `cargo fmt --check`, `cargo clippy --workspace --all-targets --all-features -- -D warnings`, `cargo test --workspace`, `cargo +1.85 check` of every crate except `la-app`, the parity check, `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps`; push, open the pull request as a draft, wait for all required CI checks on both systems; list in the pull request the follow-ups that need the user (the wording of Principle IX's overlay bound against Principle VIII, native-speaker review of the new strings, the Debian manual checks, the decision on process isolation for future user-written decoders)

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: none
- **Foundational (Phase 2)**: depends on Setup and blocks every story; tests T005–T011 are independent of each other, T012–T017 implement them (T016 needs T013; T017 needs T014–T016), T018 can follow T011 at any time
- **US1 (Phase 3)**: depends on Foundational; the MVP; its tests T020–T023 are independent of each other; T027 needs T025 and T026; T029 and T030 need T027 and T028
- **US6 (Phase 4)**: depends on US1 (the sample decoder and the host wiring); done second so later phases can be tried without hand-built signals
- **US2 (Phase 5)**: depends on US1 (the worker and the store); T041 (test decoders) also serves US3 and US4
- **US3 (Phase 6)**: depends on US1 and benefits from US2 (budgets, supervisor); its benchmarks need only the Foundational store and lanes
- **US4 (Phase 7)**: depends on US1 and US2 (jobs, faults); the stacking tests use the test decoders of T041
- **US5 (Phase 8)**: depends on US1 and US4 (the setup includes the stack); T068 (session minor 2) is independent and can land first
- **US7 (Phase 9)**: depends on US1; the render and text checks need the screens of all earlier phases
- **Polish (Phase 10)**: depends on all stories

### Within Each User Story

- Tests first and failing before implementation
- Decoder-crate logic and resources (English first, then five translations) before the host and the interface that need them
- Run clippy with `-D warnings` and `cargo test --workspace` at the end of every phase and never commit a failing gate; commit after every phase (split into smaller logical commits where useful)
- The parity check of feature 005 is the judge for resources: every phase that adds text runs `cargo test -p la-i18n`; the host API document test and the shortcut coverage test are the judges for new commands

## Parallel Execution Examples

- Setup: T002 + T003 + T004 (different files) after T001
- Foundational: T005 + T006 + T007 + T008 + T009 + T010 + T011 (tests), then T013 + T014 + T015 in parallel, then T016, T017
- US1: T020 + T021 + T022 + T023 (tests); T024 (resources) alongside T025 and T026
- US6: T033 + T034 + T035 (tests); T036 and T037 in parallel
- US2: T041 + T042 + T043 + T044 (tests); T045 and T046 in sequence, T048 (resources) alongside T047
- US3: T050 + T051 + T052 + T053 (tests); T054 and T055 in sequence
- US5: T064 + T065 + T066 + T067 (tests); T068 can be done before everything else in the phase; T069 and T070 in sequence
- Polish: T080 + T081 + T082 in parallel

## Implementation Strategy

**MVP**: Phases 1–3 (Setup, Foundational, US1): a user adds the sample decoder to a capture and sees
bytes, bits and arrows. Then US6 (the generator, so everything can be tried with no hand-built signal),
then US2 and US3, both P1: a framework that can fail a session or freeze the window is not allowed to be
used, so faults and scale come before stacking and persistence. Then US4 (stacking, ready for 008 and
010), US5 (saving, the data-safety promise for the user's setup, not to be postponed beyond US4), and
US7 (language and layout polish). Polish closes with the long-run test, the documents and the
measurements. Every phase keeps the parity check, clippy, the MSRV check and the tests green and ends in
a commit; the manual checks are recorded as they are done, not at the end.
