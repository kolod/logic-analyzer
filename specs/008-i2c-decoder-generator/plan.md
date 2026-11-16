# Implementation Plan: I2C Decoder and Generator

**Branch**: `008-i2c-decoder-generator` | **Date**: 2026-11-16 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `/specs/008-i2c-decoder-generator/spec.md`

## Summary

Add the first real protocol decoder, I2C, to the framework of feature 007, with its generator for the
virtual device. The decoder reads two channels (SCL, SDA), synchronises on the bus (first start only
after the lines have been high for one automatically measured bus period), decodes transactions
(7-bit and 10-bit addresses, repeated start, acknowledgements, errors) into lane annotations, overlays
(a dot on SDA at start/stop, an arrow on SCL at each sampled bit) and one kind of stack item. The
generator is a pure function of the sample position (frames of fixed length, 16 samples per SCL
period), reproducible from a seed, with planted errors on request, and the round trip generate →
decode → compare is a test over a thousand seeds. The decoder is added by registration; the only change
to the framework contract is one optional method, `Run::final_through` (decoder API 1.0 → 1.1), needed
so that a decoder that waits for the bus period keeps the watermark of a stack truthful. The
application shell is not touched.

## Technical Context

**Language/Version**: Rust, stable; MSRV 1.85 as recorded in the workspace (no new language features).

**Primary Dependencies**: none new. Existing: `la-decoders`, `la-devices` (the `SignalSource` trait),
`la-storage` (through the reader), `la-core` (errors). Principle V is met by adding nothing.

**Storage**: none of its own. Setup in the `DECO` section and results in `DANN` of feature 007,
unchanged. Fixtures: text files of run-length waveforms under `crates/decoders/tests/fixtures/i2c/`.

**Testing**: `cargo test` (unit, recorded signals, round trip over seeds, chunk invariance, random
signals, stacking, host end to end); the decode benchmark with its recorded baseline.

**Target Platform**: Windows 10/11 and Debian 13 (x86-64 both; the code has no platform-specific part).

**Project Type**: desktop-app (Rust workspace: UI-independent core crates + UI shell). This feature is
entirely in the UI-independent crates plus resources and documents.

**Performance Goals**: decoding dense traffic at a constant cost per edge (benchmark baseline recorded);
a capture of ten million samples decodes in the background with no interface pause above 100 ms; the
drawing cost does not depend on the capture length (feature 007's planner, unchanged).

**Constraints**: a decoder call takes milliseconds (at most 262,144 edges of calibration or catch-up per
call); state is bounded by constants (a 256-bin histogram, a ring of 32 intervals and a few counters); no panic on any input;
no new dependency; the shell is untouched (SC-008).

**Scale/Scope**: millions of transactions per capture; about 11 annotations per byte; one decoder, one
generator, one item kind, two options, about 20 message keys in six languages.

## Constitution Check

*GATE: passed before Phase 0 research and re-checked after Phase 1 design.*

- [x] **I. Stability & Data Integrity**: the decoder uses checked arithmetic and no indexing on external
  data (it reads edges through the reader's iterator; the histogram index is computed from a `u64` with
  `leading_zeros` and bounded to 255); typed `DecodeError` only for a decoder that cannot continue (it
  has no such case); a random-signal test of a million chunks; the fault containment of 007 stays the
  backstop. Nothing writes files here.
- [x] **II. Hardware**: not affected. No driver is touched; the generator is a virtual-device signal source
  behind the existing `SignalSource` trait.
- [x] **III. Test-First**: tests are written before the code they test (tasks order them first):
  recorded signals and the round trip fail until the decoder and generator exist; malformed, truncated
  and random inputs are in; every bug found gets a regression test. CI runs fmt, clippy, test, deny,
  MSRV on both systems.
- [x] **IV. Architecture**: all logic in `la-decoders`. Host API: no change. Decoder API 1.0 → 1.1
  (one optional method), documented, implemented and tested in the same merge; no consumer outside the
  framework needs changing. Work is background and cancellable through the existing job (a call is
  bounded); memory is bounded by the framework's budgets and the decoder's small state.
- [x] **V. Simplicity**: no new dependency. The one extra mechanism (`final_through`) is justified in
  Complexity Tracking.
- [x] **VI. Observability**: the decoder logs nothing about signal data (rule of 007); counts and codes
  only. Errors a user sees are annotations with messages that say what was wrong, and the framework's
  existing coded notices.
- [x] **VII. Localization**: about 20 new keys in `ui.ftl` for en/de/fr/es/ja/uk (machine drafts for
  the owner to review); no plural is used; values (addresses, bytes) are not translated; the parity
  check covers all keys; documents in English.
- [x] **VIII. Rendering**: no change to rendering. The decoder only produces annotations; the planner and
  the store of 007 bound the drawing cost by the visible window, and a test with dense traffic checks
  that the cost per frame does not grow with the capture. The decode benchmark has a recorded baseline.
- [x] **IX. Decoding**: decoder faults are contained by the framework; overlays on SDA (start/stop dots)
  and SCL (sampling arrows) are produced as the constitution's example describes; stacking output is
  defined (contracts/i2c-items.md); recorded-signal tests include malformed and truncated input.
- [x] **X. Export**: not in this feature (009). The items carry what the export needs (address,
  direction, acknowledgement, positions), and incomplete regions and errors are marked in the results.
- [x] **XI. Virtual device**: the decoder has a generator, seeded and reproducible, with planted errors
  (NAK, cut short), the round-trip test is required and planned (SC-001), generation is a pure function
  of the position so it never blocks and is cancellable with the capture, and generated data stays
  labelled as simulated by the device (no change).
- [x] **XII. Original design & third-party code**: no code from PulseView, libsigrok, libsigrokdecode or
  any other project; written from the I2C-bus specification (NXP UM10204). The look of the lane and the
  overlays follows feature 007's own design.
- [x] **Platforms**: nothing platform-specific; tests run on both systems in CI. No driver is touched, so
  the device-access documents are unchanged.

## Project Structure

### Documentation (this feature)

```text
specs/008-i2c-decoder-generator/
├── plan.md              # This file
├── research.md          # Phase 0: decisions R1–R14
├── data-model.md        # Phase 1: descriptor, state, events, generator
├── quickstart.md        # Phase 1: how to validate
├── contracts/
│   ├── decoder-api-1.1.md   # the one change to the framework contract
│   ├── i2c-items.md         # what a stacked decoder receives
│   └── i2c-generator.md     # the generator's functions, options and signal
├── checklists/requirements.md
└── tasks.md             # Phase 2 output (/speckit-tasks — not created by this command)
```

### Source Code (repository root)

```text
crates/decoders/
├── src/
│   ├── decoders/
│   │   ├── mod.rs                 # + pub mod i2c;
│   │   └── i2c/
│   │       ├── mod.rs             # descriptor, options, create(), the Run implementation
│   │       ├── period.rs          # R1: the interval histogram and the calibration condition
│   │       ├── bus.rs             # R4: levels, edges, start/stop/bit, the transaction state machine
│   │       └── text.rs            # R5/R8: annotation texts, address and radix formatting
│   ├── generators/
│   │   ├── mod.rs                 # + pub mod i2c;
│   │   └── i2c.rs                 # contracts/i2c-generator.md
│   ├── registry.rs                # + the I2C descriptor, create and generator claim in builtin()
│   ├── run/mod.rs                 # + Run::final_through (default None)
│   ├── run/job.rs                 # coverage = min(chunk end, final_through)
│   └── lib.rs                     # DECODER_API_VERSION = "1.1"
├── tests/
│   ├── i2c.rs                     # unit and recorded-signal tests
│   ├── i2c_round_trip.rs          # generate → decode → compare, chunk invariance, random signals
│   ├── i2c_stacking.rs            # test-only upper decoder on kind `i2c`
│   ├── final_through.rs           # the framework test of contracts/decoder-api-1.1.md
│   ├── common/                    # + a builder from bus events to samples, a fixture loader
│   └── fixtures/i2c/*.txt         # recorded signals and their expected decodes
└── benches/decode_throughput.rs   # + the I2C case; baseline.json updated

crates/host/tests/
└── decoder_i2c.rs                 # virtual device end to end, 10M samples, session round trip

resources/i18n/{en,de,fr,es,ja,uk}/ui.ftl   # + the keys of research R9
docs/
├── decoder-api.md                 # version 1.1, final_through, common generator options
├── decoders/i2c.md                # the decoder's reference (new)
└── user-guide.md                  # + "Decoding I2C"
roadmap.md                         # 008 status when merged
```

**Structure Decision**: the decoder is a module directory inside the existing `la-decoders` crate, beside
the sample decoder, because registration in `Registry::builtin()` is what the contract promises (feature
007, "Adding a decoder"); splitting it into files by concern (period, bus, text) keeps each file small
and independently testable. The generator mirrors `generators/clocked_bits.rs`. No new crate. Nothing
changes in `crates/app`, `crates/host/src` or `crates/session` (SC-008); the host gets tests only.

## Complexity Tracking

| Violation | Why Needed | Simpler Alternative Rejected Because |
|-----------|------------|-------------------------------------|
| A change to the decoder contract (`Run::final_through`, API 1.1), against the aim of FR-022 to add the decoder by registration only | The synchronisation rule (clarified by the user) needs the bus period, which is known only after the first clocks; the decoder must hold work back and catch up. Without telling the framework, its coverage and the stack watermark would run ahead of the decoder's output, and feature 010 would receive items behind a watermark that had passed them | Lagging silently (breaks the stack guarantee); a restart request in the chunk loop (a larger change nothing else needs); replaying everything inside one call (unbounded call time, abandoned by the supervisor on a long noisy start-up). The method is optional, defaults to today's behaviour, and the sample decoder and all tests of 007 are untouched |
| Two fixed constants chosen here and not by the user: 16 samples per SCL period in the generator, and 262,144 edges of work per call in the decoder | The generator must be a pure function of the position and must suit any sample rate; a call must stay at milliseconds whatever the start-up | A period scaled to the sample rate needs a change to the claim interface of 007; an unbounded call is what the supervisor abandons. Both constants are documented and tested |
