# Data Model: Signal Storage Core

All types live in the new crate `la-storage`. Nothing is persisted (file format is
roadmap item 003). Sizes are in bytes.

## Configuration

### CaptureConfig

| Field | Type | Validation |
|-------|------|------------|
| `channels` | `u8` | 1..=32, else `storage.invalid_channel_count` |
| `sample_rate_hz` | `u64` | > 0, else `storage.invalid_sample_rate` |
| `limits` | `Limits` | see below, else `storage.invalid_limits` |

### Limits

| Field | Type | Default | Validation |
|-------|------|---------|------------|
| `memory_budget_bytes` | `u64` | 1 GiB | >= 1 MiB |
| `max_chunk_samples` | `u32` | 65,536 | 1..=1,048,576 |
| `max_pixels` | `u32` | 16,384 | 1..=65,536 |

## Core entities

### Capture (producer handle)

Single owner, not `Clone`. Methods take `&mut self` (append, gap, close). State:

| Field | Meaning |
|-------|---------|
| `config` | as created |
| `length` | number of positions so far (samples + gap positions) |
| `previous_word` | last sample word, `None` before the first sample and right after a gap |
| `pending_resync` | a gap ended the data and no sample followed yet |
| `used_bytes` | exact memory accounting (research R6) |
| `status` | `CaptureStatus` |
| `channels` | per-channel builders: transition count, open (unsealed) block |
| `published` | the `Arc<Published>` readers see (swapped per chunk) |

State transitions of `status`:

```text
open --record_gap()--------> open, lost_positions += length   (can never become complete)
open --append() over budget-> open, stopped_by_limit = true   (that chunk rejected;
                                                               later appends also rejected;
                                                               stored data stays readable)
open --close()-------------> closed, complete = (lost_positions == 0 && !stopped_by_limit)
closed: append() and record_gap() fail with storage.closed
```

### CaptureStatus (value returned to readers)

| Field | Type | Meaning |
|-------|------|---------|
| `state` | `InProgress` / `Closed` | the producer has called `close()` or not |
| `complete` | `bool` | `Closed` AND no lost positions AND not stopped by limit AND not interrupted |
| `lost_positions` | `u64` | sum of all gap lengths |
| `stopped_by_limit` | `bool` | an append was rejected because of the memory budget |
| `interrupted` | `bool` | the capture was ended by a failure outside it via `close_interrupted()` (added by feature 004) |

`complete` is false for every capture that is not closed, so a capture is never
presented as complete before it is finished (FR-024).

### Block (immutable, shared)

| Field | Type | Notes |
|-------|------|-------|
| `base` | `u64` | position of the block's origin |
| `first_rank` | `u64` | number of transitions of the channel before this block |
| `offsets` | `Box<[u32]>` | 1..=1,024 strictly increasing offsets from `base` (transition position = `base + offset`) |

Invariants: offsets strictly increasing; `base + max offset` does not overflow;
a block is closed early when the next offset would not fit in `u32`. Size =
`4 × len + 32`.

### Segment

An append-only array of 256 `OnceLock<Arc<Block>>` slots; a channel owns a growing
list of segments. A slot is written once, before it is published.

### ChannelData (inside `Published`)

| Field | Meaning |
|-------|---------|
| `initial_level` | level of the first sample (bit of sample 0) |
| `segments` | shared list of segments |
| `sealed_blocks` | how many blocks are visible to this snapshot |
| `tail` | copy of the unsealed offsets (<= 1,024) with its `base` and `first_rank` |

### Gap

| Field | Type | Validation |
|-------|------|------------|
| `start` | `u64` | equals the capture length at the time of recording |
| `length` | `u64` | >= 1, `start + length` must not overflow, else `storage.invalid_gap` / `storage.position_overflow` |

Gaps are kept sorted and non-overlapping (they are appended at the current end, and
adjacent gaps are merged).

### Published (immutable snapshot payload)

`length`, per-channel `ChannelData`, `Arc<Vec<Gap>>`, `CaptureStatus`,
`used_bytes`. Created per published chunk; readers clone its `Arc`.

### Snapshot / CaptureReader

`CaptureReader` is `Clone + Send + Sync` and hands out `Snapshot`s (a clone of the
current `Arc<Published>`). All queries are methods of `Snapshot`; it never changes
after creation.

## Query result types

### LevelState

`Low`, `High`, `Missing` (no data: inside a gap or at or beyond the capture length).

### Edge

`position: u64`, `rising: bool` (`true` = low to high). The edge at `position` is the
change between sample `position - 1` and sample `position`.

### Activity (per pixel)

| Variant | Meaning |
|---------|---------|
| `None` | no transition in the column |
| `One(Edge)` | exactly one transition, with its exact position and direction |
| `Many { count: u32 }` | two or more transitions; `count` saturates at `u32::MAX` |

### PixelSummary

| Field | Type | Meaning |
|-------|------|---------|
| `start_level` | `LevelState` | level at sample `a_i` |
| `end_level` | `LevelState` | level at sample `a_{i+1}` |
| `activity` | `Activity` | see above |
| `missing_data` | `bool` | the closed range `[a_i, a_{i+1}]` touches a gap |
| `beyond_end` | `bool` | the whole column lies at or beyond the capture length |

Invariants (checked by tests): `end_level(i) == start_level(i+1)`; for a column
without `missing_data` and not touching the end,
`start_level XOR (count mod 2) == end_level`.

### ViewportResult

`pixels: Vec<PixelSummary>` (length = requested pixel count) and `stats: QueryStats`.

### QueryStats

`probes: u64` — the number of block-index and offset comparisons performed, used by the
deterministic complexity test (research R9). It is cheap, always collected, and has no
effect on results.

## Relationships

```text
Capture (1) ──publishes──> Published (many over time, immutable)
Published (1) ──> ChannelData (channels)
ChannelData (1) ──> Segment list ──> Block slots (shared, immutable)
CaptureReader (1) ──creates──> Snapshot (many) ──holds──> Published
Snapshot ──query──> ViewportResult / Edge iterator / LevelState
```
