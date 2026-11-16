# Data Model: Decoder Framework and Annotations

**Feature**: 007-decoder-framework-annotations | **Date**: 2026-11-15

The model is plain data. Nothing here depends on a user interface toolkit. Types are described by what
they hold and the rules they obey; the Rust spelling is decided in implementation. "Position" always
means a sample position of the capture (a `u64`), "channel" a channel number (`u8`, below 32).

## Limits (defaults; the settable ones are marked)

| Limit | Value | Where it applies |
|-------|-------|------------------|
| Decoder instances per capture | 8 | FR-003; `decoder.too_many_instances` |
| Stack depth (instances in a chain) | 4 | FR-018; `decoder.stack_too_deep` |
| Annotations per instance | 5,000,000 | FR-011; `decoder.annotation_limit` |
| Decoder results memory (all instances together) | 256 MiB, **setting**, range 16 MiB to 4 GiB | FR-011; `decoder.memory_limit` |
| Annotation text | 256 bytes of UTF-8 (literal) | FR-009 |
| Message arguments per annotation | 4, each at most 64 bytes | FR-009 |
| Output item payload | at most 64 bytes | FR-009 |
| Instance name | 64 characters, sanitized like channel names | FR-002 |
| Chunk | at most 65,536 edges; a bounded position span | R6 |
| Chunk deadline | 5 s | FR-021 |
| Cancel deadline | 1 s | FR-012, FR-021 |
| Abandoned (stuck) workers before new passes are refused | 4 | `decoder.too_many_stuck` |
| Decode threads running at once | `max(1, cores / 2)` | R8 |
| Rows per lane in the plan | 4 | R5 |
| Unsorted tail of the index, runs of the index | 4,096 annotations, 16 runs | R4 |

## Decoder descriptor (static, defined by the decoder)

What a decoder says about itself. Immutable; registered once in a registry keyed by `id`.

| Field | Meaning and rules |
|-------|-------------------|
| `id` | stable identifier, ASCII lowercase letters, digits, `.` and `_`, at most 48 characters, unique |
| `version` | `u16`, starts at 1; bumped when saved options or behaviour change incompatibly |
| `name`, `description` | message keys (static) for the localized name and description |
| `signals` | ordered list of signal specs (below), at most 16 |
| `options` | ordered list of option specs (below), at most 16 |
| `input` | `Signals` (reads channels) or `Items(kind)` (reads the output of another instance) |
| `outputs` | list of item kinds it produces (possibly empty) |
| `generator` | whether a generator exists for it (R14) |

**Signal spec**: `role` (identifier, unique in the decoder), `label` (message key), `required`
(boolean). **Option spec**: `id` (identifier, unique), `label` (message key), a type with its rules and
a default:

| Type | Rules |
|------|-------|
| `Choice` | a non-empty list of `(value id, label key)`; the default is one of them |
| `Integer` | inclusive `min` and `max` and a default inside them |
| `Boolean` | a default |

A descriptor is validated when registered (identifiers well-formed and unique, defaults inside their
ranges, counts within limits); an invalid descriptor is refused and logged, and the decoder does not
appear (a developer error, never reaching the user as a crash).

## Decoder setup (what the user chooses for one instance)

| Field | Meaning and rules |
|-------|-------------------|
| `instance` | the instance id (below) |
| `decoder` | descriptor id; must be registered |
| `name` | the instance's name; sanitized, at most 64 characters; empty means the localized default of the decoder |
| `source` | `Capture`, or `Instance(id)` for a stack; must match the descriptor's `input` |
| `channels` | for each signal role, a channel of the capture or none; required roles must be assigned; two roles of one instance may not share a channel unless the descriptor allows it (a flag per signal, default no) |
| `options` | for each option, a value of its type inside its rules; missing options take the default |
| `lane_visible` | whether the lane is shown |
| `generator` | the generator's own options (for example deliberate errors on or off and their rate), kept with the instance |

Validation of a setup (`decoder.invalid_setup` carries which setting and why): unknown decoder,
unknown or out-of-range channel (against the capture's channel count, or the device's before a
capture exists), missing required role, a shared channel where it is not allowed, an option of the
wrong type or outside its range, a source that does not exist, produces nothing the decoder accepts,
or closes a cycle (`decoder.stack_cycle`, `decoder.stack_incompatible`, `decoder.stack_too_deep`).

## Decoder instance (a decoder in use on the capture on screen)

| Field | Meaning |
|-------|---------|
| `id` | `u32`, unique among the instances of the session, never reused |
| `setup` | the setup above |
| `setup_version` | increases with every change that invalidates results (channels, options, source, decoder) |
| `source_version` | the `setup_version`/`result_version` of the source that the current results were made from |
| `state` | see the state machine below |
| `progress` | covered position, capture length and the fraction processed |
| `results` | the annotation store and the item store |
| `fault` | the last fault (code, technical detail), if any |
| `flags` | `incomplete`, `outdated`, `loaded` |

### State machine

```text
            add / re-run / setup changed
 (new) ───────────────┐
                      ▼
                  Waiting ──(a decode thread is free)──▶ Running
                                                            │
        ┌───────────────────────┬───────────────────────────┤
        ▼                       ▼                           ▼
   Finished             Cancelled (kept, incomplete)     Failed (kept, incomplete)
   (complete, or "following": waits for a growing capture)
```

- `Running` includes *following*: the capture is still growing and the job waits for more data; it
  stays `Running` until the capture ends and the rest is processed, then becomes `Finished`.
- `Cancelled` and `Failed` keep the annotations produced so far, flagged `incomplete`. Both can be
  started again (`Waiting`).
- `outdated` is a flag, not a state: set on an instance whose results no longer match its source or
  setup (R12) and cleared when a new pass finishes. `loaded` is set when the results came from a file
  and cleared when a pass replaces them.
- Closing the capture, opening a file or starting a new capture cancels every job and removes all
  instances (after the unsaved-data question of feature 006); the setup is saved if the user saves.

## Annotation

| Field | Meaning and rules |
|-------|-------------------|
| `kind` | `Range { start, end }` with `start <= end` (a zero-length range is accepted and drawn as a point), `Point { at }`, or `Edge { at, rising }` |
| `target` | `Lane` (the instance's lane) or `Channel(c)` (an overlay on channel `c`; the channel must exist in the capture) |
| `style` | a colour index (0 to 7) and a shape (`Dot`, `Arrow`, `Bar`, `Flag`, `Cross`); a range ignores the shape; an edge defaults to `Arrow` |
| `text` | `Message { key, args }` (key a static identifier of the application's messages, at most 4 arguments) or `Literal(text)` (at most 256 bytes, sanitized for display) |
| `error` | boolean; error annotations are counted and drawn distinctly |
| `instance` | the owning instance (implied by the store) |

Validation when an annotation enters the store: positions below the capture length (or, for a
growing capture, below the length of the snapshot the chunk was cut from), `start <= end`, an
existing target channel, a style from the fixed sets, text within limits. A refused annotation is
counted and, once per pass, reported as `decoder.invalid_output`; it is never stored.

## Output item (what an upper decoder reads)

| Field | Meaning and rules |
|-------|-------------------|
| `kind` | an identifier the producing decoder declared in `outputs` |
| `start`, `end` | span in positions, `start <= end` |
| `payload` | an integer (`u64`), a short byte string, or a short text, at most 64 bytes |
| `error` | boolean, set when the item stands for an erroneous event |

Items are stored in emission order, which is non-decreasing in `end` (a decoder emits an item when the
event is complete); the framework refuses an item that would go backwards in `end`, which lets an upper
decoder read them as a stream and gives the watermark of R12.

## Annotation store (one per instance)

Holds the annotations and items of the latest results.

- **Index**: sorted runs with a maximum-end tree plus a small unsorted tail (R4). Window query returns
  the annotations that intersect `[a, b)`; rank query returns how many annotations start before a
  position; neighbour query returns the next or previous annotation by start (for navigation).
- **Accounting**: bytes in use and annotation count; both checked against the limits on every insert.
- **Coverage**: the covered position (everything before it has been processed), the incomplete regions
  (gaps and the places where a pass was cut), and the counts of refused annotations.
- **Snapshots**: the interface reads an immutable snapshot (the runs and the tail at a moment); writing
  never blocks reading beyond swapping a handle.

## Lane plan and overlay plan (what the frame plan carries)

Built per frame from one window query per instance; part of `FramePlan` (host API 1.1).

| Part | Fields |
|------|--------|
| Lane plan | the instance id and name, state flags, up to 4 rows; each row a list of cells ordered by column |
| Cell | `Span { from_column, to_column, text, error, style }`, `Mark { x, style, error, id }` (exact sub-pixel position and the annotation id for selection), `Dense { column, count, errors }` |
| Overlay plan | per channel, the marks and edges that target it: `Mark`, `Edge { x, rising, style, id }`, `Dense` |

Invariants: cells are sorted and do not overlap within a row; every annotation in the window is
represented by a cell, a span or a dense count (none dropped silently); the number of cells is at most
`4 · W` per lane and `2 · W` per channel; a `Dense` cell never replaces an error annotation without
reporting `errors > 0`.

## Persisted forms

`DECO` (setup) and `DANN` (results) sections of the session file: see `contracts/deco-section.md`. In
memory they are the decoder setups and the annotation stores above; on disk they are canonical, bounded
and validated on every read.

## Settings added (feature 006's settings file)

| Setting | Default | Rule |
|---------|---------|------|
| Decoder results memory limit (bytes) | 256 MiB | 16 MiB to 4 GiB; validated like the capture limit |
| Save decoded results with the session | off | boolean |
| Last used decoder options per decoder | none | optional, validated again before use (a damaged value falls back to the default) |
