# Data Model: Session File Format

Entities, fields, validation rules and states. Byte-level layout is in
[contracts/file-format.md](contracts/file-format.md); Rust signatures in
[contracts/session-api.md](contracts/session-api.md).

## Session (in memory)

The unit that is saved and loaded.

| Field | Type | Notes |
|-------|------|-------|
| description | `CaptureDescription` | from `la-devices`; fields below |
| capture | closed `Capture` (loaded) or `Snapshot` (to save) | from `la-storage` |
| extras | list of `Section` | optional sections kept opaque; unique tags, ascending on save |

A loaded session's capture is always closed. Saving a snapshot of a capture that is still being
recorded sets `interrupted` in the file (it is then loaded as closed and interrupted).

## Capture description (META section)

| Field | Rule |
|-------|------|
| simulated | bool; preserved exactly; no setter exists on a description |
| seed | optional `u64` |
| device id | text `provider:unique`, provider non-empty, at most 4,096 bytes |
| device name | text, at most 4,096 bytes, any valid UTF-8 (including empty is rejected by the device layer; the loader accepts what `restore` accepts) |
| channels | `1..=32`, equal to the capture's channel count |
| enabled mask | non-zero, no bit at or above `channels` |
| requested sample rate, applied sample rate | `> 0`; the applied rate equals the capture's rate |
| trigger | optional: channel (enabled) and kind (level low/high, rising, falling, any edge) |
| samples before trigger | `u64` |
| start time | seconds since the Unix epoch (`i64`) and nanoseconds (`0..10^9`) |

Cross-checks between sections (checked after both are decoded): `channels` and the applied sample
rate of the description equal those of the capture; otherwise `session.inconsistent`.

## Capture (CAPT section)

| Field | Rule |
|-------|------|
| channels | `1..=32` |
| sample rate | `> 0` |
| length | number of positions (samples plus gap positions) |
| flags | `stopped_by_limit`, `interrupted`; other bits zero |
| gaps | sorted, each `length >= 1`, `start + length <= capture length`, no two adjacent or overlapping (adjacent gaps are merged by the recorder, so an adjacent pair is damage) |
| per channel: initial level | `0` or `1`; `0` if the capture has no stored sample |
| per channel: transitions | strictly increasing positions in `1..length`; none inside a gap; none at or before the first stored position (position 0, or the end of a gap that starts at 0) |

Derived, never stored: lost positions (sum of gap lengths), `complete`
(`closed && lost == 0 && !stopped_by_limit && !interrupted`), `closed` (always true when loaded).
A transition at the first position after a gap is legal (the real time of the level change was
lost); a transition needs a stored sample before it.

Memory the capture needs once loaded is computed from the channel table before any transition is
read: for every channel `4 * transitions + 32 * ceil(transitions / 1024)` plus segment overhead,
plus the gap list, the description and the extras; it must fit the memory budget.

## Section

| Field | Rule |
|-------|------|
| tag | exactly 4 ASCII letters or digits |
| critical | bool; a reader that does not know a critical tag refuses the file |
| payload | opaque bytes for unknown tags; length checked against the file before reading |

Order in a file: `META`, `CAPT`, then optional sections in strictly ascending tag order (byte
comparison). This makes one session have exactly one encoding and makes duplicate tags impossible
to represent. Tags `META` and `CAPT` are reserved; callers cannot supply them as extras.

## Format version

`major.minor` as two `u16`. Current: `1.0`. Readers accept their major with any minor. Reserved header
and section flag bits are zero; a non-zero reserved bit is damage in 1.x.

## Limits (loader)

| Limit | Default | Checked |
|-------|---------|---------|
| memory budget (`la_storage::Limits`) | storage default (1 GiB) | before decoding data, and incrementally |
| file size | 64 GiB | against the file's real length |
| text field | 4,096 bytes | before reading the text |
| sections | 64 | against the header's count |
| gap count | bounded by the section's remaining bytes and the memory budget | before reading gaps |
| transitions per channel | bounded by the stream's declared bytes and the memory budget | before reading the stream |
| bytes of extras | counted in the memory budget | as they are read |

## Save operation / load operation

States: `Running` → one of `Succeeded`, `Failed`, `Cancelled`. Exactly one outcome, set once.
Progress: `done_bytes <= total_bytes`, non-decreasing; at `Succeeded` they are equal.

| Operation | Success value | Failure |
|-----------|---------------|---------|
| save | bytes written, section count | typed error; old target untouched, temporary file removed |
| load | `Session` | typed error; nothing partially loaded |
| either, cancelled | none | `session.cancelled`; same guarantees as failure |

## Relationships

```text
SessionData ──save──▶ file ──load──▶ Session
   │ description ──▶ META            │ description ◀── META
   │ snapshot    ──▶ CAPT            │ capture     ◀── CAPT (via CaptureRestorer)
   └ extras      ──▶ optional ...    └ extras      ◀── optional ...
```
