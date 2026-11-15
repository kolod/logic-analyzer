# Implementation Plan: UI Shell and Signal View

**Branch**: `006-ui-shell-signal-view` | **Date**: 2026-11-14 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `/specs/006-ui-shell-signal-view/spec.md`

## Summary

The first runnable application. The UI toolkit is **iced 0.14** (research R1, chosen against egui
and Slint on text support, architecture and footprint). All behaviour lives in a new UI-free crate,
`la-host`: the shell state, the commands, the unsaved-data guard, zoom/pan/marker/presentation logic,
the **frame plan** (the signal view reduced to a few segments per channel through the storage's
indexed viewport lookup), the settings file and the localized notices. A thin binary crate, `la-app`,
draws the plan on an iced canvas, turns input into commands and hosts native dialogs. The contract
between them is the versioned host API (`docs/host-api.md`, design in `contracts/host-api.md`),
which closes the constitution's `TODO(HOST_API_CONTRACT)`. Channel presentation and markers travel
in a new optional `VIEW` session section (file format 1.1). The UI font set is bundled (subsets of
OFL fonts) so Japanese and Cyrillic render on a bare Debian, with system fallback for user text.
Almost everything is verifiable headlessly on both CI platforms; only windowed frame timing, IME input
and a few visual checks are manual and recorded.

## Technical Context

**Language/Version**: Rust 2024 edition; core crates keep MSRV 1.85; `la-app` declares its own
higher `rust-version` (1.92 at plan time); toolchain pinned by `rust-toolchain.toml` (1.100.0)

**Primary Dependencies**: `iced` 0.14 (canvas, wgpu with tiny-skia fallback), `rfd` (native file
dialogs), `serde` + `serde_json` (settings), `directories` (per-user paths); existing `la-core`,
`la-diagnostics`, `la-storage`, `la-devices`, `la-session`, `la-i18n`, `tracing`. All MIT or
Apache-2.0; `deny.toml` gains `BSL-1.0` (research R13)

**Storage**: settings file in the per-user configuration directory (atomic write); session files via
`la-session` with the new optional `VIEW` section; fonts compiled into the binary

**Testing**: `cargo test` (unit, scenario, property-style and mutation tests in `la-host`; headless
view tests in `la-app` where the spike confirms feasibility), a 100,000-action random walk, a save
interruption sweep through `FaultFs`, a criterion benchmark for the frame plan, manual windowed
checks recorded in `verification.md`

**Target Platform**: Windows 10/11 and Debian 13 (x86_64); GPU renderer with software fallback

**Project Type**: desktop application; adds `crates/host` (library) and `crates/app` (binary) to the
workspace, `resources/i18n/<lang>/ui.ftl`, `docs/host-api.md`, bundled font subsets

**Performance Goals**: frame time p95 < 16 ms, worst < 50 ms on 100 M samples × 16 channels,
independent of capture length (SC-002); no input stall > 100 ms (SC-004); language switch repaint
< 200 ms (SC-009); idle CPU < 1 % of one core (SC-010)

**Constraints**: no panic on any user action, device event or file (workspace lints apply to both
new crates); rendering bound of Principle VIII; memory bounded by the visible limit; the UI never
blocks on work over 100 ms; no text in code; no signal data in logs

**Scale/Scope**: up to 32 channels, captures to the storage limit (hundreds of millions of samples
for the benchmark), six languages, about 150 new UI messages per language, 7 new error codes

## Constitution Check

*GATE: passes before Phase 0 research; re-checked after Phase 1 design (still passes).*

- [x] **I. Stability & Data Integrity**: commands run inside a panic boundary on a working copy of
  the state (R9); the unsaved-data guard is a host state machine that cannot be skipped by the UI
  (SC-011); saves are the atomic operations of feature 003; a damaged file or a bad `VIEW` section
  never crashes or blocks opening (R7); a failing background task is contained and reported; the
  capture on screen is immutable data (`Snapshot`), so a failure elsewhere cannot corrupt it.
  Workspace lints (no unwrap, indexing, unchecked arithmetic) apply to both new crates.
- [x] **II. Hardware**: the UI uses only the device abstraction and the registry (FR-002); no device
  logic in `la-host` or `la-app`; drivers stay separate roadmap items.
- [x] **III. Test-First**: expected tables for zoom/pan, axis ticks, marker maths and the `VIEW`
  layout are written first; the frame plan is checked against a brute-force reference; fuzzers for
  the `VIEW` codec and the action walk; every error code swept; CI runs fmt, clippy and tests on both
  OSes.
- [x] **IV. Architecture**: domain logic and view model are in UI-free crates; the UI talks to the
  core only through the documented, versioned host API (location decided here: `docs/host-api.md`);
  long operations are the existing background operations, polled by the shell; memory bounded by the
  visible limit. *Constitution text still lists `TODO(HOST_API_CONTRACT)`; amending it needs the
  user's approval (Complexity Tracking).*
- [x] **V. Simplicity**: four new third-party dependencies, each justified in research (iced R1, rfd
  R6, serde/serde_json and directories R8); no async runtime of our own, no plugin system, no custom
  GPU code unless the canvas misses its budget (R4 ordered fallbacks); the `la-host`/`la-app` split
  is the minimum that makes the shell testable.
- [x] **VI. Observability**: commands, lifecycle events and every notice are logged with codes; the
  log location is reachable from the UI; messages carry what failed and what to do; no signal data
  is logged.
- [x] **VII. Localization**: every string from `ui.ftl` through `la-i18n`; runtime switch with
  immediate effect; Japanese rendered with a bundled font; long German handled by measured layout and
  a text-fit check; user text isolated and never translated; numbers, sizes and frequencies through
  the localizer; parity check in CI covers the new file.
- [x] **VIII. Rendering**: the view uses `Snapshot::viewport` and its summaries only; the frame-plan
  benchmark with a recorded baseline guards the complexity class; dense activity is drawn
  distinctly and zoom reveals every transition (SC-003).
- [x] **IX. Decoding**: not implemented; the decoder-lane region and the registration point are
  reserved (FR-037).
- [x] **X. Export**: not implemented; the formatter used for the screen is not used for exports.
- [x] **XI. Virtual device**: the simulated device is the default and only device in this feature;
  "simulated" is shown in the status area, a view badge, and the saved file.
- [x] **XII. Original design & third-party code**: the layout and palette are original; no Saleae or
  PulseView code or assets; fonts are OFL data with their licence files recorded in the third-party
  notices; libraries are used as dependencies only.
- [x] **Platforms**: Windows 10/11 and Debian 13; OS-specific code limited to dependencies (`winit`,
  `rfd`) and the per-user directory lookup.

## Project Structure

### Documentation (this feature)

```text
specs/006-ui-shell-signal-view/
├── plan.md              # This file
├── research.md          # Phase 0: decisions (toolkit, architecture, rendering, fonts, testing)
├── data-model.md        # Phase 1: state, frame plan, presentation, markers, notices, settings
├── quickstart.md        # Phase 1: automated and manual validation
├── contracts/
│   ├── host-api.md      # commands, state, events, rules (version 1.0)
│   ├── view-section.md  # the VIEW session section (format 1.1)
│   └── shell-layout.md  # regions, commands, shortcuts, dialogs, layout rules
├── checklists/requirements.md
└── tasks.md             # Phase 2 (/speckit-tasks, not created here)
```

### Source Code (repository root)

```text
crates/host/                         # NEW crate la-host (UI-free, MSRV 1.85)
├── Cargo.toml
├── src/
│   ├── lib.rs             # public API, HOST_API_VERSION, #![deny(missing_docs)]
│   ├── shell.rs           # Shell: new, apply, poll, state, frame_plan, text
│   ├── command.rs         # Command, Outcome, Changes, Decision
│   ├── state.rs           # State, CaptureView, Activity, Unsaved, Versions
│   ├── guard.rs           # unsaved-data guard (deferred guarded commands)
│   ├── window.rs          # ViewWindow, zoom/pan/fit with checked arithmetic
│   ├── axis.rs            # tick step selection, tick labels and precision
│   ├── plan.rs            # FramePlan, Segment, building from Snapshot::viewport
│   ├── presentation.rs    # channel names, colours, order, visibility, palette
│   ├── markers.rs         # markers, snapping, measurement
│   ├── notices.rs         # Notice, grouping, bounded list, text through la-i18n
│   ├── devices.rs         # device list, config draft, validation problems
│   ├── capture.rs         # starting, polling and ending a capture
│   ├── sessions.rs        # save/load operations, recent files, VIEW integration
│   ├── view_section.rs    # VIEW codec (contracts/view-section.md)
│   ├── settings.rs        # settings model, validation, atomic file
│   ├── sanitize.rs        # display sanitizing of user text
│   ├── shortcuts.rs       # the command/menu/shortcut table
│   └── error.rs           # host.* error codes
├── tests/                 # scenarios per user story, view_section, settings, shortcuts,
│                          #   host_api_doc, random_walk (+ ignored long variants), interruptions
└── benches/frame_plan.rs + baseline.json
crates/app/                          # NEW crate la-app (binary `logic-analyzer`, iced)
├── Cargo.toml                       # own rust-version
├── src/
│   ├── main.rs            # logging, panic hook, localizer, iced application
│   ├── app.rs             # State of the iced app, Message, update, subscription
│   ├── signal_view.rs     # canvas::Program drawing a FramePlan, input to commands
│   ├── views/             # menu bar, toolbar, side panel, status bar, banner, dialogs
│   ├── theme.rs           # palette, light/dark/high contrast, original visual identity
│   ├── fonts.rs           # embedded subsets, registration
│   ├── dialogs.rs         # rfd file dialogs as tasks
│   └── frame_log.rs       # LA_FRAME_STATS frame-time log
├── assets/fonts/          # OFL subsets + licence files, build script in tools/
└── tests/                 # headless view and text-fit checks (if the spike allows)
crates/session/            # MODIFIED: writer emits minor version 1, tests for an extra VIEW section
crates/i18n/               # MODIFIED: ui.ftl added to the embedded resource list
resources/i18n/<lang>/ui.ftl         # NEW, six languages; errors.ftl gains the host.* codes
docs/host-api.md                     # NEW, authoritative host API contract
docs/third-party-notices.md          # NEW or extended: fonts and libraries
tools/build-font-subsets.*           # documented, reproducible subsetting script
.github/workflows/ci.yml             # MODIFIED: msrv excludes la-app, deny allows BSL-1.0
deny.toml                            # MODIFIED: BSL-1.0
```

**Structure Decision**: two crates because the shell has to be testable without a window and the
constitution requires the UI to reach the core only through a documented contract: `la-host` is the
core side of that contract and `la-app` the first consumer. Modules inside `la-host` follow the
state/command/plan split of the data model so each can be tested alone; none is a crate of its own
until it grows.

## Complexity Tracking

| Item | Why needed | Simpler alternative rejected because |
|------|------------|--------------------------------------|
| A second MSRV (`la-app` > 1.85) | every candidate toolkit needs Rust 1.92 or newer (research R1, R12) | holding the whole workspace at 1.85 would exclude all candidate toolkits; raising everyone to 1.92 weakens the library MSRV for no benefit |
| `BSL-1.0` added to the licence allow-list | two Windows-only clipboard crates in the iced tree use the Boost licence | removing clipboard support loses text copy in input fields; Boost is permissive and GPL-3.0 compatible |
| Constitution follow-up (not edited here) | `TODO(UI_TOOLKIT_AND_L10N_FORMAT)` is now fully resolved (format in 005, toolkit here) and `TODO(HOST_API_CONTRACT)` has its location; the "format chosen in the first plan" wording is stale | amending the constitution needs the user's approval and a version bump; proposed as a separate change after merge |
| Bundled font subsets (committed binary assets) | Principle VII requires Japanese and Cyrillic to render on a machine without suitable system fonts | relying on system fonts alone fails on a minimal Debian |
| A format minor bump (1.1) in `la-session` | channel presentation and markers must travel with the file (FR-018/020) | a sidecar file can be separated from the session; a critical-section change would need a major version |
