# Data Model: Single and Pair Markers

**Feature**: [spec.md](spec.md) | **Plan**: [plan.md](plan.md) | **Decisions**: [research.md](research.md)

All types live in `la-host` unless stated. They are plain data with no toolkit types. Positions are sample
positions of the capture on screen (`u64`, always `< length`, or `0` for an empty capture).

## `Markers` (in `State.markers`)

| Field | Type | Meaning |
|-------|------|---------|
| `list` | `Vec<Marker>` | the markers in creation order; at most `MAX_MARKERS` = 64 |
| `selected` | `Option<u32>` | the id of the selected marker |
| `origin` | `u64` | the position that is time zero for the axis and the marker times; 0 = the start |
| `last_id` | `u32` | the highest identity ever given (the next marker gets one more); never reused in the session |
| `next_number` | `[u32; 2]` | the next number of each kind (`[single, pair]`), starting at 1; never reused |

Invariants (checked by a function `Markers::check(length)` used in tests and after every command in the
random walk): ids are unique; every position is `< max(length, 1)`; for an interval `start <= end`; numbers
are unique within a kind and below `next_number`; `origin < max(length, 1)`; `list.len() <= 64`;
`selected` names an existing marker or is `None`.

## `Marker`

| Field | Type | Meaning |
|-------|------|---------|
| `id` | `u32` | identity |
| `number` | `u32` | the number within its kind (`T3` is a single with number 3) |
| `colour` | `u8` | index into the eight marker colours, `0..MARKER_COLOURS` |
| `comment` | `String` | sanitized with `sanitize_text(…, MAX_COMMENT_CHARS = 80)`; may be empty |
| `shape` | `Shape` | `Point(u64)` or `Interval { start: u64, end: u64 }` |

Derived, never stored: the kind (from the shape); the name (`"T"`/`"P"` + number); for an interval the
`Measurement` (distance in samples, seconds, hertz; `la-host::markers::measurement`, kept as it is, with
`hertz = None` for a distance of zero) and its text in the current language (`MeasurementText`, kept).

## `Placement` (in `State.placement`)

`Option<Placement>`; `None` when no marker is being placed.

| Field | Type | Meaning |
|-------|------|---------|
| `kind` | `Kind` | `Single` or `Pair` |
| `first` | `Option<u64>` | the half of a pair that is already fixed (always `None` for a single) |
| `colour` | `u8` | the colour the marker will have: the first of the palette (`0`) |

`State.snap_zoom: Option<(u64, u64)>` (outside `Placement`, because a drag uses it too) holds the window
`(start, length)` that an automatic zoom replaced and must put back (research R4).

Transitions:

```text
None --BeginPlacement(Single)--> Placement{Single, None}
None --BeginPlacement(Pair)----> Placement{Pair, None}
Placement{k, _} --BeginPlacement(k)--> None                      (the same button again cancels)
Placement{k, _} --BeginPlacement(other)--> Placement{other, None} (switches; a fixed half is dropped)
Placement{Single, None} --FixPlacement--> None + a Point marker
Placement{Pair, None}   --FixPlacement--> Placement{Pair, Some(p)}
Placement{Pair, Some(a)} --FixPlacement(b)--> None + an Interval marker {min(a,b), max(a,b)}
any --CancelPlacement--> None
any --OpenSession | NewCapture | StartCapture | Quit | cut/trim start--> None   (dropped, nothing added)
```

A placement refuses to start (the command is ignored with `host.invalid_command`, and the interface says
why) when there is no capture, when 64 markers exist, or when a save, a load or a crop is running.

## `Pointer`, `Resolved` (the query of research R3 and R4)

```text
Pointer  { x: f32, lane: Option<u8> }             // x: pixels from the left of the signal area;
                                                   // lane: the channel whose band the pointer is in
Resolved { position: u64, snapped: Option<Snapped>, crowded: bool }
Snapped  { channel: u8, position: u64 }            // the transition to highlight (== position)
                                                   // crowded: the zone holds a second transition
ZoomTarget { start: u64, length: u64 }             // the window of the automatic zoom (R4)
```

`resolve_pointer` is total: any `x` (negative, beyond the width, NaN) gives a position inside the capture; NaN
is read as 0.

## Pure rules (all unit-tested without a shell)

| Function | Rule |
|----------|------|
| `Markers::add(shape) -> Option<u32>` | `None` at the limit; allocates `id` and the next number of the kind; colour 0 |
| `Shape::interval(a, b)` | sorts the two ends |
| `Markers::move_end(id, end, position)` | `end` is `Point`, `Start` or `End`; moving past the other end of an interval swaps the roles (the result is re-sorted) |
| `Markers::set_origin(id)` | `origin = position of the single marker id`; refuses an interval |
| `Markers::split(id)` | an interval becomes two points at its ends, with two new single numbers, the colour, the comment on the first; refuses at 63 markers or more |
| `Markers::join(a, b)` | two points become one interval (sorted), with a new pair number, the colour and comment of the earlier; refuses anything but two different points |
| `Markers::remap(start, end)` | keeps `start..end`: every position `p` becomes `p - start`; a point outside is removed; an interval is removed unless both ends are inside; `origin` becomes `origin - start` if inside, else 0; `selected` cleared if removed; `next_*` unchanged |
| `crowded_zoom_target(snapshot, window, limits, pointer)` | R4: the window at which the transitions near the pointer are two snapping distances apart, the point under the pointer kept; `None` if not crowded |
| `ordered(&Markers) -> Vec<&Marker>` | by start position, then id |
| `jump_target(&Markers, from, direction)` | the previous or next marker position (an interval counts at its start), wrapping round at the ends |
| `fit_window(limits, start, end, mark_px)` | the window of research R11 |
| `centre_window(limits, window, position)` | the window with the same length whose centre is `position`, clamped |

## The frame plan

`MarkerPlan` (was: id, position, x, name) becomes:

| Field | Type | Meaning |
|-------|------|---------|
| `id` | `u32` | identity |
| `name` | `String` | `T3`, `P1` (never translated) |
| `colour` | `u8` | palette index |
| `comment` | `String` | shown as the tooltip |
| `selected` | `bool` | |
| `shape` | `PlanShape` | `Point { x: f64 }` or `Interval { start_x: f64, end_x: f64 }`, x in pixels from the left of the signal area (a value outside `0..width` means that end is out of the window; the marker is in the plan when any part of it is inside, so the half-visible pair still draws its inside line) |

`FramePlan.axis` ticks are labelled relative to `origin`.

## Session data

`VIEW` payload version 2: [contracts/view-section-2.md](contracts/view-section-2.md). `State.placement` is
not saved. `snap` stays a setting (`snap_markers`, default on).
