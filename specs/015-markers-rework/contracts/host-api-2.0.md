# Contract: host API 1.2 → 2.0

A **major** version: commands and state fields are removed. `HOST_API_VERSION` and the version line of
`docs/host-api.md` become `2.0` in the same merge as the implementation; the command table of the document is
rewritten for the markers (the test `host_api_doc.rs` compares it with the code) and the interface
(`la-app`) is changed in the same merge. There is no release to keep compatible with.

## Removed

```text
State.markers.selected: [Option<u32>; 2]      // the measured pair
Marker.position, Marker.name                  // the old marker
Command::PlaceMarker { position, channel }
Command::MoveMarker { id, position, channel }  // replaced, see below
Command::RenameMarker { id, name }
Command::SelectMarkers { first, second }
Command::GoToMarker(u32)                      // replaced by ScrollToMarker
Shell::measurement_text()                     // replaced by Shell::marker_texts()
```

Kept unchanged: `RemoveMarker(u32)`, `RemoveAllMarkers`, `SetSnap(bool)`.

## State

```text
State.markers: Markers          // data-model.md
State.placement: Option<Placement>
State.snap_zoom: Option<(u64, u64)>      // the window to restore after an automatic zoom
Activity::Cropping              // a cut or a trim is running (with progress; `CancelOperation` stops it)
FramePlan.markers: Vec<MarkerPlan>   // data-model.md
```

## Queries (no state change)

```text
Shell::resolve_pointer(Pointer) -> Resolved      // research R3; O(log N); total
Shell::crowded_zoom_target(Pointer) -> Option<ZoomTarget>   // research R4; O(log N)
Shell::marker_texts() -> Vec<MarkerText>         // localized, in list order
MarkerText { id, name, time_a: String, time_b: Option<String>,
             duration: Option<String>, frequency: Option<String> }
```

`time_*` are relative to the origin (negative before it), formatted by the localizer; `duration` and
`frequency` exist only for a pair, `frequency` not for a duration of zero; the frequency keeps only the digits
the distance resolves (the rule of 1.2).

## Commands

```text
BeginPlacement(Kind)                      Kind: Single | Pair
CancelPlacement
FixPlacement { pointer: Pointer }
MoveMarkerEnd { id: u32, end: End, pointer: Pointer }     End: Point | Start | End
EndMarkerDrag                             // the button was released: restores an automatic zoom
PlacementZoomTo { start: u64, length: u64 }  // one frame of the automatic zoom of research R4
SetMarkerColour { id: u32, colour: u8 }
SetMarkerComment { id: u32, comment: String }
SelectMarker(Option<u32>)
SetTimeOrigin(u32)                        // a single marker's id
ResetTimeOrigin
CutLeft(u32)                              // a single marker's id
CutRight(u32)                             // a single marker's id
TrimToPair(u32)                           // a pair's id
ScrollToMarker(u32)                       // any marker; a pair is centred on its start
FitToPair(u32)                            // a pair's id
SplitPair(u32)                            // a pair's id → two single markers
MakePair { first: u32, second: u32 }      // two single markers' ids → one pair
```

`SetSnap`, `RemoveMarker` and `RemoveAllMarkers` are as before. Every command that is not allowed now returns
`Outcome::Ignored(host.invalid_command)` and changes nothing (rule 4 of the document).

### Exactly

- **BeginPlacement(kind)**: ignored when there is no capture, 64 markers exist, or `activity` is `Saving`,
  `Loading` or `Cropping`. Allowed while `Capturing`. The state machine is in `data-model.md`. Selecting
  The selection is unchanged.
- **FixPlacement { pointer }**: ignored when `placement` is `None`. Resolves the pointer with
  `resolve_pointer` (same function, same input as the drawing used), then follows the state machine. A new
  marker takes `id = last_id + 1`, the next number of its kind, colour 0, an empty comment, and becomes
  `selected`. Marks the capture unsaved.
- **PlacementZoomTo { start, length }**: ignored unless a placement is active or a drag is under way
  (the interface sends `MoveMarkerEnd` first; the host remembers `dragging: Option<u32>`); sets the window
  (clamped by the zoom limits; `auto_fit` and `follow_latest` become false); if `snap_zoom` is `None` it
  first stores the window it replaces. Logged at debug level.
- **Restore**: `FixPlacement` (every click), `CancelPlacement`, `BeginPlacement`, `EndMarkerDrag`, and the
  start of any command that ends placement (open, new capture, start, cut, quit) put `snap_zoom` back as the
  window (clamped to the current capture) and clear it. Any command that changes the window by hand
  (`Zoom`, `Pan`, `ZoomToRange`, `ScrollToEdge`, `ScrollToMarker`, `FitToPair`, `GoToStart`, `GoToEnd`, …)
  clears `snap_zoom` without restoring.
- **MoveMarkerEnd**: the end named must exist for the shape (`Point` for a single; `Start`/`End` for a
  pair); resolves the pointer; sets the position; if an interval's ends cross, they are exchanged so that
  `start <= end` (the id and the number stay). Logged at debug level (a drag sends many).
- **SetMarkerColour**: `colour < 8` else ignored. **SetMarkerComment**: sanitized to 80 characters.
- **SetTimeOrigin(id)**: only a single marker; sets `origin`. **ResetTimeOrigin**: `origin = 0`; ignored when
  already 0. Neither changes data, both mark the capture unsaved (it is part of the saved view).
- **CutLeft(id) / CutRight(id) / TrimToPair(id)**: ignored while `activity` is not `Idle` or when the cut
  would delete nothing or leave nothing; otherwise the shell asks the user (the existing guarded question
  path, a new `Guarded::Crop { .. }` carrying the span kept and the span lost so the dialog can say them;
  the dialog's **Delete** is `Decision::Discard` and its **Cancel** is `Decision::Cancel`; `Guarded::Crop` is
  never answered by the remembered unsaved-data choice, like `Guarded::RemoveDecoder`),
  and on yes starts the background operation of research R6 (`Activity::Cropping`, `Changes::ACTIVITY |
  Changes::PROGRESS`); `CancelOperation` (the command that already cancels a save or a load) stops it and leaves everything as it was. On success, in one state change:
  new capture (`Changes::CAPTURE`), markers remapped (`Markers::remap`), window clamped keeping the same span
  as far as it exists, decoders restarted (`clear_decoder_runtime()`, then `restart_decoders(state)`: the old results and the annotation selection are gone, a pass starts for every instance that reads the capture and the instances stacked on them follow), `Unsaved::Dirty`, the placement dropped. A pair with only one end inside the kept range is removed with a
  `Severity::Info` notice (`marker-cut-pair-removed`, with the number of pairs removed) when at least one was.
  On failure: a
  notice with the storage error's code, nothing changed.
- **ScrollToMarker(id)**: window of the same length centred on the marker (clamped to the capture);
  `follow_latest` and `auto_fit` become false (as `GoToMarker` did).
- **FitToPair(id)**: window per research R11; same flags.
- **SplitPair(id)**, **MakePair { first, second }**: as `Markers::split` / `join`; both ignored when they would
  exceed 64 markers or when the ids are not what the command needs.
- **Shortcuts** (`shortcuts.rs`, each an `Action` with a menu path; the coverage test requires every
  command to be reachable): `PlaceSingleMarker` (M) → `BeginPlacement(Single)`; `PlacePairMarker` (Shift+M) →
  `BeginPlacement(Pair)`; `CancelPlacement` (Esc, enabled only while placing); `PreviousMarker`/`NextMarker`
  (`[`, `]`) → `ScrollToMarker` of `jump_target`; `RemoveSelectedMarker` (Delete) → `RemoveMarker(selected)`;
  `ToggleSnap` (S).

## `changes` and versions

`Changes::MARKERS` is raised by every command above except the queries; `placement` changes raise
`Changes::MARKERS` too (the panel shows "placing…"). `versions.markers` keys the caches of the frame plan.
Hover does not raise anything (it is a query).
