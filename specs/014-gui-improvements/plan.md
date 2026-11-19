# Implementation Plan: GUI Improvements

**Branch**: `014-gui-improvements` | **Date**: 2026-11-16 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `/specs/014-gui-improvements/spec.md`

## Summary

Rework the interface of feature 006: channels are edited in the legend strip (colour bar, name, height by
dragging a border, order by dragging a lane) and the Channels panel goes away; the right edge becomes one
panel with an icon strip and one view at a time (Capture, Decoders, Markers) that remembers its state; the
capture is set up in the Capture view (device, rate, length in seconds, channel buttons eight to a row);
a button at the end of a signal scrolls to the next edge, placing it a quarter of the view from the side it
came from; File → Open recent becomes a submenu of 15; the application and the session file get icons;
and Decoders → Add decoder and the D key work again. The logic that can be tested without a screen goes
into `la-host` (host API 1.1 → 1.2, an optional `LAYT` session section for the heights), two small indexed
queries go into `la-storage` (next and previous transition), and `la-app` keeps geometry and drawing,
with the lane layout as a pure, tested module.

## Technical Context

**Language/Version**: Rust, stable. `la-app` needs 1.92 (the iced tree), the other crates keep MSRV 1.85.

**Primary Dependencies**: no new runtime dependency. The interface keeps `iced 0.14` (canvas, widgets,
`iced_test` for headless rendering). One new **build** dependency, only on Windows: `winresource` (MIT,
maintained, small; purpose: put the application icon into the executable so Explorer and the task bar
show it; fallback if `cargo deny` or the MSRV job objects: leave the resource to packaging and keep the
runtime window icon). Icon rendering uses Python and Pillow in a documented script that is not part of
the build.

**Storage**: the session gains an optional section `LAYT` (channel heights); the settings gain an
optional `side_panel`; nothing else is stored differently.

**Testing**: `cargo test` (host and storage tests without a display, application unit tests for the
layout, headless rendering for what is drawn), the frame-plan benchmark with its baseline, compatibility
tests with files of earlier versions.

**Target Platform**: Windows 10/11 and Debian 13 (x86-64 both). Icons and the declarations for each.

**Project Type**: desktop-app (Rust workspace: UI-independent core crates + UI shell). This feature is
mostly the shell, with its logic in the core.

**Performance Goals**: the frame plan keeps its cost O(log N + W·(1 + log(1 + N_w/W))) with two extra
O(log N) lookups per shown channel; a click on an edge button brings the edge into view in under 100 ms
on a capture with a flat stretch of 100 million samples; drawing stays smooth with 32 lanes of automatic
or custom height (the cached layer is rebuilt only when plan, theme, language or layout change; the
hover buttons live in the cheap layer).

**Constraints**: no panic on any input (the settings, the `LAYT` payload, a path in the recent files, the
text typed for the length); the interface thread never blocks (the existence check of recent files runs
when the menu opens and is a handful of `metadata` calls); nothing changes in how a capture is stored or
decoded; files of earlier versions open unchanged and files of this one open in the version before it.

**Scale/Scope**: up to 32 channels; 15 recent files; 3 views; ~30 new message keys in six languages; a
new host API minor version with five commands.

## Constitution Check

*GATE: passed before Phase 0 research and re-checked after Phase 1 design.*

- [x] **I. Stability & Data Integrity**: the new inputs (the text of the length, `LAYT`, the settings, the
  paths of recent files, drag coordinates) are parsed or clamped into typed results; `LAYT` is validated
  and a bad one is dropped with a warning, never failing the open; the session is still written
  atomically; nothing new can lose a capture (the unsaved-data rules are untouched; opening a recent file
  goes through them).
- [x] **II. Hardware**: not affected; the Capture view talks to the device only through the existing
  configuration and discovery of the device abstraction.
- [x] **III. Test-First**: tests are planned before each part (tasks order them first): host tests for every
  command and the section, the layout unit tests, the rendering tests, the property tests of the seconds
  conversion and of the transition queries, the compatibility tests; fmt, clippy, test run in CI on both
  systems.
- [x] **IV. Architecture**: the decisions are in `la-host` and `la-storage` (UI-independent); the host API
  contract changes to 1.2 and is documented, implemented and consumed in the same merge; the lookups are
  cheap and run on the interface thread inside the frame plan (O(log N)), the existence check of files is
  bounded; nothing long-running is added.
- [x] **V. Simplicity**: no new runtime dependency; one Windows build dependency justified above with a
  fallback; the icons are drawn by the canvas and not by an icon font or an image library; the window
  icon is a raw RGBA array (no decoder).
- [x] **VI. Observability**: new refusals use the stable error codes (`host.invalid_command`,
  `host.layout_invalid`); the log never contains file names beyond what it already holds; a damaged
  `LAYT` is reported with a notice that says what to do (nothing: automatic heights are used).
- [x] **VII. Localization**: about 30 new keys in six languages; no ad-hoc plural (the sample count next to
  the seconds is a number formatted by the locale); layout tests for the minimum window size in every
  language including long German and Japanese; names of channels are user text and are not translated.
- [x] **VIII. Rendering**: the frame plan gains two O(log N) lookups per shown channel; the cached layer
  is not invalidated by hover; the benchmark of the plan gets the lookups with a recorded baseline and a
  regression of the complexity class blocks the merge.
- [x] **IX. Decoding**: not affected, except that decoder lanes keep their place in the layout (automatic
  height, not draggable) and the Add decoder entry works for any number of decoders.
- [x] **X. Export**: not affected.
- [x] **XI. Virtual device**: not affected; the Capture view keeps every setting of the simulated device
  (seed, density) and the simulated label.
- [x] **XII. Original design & third-party code**: the icons are original artwork, recorded as ours in
  `docs/third-party-notices.md`; the layout follows general ideas (a side bar of views, an editable legend)
  re-implemented independently, with no Saleae asset, name or exact layout; nothing is taken from VS Code
  except the idea of a view switcher, which is not code.
- [x] **Platforms**: tests run on both systems in CI; the icon, the desktop entry and the MIME type for
  Debian and the `.ico` and the resource for Windows are supplied; device-access documents are unchanged.

## Project Structure

### Documentation (this feature)

```text
specs/014-gui-improvements/
├── plan.md              # This file
├── research.md          # Phase 0: decisions R1–R14
├── data-model.md        # Phase 1: state, commands, pure functions, session
├── quickstart.md        # Phase 1: how to validate
├── contracts/
│   ├── host-api-1.2.md      # state, commands, pure functions, storage additions
│   ├── layt-section.md      # the optional session section for the heights
│   └── ui-interaction.md    # legend zones, side panel, edge buttons, menus
├── checklists/requirements.md
└── tasks.md             # Phase 2 output (/speckit-tasks — not created by this command)
```

### Source Code (repository root)

```text
crates/storage/src/query.rs              # + Snapshot::next_transition, previous_transition
crates/storage/tests/transition_search.rs

crates/host/src/
├── presentation.rs                      # + ChannelStyle.height, set_height, reset
├── layout_section.rs                    # new: the LAYT payload (encode, decode, validate)
├── command.rs                           # + SetChannelHeight, SetSidePanel, ScrollToEdge,
│                                        #   RemoveRecentFile, ClearRecentFiles
├── state.rs                             # + side_panel
├── settings.rs                          # + side_panel, MAX_RECENT_FILES = 15
├── sessions.rs                          # + LAYT in save and open, recent file rules
├── plan.rs                              # + edge_before, edge_after
├── window.rs                            # + the placement of ScrollToEdge
├── config.rs (or devices.rs)            # + samples_for_seconds, seconds_for_samples
├── shell.rs, lib.rs                     # the commands, HOST_API_VERSION = "1.2"
└── shortcuts.rs                         # the D key and the Add decoder entry
crates/host/tests/                       # layout_section.rs, side_panel.rs, scroll_to_edge.rs,
                                         # recent_files.rs, seconds.rs, sessions_compat.rs,
                                         # random_walk.rs (new commands), host_api_doc.rs
crates/host/benches/frame_plan.rs        # + the edge lookups, baseline

crates/app/src/
├── app.rs                               # view(): row [signal, panel, strip]; capture_panel; menus
│                                        #   with submenus; channel_panel removed; the editor overlay
├── lane_layout.rs                       # new: rectangles, zones, scrolling, landing (pure)
├── signal_view.rs                       # uses lane_layout; legend zones; drag and resize;
│                                        #   edge buttons in the overlay layer
├── side_panel.rs                        # new: the icon strip and the panel frame
├── icons.rs                             # new: the vector icons drawn by the canvas
├── theme.rs                             # + channel button styles, the contrast function
├── main.rs                              # the window icon
├── build.rs                             # Windows: the executable icon (winresource)
└── assets/icon/                         # app.svg, session.svg, app.ico, png sizes, app-64.rgba

packaging/
├── linux/logic-analyzer.desktop
├── linux/logic-analyzer-session.xml
└── windows/README.md

tools/make_icons.py                      # renders the icons (not part of the build)

resources/i18n/{en,de,fr,es,ja,uk}/ui.ftl   # the new keys
docs/host-api.md                         # version 1.2 and the command table
docs/user-guide.md                       # the new interface
docs/third-party-notices.md              # the icons are ours
roadmap.md                               # 014 merged
```

**Structure Decision**: no new crate. The testable decisions are in `la-host` so a second shell would get
them for free; the storage queries live beside the existing `transitions` on the same index; the
interface adds three small modules (`lane_layout`, `side_panel`, `icons`) so that `app.rs`, already over
3,000 lines, does not grow by the whole feature, and the layout is a pure function that tests can
call.

## Complexity Tracking

| Violation | Why Needed | Simpler Alternative Rejected Because |
|-----------|------------|-------------------------------------|
| A new build dependency (`winresource`), Windows only | Windows shows the icon of an executable from a resource inside it; the window icon alone does not give Explorer or the task-bar pin the icon | Leaving it to the installer (016+) leaves a developer build without an icon in Explorer; the fallback is exactly that, taken if the dependency is refused by `cargo deny` or the MSRV job |
| A new optional session section (`LAYT`) instead of extending `VIEW` | A reader without this feature must still open names, colours and order | `VIEW` version 2 would be unreadable to the version before (it is treated as newer and ignored), so a user who goes back would lose names and colours |
| Two new queries in `la-storage` | The next and the previous edge in O(log N); forward-only `transitions` cannot find the previous edge without a scan | A scan from the start is O(N) in the capture, which Principle VIII forbids |
