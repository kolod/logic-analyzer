# Implementation Plan: Single and Pair Markers

**Branch**: `015-markers-rework` | **Date**: 2026-11-18 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `/specs/015-markers-rework/spec.md`

## Summary

Replace the one kind of marker of feature 006 (two chosen of many for a measurement) with two kinds. A
**single marker** (`T1`, `T2`, ...) is one sample position; a **pair marker** (`P1`, `P2`, ...) is an
interval. Both are put in place with the mouse in a **placement mode** (the add command only starts it; a
dotted line in the marker's colour follows the pointer and snaps to the transitions of the lane under the
pointer; a click fixes it, a pair takes two clicks and the left half becomes the start), have a colour from
eight, a short comment and a delete action, and offer the actions of their kind: time origin, cut left, cut right
and scroll to for a single marker; trim, fit and split into two singles for a pair; two singles can be made
a pair (an action of one, then a click on the other). The logic goes to `la-host` (a new `markers` model, placement and snapping resolution, the actions;
host API 1.2 → 2.0, since commands are removed) and to `la-storage` (one new operation that builds a cropped
copy of a capture, run in the background because it is O(kept transitions)); the `VIEW` section payload goes
to version 2 (no reader for version 1: such a section is dropped with the existing warning); `la-app` keeps
geometry and drawing: the marks, the dotted and solid lines, the highlight of the snapped transition, the
Markers view and its menus. When the pointer rests in a crowded snapping zone the view zooms in on its own and
returns to the previous zoom and position when the marker is fixed or the placement ends (host: the target
and the restore; interface: the clock and the animation).

## Technical Context

**Language/Version**: Rust, stable; `la-app` needs 1.92 (the iced tree), the other crates keep MSRV 1.85.

**Primary Dependencies**: none new. `iced 0.14` (canvas, widgets, `iced_test`) as before.

**Storage**: the `VIEW` section of the session file gets payload version 2 (markers of both kinds, numbering
state, colours, comments, the time origin; the measured pair of version 1 is gone). One new function in
`la-storage` that builds a closed capture from a range of another (`crop`), on top of the existing
`CaptureRestorer`; nothing else about how data is stored changes.

**Testing**: `cargo test` — host tests without a display for every command and for the pure functions
(numbering, ordering, cuts, origin, conversions, snapping resolution), storage tests and property tests for
`crop` (levels at every position equal those of the source, shifted), a round-trip and malformed-input test
set for the new `VIEW` payload (the existing fuzz/property harness for `view_section`), the random-walk test
extended with the new commands, application tests for drawing and the panel (`iced_test`, headless), the
frame-plan benchmark with its baseline.

The compatibility fixtures of earlier session files (`crates/host/tests/fixtures`, `sessions_compat.rs`,
`make_compat_fixtures.rs`) hold `VIEW` version 1 sections: the expectation for them changes to "opens, no
markers, `host.view_invalid` warning", and the fixtures with markers are regenerated for version 2.

**Target Platform**: Windows 10/11 and Debian 13 (x86-64 both).

**Project Type**: desktop-app (Rust workspace: UI-independent core crates + UI shell).

**Performance Goals**: the frame plan keeps O(log N + W·(1 + log(1 + N_w/W))); markers add O(M) with
M ≤ 64 to it. Resolving the pointer to a position (snapping) is O(log N) per pointer move and never
iterates over the transitions between the pointer and the answer. A cut or trim of a capture of 100 million
samples runs in the background with progress and can be cancelled; the interface stays responsive (no frame
over 100 ms while it runs).

**Constraints**: no panic on any input (the `VIEW` payload, pointer positions, positions beyond the capture,
comments); positions are always `< capture length` (or 0 for an empty capture); at most 64 markers; a
cancelled or failed cut leaves the capture exactly as it was; memory stays within the capture's budget (the
cropped copy is built while the original is kept, so the budget is checked before the work, R6).

**Scale/Scope**: ≤ 64 markers, 8 colours, 2 kinds; about 40 new message keys in six languages; ~25 host
commands replacing 8.

## Constitution Check

*GATE: passed before Phase 0 research and re-checked after Phase 1 design.*

- [x] **I. Stability & Data Integrity**: the new inputs (the `VIEW` v2 payload, pointer positions, comments,
  ids in commands) are validated or clamped into typed results; a bad `VIEW` is dropped with the existing
  warning and never fails the open; the three data-changing actions (cut left, cut right, trim) run on a
  copy and replace the capture only when it is complete, so a failure, a cancel or a panic in the worker
  leaves the capture untouched; they ask for confirmation, are refused while a capture runs, and mark the
  capture unsaved (the unsaved-data rules apply to what follows); a save is still atomic.
- [x] **II. Hardware**: not affected (the simulated device and the drivers are used only through the
  existing abstraction; placement works the same on captures from any device).
- [x] **III. Test-First**: tests are ordered before each part in `tasks.md`; host tests per command, property
  tests for `crop`, fuzz-style malformed-input tests for the payload, extended random walk; fmt, clippy and
  test run in CI on both systems.
- [x] **IV. Architecture**: the rules (numbering, ordering, cuts, origin, snapping) live in `la-host` and
  `la-storage`; the host API contract changes to 2.0 and is documented, implemented and consumed in the
  same merge; the one long-running operation (crop) is background and cancellable; its memory is checked
  against the budget first.
- [x] **V. Simplicity**: no new dependency; the marks are drawn by the canvas (a path for the pentagon, a
  rectangle for the squares); no new storage structure; placement state is a small enum in `State`.
- [x] **VI. Observability**: new commands log by stable name (never the comment text); refusals use the
  stable error codes (`host.invalid_command`, a failed or cancelled crop is reported through the
  existing notice path with the storage error's code (`storage.cancelled`, `storage.memory_limit`, ...)); a dropped `VIEW` produces the existing `host.view_invalid` warning.
- [x] **VII. Localization**: about 40 keys in six languages; names `T`/`P` + number are not translated; the
  comment is user text; duration and frequency use the localizer; layout tests at the smallest panel width
  with long German and Japanese text.
- [x] **VIII. Rendering**: the frame plan gains the markers inside the window (O(M)); the cached layer is
  not invalidated by hover, the dotted placement line and the highlight live in the cheap overlay layer;
  the pointer resolution is O(log N) (no scan); the frame-plan benchmark gets a case with 64 markers and a
  recorded baseline.
- [x] **IX. Decoding**: decoders are untouched except that a cut or trim makes them run again on the
  shortened capture (the same path as a new capture); lanes of decoders take no snapping.
- [x] **X. Export**: not affected.
- [x] **XI. Virtual device**: not affected; the simulated flag survives a cut or trim and the label stays.
- [x] **XII. Original design & third-party code**: marks and interactions are our own design (a pentagon
  arrow and squares in the colour of the marker); no Saleae or sigrok code or asset is used; nothing is
  taken from another project.
- [x] **Platforms**: nothing OS-specific; tests run on both systems in CI.

## Project Structure

### Documentation (this feature)

```text
specs/015-markers-rework/
├── plan.md              # This file
├── research.md          # Phase 0: decisions R1–R10
├── data-model.md        # Phase 1: state, rules, pure functions, the VIEW v2 payload in outline
├── quickstart.md        # Phase 1: how to validate
├── contracts/
│   ├── host-api-2.0.md      # commands removed and added, state, queries, behaviour of each
│   ├── view-section-2.md    # the VIEW payload, version 2
│   └── ui-interaction.md    # marks, lines, placement, panel, menus, keys
├── checklists/requirements.md
└── tasks.md             # Phase 2 output (/speckit-tasks — not created by this command)
```

### Source Code (repository root)

```text
crates/storage/src/crop.rs                # new: crop(snapshot, start, end, progress) -> Result<Capture>
crates/storage/src/lib.rs                 # export it
crates/storage/tests/crop.rs              # property + boundary tests (levels, gaps, status flags, budget)

crates/host/src/
├── markers.rs                            # rewritten: Marker (Point | Interval), numbering, origin, selection,
│                                         #   pure rules (add, move, convert, cut/trim remap), Placement,
│                                         #   resolve_position (snapping), Measurement (kept)
├── marker_ops.rs                         # new: the commands' bodies and the background crop operation
├── view_section.rs                       # payload version 2 (read/write markers, numbering, origin)
├── command.rs                            # - PlaceMarker, MoveMarker, RenameMarker, SelectMarkers,
│                                         #   GoToMarker; RemoveAllMarkers stays; + the new set
├── state.rs                              # State.placement, State.origin via Markers; Activity::Cropping
├── plan.rs                               # MarkerPlan carries kind, number, colour, comment, both ends
├── axis.rs                               # build_ticks takes the origin (negative labels)
├── shell.rs                              # executes the commands, Shell::resolve_pointer, frame plan
├── shortcuts.rs                          # M, Shift+M, Esc; menu entries
├── sessions.rs                           # saves/restores markers and the origin; `unsaved` on cuts
└── error.rs                              # + cut notices

crates/i18n/…                             # (only if negative durations need a fix, research R7) ; resources/i18n/*/ui.ftl: ~40 keys × 6 languages

crates/app/src/
├── signal_view.rs                        # marks and lines, placement overlay, highlight, hit-testing, drag
├── marker_view.rs                        # new: the Markers view (buttons, rows, menus, colour palette)
├── app.rs                                # messages for the new commands, placement pointer messages
├── icons.rs                              # + marker icons for the two add buttons (pentagon, two squares)
└── theme.rs                              # + the eight marker colours for light/dark/high contrast

docs/host-api.md, docs/user-guide.md      # updated at implementation (version 2.0)
specs/006-ui-shell-signal-view/…          # already marked superseded (this feature's spec lists them)
```

**Structure Decision**: the rules and the state go to `la-host` because everything must be testable without
a screen (spec FR-026); the data copy goes to `la-storage` because only it knows how a capture is built;
the application only draws and forwards pointer events, as in feature 014.

## Complexity Tracking

| Item | Why needed | Simpler alternative rejected because |
|------|-----------|--------------------------------------|
| A background crop operation (new `Activity::Cropping`, progress, cancel) | A cut or trim copies the kept transitions: O(N_kept), seconds for 10⁸ samples; Principle IV forbids blocking the interface thread and requires cancellation | Doing it in `apply` would freeze the window on large captures; an in-place truncation of the snapshot is impossible (snapshots are immutable and shared with decoders and the view) |
| Host API **major** version (2.0) | Eight commands and the state field `selected: [Option<u32>; 2]` are removed | Keeping them would leave dead, contradictory commands (the old measured pair); there has been no release, so no consumer breaks |
| The dwell zoom (R4): a saved window in `State`, two commands (`PlacementZoomTo`, `EndMarkerDrag`) and a timer in the interface | At a zoom where one pixel holds many samples the pointer cannot be moved by less than a pixel, so without it some transitions would be out of reach (FR-032, FR-034, SC-008); it is the owner's chosen mechanism | Arrow-key stepping (the first proposal) is hidden and needs the keyboard; sub-pixel pointer motion is not given by the platforms; leaving it to the wheel contradicts the requirement |
