---

description: "Task list for GUI Improvements"
---

# Tasks: GUI Improvements

**Input**: Design documents from `/specs/014-gui-improvements/`

**Prerequisites**: plan.md, spec.md, research.md (R1–R14), data-model.md, contracts/
(`host-api-1.2.md`, `layt-section.md`, `ui-interaction.md`), quickstart.md

**Tests**: REQUIRED (constitution Principle III). In every phase the test tasks come first and MUST fail
(usually they do not compile yet) before the implementation tasks start. If a test passes at once
because earlier work already covers it, run a mutation check (temporarily break the line it guards, see
the test fail, restore) before relying on it. Expected values (bytes of `LAYT`, rectangles, window
starts, texts) are written **by hand** from the contracts, never computed by the code under test.

**Organization**: tasks are grouped by user story (US1–US7, numbered as in spec.md). They run in this
order: Setup, Foundational, **US1** (the legend, P1), **US2** (the side panel, P1), **US3** (the Capture
view, P2; it removes the Channels panel), **US4** (quick scroll, P2), **US7** (Add decoder, P2; it
builds the submenu mechanism), **US5** (recent files, P3; it uses that mechanism), **US6** (icons, P3),
Polish.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: can run in parallel (different files, no dependency on an incomplete task)
- **[Story]**: user story from spec.md
- All paths are relative to the repository root.

## Path Conventions

Core in `crates/host/src/` and `crates/storage/src/`, their tests in `crates/host/tests/` and
`crates/storage/tests/`; interface in `crates/app/src/` (tests in the same files under `#[cfg(test)]`);
strings in `resources/i18n/<lang>/ui.ftl` and `errors.ftl`; documents in `docs/`; icons in
`crates/app/assets/icon/`, declarations in `packaging/`. "The host contract" is
`specs/014-gui-improvements/contracts/host-api-1.2.md`, "the section contract" `contracts/layt-section.md`,
"the interaction contract" `contracts/ui-interaction.md`; decisions are `research.md` R1–R14.

## Lint reminder (applies to every Rust task)

The workspace denies `unwrap`, `expect`, indexing (`v[i]`), unchecked arithmetic, `panic!` and `unsafe`
in production code of `la-host`, `la-storage` and `la-app`; use `get`, `checked_*`, `saturating_*`, `u128`
intermediates and `?`. Test code may relax lints with the file-level `#![allow(...)]` used by the other
tests. `la-host` and `la-storage` keep **MSRV 1.85** (no `let` chains: check with `cargo +1.85 check -p
la-host -p la-storage`); `la-app` needs 1.92. Never commit while clippy fails: a helper that only a later
task uses gets `#[expect(dead_code, reason = "used by T0xx")]`, removed by that task. Every UI string
comes from `ui.ftl` or `errors.ftl` (English first, then the five translations as drafts, **in the task
that first uses the key**, so the parity check "no missing, no unused key" is green at every commit);
keys are string **literals** in the code. Run `cargo fmt` before the next edit. Windows scripts that
patch files: write them to a file, not a shell heredoc (apostrophes break it). Commits are signed; if
signing fails, ask the owner to unlock GPG, never bypass it.

---

## Phase 1: Setup

- [X] T001 Create the empty modules with their doc comments and declarations: `crates/app/src/lane_layout.rs`,
  `crates/app/src/side_panel.rs`, `crates/app/src/icons.rs` (declared in `crates/app/src/main.rs`) and
  `crates/host/src/layout_section.rs` (declared in `crates/host/src/lib.rs`). Nothing uses them yet; the
  workspace builds and clippy is clean.
- [X] T002 Keep what earlier versions wrote, **before** anything changes: add an `#[ignore]`d test
  `crates/host/tests/make_compat_fixtures.rs` that saves two session files with the current code (a
  capture from the virtual device with names, colours, a marker and a saved window, as feature 006 and 007
  write them; and one with an I2C decoder with saved results, as feature 008 writes it) into
  `crates/host/tests/fixtures/`, run it once, commit the two `.lasession` files, and leave the test in
  place with a comment saying it is not to be re-run (re-running would make the fixtures new). T0xx
  `sessions_compat` (Polish) reads them.

---

## Phase 2: Foundational

**Purpose**: the version, the types and the commands that every story's tests refer to.

- [X] T003 `HOST_API_VERSION = "1.2"` in `crates/host/src/lib.rs` and `Version: 1.2` in `docs/host-api.md`
  (the test `host_api_doc.rs` keeps them in step).
- [X] T004 Add the types and the commands of the host contract with no behaviour yet: `SidePanel` (`Capture`,
  `Decoders`, `Markers`, `Hidden`) and `EdgeDirection` (`Previous`, `Next`) in `crates/host/src/state.rs`
  (re-exported from `lib.rs`), and the commands `SetChannelHeight { channel, height }`, `SetSidePanel(SidePanel)`,
  `ScrollToEdge { channel, direction }`, `RemoveRecentFile(PathBuf)` and `ClearRecentFiles` in
  `crates/host/src/command.rs` with their `name()` entries; until their tasks they are answered with
  `Outcome::Ignored(host.invalid_command)`. Add a row for each to the command table of `docs/host-api.md`
  with a one-line meaning (the document test compares the table with the code in both directions).
  `State` gains `side_panel: SidePanel` (default `Capture`).

**Checkpoint**: the workspace builds; `cargo test -p la-host` is green.

---

## Phase 3: User Story 1 - Edit a channel where it is shown (Priority: P1) 🎯 MVP

**Goal**: rename, recolour, resize and move channels in the legend strip; heights are saved.

**Independent Test**: with a capture on screen, rename a channel, change its colour twice, make one signal
taller and another lower, move the first to the bottom, save, open: everything is back.

### Tests for User Story 1 (write first; they must fail)

- [X] T005 [P] [US1] `crates/host/tests/channel_height.rs`: `SetChannelHeight` for a shown channel stores the
  height (`ChannelStyle.height`) and answers `Done` with the presentation change; a height of 0 means
  automatic; 10 is clamped to 24 and 999 to 240; an unknown channel is `Ignored(host.invalid_command)`; it works
  with no capture (the defaults for the device's channels), while a capture runs, and after `MoveChannel` (the
  height follows its channel); `ResetPresentation` sets every height to 0; heights are kept for the next
  capture (the same rule as names and colours); the frame plan carries the height of each channel
  (`ChannelView.height`).
- [X] T006 [P] [US1] `crates/host/tests/layout_section.rs`: the `LAYT` payload for channels 1 with 40 and 31 with
  240 is exactly `01 02 01 28 1f f0 01` (version, count, channel, height varints) written by hand; the writer is
  deterministic; a session with no custom height has no `LAYT` section; every rejection of the section contract
  (version 2 is `Newer`, non-canonical varint, count 33, channel not below the channel count, repeated or
  descending channel, height 23 and 241, trailing byte) gives `host.layout_invalid` through the loader, a
  warning notice, automatic heights and a dropped section; a newer version is kept untouched with an Info notice;
  round trip: save with heights, open, the same heights; an old file (the fixtures of T002) opens with automatic
  heights and no notice; a `LAYT` section in an extras list survives a save by a reader that does not know it
  (the extras helper of `decoder_sessions.rs`); a property test with random bytes (nothing panics; an accepted
  payload re-encodes to the same bytes).
- [X] T007 [P] [US1] The unit tests of `crates/app/src/lane_layout.rs` (written first in its `#[cfg(test)]` module,
  by hand): with channels of heights [automatic, 60, 40] in an area of 300 px, the rectangles of the lanes, the
  zones of each (colour bar `x 0..8`, name `x 8..112`, border `y+h-3 .. y+h+4`), the hit test at the corners of
  every zone and one pixel outside, a vertical offset that scrolls the lanes and keeps the axis, the total height
  and whether a bar is needed, the landing index of a dragged lane for pointer positions above the first, between
  two, and below the last lane, the resize clamp to `24..=240`, decoder lanes without colour bar, name or border
  zones and not draggable, and the automatic height rule of feature 006 for lanes with height 0 (the same values
  as `lane_height` gives today).
- [X] T008 [P] [US1] A headless rendering test in `crates/app/src/app.rs` tests: a capture with heights set, the legend
  with the in-place editor open over a name, in English and Ukrainian, at the minimum window size: renders
  without panic and (with `LA_SNAPSHOT_DIR`) writes the pictures.

### Implementation for User Story 1

- [X] T009 [US1] `crates/host/src/presentation.rs`: `ChannelStyle.height: u16` (default 0), `Presentation::set_height` (0 or
  clamp into `24..=240`; false for an unknown channel), `defaults` and `resize` set 0, `reset` sets 0, and
  `ChannelView.height` in `crates/host/src/plan.rs` from the style. T005 (state parts) passes.
- [X] T010 [US1] `crates/host/src/layout_section.rs` per the section contract: `TAG = "LAYT"`, `VERSION = 1`,
  `encode(&Presentation) -> Option<Vec<u8>>` (None when no height is custom), `decode(&[u8], channels) ->
  Result<Decoded>` with `Decoded::Heights(Vec<(u8, u16)>)` and `Decoded::Newer`, canonical varints (reuse the
  helpers of `view_section.rs`), `host.layout_invalid` in `crates/host/src/error.rs` with its constructor. T006 (format
  parts) passes.
- [X] T011 [US1] `crates/host/src/sessions.rs`: write `LAYT` when a height is custom and remove it otherwise; read it when
  a session opens (invalid: warning notice `host.layout_invalid`, section dropped; newer: keep and Info notice
  `notice-layout-newer`); add the message of `host.layout_invalid` to `errors.ftl` and `notice-layout-newer` to
  `ui.ftl` in six languages (the `error_sweep.rs` test finds a missing one). T006 passes.
- [X] T012 [US1] `crates/host/src/shell.rs`: the command `SetChannelHeight` (the presentation change flag, allowed at any time);
  `ResetPresentation` also resets heights; the random walk (`crates/host/tests/random_walk.rs`) draws
  `SetChannelHeight` with heights from 0 to 300. T005 passes.
- [X] T013 [US1] `crates/app/src/lane_layout.rs` per research R4 and the interaction contract: `Layout::new(plan_lanes, area_height,
  offset)`, `rect(index)`, `zone_at(point)`, `landing(y)`, `clamp_height`, `total_height`, with the automatic rule
  moved here from `signal_view::lane_height`. T007 passes.
- [X] T014 [US1] `crates/app/src/signal_view.rs`: draw and hit-test through `lane_layout` (variable heights, the vertical
  offset, the thin bar when the lanes are taller than the area); the legend zones: a click on the colour bar
  sends `Message::Run(Command::SetChannelColour { channel, colour: next palette entry })` (a custom colour goes to
  palette 0), a press and release on the name sends `Message::EditChannelName(channel)`, a movement over 4 px
  from a press on a name starts a drag (a translucent lifted copy and a landing line; release sends `MoveChannel`;
  Escape cancels), a press on a border resizes the lane above live and sends `SetChannelHeight` on release, a
  double click on a border sends height 0, the wheel over the legend scrolls the offset while over the waveforms
  it still zooms; the cached layer is rebuilt when the layout changes.
- [X] T015 [US1] `crates/app/src/app.rs`: the in-place editor: `Message::EditChannelName`, `ChannelNameInput`,
  `ChannelNameAccept`, `ChannelNameCancel`; a `text_input` placed over the name at the rectangle of
  `lane_layout` on a layer above the canvas (select all on open; Enter accepts; Escape cancels; a click
  elsewhere accepts when the text changed and cancels when it did not) that sends `RenameChannel`. The Channels
  panel stays until T0xx of US3 (the flags have no other home yet). T008 passes.

**Checkpoint**: US1 works; `cargo test -p la-storage -p la-host -p la-app` green.

---

## Phase 4: User Story 2 - One panel on the right, one view at a time (Priority: P1)

**Goal**: one right panel with an icon strip; Capture, Decoders and Markers as views; the state is kept.

**Independent Test**: click each icon, check that one view shows each time, that the shown icon hides the
panel, and that the choice returns after a restart.

### Tests for User Story 2 (write first; they must fail)

- [X] T016 [P] [US2] `crates/host/tests/side_panel.rs`: the default is `Capture`; `SetSidePanel` sets each of the
  four values (no capture needed) and answers `Done`; the choice is saved in the settings (`side_panel` as
  `"capture"`, `"decoders"`, `"markers"`, `"hidden"`) and a new `Shell` on the same directory starts with it;
  an unknown or wrong-typed value reads as the default and does not stop the application; a settings file with
  an extra field from a newer version keeps it (the existing flatten test pattern of `settings.rs`).
- [X] T017 [P] [US2] A test in `crates/app/src/app.rs` for the pure part of the strip, in `side_panel.rs`'s own tests:
  `next_state(current, clicked) -> SidePanel` is `clicked` unless `clicked == current`, then `Hidden`; the
  rectangles of the three icons (28 px buttons in a 36 px strip, top to bottom) and which one is active.
- [X] T018 [P] [US2] Headless rendering tests in `crates/app/src/app.rs`: the application in each of the four panel
  states, in English, at the minimum window size and at 1180x720; no clipping of the strip; exactly one view's
  heading is present when a view is shown and none when hidden.

### Implementation for User Story 2

- [X] T019 [US2] `crates/host/src/settings.rs`: `side_panel: Option<String>` with the conversion to and from `SidePanel`
  (unknown gives the default); `crates/host/src/shell.rs`: the command `SetSidePanel` (sets `State.side_panel`,
  saves the settings) and the initial value from the settings. T016 passes.
- [X] T020 [US2] `crates/app/src/icons.rs`: vector icons drawn with the canvas: a circle with a dot (Capture), a stack of
  three boxes (Decoders), a flag (Markers), each in the text colour of the theme, 20 px in a 28 px button; unit
  tests that every icon draws within its box (the path bounds).
- [X] T021 [US2] `crates/app/src/side_panel.rs`: the strip (three buttons with a 3 px bar on the left of the shown one,
  tooltips from the keys `panel-view-capture`, `panel-view-decoders`, `panel-view-markers`) and the frame of the
  panel (280 px, minimum 220); `next_state`. T017 passes.
- [X] T022 [US2] `crates/app/src/app.rs`: `view()` becomes `row![signal, panel?, strip]`; the panel shows the content
  of the current view: the markers panel (`markers_panel()`), the decoders panel (`decoders_panel()`) or a new
  `capture_panel()` that for now **moves the controls out of `toolbar()` unchanged** (device, rate, length
  in samples, seed, density, real time; the toolbar keeps Start, Stop and the explanations) ; the right-hand
  column of the three stacked panels is removed from `view()` (the Channels panel is removed in US3, so for
  now it is shown at the bottom of the Capture view); `Message::SidePanel(SidePanel)` sends the command; add
  the six languages for the three tooltip keys and the heading of the Capture view. T018 passes.

**Checkpoint**: US1 and US2 work; the signals take the width the second panel had.

---

## Phase 5: User Story 3 - Set up a capture in the Capture view (Priority: P2)

**Goal**: length in seconds and channel buttons in the Capture view; the Channels panel goes away.

**Independent Test**: 2.5 s at 1 MHz makes 2,500,000 samples; the buttons choose the channels recorded.

### Tests for User Story 3 (write first; they must fail)

- [ ] T023 [P] [US3] `crates/host/tests/seconds.rs`: `samples_for_seconds` by hand: (1_000_000, "2.5") = 2_500_000, (1_000_000,
  "1") = 1_000_000, (1_000, "0.0004") is refused (less than a sample is zero), (3, "0.5") = 2 (1.5 rounds up),
  (3, "0.1666666667") = 1, (1_000_000_000, "0.000000001") = 1, (u64::MAX, "2") is refused (too large), rate 0 is
  refused, "", " ", "-1", "1e3", "abc", "1,5", "1.", ".5" (accepted as 0.5? the contract says digits on both sides of
  the dot are optional only for the integer part: ".5" is refused, "1." is refused), 10 decimals is refused,
  a 5,000-character string is refused without allocating; `seconds_for_samples` by hand: (1_000_000, 2_500_000) =
  "2.5", (1_000_000, 1) = "0.000001", (1_000, 1) = "0.001", (3, 1) = "0.333333333", (1, 7) = "7", 0 samples is
  "0"; a property test (an LCG) that for rates that divide 10^9 the round trip is exact and for others within
  one sample.
- [ ] T024 [P] [US3] A unit test in `crates/app/src/theme.rs` for `channel_button_text(colour_background) -> black or white`:
  for every palette entry in both themes the chosen text colour has a contrast ratio of at least 3 : 1 on the
  background (WCAG relative luminance, written in the test by hand), and for pure black and pure white
  backgrounds the answers are white and black.
- [ ] T025 [P] [US3] Headless rendering tests in `crates/app/src/app.rs`: the Capture view with a device of 8, 16 and 32
  channels (one, two and four rows of buttons), with channels on and off, in English, German and Japanese at the
  minimum window size without clipping; the length field with seconds and the samples beside it; a capture
  running (buttons inert).
- [ ] T026 [P] [US3] An application test (no display) for the state of the length: typing "2.5" with the rate 1,000,000
  sends `ConfigEdit::Length(2_500_000)`; changing the rate to 2,000,000 keeps "2.5" in the field and sends
  `ConfigEdit::Length(5_000_000)`; an invalid text sends nothing and shows the reason; the samples beside the
  field follow.

### Implementation for User Story 3

- [ ] T027 [US3] `crates/host/src/devices.rs` (or a new `config.rs` declared in `lib.rs`): `samples_for_seconds`,
  `SecondsError`, `seconds_for_samples` per the data model, integer arithmetic only. T023 passes.
- [ ] T028 [US3] `crates/app/src/theme.rs`: `channel_button_style(colour, on, theme) -> button::Style` (off: text and border in
  the channel colour on the panel background; on: border and background in the channel colour, text by
  `channel_button_text`), and `channel_button_text`. T024 passes.
- [ ] T029 [US3] `crates/app/src/app.rs`: `capture_panel()` is finished: the length is entered in seconds with the
  samples beside it (`length_seconds_text` state, `Message::LengthSecondsText`), the rate change recomputes the
  samples from the seconds, the channel buttons (eight per row, 32 px, the channel number inside, the name as
  tooltip, `SetChannelVisible`, inert while a capture runs), "Reset channels" (`ResetPresentation`) below them,
  the reason Start is unavailable when no channel is on (the key `capture-no-channel`), the explanations of
  device problems; keys `capture-length-seconds`, `capture-length-samples`, `capture-channels`,
  `capture-reset-channels` (reuse the existing one if it exists) in six languages. T025 and T026 pass.
- [ ] T030 [US3] Remove the Channels panel: delete `channel_panel()` and its messages (`ChannelName`, `ChannelVisible`,
  `ChannelColour`, `ChannelMove`) and the keys that only it used from the six `ui.ftl` files (the parity test
  finds an unused key); update the tests that used them (`app.rs` tests, `docs`). `cargo test -p la-app` passes.

**Checkpoint**: US1, US2 and US3 work; the Channels panel is gone and nothing it did is lost.

---

## Phase 6: User Story 4 - Quick scroll to the next edge (Priority: P2)

**Goal**: a button at the end of a signal scrolls to the next edge, a quarter of the view from the side.

**Independent Test**: on a capture with a long flat stretch, one click puts the next edge at a quarter of
the view from the left with the zoom unchanged; the same to the left.

### Tests for User Story 4 (write first; they must fail)

- [ ] T031 [P] [US4] `crates/storage/tests/transition_search.rs`: `Snapshot::next_transition(channel, from)` and
  `previous_transition(channel, before)` against a brute-force scan (the list of `transitions(channel, 0, length)`)
  on 300 random captures (an LCG; runs of random lengths; some with gaps; some empty channels): for every
  position tried (0, 1, every transition position and one either side, the last sample, the length) the answers
  equal the scan; an out-of-range channel is `storage.channel_out_of_range`; edge cases by hand: a channel with
  no transition gives `None` both ways, a transition at position 1 (the first possible) and at the last sample.
- [ ] T032 [P] [US4] `crates/host/tests/scroll_to_edge.rs`: on a constructed capture of 200,000 samples with edges on
  channel 0 at 1,000 and 90,000 and a window of 400 samples at 0: `ScrollToEdge { 0, Next }` gives the window
  start 900 (edge 1,000 minus 100) and length 400; again gives 89,900; `Previous` from there gives
  `1,000 - 300 = 700`; the zoom (length) never changes; following the newest data is off afterwards; a window
  near the end is clamped to `capture_length - length`; a window longer than the capture starts at 0; no edge on
  that side, a hidden channel, an unknown channel and no capture are `Ignored(host.invalid_command)` and change
  nothing; a transition next to a gap is skipped (the capture built with `Part::Gap`); the plan's `edge_after` is
  false when no edge lies beyond the window and `edge_before` is true only when one lies before, for every shown
  channel; hidden channels are not in the plan; cost: a capture of 100,000,000 samples answers in under 50 ms
  (a timing test with a generous limit, `#[ignore]`d in debug builds).
- [ ] T033 [P] [US4] Headless rendering tests in `crates/app/src/app.rs`: with the pointer at 10 px from the right end of a
  lane whose plan has `edge_after`, the arrow button is drawn (the overlay layer, not the cached one: the
  cache is not invalidated), none when `edge_after` is false, none when the pointer is over the legend or the
  axis or in the middle of the lane, and none for a decoder lane; the same on the left with `edge_before`.

### Implementation for User Story 4

- [ ] T034 [US4] `crates/storage/src/query.rs`: `next_transition` and `previous_transition` on the same block index as
  `TransitionIter::new` (binary search of the block, then of the position inside it), O(log N), returning the
  existing transition type. T031 passes.
- [ ] T035 [US4] `crates/host/src/plan.rs`: `ChannelPlan.edge_before` and `edge_after` from the two queries (edges
  next to a gap skipped, bounded by the number of gaps) for the shown channels, once per plan; `crates/host/src/window.rs` and
  `shell.rs`: the command `ScrollToEdge` per the host contract (start `edge - length / 4` rightwards,
  `edge - (3 * length) / 4` leftwards, clamped, length unchanged, follow off). T032 passes.
- [ ] T036 [US4] `crates/host/benches/frame_plan.rs`: the plan of a large capture now includes the edge flags; record
  the baseline (`-- --write-baseline`) in the benchmark's baseline file and check that the cost does not grow
  with the capture length (the bench already has the sizes).
- [ ] T037 [US4] `crates/app/src/signal_view.rs`: the pointer state of the canvas knows the lane and the distance to its left
  and right end; in the overlay layer draw the 24 px rounded square with an arrow at the end when the plan allows
  it (interaction contract); a press on it sends `ScrollToEdge` and is not a pan, a marker or an annotation click;
  tooltips are not available on a canvas, so the arrow is the whole affordance (the keys `edge-previous` and
  `edge-next` are used for the accessible text in the status line while hovering, in six languages). T033 passes.

**Checkpoint**: US4 works with the zoom unchanged.

---

## Phase 7: User Story 7 - Add a decoder from the menu and the keyboard (Priority: P2)

**Goal**: Decoders → Add decoder and D offer the decoders by name, whatever their number.

**Independent Test**: with the two built-in decoders, choose each from the menu; press D and do the same.

### Tests for User Story 7 (write first; they must fail)

- [ ] T038 [P] [US7] Application tests (no display) in `crates/app/src/app.rs`: with a capture on screen and two decoders,
  `Message::MenuToggled` of Decoders and the entry Add decoder opens a submenu listing both names in catalogue
  order; choosing one adds that decoder; with one, three and eight registered decoders (test descriptors)
  the same holds; the key D opens the Decoders menu with the submenu expanded; with no capture the entry is
  disabled and its reason is shown; the old behaviour (`AddDecoderMenu` adding the only decoder) is gone and the
  tests that relied on it use the submenu.
- [ ] T039 [P] [US7] Headless rendering tests: the Decoders menu with the submenu open, in English and German, at the
  minimum window size: the submenu is inside the window.

### Implementation for User Story 7

- [ ] T040 [US7] `crates/app/src/app.rs`: generalise the menu overlay: an entry can have children (a submenu drawn to the
  right of its menu, opening on press); state `open_submenu: Option<usize>`; `Action::AddDecoder` becomes an
  entry with the catalogue as children (`Message::AddDecoderPicked`); the D shortcut opens the Decoders menu and
  expands the submenu; remove the `AddDecoderMenu` one-decoder branch. T038 and T039 pass. Update the user guide
  sentence in `docs/user-guide.md` (step 1 of Decoders) to the menu and the key.

**Checkpoint**: the defect of feature 008 is fixed and the submenu mechanism is there for US5.

---

## Phase 8: User Story 5 - Recent files in a submenu (Priority: P3)

**Goal**: File → Open recent holds the 15 latest files.

**Independent Test**: use 16 files; the submenu holds the last 15, newest first; a deleted one is disabled.

### Tests for User Story 5 (write first; they must fail)

- [ ] T041 [P] [US5] `crates/host/tests/recent_files.rs`: `MAX_RECENT_FILES` is 15; saving or opening 16 files keeps 15 newest first;
  re-using an old one moves it to the top without a duplicate; `recent_files()` marks a deleted file
  `exists: false` and an existing one `true`; `RemoveRecentFile` removes an entry and ignores one that is not
  listed; `ClearRecentFiles` empties the list; a settings file with 30 entries is truncated to 15 on load; the
  lists are saved to the settings.
- [ ] T042 [P] [US5] Application tests (no display): the File menu has Open recent as a submenu; up to 15 entries with the file
  name and the folder as secondary text; a missing file's entry has no press action and a marker; the last
  entries "Remove unavailable entries" (only when one is missing) and "Clear the list"; an empty list shows
  one disabled entry; choosing an entry sends `OpenSession`. Headless rendering of the submenu in English and
  Japanese at the minimum window size.

### Implementation for User Story 5

- [ ] T043 [US5] `crates/host/src/settings.rs` (15), `crates/host/src/sessions.rs`: `RecentFile { path, exists }`,
  `Shell::recent_files()` returning it, the commands `RemoveRecentFile` and `ClearRecentFiles` (saving the settings).
  T041 passes.
- [ ] T044 [US5] `crates/app/src/app.rs`: the Open recent submenu on the mechanism of T040; keys `menu-file-recent`
  (exists, now the submenu title), `menu-file-recent-empty`, `menu-file-recent-missing`,
  `menu-file-recent-remove-missing`, `menu-file-recent-clear` in six languages; the flat list under File is
  removed. T042 passes.

**Checkpoint**: US5 works.

---

## Phase 9: User Story 6 - Icons (Priority: P3)

**Goal**: an application icon and a session-file icon, with the declarations the installer will use.

**Independent Test**: the window and the executable show the icon; the repository holds the icon files and
well-formed declarations.

### Tests for User Story 6 (write first; they must fail)

- [ ] T045 [P] [US6] `crates/app/tests/icons.rs`: the files exist and have the right size: `assets/icon/app.svg` and
  `session.svg` are well-formed XML with a `viewBox`; `app-16.png` to `app-256.png` (16, 32, 48, 64, 128, 256)
  start with the PNG signature and their IHDR width and height equal their names; `app.ico` starts with the ICO
  header and holds 16, 32, 48 and 256; `app-64.rgba` is exactly 64 x 64 x 4 bytes and not all one value;
  `packaging/linux/logic-analyzer.desktop` has `[Desktop Entry]`, `Name`, `Exec=logic-analyzer %f`, `Icon`,
  `MimeType=application/x-logic-analyzer-session;` and `Type=Application`; `packaging/linux/logic-analyzer-session.xml` is
  well-formed, declares `application/x-logic-analyzer-session` with a glob for the extension of the session
  format (read the extension from `la_session`), and an icon name that exists in `assets/icon/`;
  `packaging/windows/README.md` names the extension and the `.ico`.

### Implementation for User Story 6

- [ ] T046 [US6] The artwork and the script: hand-drawn `crates/app/assets/icon/app.svg` (a rounded square with a stylised
  two-colour digital waveform) and `session.svg` (a page with a folded corner carrying the same waveform), original
  and with no raster or font; `tools/make_icons.py` (Python with Pillow, documented at its top: how to run, that it
  is not part of the build) that draws the same shapes and writes the PNGs, the `.ico` and the raw RGBA; run it and
  commit the outputs. Record in `docs/third-party-notices.md` that the icons are original works of the project under
  GPL-3.0.
- [ ] T047 [US6] `crates/app/src/main.rs`: the window icon from `include_bytes!("../assets/icon/app-64.rgba")` through
  `iced::window::icon::from_rgba` in the window settings (a failure to build it logs a warning and runs without,
  never stops the application).
- [ ] T048 [US6] `crates/app/build.rs` and `crates/app/Cargo.toml`: on Windows, put `assets/icon/app.ico` into the executable
  with `winresource` (a `[target.'cfg(windows)'.build-dependencies]` entry, pinned by `Cargo.lock`); the script does
  nothing on other systems and does not fail the build when the resource compiler is missing (a warning). Run
  `cargo deny check` (CI) and the MSRV job: if either refuses the dependency, delete this task's code, say so in
  `plan.md`'s Complexity Tracking and `verification.md`, and keep T047 only.
- [ ] T049 [US6] `packaging/linux/logic-analyzer.desktop`, `packaging/linux/logic-analyzer-session.xml`,
  `packaging/windows/README.md` per the test of T045 (what the installer of 015+ must register). T045 passes.

**Checkpoint**: US6 works.

---

## Phase 10: Polish & Cross-Cutting Concerns

- [ ] T050 [P] Compatibility test `crates/host/tests/sessions_compat.rs`: the two fixtures of T002 open with nothing lost (names,
  colours, order, markers, window, decoder setup and results), automatic heights, no warning; saving one and
  opening the result keeps all of it and writes no `LAYT`; a file saved with heights, with its `LAYT` removed by a
  helper (what a build without the feature would drop?) no: with the `LAYT` kept as an unknown extra, opens
  in the extras-only reader of feature 003 and is written back byte for byte.
- [ ] T051 [P] The random walk (`crates/host/tests/random_walk.rs`) draws every new command (`SetSidePanel`,
  `ScrollToEdge` with random channels and directions, `RemoveRecentFile`, `ClearRecentFiles`) and its invariants hold
  (a window never longer than the capture, `follow` off after a scroll); run it with several seeds.
- [ ] T052 [P] Documents (English): `docs/host-api.md` (state, the five commands, the two plan fields, the pure functions,
  the settings field, the `LAYT` section and the two storage queries, with the rules of `ScrollToEdge` exactly);
  `docs/user-guide.md` (the new layout: the side panel and its icons, editing channels in the legend, the Capture
  view with seconds and the channel buttons, the edge buttons, Open recent, Add decoder, the icons; remove the
  chapters of the Channels panel and the old toolbar); `docs/localization.md` if it lists keys by area;
  `roadmap.md` (014 merged when the pull request is merged, not before).
- [ ] T053 [P] Localization review: every new key exists in six languages and `cargo test -p la-i18n` passes; the headless
  tests of T018, T025, T033, T039 and T042 cover English, German, Ukrainian and Japanese at the minimum window
  size; list the Japanese and long German texts a person should look at in `verification.md`.
- [ ] T054 Data-safety and Principle I review of the new code: no `unwrap`, `expect`, indexing or unchecked arithmetic
  (`cargo clippy --workspace --all-targets -- -D warnings`), `cargo +1.85 check -p la-host -p la-storage`; every
  parser has a hostile-input test (`LAYT`, the seconds text, the settings value, recent paths).
- [ ] T055 Mutation checks for tests that passed the first time they ran (the zone borders of `lane_layout`, the
  quarter of `ScrollToEdge`, the clamp of heights, the 15 of the recent files, the toggling of the panel by
  `next_state`): break the guarded line, see the test fail, restore; record in `verification.md`.
- [ ] T056 SC check and `specs/014-gui-improvements/verification.md`: traceability of every FR and SC to the tests that
  prove it; SC-002 measured (the signal width with and without the panel at 1180 px); SC-004 measured (the click on
  100 million samples); the benchmark numbers against the baseline; the manual checks left to the owner
  (quickstart section 4 on Windows and Debian, the icons in Explorer and a Debian file manager, the translations).
- [ ] T057 Run the whole quickstart (`specs/014-gui-improvements/quickstart.md` sections 1–3 and 5), `cargo fmt --check`,
  `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`, and the CI of the pull request
  on both systems.

---

## Dependencies & Execution Order

### Phase dependencies

- Setup (T001–T002) → Foundational (T003–T004) → US1 → US2 → US3 → US4 → US7 → US5 → US6 → Polish.
- US2 needs nothing of US1 but is ordered after it (both P1). US3 needs US2 (the Capture view lives in the panel) and
  removes the Channels panel, so it comes after US1 (the legend replaces what the panel did for names, colours and
  order). US4 needs only the foundations and can be done earlier. US5 needs the submenu mechanism of US7. US6 is
  independent of everything else.

### Within each story

Tests first and failing, then the host (state, commands, storage), then the interface. Files shared by tasks of
several stories (`app.rs`, `signal_view.rs`, `ui.ftl`, `docs/host-api.md`) are edited one task after another, never
in parallel.

### Parallel opportunities

- T005–T008 together; T016–T018; T023–T026; T031–T033; T038–T039; T041–T042.
- T045 can be written at any time before T046–T049; T050–T053 together in Polish.

## Implementation Strategy

### MVP first

Setup → Foundational → **US1**: channels edited in the legend, heights saved in `LAYT`. Stop and validate with
quickstart section 4, steps 3.

### Incremental delivery

US2 gives the panel, US3 completes the capture setup and removes the old panel, US4 adds the quick scroll, US7
repairs the menu, US5 and US6 finish the polish. One pull request for the feature is the plan (as 007 and 008),
with a commit per task or logical group.

## Notes

- [P] tasks = different files, no dependencies.
- Expected values are by hand, from the contracts.
- Never commit with failing clippy, fmt or tests.
- The Channels panel is removed in T030 and not earlier: until then the flags have no other place.
