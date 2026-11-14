# Implementation Plan: [FEATURE]

**Branch**: `[###-feature-name]` | **Date**: [DATE] | **Spec**: [link]

**Input**: Feature specification from `/specs/[###-feature-name]/spec.md`

**Note**: This template is filled in by the `/speckit-plan` command; its definition describes the execution workflow.

## Summary

[Extract from feature spec: primary requirement + technical approach from research]

## Technical Context

<!--
  ACTION REQUIRED: Replace the content in this section with the technical details
  for the project. The structure here is presented in advisory capacity to guide
  the iteration process.
-->

**Language/Version**: Rust (stable; record the MSRV) [or NEEDS CLARIFICATION]

**Primary Dependencies**: [e.g., FastAPI, UIKit, LLVM or NEEDS CLARIFICATION]

**Storage**: [if applicable, e.g., PostgreSQL, CoreData, files or N/A]

**Testing**: [e.g., pytest, XCTest, cargo test or NEEDS CLARIFICATION]

**Target Platform**: Windows 10/11 and Debian 13 (both required; record CPU architectures)

**Project Type**: desktop-app (Rust workspace: UI-independent core crates + UI shell)

**Performance Goals**: [domain-specific, e.g., 1000 req/s, 10k lines/sec, 60 fps or NEEDS CLARIFICATION]

**Constraints**: [domain-specific, e.g., <200ms p95, <100MB memory, offline-capable or NEEDS CLARIFICATION]

**Scale/Scope**: [domain-specific, e.g., 10k users, 1M LOC, 50 screens or NEEDS CLARIFICATION]

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

Verify each gate against `.specify/memory/constitution.md` and record PASS / FAIL (with
justification in Complexity Tracking) for each:

- [ ] **I. Stability & Data Integrity**: no panics on external input; typed errors;
  atomic file writes; captured data survives disconnects/overruns; faults isolated.
- [ ] **II. Hardware**: UI/core use only the common device abstraction; each affected
  driver (WeAct, Logic16 clone, Pico) has hardware-free tests and a manual
  verification procedure; no other driver is affected.
- [ ] **III. Test-First**: tests planned before implementation; malformed-input and
  fuzz/property tests for parsers; fmt + clippy + test run in CI.
- [ ] **IV. Architecture**: logic in UI-independent crates; host API contract change
  (if any) documented and implemented on all platforms in the same merge; background
  work is cancellable; memory is bounded.
- [ ] **V. Simplicity**: new dependencies justified (purpose, maintenance, license).
- [ ] **VI. Observability**: structured logging and actionable user-facing errors.
- [ ] **VII. Localization**: all new UI strings have keys in English and translations
  for en/de/fr/es/ja/uk; plurals, locale formatting and CJK/long-German layout
  considered; docs and comments in English.
- [ ] **VIII. Rendering**: visible-range cost stays O(log N + W); indexed storage and
  zoomed-out summaries; dense activity visible; benchmark with recorded baseline.
- [ ] **IX. Decoding**: decoder faults contained and cancellable; annotations support
  lane and signal-overlay placement (points, edge markers, ranges); recorded-signal
  tests including malformed input; overlay lookup stays indexed.
- [ ] **X. Export**: decoded log export is documented, versioned, locale-independent,
  streaming, atomic and cancellable; errors/incomplete regions included.
- [ ] **XI. Virtual device**: every decoder usable with the virtual device has a
  generator, seeded and reproducible, with a round-trip test; simulated data is
  labelled in UI, captures and exports.
- [ ] **XII. Original design**: no Saleae/GPL code, assets, names or exact visuals;
  any license decision is recorded here.
- [ ] **Platforms**: works on Windows 10/11 and Debian 13; OS-specific code is isolated;
  device-access instructions (driver / udev) updated if drivers are touched.

## Project Structure

### Documentation (this feature)

```text
specs/[###-feature]/
├── plan.md              # This file (/speckit-plan command output)
├── research.md          # Phase 0 output (/speckit-plan command)
├── data-model.md        # Phase 1 output (/speckit-plan command)
├── quickstart.md        # Phase 1 output (/speckit-plan command)
├── contracts/           # Phase 1 output (/speckit-plan command)
└── tasks.md             # Phase 2 output (/speckit-tasks command - NOT created by /speckit-plan)
```

### Source Code (repository root)
<!--
  ACTION REQUIRED: Replace the placeholder tree below with the concrete layout
  for this feature. Delete unused options and expand the chosen structure with
  real paths (e.g., apps/admin, packages/something). The delivered plan must
  not include Option labels.
-->

```text
# Default: Rust workspace (adjust crate names to this feature; remove unused entries)
Cargo.toml                 # workspace
crates/
├── core/                  # signal storage, indexing, sessions, file formats (no UI deps)
├── devices/               # common device abstraction + virtual device
│   └── drivers/           # weact/, logic16/, pico/ (one module per device)
├── decoders/              # decoder framework, built-in decoders + generators
├── export/                # decoded-log and raw-data export
├── i18n/                  # localization loading, plural rules, locale formatting
└── ui/                    # UI shell (only crate allowed to depend on the UI toolkit)

resources/
└── i18n/                  # en (source of truth), de, fr, es, ja, uk

tests/
├── contract/              # host API contract tests
├── integration/
└── fixtures/              # recorded device transports and signals

benches/                   # rendering and ingestion benchmarks
```

**Structure Decision**: [Document the selected structure and reference the real
directories captured above]

## Complexity Tracking

> **Fill ONLY if Constitution Check has violations that must be justified**

| Violation | Why Needed | Simpler Alternative Rejected Because |
|-----------|------------|-------------------------------------|
| [e.g., 4th project] | [current need] | [why 3 projects insufficient] |
| [e.g., Repository pattern] | [specific problem] | [why direct DB access insufficient] |
