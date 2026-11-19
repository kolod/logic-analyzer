# Contract: how the markers look and respond

What `la-app` draws and what it sends to the host. Sizes are in logical pixels (the interface scale applies).
Colours come from `theme.rs`: eight marker colours per theme (light, dark, high contrast), each legible
against the lane background and distinct from its neighbours (the legibility checks of feature 006).

## Marks and lines

| Item | Shape |
|------|-------|
| Single marker line | a solid 1.5 px line from the ruler to the bottom of the lanes, in the marker's colour |
| Single marker mark | a pentagon-arrow in the ruler strip: a 14 × 9 rectangle on a 5 px triangle pointing down, the tip on the line; filled with the colour, 1 px outline in the text colour |
| Pair marker lines | two solid lines as above |
| Pair marker marks | a 12 × 12 square filled with the colour, 1 px outline; for the **start** it touches its line from the **left**, for the **end** from the **right** |
| Name | `T3` / `P1` in the text colour next to the mark, on the side away from the other end of a pair; staggered one text line higher when it would overlap another marker's name |
| Selected marker | the lines 2.5 px and the outline 2 px |
| Tooltip on a mark | the name, the time (a pair: both times, the duration and the frequency) and the comment, from `marker_texts()` |

The marks are inside the ruler strip. A mark that would leave the signal area at its end (a marker at the
very start or end of the capture, or a pair fitted at an edge) is clipped by the area.

## Placement

1. The button or the shortcut sends `BeginPlacement(kind)`. While `State.placement` is `Some`, the status
   line says what is being placed ("Click to place the marker. Esc cancels." and, for the second half of a
   pair, "Click to place the second half.").
2. On every pointer move over the signal area the interface sends nothing; it calls
   `Shell::resolve_pointer(Pointer { x, lane })` and draws, in the overlay layer, the **dotted** line
   in the marker's colour at `Resolved.position` with its mark; for a pair, the fixed half is drawn solid,
   and the mark of each half sits on the side of its role (start: left of its line; end: right of its line),
   so it jumps when the pointer crosses the fixed half.
3. If `Resolved.snapped` is `Some`, the transition is highlighted in its lane: a 6 px ring around the edge,
   in the marker's colour, and a thin tick over the lane's height.
4. `lane` is the channel whose band contains the pointer's `y`; it is `None` over the ruler, the legend, the
   area between lanes, a decoder lane or outside the area.
5. **Dwell zoom** (research R4). The interface keeps the position of the pointer and the time it last moved
   by 3 px or more. When `Resolved.crowded` is true and the pointer has rested for 2 s (a subscription tick
   while a placement or a drag is active), it asks `Shell::crowded_zoom_target(pointer)` once; for a
   `Some(target)` it sends `PlacementZoomTo` once per frame for 0.4 s with an ease-out interpolation from
   the current window to the target (both `start` and `length` interpolated), and then waits. It does not ask
   again until the pointer has moved. Moving the pointer during the animation lets the animation finish
   (it is short); the wheel or any other window command ends it (and the host clears the saved view).
   The status line says "Zoomed in to find the edge. The view returns when you click or press Esc."
   while `State.snap_zoom` is `Some`. The restore is done by the host on the click or the cancel and shows
   at once.
6. A left click inside the signal area sends `FixPlacement { pointer }` with the same pointer as the last
   drawing. A click on the legend, the ruler's text or outside the area does nothing. Esc, the right
   mouse button, or the same add button sends `CancelPlacement` / `BeginPlacement(kind)` again.
7. The wheel zooms and Shift+wheel pans during placement (they are not captured by it); dragging the view
   to pan is disabled (the click would be ambiguous).
8. The pointer leaving the signal area hides the dotted line.

## Dragging a fixed marker

Pressing within 4 px of a line (or on its mark) selects the marker (`SelectMarker`) and starts a drag of
that end. The dragged line is drawn dotted like a placement, with the same snapping, highlight and dwell zoom;
every pointer move sends `MoveMarkerEnd` (the host does the resolution); releasing the button sends `EndMarkerDrag` and ends the drag
and the line is solid again. A press on the mark with the right button opens the action menu (below) at the
pointer and selects the marker.

## The Markers view

```text
┌───────────────────────────────┐
│ [▼ Single marker] [▯▯ Pair marker]   ← two buttons, each with its mark
│ ─────────────────────────────  │
│ ▼ T1  ■  Rising edge      12.5 µs   ⋯  🗑
│ ▯▯ P1  ■  Bit 3     1.25 µs · 800 kHz  ⋯  🗑
└───────────────────────────────┘
```

- The buttons are at the top and always visible. The one of the kind being placed is shown pressed; pressing
  it again cancels.
- One row per marker, in the order of `ordered()`: the kind's mark icon, the name, a colour swatch (opens
  the colours), the comment (truncated; full text in the tooltip), the time (a single) or the duration and
  the frequency (a pair), the `⋯` button, and the **bin** icon (`Icon::Trash`, the same widget, size and
  place at the right of the row as in the Decoders view; no confirmation).
- A click on a row selects the marker (and the mark in the view shows selected); the Delete key removes it.
- Checkmarks: a row of a single marker has a small checkbox; with exactly two single markers checked, the
  button "Make a pair" appears under the list (sending `MakePair`); the checks are cleared when it is
  used and when a checked marker disappears.
- Empty list: the text "No markers. Use the buttons above to add one."
- The `⋯` button opens the action list (below) under the row.

## The action list (the row's `⋯` and the right click on a mark)

An overlay panel of buttons with text, one per action, closed by Esc, by a click outside or after an action:

| Kind | Actions (send) |
|------|----------------|
| Single | Count time from here (`SetTimeOrigin`) — shown as "Time counts from the start" (`ResetTimeOrigin`) when this marker is the origin; Delete data to the left (`CutLeft`); Delete data to the right (`CutRight`); Scroll to it (`ScrollToMarker`) |
| Pair | Trim to the pair (`TrimToPair`); Fit the view to the pair (`FitToPair`); Convert to two single markers (`SplitPair`) |
| Both | a row of eight colour swatches (`SetMarkerColour`); a text field for the comment, limited to 80 characters (`SetMarkerComment` on every edit); Delete (`RemoveMarker`) |

The data-changing entries are disabled (greyed, with a tooltip saying why) while `activity` is not `Idle`
and when they would delete nothing. Their confirmation is a dialog built from the `Guarded::Crop` question:
"Delete the data to the left of T2? 1.2 ms (35%) will be lost; the capture will be 2.2 ms long. This cannot be
undone." with **Delete** and **Cancel**. While `Activity::Cropping` runs, the status line shows the progress
and **Cancel**.

## Menus and keys

| Where | Entry | Key |
|-------|-------|-----|
| Markers menu | Place single marker | M |
| | Place pair marker | Shift+M |
| | Cancel placement (enabled while placing) | Esc |
| | Previous marker / Next marker | `[` / `]` |
| | Remove marker (the selected one) | Delete |
| | Remove all markers | — |
| | Snap to transitions (checked) | S |

## Text (new keys, six languages; none for the names `T`/`P`)

Keys are added to `ui.ftl` under `marker-` and replace the unused `marker-default`, `marker-measure-first`,
`marker-measure-second`, `measure-pick`: the two add buttons, the status lines of placement, the action
names, the confirmation texts (with `{ $side }`, `{ $lost }`, `{ $percent }`, `{ $kept }`), the empty-list
text, the tooltips of the swatches (colour names), and `measure-duration`, `measure-frequency` for pairs
(the existing `measure-distance`, `measure-frequency` are reused where their text fits). The parity check
of feature 005 must pass, with long German and Japanese layout tests at the smallest panel width (220 px).
