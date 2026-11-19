# Research: Single and Pair Markers

Decisions taken before design. Each: decision, why, what else was weighed. Facts about the existing code
were read from the sources named in brackets on 2026-11-18.

## R1 — The marker model

**Decision**: one `Marker` type with a shape, in `la-host::markers`:

```text
Marker { id: u32, number: u32, colour: u8, comment: String, shape: Shape }
Shape  = Point(u64) | Interval { start: u64, end: u64 }      // start <= end, both < capture length
Kind   = Single | Pair                                       // derived from the shape
```

`id` is the identity (unique in the session, never reused; the commands use it). `number` is what the user
sees (`T3`, `P1`); there is one counter per kind, `next_number: [u32; 2]`, saved with the markers, so a
number is never given twice even after deletions or conversions (spec FR-006). The two are separate because
the id must also be unique across kinds, the number must not.

**Why**: the old `Marker { id, position, name }` is replaced, not extended: there is no name any more (the
name is derived from kind and number), and the selected pair (`selected: [Option<u32>; 2]`) goes. An enum
shape makes "a pair never has its start after its end" true by construction in the one place that builds
one (`Shape::interval(a, b)` sorts).

**Alternatives**: two collections (`singles`, `pairs`): rejected, every list (ordering, limit of 64,
selection, drawing, saving) would be written twice. A pair as two single markers with a link: rejected, a
half could be deleted or moved alone and the invariants multiply.

## R2 — Placement is state of the host, hover is not

**Decision**: `State.placement: Option<Placement>` with `Placement { kind, first: Option<u64> }` (`first` is
the half of a pair that is already fixed). Commands: `BeginPlacement(Kind)` (starting the same kind again
cancels it, which is the "press the button again" of the spec; starting the other kind switches),
`CancelPlacement`, `FixPlacement { pointer: Pointer }`. The pointer position while it moves is **not**
state: the shell offers a pure query `Shell::resolve_pointer(pointer) -> Resolved` (R3) which the interface
calls on every pointer move and draws from; `FixPlacement` runs the same function on the same input, so
what was drawn is what is fixed.

**Why**: placement must be testable without a screen (spec FR-026), so "the first click fixed a half and the
second ended it" lives in the host. But a pointer move is many events per second; making each a command
would bump the state version and invalidate caches. A query has no such cost.

**Alternatives**: placement kept in the application: rejected (untestable without the screen, and the
random-walk test could not cover it). Hover as state in the host: rejected (cache churn, R5).

## R3 — Resolving the pointer to a position (snapping)

**Decision**: `Pointer { x: f32, lane: Option<u8> }`, where `x` is in pixels from the left
of the signal area (the shell knows the window and the pixel width, `plan_pixels`) and `lane` is the channel
whose band the pointer is in (`None` over the ruler, the legend, empty space or a decoder lane). The
function:

1. `t = window.start + x · window.length / pixels`, rounded to the nearest sample and clamped to
   `0..length-1` (an empty capture gives 0). This is the "whole sample position" of FR-031.
2. If snapping is off, or `lane` is `None`, the result is `t` with no highlight (FR-033).
3. Else `radius = 12 · window.length / pixels` samples (at least 1; the value feature 006 used). The
   candidate is the transition of that channel nearest to `t` within `radius` (ties: the earlier one),
   found with the indexed `previous_transition`/`next_transition` of `la-storage` (O(log N) each), not by
   iterating the transitions in between; the existing `nearest_transition` helper is rewritten that way
   (its current `.last()` walks every transition between the pointer and the answer, which at a crowded
   zoom is up to millions).
4. The result is `Resolved { position, snapped: Option<{channel, position}>, crowded: bool }`; `snapped` is
   what the view highlights; `crowded` is true when the zone holds a second transition (found with one more
   `next_transition` after the nearest, O(log N)), which is what lets the interface start the dwell timer
   of R4.

**Real edges only**: a transition next to a gap (lost samples) is not a measured edge (feature 014 FR-016:
`real_edge_before/after`); snapping skips them with the same `is_real` test, so a marker is never put on an
artefact of a gap.

**Why**: this gives FR-032 and FR-033 exactly: only the lane under the pointer attracts, and the cost is
O(log N) per move.

**Alternatives**: snapping in the application from the frame plan's pixel summaries: rejected (the summary
has no exact positions at a crowded zoom, which is the very case the requirement is about).

## R4 — Reaching every transition when a pixel holds many samples: dwell, then zoom in

**Problem**: the pointer moves by whole pixels, so at 10 samples per pixel `t` has a step of 10 samples; with
transitions 1–3 samples apart only the one nearest to each `t` can be chosen and the others are unreachable
(SC-008).

**Decision** (the owner's, 2026-11-18): when the zone of the pointer is crowded (`Resolved.crowded`) and the
pointer rests (moves less than 3 px) for 2 seconds, the view zooms in smoothly around the pointer until the
transitions near it are far enough apart to have zones of their own; when the marker is fixed or the
placement ends, the previous zoom and position are restored.

- **Target**: a pure query `Shell::crowded_zoom_target(Pointer) -> Option<ZoomTarget { start, length }>`.
  Let `t` be the resolved sample under the pointer and `g` the smallest gap among the transitions of the lane
  nearest to `t` (up to two before and two after, from `previous_transition`/`next_transition`; O(log N));
  the target has `samples_per_px = g / (2 · 12)` (two snapping distances of 12 px between neighbours), the
  length `pixels · samples_per_px` clamped by `WindowLimits` (so never beyond the maximum zoom), and `start`
  such that `t` stays under the same pixel `x`. `None` when the zone is not crowded or the window is already
  at least that close.
- **Animation**: the interface owns the time: it tracks the stillness and the 2 s dwell (a subscription
  tick), then for about 0.4 s sends `PlacementZoomTo { start, length }` per frame with an eased
  interpolation from the current window to the target. The host does the arithmetic and the testing of the
  restore; the interface does the clock.
- **Saved view**: the first `PlacementZoomTo` stores the window it replaces in `State.snap_zoom`
  (`Option<(start, length)>`); `FixPlacement`, `CancelPlacement`, a new `BeginPlacement`, `EndMarkerDrag`,
  and every command that ends placement restore it at once and clear it (clamped to the capture, which may
  have grown). Any command that changes the window by the user's hand (`Zoom`, `Pan`, `ZoomToRange`,
  `ScrollToEdge`, `ScrollToMarker`, `FitToPair`, the keys for start and end of the capture, …) clears
  `snap_zoom` without restoring: the view is the user's.
- **Dragging**: a drag of a fixed marker is not a placement, so it ends with a command `EndMarkerDrag`
  sent on release (it restores like a fix). Same dwell and zoom.
- **Both placement and drag** use the same functions; nothing in the frame plan depends on hover, so the
  cached layer is rebuilt only when the window changes (each animation frame is a window change, as a zoom
  by the wheel is).

**Why**: it works at every zoom and with any pointing device, needs no key to remember, and matches how a
person looks for an edge ("hold still, it zooms"). The arithmetic and the restore are in the host, so tests
need no screen or clock.

**Alternatives**: arrow keys stepping between the candidates (the first proposal, in the first version of
this plan): hidden and needing a keyboard while the mouse is busy; the owner chose the dwell zoom. Mapping
sub-pixel motion to candidates: mice report whole device pixels. A permanent magnifier: more drawing code
for the same result.

## R5 — What is drawn, and in which layer

**Decision**:

- *Fixed marker* (from `FramePlan.markers`, drawn in the cached layer with the lanes): a **solid** 1.5 px line
  across the signal area in the marker's colour; the **mark** at the top, in the ruler strip. Single: a
  pentagon-arrow pointing down — a rectangle 14 px wide and 9 px high above a triangle 5 px high, the tip
  on the line; filled with the colour, a 1 px outline in the text colour for contrast. Pair: two lines; a
  **square** 12 px × 12 px, filled, touching its line from the left for the start and from the right for the
  end, with the same outline. The name (`T3`, `P1`) is written next to the mark; times are in the panel and
  in the tooltip (as before).
- *Unfixed marker / placement* (overlay layer, redrawn on pointer moves, never invalidates the cache): a
  **dotted** line (1.5 px, 4 px dash, 3 px gap) in the colour of the marker about to be placed, with its
  mark at the top; the half already fixed of a pair is drawn solid. While the second half is placed the mark
  of each half is on the side of its role (FR-030): the half at the left of the other has its square on the
  left of its line, the other on the right; the pointer line is the start when the pointer is left of the
  fixed half, else the end. The **highlight** of the snapped transition: a ring (6 px) around the edge of the
  lane at the snapped position plus a thin vertical tick on the lane, in the marker's colour; it is drawn
  only while `Resolved.snapped` is `Some`.
- A drag of a fixed marker uses the same overlay (dotted while dragging, solid when released).

**Colours**: eight, added to `theme.rs` for light, dark and high contrast, chosen with the legibility checks
of feature 006 (FR-013: against the lane background and against each other, not by colour alone). The
colour index (`0..8`) is what the host stores; a new marker takes index 0 (spec FR-018).

**Why**: the cache is rebuilt only when plan, theme, language or layout change (feature 014); pointer
motion must not trigger that.

## R6 — Cut left, cut right and trim: a cropped copy, in the background

**Decision**: `la-storage::crop(&Snapshot, start, end, &Progress) -> Result<Capture>` builds a new closed
capture of the positions `start..end` with the `CaptureRestorer`:

- length `end - start`; for each channel, the level at `start` (`level_at`) is the initial level and every
  transition `p` in `start < p < end` is pushed as `p - start` (a transition exactly at `start` is not a
  transition of the copy: it is folded into the initial level);
- the gaps clipped to the range and shifted; the status flags (`stopped_by_limit`, `interrupted`) kept, so
  a capture that was incomplete stays marked so; the sample rate and the channel count unchanged;
- it checks the memory budget first (`check_budget` with the counts it will push) and refuses with
  `storage.memory_limit` before copying, since the original stays alive until the copy replaces it;
- it polls the progress/cancel token per channel and per block of transitions, so it is cancellable.

The host runs it on a worker thread as a new `Activity::Cropping` (next to `Saving` and `Loading`, with the
same progress and Cancel in the status line); at the end the shell swaps the capture in **one** state
change: `capture` replaced, markers remapped, the origin remapped, the window clamped to keep the same
span as far as it exists, `Unsaved::Dirty`, the decoders restarted (`clear_decoder_runtime()` then `restart_decoders(state)`, the path that a new capture
takes). Commands that change the capture wait while it runs (they are added to `waits_for_operation`).

Remap of a position `p` after keeping `start..end`: `p' = p - start` if `start <= p < end`, else the marker
is removed; for a pair, **both** ends must be inside to stay (a pair half inside is removed with the pair:
documented in the contract, with a notice). The marker the cut was made at stays: cut left at `m` keeps
`m..length`, so `m` goes to position 0; cut right at `m` keeps `0..=m`, i.e. `end = m + 1`, so that `m`
stays the last sample. A trim to a pair `a..=b` keeps `a..=b` (`end = b + 1`) and the pair sits at its
ends.

**Why**: transition-based storage cannot be cut in place (snapshots are immutable and shared), and the
restorer already enforces every invariant of a capture, so the copy cannot be malformed.

**Alternatives**: keeping the data and hiding what is outside (a "view range"): rejected, the spec says the
data is deleted (and the file shrinks); a lazy view over the old snapshot: rejected, every query would
need an offset and the next cut would stack another.

## R7 — The time origin

**Decision**: `Markers.origin: u64` is the sample position that counts as time zero (0 by default). It is
data of the view, saved in the `VIEW` section, never touched by anything but "count time from here" and its
reset, and remapped by the cuts (kept if its position remains, else 0). The axis builder takes it:
`build_ticks(window, pixels, rate, origin)` produces tick labels as seconds relative to the origin
(negative before it); the times of the markers shown by the panel and the tooltips are relative to it. The
duration of a pair is a difference and does not depend on it.

**To verify first** (task, with a test before the change): that `Localizer::format_duration` formats a
negative number of seconds correctly in every language (sign, unit chosen by magnitude, the locale's minus).
It was written for non-negative values; if it is not right, the fix goes in `la-i18n` with its own tests.

## R8 — The `VIEW` section, version 2

**Decision**: the payload version becomes **2** and is written and read by the new code only. Reading a
version 1 payload goes down the existing rejection path (`host.view_invalid`: the capture opens with default
presentation and no markers, a warning is shown, the section is dropped and not written back) — the spec's
FR-024 behaviour, with no code to read version 1. A payload of version greater than 2 is kept as it is (the
existing `Newer` rule). The channel part of the payload is unchanged; the marker part is replaced. Layout in
[contracts/view-section-2.md](contracts/view-section-2.md).

**Why**: a changed layout under the same version number would be mis-read as garbage by chance; a new
number makes the old one a clean, tested rejection.

## R9 — Host API 2.0: the command set

**Decision**: the eight marker commands of 1.2 (`PlaceMarker`, `MoveMarker`, `RenameMarker`,
`RemoveMarker`, `RemoveAllMarkers`, `SelectMarkers`, `GoToMarker`, `SetSnap`) are replaced, except
`SetSnap` and `RemoveMarker`, which stay with the same meaning. The new set and their exact behaviour is in
[contracts/host-api-2.0.md](contracts/host-api-2.0.md). `RemoveAllMarkers` stays (the menu entry exists).
`HOST_API_VERSION` and `docs/host-api.md` change to 2.0 in the same merge; the test that compares the
command table with the code (`host_api_doc.rs`) is updated first.

## R10 — Selection, ordering, limits

- **Selection**: `Markers.selected: Option<u32>` (one marker; the interface selects by clicking a mark or
  a row). The "make a pair" selection of two singles is **interface state** (checkmarks in the panel,
  `MakePair { first, second }` takes both ids): it is only an input to one command and needs no saving.
- **Ordering** for the list and for the previous/next jump: by the position of the (start) point, then by
  `id`. Computed on demand (M ≤ 64), not stored.
- **Limit**: 64 markers in all (a pair counts one); `BeginPlacement` is refused at the limit with the
  reason shown, so the user never goes through a placement that cannot end.
- **Conversions**: pair → two singles allocates two new single numbers and removes the pair; its pair number
  is not given again. Two singles → pair allocates a pair number and removes both singles; their single
  numbers are not given again. Both are one command (all or nothing), and fail when the limit would be
  exceeded (pair → two singles needs one more place).

## R11 — The window never extends beyond the capture

The window invariant of feature 006 (`WindowLimits`, `ViewWindow::is_valid`) keeps every window inside
`0..length`. "Scroll to" and "fit to the pair" therefore centre or pad as far as that allows (spec edge case
and FR-013/FR-016 say so). Changing the invariant to allow empty margins would touch zoom, pan, the plan
builder and the random walk for the sake of two actions near the ends, and is not done.
"Scroll to" is `ViewWindow::with_edge_at`-style arithmetic on the existing window type
(`reveal` is not enough: it only makes the position visible). "Fit" is `ViewWindow::zoom_to(start - margin,
end + margin)` where `margin = (mark width + gap) · samples per pixel`, found by a short fixed-point step
(the margin depends on the zoom it produces): two iterations are enough because the margin is a small
fraction of the window; the result is clamped by the existing zoom limits.

## R12 — The row menu and the context menu

**Decision**: each row of the Markers view has a button that opens the list of the actions of its kind
(plus colour and comment) as a small panel under the row; the same list opens at the pointer on a right
click of a mark in the signal view (drawn by the application with `stack!` over the canvas, closed by Esc
or a click outside). The colours are a row of eight swatches in that panel; the comment is a text input with
the limit of FR-018. One component serves both places.

**Why**: iced has no native context menu; one self-made overlay is cheaper than two, and `iced_test` can click
through it headlessly.

**Alternative**: all actions as visible buttons in every row: rejected, nine buttons in a 220 px panel do not
fit (spec SC-007).
