---

description: "Task list for Single and Pair Markers"
---

# Tasks: Single and Pair Markers

**Input**: Design documents from `/specs/015-markers-rework/`

**Prerequisites**: plan.md, spec.md, research.md (R1–R12), data-model.md, contracts/
(`host-api-2.0.md`, `view-section-2.md`, `ui-interaction.md`), quickstart.md

**Tests**: REQUIRED (constitution Principle III). In every phase the test tasks come first and MUST fail
(usually they do not compile yet) before the implementation tasks start. If a test passes at once because
earlier work already covers it, run a mutation check (temporarily break the line it guards, see the test fail,
restore) before relying on it. Expected values (bytes of the `VIEW` payload, pixel positions of marks, window
starts, texts) are written **by hand** from the contracts, never computed by the code under test.

**Organization**: tasks are grouped by user story (US1–US9, numbered as in spec.md). Order of the phases:
Setup, Foundational (the model, the section, the API shape; the old marker API goes away here), **US1**
(single marker, P1), **US2** (pair marker, P1), **US9** (exact placement: snapping and the dwell zoom, P1),
**US7** (the Markers view and the action list, P2: the home of every action), **US3** (colour, comment, bin,
P2), **US4** (actions of a single marker, P2), **US5** (actions of a pair, P2), **US6** (make a pair, P3),
**US8** (saving and opening, P2: what is left of it after the foundation), Polish. US7 comes before US3–US6
because they add entries to the action list it builds.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: can run in parallel (different files, no dependency on an incomplete task)
- **[Story]**: user story from spec.md
- All paths are relative to the repository root.

## Path Conventions

Core in `crates/host/src/` and `crates/storage/src/`, their tests in `crates/host/tests/` and
`crates/storage/tests/`; interface in `crates/app/src/` (tests in the same files under `#[cfg(test)]`);
strings in `resources/i18n/<lang>/ui.ftl` and `errors.ftl`; documents in `docs/`. "The host contract" is
`specs/015-markers-rework/contracts/host-api-2.0.md`, "the section contract" `contracts/view-section-2.md`,
"the interaction contract" `contracts/ui-interaction.md`; decisions are `research.md` R1–R12; types are in
`data-model.md`.

## Lint reminder (applies to every Rust task)

The workspace denies `unwrap`, `expect`, indexing (`v[i]`), unchecked arithmetic, `panic!` and `unsafe` in
production code of `la-host`, `la-storage` and `la-app`; use `get`, `checked_*`, `saturating_*`, `u128`
intermediates and `?`. Test code may relax lints with the file-level `#![allow(...)]` used by the other tests.
`la-host` and `la-storage` keep **MSRV 1.85** (no `let` chains: check with `cargo +1.85 check -p la-host -p
la-storage`); `la-app` needs 1.92. Never commit while clippy fails: a helper that only a later task uses gets
`#[expect(dead_code, reason = "used by T0xx")]`, removed by that task. Every UI string comes from `ui.ftl` or
`errors.ftl` (English first, then the five translations as drafts, **in the task that first uses the key**,
so the parity check "no missing, no unused key" is green at every commit); keys are string **literals** in
the code; keys that this feature makes unused (`marker-default`, `marker-measure-first`,
`marker-measure-second`, `measure-pick`, `panel-markers` if replaced) are deleted in the task that stops using
them. Run `cargo fmt` before the next edit. Scripts that patch files: write them to a file, not a shell
heredoc (apostrophes break it), and keep the line endings of the file they patch. Commits are signed; if
signing fails, ask the owner to unlock GPG, never bypass it.

---

## Phase 1: Setup

- [X] T001 Create the empty modules with their doc comments and declarations: `crates/storage/src/crop.rs`
  (declared in `crates/storage/src/lib.rs`), `crates/host/src/marker_ops.rs` (declared in
  `crates/host/src/lib.rs`), `crates/app/src/marker_view.rs` (declared in `crates/app/src/main.rs`). Nothing
  uses them yet; the workspace builds and clippy is clean.
- [X] T002 Record the starting point on this branch: `cargo test --workspace` is green, and write into
  `specs/015-markers-rework/verification.md` (new, the start of the file that Polish completes) the list of
  every place that uses the old marker API and will be changed or deleted, found with `grep -rn -i
  "marker" crates` (expected: `crates/host/src/{markers,command,state,shell,plan,sessions,shortcuts,view_section}.rs`,
  `crates/host/tests/{markers,measurement,view_section,session_scenarios,random_walk,shortcut_coverage,side_panel,sessions_compat,make_compat_fixtures}.rs`,
  `crates/app/src/{app,signal_view,side_panel,icons}.rs`, the `marker-*`, `measure-*`, `menu-markers-*`,
  `panel-markers` keys of `resources/i18n/*/ui.ftl`).

---

## Phase 2: Foundational

**Purpose**: what every story's tests refer to: the crop operation, the marker model and its rules, the
`VIEW` version 2 payload, the host API 2.0 shape, and the minimal changes that keep the application building
while the old markers go away. After this phase markers can be listed, removed, saved and opened (no way yet
to place them: the stories add that).

### Tests (write first; they must fail)

- [X] T003 [P] `crates/storage/tests/crop.rs` (the contract of research R6, expected values by hand): a capture of
  3 channels and 40 positions with known transitions: `crop(10, 30)` has length 20, the initial level of each
  channel equals `level_at(10)` of the source, a transition at 10 is folded into the initial level (not a
  transition of the copy), transitions at 11 and 29 become 1 and 19, one at 30 is not in the copy; gaps are
  clipped and shifted (a gap `5..15` in `10..30` becomes `0..5`; a gap wholly outside vanishes; a gap across
  both ends becomes the whole range); `stopped_by_limit`, `interrupted`, the sample rate and the channel count
  are kept; the full range `0..length` gives a copy equal to the source (`level_at` at every position and the
  same transitions); `start >= end`, `end > length` and an empty source are `storage.invalid_restore`-class
  errors, never a panic; a budget too small for the declared transitions is `storage.memory_limit` **before**
  anything is copied; a cancel requested at the first poll returns `storage.cancelled` and builds nothing; a
  property test (random captures from `la_storage::testing`, random ranges): for every position in the range
  `copy.level_at(c, p - start) == source.level_at(c, p)` and `transition_count` equals the count of source
  transitions inside the open range plus nothing else.
- [X] T004 [P] `crates/i18n/tests/formats.rs` (extend): `format_duration` of negative seconds in all six
  languages: the sign (the locale's minus, never `-` swallowed), the unit chosen by magnitude (`-1.5 ms`, not
  `-0.0015 s`), zero is `0`, and `-0.0` is not shown with a sign; expected texts written by hand from the CLDR
  data the crate already holds. If it passes at once, mutation-check it (research R7).
- [X] T005 [P] `crates/host/tests/markers.rs` (rewrite; the old tests are deleted with the old API, T009): the
  rules of `data-model.md` as pure-function tests on `Markers` with no shell: `add` gives ids 1, 2, ... and
  numbers per kind (`T1`, `T2`, `P1`, `T3`), a number is never given twice after `remove`, after `split` or
  after `join`; the limit of 64 (the 65th is `None`; a pair counts one); `Shape::interval(9, 3)` is `{3, 9}`;
  `move_end` past the other end swaps the roles and keeps id and number; `set_origin` accepts a single and
  refuses a pair; `split` gives two new single numbers, keeps the colour, puts the comment on the first, is
  refused at 64 markers; `join` accepts two different points only and gives a new pair number, the colour and
  comment of the earlier; `remap(start, end)` per the table (a point outside is removed; a pair with one end
  outside is removed; positions shift by `start`; the origin shifts or goes to 0; `selected` cleared when
  removed; `next_*` unchanged); `ordered` by start then id; `jump_target` next/previous with wrap-around and an
  interval counted at its start; `centre_window` and `fit_window` (window clamped, margin at least mark width
  plus gap in samples at the resulting zoom when room allows, as much as the capture has when not);
  `Markers::check(length)` accepts every state the rules produce and rejects each broken invariant (a hand-made
  duplicate id, a position beyond the length, a start after the end, a repeated number, an origin beyond the
  length).
- [X] T006 [P] `crates/host/tests/view_section.rs` (rewrite the marker part; expected bytes by hand from
  `view-section-2.md`): a payload with no presentation, one single (id 1, number 1, colour 0, position 5, comment
  "ab") and one pair (id 2, number 1, colour 3, 7..9, empty comment), `next_id 3`, `next_single 2`,
  `next_pair 2`, origin 5, selected id 2, window 0/0, follow 1 is exactly the bytes you write out; the writer
  is deterministic; every rejection of the contract gives `host.view_invalid` through the loader (version 0 and
  1, repeated id, repeated number within a kind, a reserved flag bit, colour 8, position beyond the capture,
  end before start, comment not sanitized / not UTF-8 / too long, `next_id` not above the ids, `selected`
  naming nothing, a window outside the capture, trailing byte, non-canonical varint); version 3 is `Newer` and is
  kept untouched; a property test with random bytes (nothing panics; an accepted payload re-encodes to the
  same bytes).

### Implementation

- [X] T007 `crates/storage/src/crop.rs` (+ export in `crates/storage/src/lib.rs`): `crop(&Snapshot, start, end, limits: Limits,
  &dyn CropControl) -> Result<Capture>` per research R6 with the `CaptureRestorer`; `CropControl` has
  `cancelled() -> bool` and `progress(done, total)`; polls per channel and per block; checks the budget first with
  `check_budget`; the error code `storage.cancelled` is added to `crates/storage/src/error.rs` if it is not there
  (with its message in `errors.ftl`, six languages). T003 passes.
- [X] T008 `crates/i18n/src/…` (the quantity formatting used by `Formatter::duration`): make negative durations
  correct if T004 shows they are not. T004 passes.
- [X] T009 `crates/host/src/markers.rs` rewritten per `data-model.md`: `Kind`, `Shape`, `Marker`, `Markers` with
  `list`, `selected`, `origin`, `next_id`, `next_number`, `MAX_MARKERS = 64`, `MAX_COMMENT_CHARS = 80`,
  `MARKER_COLOURS = 8`, `add`, `move_end`, `set_origin`, `split`, `join`, `remove`, `clear`, `remap`, `ordered`,
  `jump_target`, `check`, `restore`; `centre_window` and `fit_window` in `crates/host/src/window.rs` (as
  methods of `ViewWindow`, per research R11); `measurement` and `measurement_text` stay as they are;
  `nearest_transition` stays for now (US9 rewrites it). T005 passes. Delete the old tests that this makes
  meaningless (`crates/host/tests/measurement.rs` keeps only the formatting tests: move them to T005's file or
  keep them, whichever compiles).
- [X] T010 `crates/host/src/view_section.rs`: payload version 2 per the section contract (`VERSION = 2`, write
  and read the marker part, `origin`, `next_*`, `selected`; versions 0 and 1 are `view_invalid`, above 2 is
  `Newer`); `ViewState.markers: Markers`. T006 passes.
- [X] T011 Host API 2.0, **shape only**: `HOST_API_VERSION = "2.0"` in `crates/host/src/lib.rs` and `Version: 2.0` in
  `docs/host-api.md`; in `crates/host/src/command.rs` remove `PlaceMarker`, `RenameMarker`, `SelectMarkers`,
  `GoToMarker` and the old `MoveMarker`, add every command of the host contract (`BeginPlacement(Kind)`,
  `CancelPlacement`, `FixPlacement`, `MoveMarkerEnd`, `EndMarkerDrag`, `PlacementZoomTo`, `SetMarkerColour`,
  `SetMarkerComment`, `SelectMarker`, `SetTimeOrigin`, `ResetTimeOrigin`, `CutLeft`, `CutRight`, `TrimToPair`,
  `ScrollToMarker`, `FitToPair`, `SplitPair`, `MakePair`) with their `name()` entries and
  `waits_for_operation` membership (every command that changes the capture or the markers waits while a save,
  load or crop runs), answered with `Outcome::Ignored(host.invalid_command)` until their stories; the
  types `Kind`, `End`, `Pointer`, `Resolved`, `Snapped`, `ZoomTarget`, `Placement` in `crates/host/src/markers.rs`
  and `State.placement`, `State.snap_zoom`, `Activity::Cropping` in `crates/host/src/state.rs`; rewrite the
  Markers rows of the command table of `docs/host-api.md` from the contract (the test `host_api_doc.rs`
  compares the table with the code in both directions) and the "Markers" paragraphs of the document to describe
  the two kinds.
- [X] T012 Keep the host consistent with the new model: `crates/host/src/plan.rs` (`MarkerPlan` per `data-model.md`:
  id, name, colour, comment, selected, `PlanShape`), `crates/host/src/shell.rs` (`frame_plan` fills it for markers
  with any part inside the window; the commands `RemoveMarker`, `RemoveAllMarkers`, `SelectMarker`, `SetSnap`
  work on the new model; `Shell::marker_texts()` replaces `measurement_text()` returning `MarkerText` with the
  times relative to the origin, and for a pair the duration and frequency as the old measurement text did),
  `crates/host/src/sessions.rs` (save and restore the markers and the origin through `ViewState`; write the
  section when a marker exists, the origin is not 0, the presentation is not the default or a window is
  remembered; every marker command marks the capture unsaved), `crates/host/src/shortcuts.rs` (the old marker
  actions removed, `RemoveSelectedMarker`, `ToggleSnap`, and the new ones registered with their menu paths and
  keys but answered by `Ignored` until their stories: `PlaceSingleMarker` M, `PlacePairMarker` Shift+M,
  `CancelPlacement` Esc, `PreviousMarker`, `NextMarker`; the coverage test `shortcut_coverage.rs` is updated
  from the contract). `cargo test -p la-host` is green.
- [X] T013 Keep the interface building, minimally (the final panel is US7, the final drawing US1/US2):
  `crates/app/src/app.rs` and `crates/app/src/signal_view.rs`: remove the old messages and code of the measured
  pair (`MarkerMeasure`, `MarkerName`, `marker_names`, `sync_marker_names`, the measurement lines), draw each
  `MarkerPlan` as a plain solid line with its name, and make the Markers view a list of rows with the name and a
  bin button (`Command::RemoveMarker`); the menu entries of the old marker actions are removed or disabled; the
  application tests of the old behaviour (`two_markers_show_their_distance_and_frequency`, the snapshots of the
  Markers view) are deleted or ported to what exists. `cargo test --workspace` is green.

**Checkpoint**: the workspace builds, `cargo clippy --workspace --all-targets -- -D warnings` and
`cargo test --workspace` are green, a session with markers saves and opens (checked by the host tests of
T006 and the existing session tests), and there is no way yet to add a marker.

---

## Phase 3: User Story 1 - Mark a point in time with a single marker (Priority: P1) 🎯 MVP

**Goal**: put a single marker on the signals with the mouse (a placement mode with a dotted line, a click to
fix), see its pentagon and solid line, drag it, remove it, jump to the next one.

**Independent Test**: on a virtual-device capture press M, move the pointer (dotted line follows), click
(solid line with the pentagon, row `T1`), zoom and pan (it stays), drag it, add `T2`, delete `T1`, add again
(`T3`).

### Tests for User Story 1 (write first; they must fail)

- [X] T014 [P] [US1] `crates/host/tests/placement.rs` (new; the state machine of `data-model.md`, single only): `BeginPlacement(Single)`
  with a capture gives `placement = Some`, adds nothing and bumps `Changes::MARKERS`; with no capture, at 64
  markers, and while `Saving`, `Loading` or `Cropping` it is `Ignored(host.invalid_command)` and `placement`
  is `None`; allowed while `Capturing`; the same command again cancels; `BeginPlacement(Pair)` switches;
  `CancelPlacement` ends it and is `Ignored` when there is none; `FixPlacement` with a pointer over a sample
  adds `T1` at that sample with colour 0, an empty comment, selects it, marks the capture unsaved and ends
  the placement; a pointer beyond the capture (`x` negative, NaN, huge) fixes at the nearest end; `FixPlacement`
  with no placement is `Ignored`; `OpenSession`, `NewCapture`, `StartCapture`, a crop and `Quit` drop an active
  placement and add nothing; the pointer-to-position arithmetic (`x`, window start/length, width) by hand for
  three windows.
- [X] T015 [P] [US1] `crates/host/tests/marker_commands.rs` (new): `MoveMarkerEnd { End::Point }` moves a
  single (clamped to the capture, unsaved), is `Ignored` for an unknown id or for `Start`/`End` of a single;
  `RemoveMarker` and `RemoveAllMarkers` keep the numbering (add, remove, add gives `T2`); `SelectMarker`
  selects, `SelectMarker(None)` clears, an unknown id is `Ignored`; `ScrollToMarker` puts the marker at the centre of the same-length window,
  clamped at both ends of the capture, `follow_latest` and `auto_fit` become false (arithmetic by hand);
  the shortcut actions `PreviousMarker`/`NextMarker` choose `jump_target` and send `ScrollToMarker`;
  `Markers::check` holds after each command.
- [X] T016 [P] [US1] Headless rendering tests in `crates/app/src/signal_view.rs`/`app.rs` tests (`iced_test`, pixel
  checks by hand against the interaction contract): a fixed single marker draws a solid line of its colour and
  a pentagon whose lowest pixel row is on the line, with the name beside it; during a placement the overlay
  draws a **dotted** line (alternating 4 px painted, 3 px clear) at the pointer, and not in the cached layer
  (the cache is not rebuilt by a pointer move); the status line shows the placement text; Esc and the right
  button cancel; when the pointer leaves the signal area the dotted line is hidden and a click outside the area
  fixes nothing (placement stays active); two markers of different kinds at one point have their names
  staggered so they do not overlap; a left click inside the signal area publishes `FixPlacement`, a click on the legend does not.

### Implementation for User Story 1

- [X] T017 [US1] `crates/host/src/marker_ops.rs` and `crates/host/src/shell.rs`: `BeginPlacement`, `CancelPlacement`,
  `FixPlacement` (single), `MoveMarkerEnd` (point), `SelectMarker`, `ScrollToMarker` (also for a pair: its
  start), `Shell::resolve_pointer(Pointer) -> Resolved` returning only the sample under the pointer for now
  (snapping is T032), the dropping of an active placement by the commands that end it; log by stable name
  only (never the comment). T014 and T015 pass.
- [X] T018 [US1] `crates/host/src/shortcuts.rs`: M → `BeginPlacement(Single)`, Esc → `CancelPlacement` (enabled only
  while placing), `[` and `]` → `ScrollToMarker(jump_target)`, Delete → `RemoveMarker(selected)`, S; the menu
  "Markers" entries per the interaction contract; `shortcut_coverage.rs` passes with the new table.
- [X] T019 [P] [US1] `crates/app/src/theme.rs`: the eight marker colours for light, dark and high contrast
  (`Theme::marker(index) -> Color`), chosen so each is distinguishable from its neighbours and from the lane
  background by the contrast rule of feature 006 FR-013 (a unit test per theme checks the ratio against the
  lane background by hand-computed values; the colour is never the only cue because the shapes differ).
- [X] T020 [P] [US1] `resources/i18n/<en,de,fr,es,ja,uk>/ui.ftl`: keys for this story: `marker-single-add` (button
  label), `marker-placing-single` and `marker-placing-cancel` (status line), `menu-markers-place-single`,
  `menu-markers-cancel`, `menu-markers-previous`, `menu-markers-next`, `menu-markers-remove`,
  `menu-markers-remove-all`, `menu-markers-snap`, `marker-empty` (the empty-list text); remove the keys this
  story makes unused; the parity test is green.
- [X] T021 [US1] `crates/app/src/signal_view.rs` and `crates/app/src/app.rs`: draw single markers per the interaction
  contract (the pentagon path, the solid line, the name, the selected look, the tooltip from `marker_texts`);
  placement: while `placement` is `Some` every pointer move over the signal area publishes a hover message the app
  stores (the cached layer is untouched) and the overlay draws the dotted line and mark at
  `Shell::resolve_pointer` of that pointer, with the lane found from the pointer's `y` (`lane_layout`); a left
  click inside the area sends `FixPlacement`, Esc / right button / the same add command send the cancel;
  pressing within 4 px of a fixed line selects it and drags it with `MoveMarkerEnd` per pointer move, release
  sends `EndMarkerDrag` (a no-op in the host until T034); a minimal "Single marker" button at the top of the
  Markers view (the panel is rebuilt in US7) and the status line texts. T016 passes.

**Checkpoint**: US1 works with the keyboard and the mouse; quickstart section 2 steps 1–2 and 8 (bin) pass.

---

## Phase 4: User Story 2 - Measure an interval with a pair marker (Priority: P1)

**Goal**: a pair marker placed with two clicks (the left half is the start whichever was fixed first, the mark
of the fixed half jumps with the pointer), dragged by either end, with its duration and frequency in the
language of the interface.

**Independent Test**: place a pair on the two edges of a pulse of known width; check the duration and
frequency in several languages, then drag the end past the start and check the roles swap.

### Tests for User Story 2 (write first; they must fail)

- [X] T022 [P] [US2] `crates/host/tests/placement.rs` (extend): the pair state machine: the first `FixPlacement` gives
  `Placement { first: Some(p) }` and adds nothing, the second adds `P1` with `{min, max}`; fixing the right half
  first and then a position to its left gives the same pair; two clicks on the same sample give a pair of
  duration zero; `CancelPlacement` after the first click drops it and adds nothing; `BeginPlacement(Single)`
  after a first click switches and drops it; the numbering of `P1`, `P2` is independent of `T*`.
- [X] T023 [P] [US2] `crates/host/tests/marker_commands.rs` (extend): `MoveMarkerEnd` with `Start` and `End`
  moves only that end; moved past the other end the two exchange (`start <= end` holds, id and number stay);
  `Point` on a pair is `Ignored`.
- [X] T024 [P] [US2] `crates/host/tests/measurement.rs` (rewrite for pairs; numbers by hand): `marker_texts()` for a
  pair at sample rate 1 MHz, positions 100 and 600: duration `500 µs` and frequency `2 kHz` in English,
  and the correct decimal separators, units and digit grouping in German, French, Spanish, Ukrainian and
  Japanese; a duration of zero has no frequency; the frequency keeps only the digits the distance resolves
  (the rule of feature 006); times are relative to the origin.
- [X] T025 [P] [US2] Headless rendering tests (`crates/app`): a fixed pair draws two solid lines and a square
  **touching the start line from the left** and one **touching the end line from the right** (pixel checks by
  hand); while the first half is fixed and the pointer is to its right, the fixed half has its square on its
  left and the dotted half's square is on the right of the dotted line; moving the pointer to the left of
  the fixed half moves the squares to the other sides (the mark jumps); the panel row shows the duration and
  frequency.

### Implementation for User Story 2

- [X] T026 [US2] `crates/host/src/marker_ops.rs`: `FixPlacement` for a pair (the two-click machine), `MoveMarkerEnd`
  for `Start` and `End` with the swap; `Shell::marker_texts()` for pairs. T022–T024 pass.
- [X] T027 [P] [US2] `resources/i18n/*/ui.ftl`: `marker-pair-add`, `marker-placing-pair-first`,
  `marker-placing-pair-second`, `menu-markers-place-pair`, `measure-duration` (and reuse `measure-frequency`),
  `marker-pair-times` (start and end in one row), six languages; parity green.
- [X] T028 [US2] `crates/host/src/shortcuts.rs` (Shift+M → `BeginPlacement(Pair)`), `crates/app/src/signal_view.rs`
  and `app.rs`: draw pair markers (two lines, the two squares, the name on the outer side, the tooltip with
  duration and frequency), the placement overlay for a pair (fixed half solid, the pointer half dotted, the
  role-dependent side of each mark, which jumps when the pointer crosses the fixed half), dragging either end
  of a fixed pair, the pair button in the Markers view next to the single one and the status texts. T025 passes.

**Checkpoint**: US2 works; quickstart steps 4 and (partly) 9.

---

## Phase 5: User Story 9 - Place a marker exactly, by snapping to a transition (Priority: P1)

**Goal**: the line snaps to the transition near the pointer on the lane the pointer is over, the transition
is highlighted, and when many transitions crowd the zone the view zooms in after a pause and comes back after
the choice.

**Independent Test**: on a capture with transitions a few samples apart zoom out until a pixel holds many,
rest the pointer over a crowded spot, wait for the zoom, pick one transition, and check the marker's position
to the sample and that the previous view is back.

### Tests for User Story 9 (write first; they must fail)

- [X] T029 [P] [US9] `crates/host/tests/snapping.rs` (new; expected values by hand on small captures): `resolve_pointer`
  with snapping on and the pointer over the lane of channel 2 within the radius (12 px × samples per pixel) of
  a transition of channel 2 snaps to it and reports `snapped = Some { channel: 2, position }`; the same pointer
  with `lane: None`, or over channel 3 (whose transition is far), or over a decoder lane, does **not** snap
  although channel 2's transition is close in time; the nearest of two candidates wins and a tie goes to the
  earlier; a transition next to a gap is skipped (`real_edge`); snapping off reports no `snapped`; `crowded` is true
  exactly when a second transition lies in the zone; a pointer beyond the ends, `x` NaN or infinite gives a
  position inside the capture; a property test: for random captures and pointers the result is a position
  inside the capture and, when `snapped`, a real transition of that channel within the radius and the
  nearest one (compared with a brute-force scan in the test); a cost test: with 10⁶ transitions in the zone
  the number of storage probes (`QueryStats` or a counting wrapper) is bounded by a constant × log N, not
  by the distance (no scan).
- [X] T030 [P] [US9] `crates/host/tests/placement_zoom.rs` (new; research R4): `crowded_zoom_target` returns `None` when
  the zone is not crowded; for transitions 1, 2 and 3 samples apart around the pointer it returns the window at
  which the nearest neighbours are 24 px apart (arithmetic by hand), with the sample under the pointer still
  under the same pixel `x`, clamped by `WindowLimits` (the maximum zoom) and at the capture's ends;
  `PlacementZoomTo` stores the replaced window in `snap_zoom` once (a second call keeps the first); it is
  `Ignored` when neither a placement nor a drag is active; `FixPlacement`, `CancelPlacement`,
  `BeginPlacement`, `EndMarkerDrag` and each command that ends a placement restore `snap_zoom` as the window
  (clamped to the current length, also when a running capture has grown) and clear it; `Zoom`, `Pan`,
  `ZoomToRange`, `ScrollToEdge`, `ScrollToMarker`, `GoToStart`, `GoToEnd` clear it **without** restoring; a
  pair restores after its first click and may store again for the second half; a property test of 1,000 random
  transitions (a clock of random sample spacing 1–3): starting at the crowded zoom, the target window lets the
  pointer, placed on that transition's pixel, resolve to exactly that transition (SC-008), and the placement
  there restores the original window exactly.
- [X] T031 [P] [US9] Headless rendering and interaction tests (`crates/app`): the snapped transition is highlighted (a ring
  of 6 px in the marker's colour at the edge of its lane and a tick over the lane's height) and nothing is
  highlighted over another lane, over the ruler, or with snapping off; the dwell: a simulated clock (the
  interface takes its time from a message, so the test controls it) with a pointer resting for 2 s over a
  crowded zone sends `PlacementZoomTo` frames over 0.4 s with an ease-out and the final frame equals the
  target; a pointer that moves 3 px or more restarts the wait; no zoom starts when the zone is not crowded;
  the status line shows the "zoomed in" text while `snap_zoom` is `Some`; when the pointer leaves the signal
  area while zoomed in, the view stays zoomed in until the marker is fixed or the placement ends.

### Implementation for User Story 9

- [X] T032 [US9] `crates/host/src/markers.rs` (or `snap.rs`): rewrite `nearest_transition` on the indexed
  `next_transition`/`previous_transition` of the storage (no iteration of everything between the pointer and
  the answer) with the `real_edge` test from `plan.rs`, add `crowded`, and make `Shell::resolve_pointer` per
  research R3 (lane gating, radius, snapping setting). T029 passes.
- [X] T033 [US9] `crates/host/src/markers.rs`: `crowded_zoom_target` per research R4; `State.snap_zoom` handling in
  `crates/host/src/shell.rs` (`PlacementZoomTo`, `EndMarkerDrag`, the restore and clear rules, a `dragging:
  Option<u32>` set by `MoveMarkerEnd` and cleared by `EndMarkerDrag`); `Shell::crowded_zoom_target(Pointer)`.
  T030 passes.
- [X] T034 [P] [US9] `resources/i18n/*/ui.ftl`: `marker-zoomed-in` (status text); six languages; parity green.
- [X] T035 [US9] `crates/app/src/signal_view.rs` and `app.rs`: pass the lane under the pointer to the host call, draw
  the highlight (ring and tick) in the overlay layer, the dwell logic (stillness limit 3 px, 2 s, a
  subscription tick only while a placement or a drag is active, time taken from a message), the 0.4 s ease-out
  animation sending `PlacementZoomTo`, the same for dragging a fixed marker with `EndMarkerDrag` on release;
  the wheel and the other view commands keep working during placement. T031 passes.

**Checkpoint**: US9 works; quickstart steps 2–3. The three P1 stories are done: a minimal complete product.

---

## Phase 6: User Story 7 - The Markers view (Priority: P2)

**Goal**: the Markers view lists both kinds with their actions, has the two add buttons at the top and the
bin icon like the Decoders view; one action list serves the row and the right click on a mark.

**Independent Test**: add markers of both kinds, check order, buttons, rows, selection, the Delete key, the
empty list, in two languages at the smallest panel width.

### Tests for User Story 7 (write first; they must fail)

- [X] T036 [P] [US7] Headless tests in `crates/app/src/marker_view.rs`: the two buttons at the top, each with its mark
  icon, the one of the kind being placed shown pressed and pressing it again cancelling; one row per marker
  in `ordered()` order with name, colour swatch, comment, time or duration and frequency, `⋯` and the bin icon
  at the right (the same `Icon::Trash` widget, size and position as in the Decoders view: assert equality
  of the widget's bounds relative to the row); clicking a row selects it (and the mark in the view); Delete
  removes the selected one; the bin removes without a question; the empty-list text; the limit of 64 gives
  the notice text and adds nothing; `⋯` and a right click on a mark open the same action list at the right
  place and Esc, a click outside or an action close it; six languages at 220 px with the longest German and
  Japanese: nothing clipped or overlapping (the rendering layout check of features 006/014).

### Implementation for User Story 7

- [X] T037 [P] [US7] `crates/app/src/icons.rs`: `Icon::SingleMarker` (the pentagon) and `Icon::PairMarker` (two squares
  with lines), drawn by the canvas; the icon-strip icon of the Markers view (`Icon::Markers`) stays.
- [X] T038 [P] [US7] `resources/i18n/*/ui.ftl`: the texts of the panel: `panel-markers`, the tooltips of the add
  buttons, `marker-limit` (notice), `marker-delete`; six languages; parity
  green.
- [X] T039 [US7] `crates/app/src/marker_view.rs`: the view (buttons, rows, bin, selection, empty text) and
  `ActionList`, the overlay component of the interaction contract (a list of labelled buttons with enabled and
  disabled-with-reason states, opened by `⋯` under the row or by the right click on a mark at the pointer,
  closed by Esc / outside click / an action); it takes the entries from the caller so that US3–US6 only add
  entries; `crates/app/src/app.rs` wires its messages and replaces the temporary panel of T013;
  `crates/app/src/signal_view.rs` publishes the right click on a mark (select + open). T036 passes.

**Checkpoint**: US7 works; the panel is final apart from the entries US3–US6 add.

---

## Phase 7: User Story 3 - Colour, comment, remove (Priority: P2)

**Goal**: every marker's colour (eight), comment (80 characters) and deletion.

**Independent Test**: change both colours, write comments, delete one with the bin, save, reopen, compare.

### Tests for User Story 3 (write first; they must fail)

- [X] T040 [P] [US3] `crates/host/tests/marker_commands.rs` (extend): `SetMarkerColour` accepts 0..=7 and ignores 8 and
  an unknown id, marks unsaved; `SetMarkerComment` stores the sanitized text cut to 80 characters (control and
  invisible characters removed, trimmed; a comment of only spaces is empty; 81 characters become 80; the
  sanitizing is idempotent), is never translated, never logged; a new marker has colour 0 and an empty
  comment; `RemoveMarker` of a pair removes both ends at once.
- [X] T041 [P] [US3] Headless tests (`crates/app`): the action list has a row of eight swatches and a comment field
  limited to 80 characters; choosing a swatch recolours the mark, the line(s) and the swatch of the row at once
  in the three themes; typing a comment shows it in the row and as the tooltip of the mark; a long German and a
  long Japanese comment do not break the row layout at 220 px.

### Implementation for User Story 3

- [X] T042 [US3] `crates/host/src/marker_ops.rs`/`shell.rs`: `SetMarkerColour`, `SetMarkerComment`. T040 passes.
- [X] T043 [P] [US3] `resources/i18n/*/ui.ftl`: `marker-colour-0` … `marker-colour-7` (the names for the swatch tooltips),
  `marker-comment` (label and placeholder); six languages; parity green.
- [X] T044 [US3] `crates/app/src/marker_view.rs`: the swatch row and the comment field in the action list, the comment
  in the row and in the mark's tooltip. T041 passes.

---

## Phase 8: User Story 4 - Work with a point: time origin, cut, go to (Priority: P2)

**Goal**: count time from a single marker, delete the data to its left or right (confirmed, in the
background, cancellable), scroll to it.

**Independent Test**: set a marker as the origin and check the axis and the marker times; cut left and right
and check the remaining length and markers; scroll to it from far away.

### Tests for User Story 4 (write first; they must fail)

- [X] T045 [P] [US4] `crates/host/tests/marker_actions.rs` (new): `SetTimeOrigin` on a single sets `origin`, on a pair
  or an unknown id is `Ignored`; `ResetTimeOrigin` returns 0 and is `Ignored` at 0; both mark the capture unsaved
  and change no data; `marker_texts()` times are negative before the origin and zero at it, the duration of a
  pair does not change; `axis.rs` ticks are labelled relative to the origin (by-hand tick seconds for a
  window around the origin, including negative ones); `CutLeft(id)`: the shell answers with the guarded
  question carrying the span kept and lost; on yes `activity` becomes `Cropping` with progress, and at the end in
  one change: the capture is the range `position..length`, the marker is at 0, markers to its left are gone,
  the origin is kept (shifted) if inside else 0, the window shows the same span as far as it exists,
  `Unsaved::Dirty`, the decoders are flagged to run again, the simulated flag and the sample rate are kept;
  `CutRight(id)` keeps `0..=position` so the marker is the last sample; `CancelOperation` during the crop leaves the
  capture, markers, window and flags exactly as they were (compare the whole `State`); a failing worker (a
  crop that returns an error, injected through the `test-support` feature) leaves everything as it was and
  reports a notice with the storage code; both are `Ignored` while capturing, saving or loading, when they
  would delete nothing (a marker at 0 for the left cut, at the last sample for the right) and for a pair;
  placement is dropped by a crop; commands that change the capture wait while it runs; a pair with only one
  end inside the kept range is removed whole and an `Info` notice `marker-cut-pair-removed` with the count is
  shown (none when no pair went); the old decoder results and the annotation selection are gone after the
  swap and a new pass starts for each instance that reads the capture, its results (once the pass ends)
  describing the shortened capture.
- [X] T046 [P] [US4] Headless tests (`crates/app`): the action list of a single marker shows "Count time from here"
  (or "Time counts from the start" when it is the origin), "Delete data to the left", "Delete data to the
  right", "Scroll to it"; the cut entries are disabled with a tooltip while capturing and when they would
  delete nothing; the confirmation dialog names the side, the lost time and its percentage and the remaining
  length with **Delete** and **Cancel**; during the crop the status line shows progress and a **Cancel** button that sends `CancelOperation`;
  the axis shows negative labels left of the origin; six languages.

### Implementation for User Story 4

- [X] T047 [US4] `crates/host/src/axis.rs`: `build_ticks(window, pixels, rate, origin)`; the callers in `plan.rs` pass
  `state.markers.origin`; `crates/host/tests/axis.rs` updated. The ticks part of T045 passes.
- [X] T048 [US4] `crates/host/src/marker_ops.rs` and `command.rs`: `SetTimeOrigin`, `ResetTimeOrigin`; `Guarded::Crop`
  carrying the kept and lost spans for the dialog text; `CutLeft`/`CutRight` ask the question through the
  existing guarded path; the answer **Delete** is `Decision::Discard`, **Cancel** is `Decision::Cancel`, and
  `Guarded::Crop` is excluded from the remembered unsaved-data choice exactly where `Guarded::RemoveDecoder`
  is (the `.filter(|_| !matches!(guarded, Guarded::RemoveDecoder(_)))` in `crates/host/src/shell.rs`), so it
  is always asked; add a test of that exclusion to T045.
- [X] T049 [US4] `crates/host/src/marker_ops.rs` and `shell.rs`: the crop operation of research R6: a worker thread
  running `la_storage::crop` with a `CropControl` fed by `CancelOperation`, progress in `ProgressView`,
  `Activity::Cropping`; the swap in `poll` in one state change (capture, `Markers::remap`, window, decoders
  restarted with `clear_decoder_runtime()` then `restart_decoders(state)` (the path of a new capture, `crates/host/src/decoders.rs`), `Unsaved::Dirty`, `Changes::CAPTURE |
  MARKERS | WINDOW | PROGRESS | ACTIVITY | DECODERS`); failure and cancel leave the state untouched; a panic in
  the worker is contained like the other workers (`containment.rs` extended with a crop case). T045 passes.
- [X] T050 [P] [US4] `resources/i18n/*/ui.ftl`: `marker-action-origin`, `marker-action-origin-reset`,
  `marker-action-cut-left`, `marker-action-cut-right`, `marker-action-scroll`, `marker-cut-confirm-left`,
  `marker-cut-confirm-right` (with `{ $lost }`, `{ $percent }`, `{ $kept }`), `marker-cutting` (status), `marker-cut-pair-removed` (the information notice, with `{ $count }`),
  `marker-cut-disabled-running`, `marker-cut-disabled-nothing`; the `host.*`/`storage.cancelled` messages in
  `errors.ftl`; six languages; parity and `error_sweep.rs` green.
- [X] T051 [US4] `crates/app/src/marker_view.rs` and `app.rs`: the entries and the confirmation dialog (`dialogs.rs`),
  the status line with progress and a Cancel button (sending `CancelOperation`) while `Cropping`, the origin shown in the panel header ("Time from
  T2") when it is not the start. T046 passes.

---

## Phase 9: User Story 5 - Work with an interval: trim, fit, split (Priority: P2)

**Goal**: from a pair marker keep only its data, fit the view to it, or turn it into two single markers.

**Independent Test**: fit and check both squares are inside the ruler with a margin; trim and check the
capture is the interval with the pair at its ends; split and check two singles at the ends with new numbers.

### Tests for User Story 5 (write first; they must fail)

- [X] T052 [P] [US5] `crates/host/tests/marker_actions.rs` (extend): `FitToPair` window by hand: the pair 1000..2000 in a
  capture of 100000 samples at 800 px with a 12 px mark and a 4 px gap gives the window whose margins on both
  sides are `16 px × samples per pixel` (the fixed-point of research R11 converges in two steps), clamped by
  the zoom limits; at the capture's start the left margin is what the capture has and the right one is
  the full margin; `follow_latest`/`auto_fit` false; a single or an unknown id is `Ignored`; `TrimToPair`
  keeps `start..=end` (`end + 1`), asks the guarded question, runs the same operation as the cuts, the pair
  sits at positions 0 and `kept - 1`, markers outside are removed and inside are kept and shifted, it is
  `Ignored` while capturing and for a single; `SplitPair` gives two singles at the ends with the next single
  numbers (`T3`, `T4` after `T1`, `T2`; the pair's number is not reused), the colour and the comment on the
  start one, selects none, is refused at 63 or more markers.
- [X] T053 [P] [US5] Headless tests (`crates/app`): the action list of a pair shows "Trim to the pair", "Fit the view to
  the pair", "Convert to two single markers"; after "Fit" both squares lie fully inside the ruler (pixel check)
  when the capture allows; the trim confirmation text; six languages.

### Implementation for User Story 5

- [X] T054 [US5] `crates/host/src/marker_ops.rs` and `shell.rs`: `FitToPair` (with `fit_window` and the mark width in
  samples), `TrimToPair` (through the crop of T049), `SplitPair`. T052 passes.
- [X] T055 [P] [US5] `resources/i18n/*/ui.ftl`: `marker-action-trim`, `marker-action-fit`, `marker-action-split`,
  `marker-trim-confirm`; six languages; parity green.
- [X] T056 [US5] `crates/app/src/marker_view.rs`: the three entries and the pair mark width passed to the host (the
  mark width in logical pixels × scale). T053 passes.

---

## Phase 10: User Story 6 - Make a pair out of two single markers (Priority: P3)

**Goal**: two single markers become one pair.

**Independent Test**: check two single rows, press "Make a pair": one pair with the next pair number and the
singles gone.

### Tests for User Story 6 (write first; they must fail)

- [X] T057 [P] [US6] `crates/host/tests/marker_actions.rs` (extend): `MakePair { first, second }` with two different singles
  gives one pair (start the earlier, end the later whichever order the ids come in), the colour and comment of
  the earlier, the next pair number, removes both singles (their numbers are not reused) and selects the pair;
  it is `Ignored` for the same id twice, a pair, an unknown id, and when it would be refused by the limit
  (it never is: two become one).
- [X] T058 [P] [US6] Headless tests (`crates/app`): single rows have a checkbox; with exactly two checked the "Make a pair"
  button appears under the list and sends `MakePair`; with one, three, or a removed checked marker it does not;
  the checks clear after use; six languages.

### Implementation for User Story 6

- [X] T059 [US6] `crates/host/src/marker_ops.rs`: `MakePair`. T057 passes.
- [X] T060 [P] [US6] `resources/i18n/*/ui.ftl`: `marker-make-pair`; six languages; parity green.
- [X] T061 [US6] `crates/app/src/marker_view.rs`: the checkboxes and the button (the checks are interface state).
  T058 passes.

---

## Phase 11: User Story 8 - Saving and opening (Priority: P2)

**Goal**: everything about markers survives a save and an open; old files open without markers and with a
warning.

**Independent Test**: save a capture with markers of both kinds (colours, comments, a time origin), reopen it,
compare everything; open a file written before this feature.

### Tests for User Story 8 (write first; they must fail)

- [X] T062 [P] [US8] `crates/host/tests/session_scenarios.rs` and `crates/host/tests/sessions_compat.rs` (update): a capture
  with two singles and a pair (with colours 0, 3, 7, comments, an origin of 5, a selected marker, `next_*`
  not equal to the count) saved and opened gives an equal `Markers` (`PartialEq`) and the next marker added gets
  the saved next id and number; the section is not written for a capture with no markers, origin 0, default
  presentation and no window; the files of earlier versions in `crates/host/tests/fixtures/` (written with
  `VIEW` version 1, one with a marker) open: the capture and the presentation defaults are shown, there are no
  markers, and a `Warning` notice `host.view_invalid` is added and the section is not written back;
  the other fixtures (decoders, layout) still open as before; a save interrupted at any point keeps the previous
  file (the existing interruption test, extended with a marker capture); a `VIEW` of version 3 survives a save
  by this reader unchanged.
- [X] T063 [P] [US8] `crates/host/tests/make_compat_fixtures.rs`: add an `#[ignore]`d generator that writes one new fixture
  `crates/host/tests/fixtures/markers-v2.lasession` (both kinds, colours, comments, origin) with the
  current code, run it once, commit the file, and leave a comment that it is not to be re-run; the
  compat test of T062 reads it and compares with hand-written expectations (not with what the code
  produces now).

### Implementation for User Story 8

- [X] T064 [US8] `crates/host/src/sessions.rs`: whatever T062 and T063 show to be missing after T010/T012 (the warning and
  dropping of an invalid or version-1 section, the writing conditions, the unsaved flag of marker commands,
  restoring `next_*` and `origin`, the validation against the capture length at open). T062 passes.

---

## Phase 12: Polish & Cross-Cutting Concerns

- [X] T065 [P] `crates/host/tests/random_walk.rs`: add to the action alphabet `BeginPlacement(Single|Pair)`,
  `FixPlacement` with random pointers (lane `Some`/`None`, beyond-edge and NaN `x`), `CancelPlacement`,
  `MoveMarkerEnd`, `EndMarkerDrag`, `PlacementZoomTo`, `SetMarkerColour`, `SetMarkerComment`,
  `SelectMarker`, `SetTimeOrigin`, `ResetTimeOrigin`, `CutLeft`, `CutRight`, `TrimToPair` (including
  `CancelOperation` mid-crop), `ScrollToMarker`, `FitToPair`, `SplitPair`, `MakePair`, `RemoveMarker`, save and open;
  after every action `Markers::check(length)` holds, ids and numbers are unique per kind, no marker lies
  outside the capture, `snap_zoom` is `None` whenever `placement` is `None` and no drag is under way, and
  nothing panics (SC-005: at least 100,000 actions; keep the "did not settle" diagnostics already in the
  test). A crop runs on a worker thread: after every crop action the walk polls the shell until `activity` is
  `Idle` again (with a bounded number of polls, failing with the diagnostics of the "did not settle" check
  if it does not), and a `CancelOperation` is sent at a random point of some crops.
- [X] T066 [P] `crates/host/benches/frame_plan.rs` and `crates/host/benches/baseline.json`: a case with 64 markers (32
  pairs) on a large capture and a case for `resolve_pointer` with 10⁶ transitions in the snapping zone;
  record the baselines; the class stays O(log N + W·(1 + log(1 + N_w/W))) + O(M).
- [X] T067 [P] `docs/host-api.md`: the text of the Markers sections matching the contract (state, placement,
  the pointer query, the dwell zoom, the crop operation, the `VIEW` version 2 and the ordering), the removed
  commands gone, the table already right (T011); `docs/user-guide.md`: rewrite "Markers and measuring" for
  the two kinds, placing by mouse, the dotted line, snapping and the dwell zoom, the actions, colours,
  comments, the time origin, cutting and trimming (with the warning that it cannot be undone), the keys, the
  limit of 64, and the sentence that files made before this version open without their markers; `README.md`
  mention if it lists markers.
- [X] T068 [P] Layout check: the existing language/layout test harness (features 006/014) extended with the Markers
  view in every language at 220 px and at the smallest window with markers of both kinds, long comments, the
  action list open and the confirmation dialog: nothing clipped or overlapping; fix the strings or the layout
  where it fails.
- [X] T069 [P] `crates/host/tests/containment.rs`/`error_sweep.rs`: every new error code and notice has a localized
  message in six languages and a log entry; a panic injected into the crop worker and into `resolve_pointer`
  is contained and leaves the state intact.
- [X] T070 `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo +1.85 check -p
  la-host -p la-storage`, `cargo test --workspace`, `cargo deny check` on this system; push and confirm CI on
  Windows and Debian is green (the same MSRV job as before).
- [X] T071 Walk through `quickstart.md` by hand on both systems with the simulated device and write the results,
  with the commands run and the test names, into `specs/015-markers-rework/verification.md` (started in T002);
  record in it every deviation from the spec and decision taken during the work.
- [X] T072 `roadmap.md`: mark 015 as merged when the pull request is merged; check that `specs/006-…` and
  `specs/014-…` still read correctly next to this feature (the pointers added with the spec); update
  `specs/015-markers-rework/checklists/requirements.md` if a requirement changed during the work.

---

## Dependencies & Execution Order

### Phase dependencies

- **Setup (1)** first. **Foundational (2)** blocks everything: it removes the old marker API.
- **US1 (3)** needs Phase 2. **US2 (4)** needs US1 (it extends the placement and the drawing). **US9 (5)** needs
  US1 (`resolve_pointer`, the overlay) and is best after US2 (the pair restores after each click).
- **US7 (6)** needs US1 and US2 (what its rows show). **US3, US4, US5, US6 (7–10)** need US7 (the action list);
  US4 and US5 share the crop operation (T049 comes first); US6 only needs the model and US7's list.
- **US8 (11)** needs Phase 2 and is meaningful after US1/US2; run it any time after Phase 5.
- **Polish (12)** after everything.

### Within each story

- Tests first, and they fail; then the host, then the strings, then the interface.
- In Phase 2, T003–T006 are written first, then T007–T013 in order (T013 last: it needs the host of T012).

### Parallel opportunities

- Phase 2: T003, T004, T005, T006 together; then T007, T008, T009, T010 are independent of each other
  (storage, i18n, host model, view section); T011–T013 are sequential.
- US1: T014, T015, T016 together; T019 and T020 any time after T012.
- US2: T022–T025 together; T027 with T026.
- US9: T029, T030, T031 together; T034 with T032–T033.
- US3–US6: the strings (T043, T050, T055, T060) in parallel with the host tasks of their story.
- Polish: T065–T069 together.

## Implementation Strategy

### MVP first

Setup → Foundational → **US1**: a single marker placed with the mouse, drawn with the pentagon and solid line,
dragged, removed and saved. Stop and validate with quickstart section 2 steps 1–2. US2 and US9 complete the P1
set (a pair, exact placement) and are the first thing that is a replacement for what feature 006 offered;
do not merge before they are done.

### Incremental delivery

US7 gives the final panel, US3–US6 add the actions, US8 closes saving, Polish verifies and documents. One pull
request for the feature (as 007, 008 and 014), with a commit per task or logical group.

## Notes

- [P] tasks = different files, no dependencies.
- Expected values are by hand, from the contracts.
- Never commit with failing clippy, fmt or tests.
- The old marker API is removed in Phase 2 (T011–T013) and not earlier or later: until then nothing else can
  compile against the new types, and after it nothing may use the old ones.
- No code to read `VIEW` version 1 is written (spec FR-024); T062 proves such a file still opens.
