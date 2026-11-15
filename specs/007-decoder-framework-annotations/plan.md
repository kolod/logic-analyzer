# Implementation Plan: Decoder Framework and Annotations

**Branch**: `007-decoder-framework-annotations` | **Date**: 2026-11-15 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `/specs/007-decoder-framework-annotations/spec.md`

## Summary

A protocol-decoder framework, proven by one sample decoder. All behaviour lives in a new UI-free crate,
`la-decoders`: the **annotation model** (ranges, points and edge markers, placed in a decoder's own lane or
as an overlay on a channel, with a style and text that is a localizable message or literal), a **decoder
interface** that is a resumable state machine fed in chunks and reads the capture only through a read-only
reader (research R2, `contracts/decoder-api.md`), an **annotation store with a time index** (sorted runs with
a maximum-end tree; window, rank and neighbour queries, research R4), **execution** as supervised background
jobs with chunking, cancellation, budgets and a watchdog that abandons a decoder that does not return (R6, R7),
**stacking** as an acyclic graph with outdated-result propagation (R12), and the **persisted setup and results**
in two new optional session sections `DECO` and `DANN` (format minor 2, `contracts/deco-section.md`).
The built-in **sample decoder "Clocked bits"** with its seeded **generator** (R13, R14) exercises every part.
`la-host` orchestrates (commands, state, frame plan with lane and overlay plans, settings, the section
mapping; host API 1.1, `contracts/host-api-1.1.md`) and `la-app` draws lanes, overlays, a decoder panel and
a generic dialog built from decoder descriptors, which is what makes a registered decoder appear without a
shell change (R15, `contracts/decoder-ui.md`). Isolation is in-process containment (R9); no dependency is
added. Almost everything is verified headlessly on both CI platforms; the windowed checks (look, feel, idle
CPU, Debian) are manual and recorded.

## Technical Context

**Language/Version**: Rust 2024 edition; the new crate and every change to `la-host` keep MSRV 1.85 (no
`let` chains); `la-app` keeps its own higher `rust-version`; toolchain pinned by `rust-toolchain.toml`

**Primary Dependencies**: none new. The standard library and the workspace crates `la-core`, `la-storage`,
`la-devices` (for the generator trait), `la-diagnostics` (supervised workers, lock recovery), `la-i18n`,
`la-session`; `tracing`. The persisted format uses its own small canonical varint code, as `view_section`
does (research R16)

**Storage**: session files via `la-session` with two new optional sections (`DECO`, `DANN`); two settings
added to the existing settings file (decoder results memory limit, save results); decoder results in memory
only, bounded by an explicit limit

**Testing**: `cargo test` (unit and property tests for the model and the index against a brute-force
reference, fault-injection decoders behind `test-support`, execution and growth tests, hand-written byte
vectors and a mutation fuzzer for `DECO` and `DANN`, a 10,000-seed round trip, host scenarios, headless view
renders in six languages), a 100,000-action random walk extended with decoder actions (two million nightly),
benchmarks with a recorded baseline (annotation plan, decode throughput, store insertion), manual windowed
checks recorded in `verification.md`

**Target Platform**: Windows 10/11 and Debian 13 (x86_64); no platform-specific code is added

**Project Type**: desktop application; adds `crates/decoders` (library); modifies `crates/host`,
`crates/app`, `crates/session` (format minor 2, tests and fixtures) and `crates/i18n` resources

**Performance Goals**: frame time p95 < 16 ms and worst < 50 ms with a finished decode of ≥ 1,000,000
annotations on 100 M samples × 16 channels, and < 2× growth for a 10× longer capture (SC-002); the
interface never unresponsive over 100 ms and a cancel within one second (SC-004); results shown within
one second of the data on a running capture and no more lost samples than without a decoder (SC-009); idle
CPU < 1 % of one core with decoders finished (SC-011)

**Constraints**: no panic on any decoder output, setup or file (workspace lints apply to the new crate); a
decoder can never write to or damage the capture; memory bounded by explicit limits and refusal with a
handled message; the capture's ingestion has absolute priority (decoders yield and are limited in
threads); no signal data or annotation text in logs; no text in code; no network, no execution of code from
files

**Scale/Scope**: up to 8 instances per capture, stack depth 4, 5,000,000 annotations per instance, 256 MiB
default results memory; about 60 new UI messages and 15 new error codes per language; one decoder shipped

## Constitution Check

*GATE: passes before Phase 0 research; re-checked after Phase 1 design (still passes).*

- [x] **I. Stability & Data Integrity**: every decoder fault (error, panic, no response, memory or count
  overrun, invalid output) is contained at the job boundary and mapped to a `decoder.*` code; the decoder
  interface is read-only so the capture cannot be changed (R2, R6, R7); `DECO` and `DANN` are read as
  untrusted bytes with bounded counts, canonical varints and typed errors, fuzzed, and a bad section drops
  with a warning while the capture opens (R11); files stay atomic through the unchanged session writer; the
  workspace lints forbid unwrap, indexing and unchecked arithmetic in the new crate.
- [x] **II. Hardware**: no driver is touched; decoding reads captures only through the storage snapshot, and
  the generator attaches through the existing virtual-device trait, so no device-specific logic enters the
  decoders (R14). The three drivers are unaffected.
- [x] **III. Test-First**: the tasks write the tests before the implementation for each part: index against
  a reference, fault injection, round trip, section fixtures and fuzzing, host scenarios (quickstart lists
  them); malformed and truncated signals for the sample decoder; every fix gets a regression test; fmt,
  clippy and test run on both platforms in CI.
- [x] **IV. Architecture**: all logic is in `la-decoders` and `la-host` with no UI dependency; the change to
  the host API (1.1) is documented in `contracts/host-api-1.1.md`, merged into `docs/host-api.md` and
  consumed by `la-app` in the same merge; all decoding is background, cancellable and progress-reporting;
  memory is bounded by explicit visible limits (R6).
- [x] **V. Simplicity**: no new dependency (R16). The two pieces of non-trivial structure, the logarithmic
  index and the watchdog with abandoned workers, are recorded in Complexity Tracking with the simpler
  alternatives and why they fail.
- [x] **VI. Observability**: lifecycle and every fault are logged with code, decoder id and position, never
  signal data or annotation text; each message says what failed and what the user can do; panics are caught
  at the worker boundary and reported while the capture is preserved (R7).
- [x] **VII. Localization**: decoder names, descriptions, signal and option labels, standard annotation texts
  of built-in decoders, panel, dialog, menu and the `decoder.*` messages are keys with static literals in six
  languages (parity-checked); instance names and literal annotation text are outside text, shown as typed and
  sanitized; the string budget and the layout renders cover them; documents and comments in English (R15).
- [x] **VIII. Rendering**: lane and overlay plans come from one indexed window query per instance plus rank
  counts per column; the plan is bounded by `4W` cells per lane; dense cells keep every annotation visible;
  the cost on a finished decode is `O(log n + W log n)` independent of the capture length, and on a growing
  one `O(R log n + 4096)` with `R ≤ 16` (stated honestly in R4 and pinned by a probe-count test); a benchmark
  with a recorded baseline fails on a loss of the complexity class (R4, R5).
- [x] **IX. Decoding**: stackable (R12), configured per channel (setup), annotations in lane and overlay
  with range, point, edge marker, target, style and text (R3); faults contained and cancellable; recorded
  signals including malformed and truncated input for the built-in decoder (R17); overlay lookup is indexed;
  the decoder interface is documented in English (`contracts/decoder-api.md`, then `docs/decoder-api.md`).
- [x] **X. Export**: not part of this feature. The annotation and item stores are queryable in order and
  carry incompleteness flags, which is what feature 009 needs; nothing here makes the export harder.
- [x] **XI. Virtual device**: the sample decoder has a seeded, reproducible generator behind the existing
  `SignalSourceFactory` that follows the decoder setup; planted errors; a round-trip test over 10,000 seeds;
  simulated data stays labelled everywhere (R13, R14).
- [x] **XII. Original design & third-party code**: everything is written from scratch; the decoder interface
  is this project's; no PulseView, sigrok or Logic 2 code, assets, names or layouts are used; the sample
  decoder is a generic invention, not a copied protocol; no third-party code is reused.
- [x] **Platforms**: nothing platform-specific; both CI platforms run every test; no device-access
  documentation changes.

*Post-design re-check (after `research.md`, `data-model.md`, `contracts/`, `quickstart.md`)*: all gates still
pass. The one place where the plan stretches a letter of the constitution is stated in Principle VIII's line:
a store that is still growing has up to 16 index runs, so its window query is `O(R log n)`, a bounded constant
factor, until the pass finishes and compacts. It is within the principle's intent (cost independent of the
capture length) and is covered by a test, so no waiver is needed; it is recorded below for review.

## Project Structure

### Documentation (this feature)

```text
specs/007-decoder-framework-annotations/
├── plan.md                  # this file
├── research.md              # Phase 0: decisions R1 to R17
├── data-model.md            # Phase 1: entities, limits, states
├── quickstart.md            # Phase 1: how to validate end to end
├── contracts/
│   ├── decoder-api.md       # the decoder interface (becomes docs/decoder-api.md)
│   ├── deco-section.md      # the DECO and DANN session sections (format minor 2)
│   ├── host-api-1.1.md      # additions to the host API (merged into docs/host-api.md)
│   └── decoder-ui.md        # lanes, overlays, panel, dialog
├── checklists/requirements.md
└── tasks.md                 # Phase 2 (created by /speckit-tasks, not by this command)
```

### Source Code (repository root)

```text
crates/decoders/                 # NEW: la-decoders, UI-free
├── Cargo.toml                   # features: test-support (fault-injection decoders)
├── src/
│   ├── lib.rs                   # DECODER_API_VERSION, registry entry points
│   ├── error.rs                 # decoder.* codes (found by the error sweep)
│   ├── model.rs                 # Annotation, Style, Target, Text, Item, Payload, validation
│   ├── descriptor.rs            # Descriptor, SignalSpec, OptionSpec, validation
│   ├── setup.rs                 # Setup, validation (channels, options, source, stack rules)
│   ├── registry.rs              # registration of descriptors, create and generator hooks
│   ├── input.rs                 # Chunk, SignalReader (edges, level, gaps), ItemChunk
│   ├── store/                   # AnnotationStore: runs, tree, tail, queries, accounting
│   ├── lanes.rs                 # window query to lane and overlay cells (rows, dense cells)
│   ├── run/                     # Job, Supervisor, budgets, chunking, yielding
│   ├── stack.rs                 # instance graph, versions, propagation
│   ├── persist/                 # DECO and DANN readers and writers, limits
│   ├── decoders/clocked_bits.rs # the sample decoder
│   ├── generators/clocked_bits.rs  # its generator and the expected content
│   └── testing.rs               # test decoders: error, panic, hang, memory, invalid, healthy (test-support)
├── tests/                       # round_trip, fault_injection, index, lanes, execution, stacking,
│                                # deco_section, dann_section, fuzz
└── benches/                     # store_insert, decode_throughput (baseline.json)

crates/host/                     # MODIFIED: host API 1.1
├── src/decoders.rs              # NEW: state, commands, supervisor poll, settings glue
├── src/plan.rs                  # lane and overlay plans in FramePlan
├── src/sessions.rs              # DECO and DANN mapping, kept unknown records
├── src/{command,state,shell,shortcuts,settings,error}.rs   # new commands, state, rows, settings, codes
├── tests/                       # decoder_commands, decoder_faults, decoder_sessions, random_walk (extended)
└── benches/annotation_plan.rs   # NEW: plan cost at 1 and 10 million annotations

crates/app/                      # MODIFIED: the shell stays thin
└── src/                         # decoder lanes and overlays in signal_view, decoder panel, add and
                                 # configure dialogs generated from descriptors

crates/session/                  # MODIFIED: writer emits minor 2; tests with extra DECO and DANN, fixtures v1_2
crates/devices/                  # unchanged API; a test that a composite factory fills only claimed channels
resources/i18n/<lang>/ui.ftl, errors.ftl   # ~60 messages and 15 codes per language
docs/decoder-api.md              # NEW, authoritative (copied from the contract, version line 1.0)
docs/host-api.md                 # MODIFIED: version 1.1
docs/user-guide.md               # MODIFIED: decoders section
specs/003-session-file-format/contracts/file-format.md   # MODIFIED: minor 2, DECO and DANN named
.github/workflows/{bench,fuzz}.yml        # MODIFIED: new benchmarks and long runs
```

**Structure Decision**: one new library crate, because the decoder framework must be testable, fuzzed and
benchmarked without a window or the host, and feature 009's export must be able to use it without the
host. The generator uses the device crate's existing trait, so the dependency points from decoders to
devices and never back. The host stays the only crate that knows both sides and the interface stays a
renderer and a form builder, which is what keeps a new decoder from needing a change in the shell.

## Complexity Tracking

| Item | Why needed | Simpler alternative rejected because |
|------|------------|--------------------------------------|
| A logarithmic-method index (sorted runs with a maximum-end tree) instead of one sorted array | results grow while a pass runs, and the interface must show them progressively (US3) with `O(log n)` insertion | re-sorting the whole store per chunk is `O(n)` per publish and breaks progressive display on million-annotation results; a mutable interval tree makes snapshots for the interface thread awkward (research R4) |
| A watchdog that abandons a stuck worker and counts leaked threads | a decoder that never returns must not block the interface or the capture, and a thread cannot be killed safely (Principle IX) | waiting for the decoder blocks the user; a separate process per decoder is a new IPC design and a packaging burden, deferred and made possible by the narrow interface (R6, R9) |
| Two new session sections instead of one | the setup is small and must always load; the results are large, optional and must be droppable on their own | one combined section would let a damaged result cost the setup (R11) |
| A format minor bump (1.2) in `la-session` | the setup and results must travel with the file (FR-031, FR-032) | a sidecar file can be separated from the session; a critical section would make older readers refuse files they could otherwise open |
| Test decoders in the framework crate behind `test-support` | fault injection needs decoders that misbehave on purpose (SC-005) | injecting faults with mocks inside the host would test the host's wrapper, not the real boundary |
| Constitution follow-up (not edited here) | the constitution's Principle IX says "overlay rendering MUST keep the O(log N + W) bound"; the plan meets the intent with `O(R log n)` while a pass grows and `O(log n)` once finished | none needed for a waiver; the wording could be aligned with the bound already in Principle VIII at the next amendment |
