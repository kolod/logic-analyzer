# Contract: Session File Format 1.0

What an independent reader or writer implements. All integers are little-endian. "varint" is
unsigned LEB128 (7 bits per byte, low group first, high bit set on all but the last byte) in
**canonical form**: at most 10 bytes, the last byte non-zero unless the value is 0, and the 10th byte
at most 1. CRC is CRC-32C (Castagnoli, reflected, polynomial 0x1EDC6F41, initial value 0xFFFFFFFF,
final XOR 0xFFFFFFFF; check value of `"123456789"` is `0xE3069283`).

A session file is exactly: **header, sections, footer**, with nothing before or after.

## Header (32 bytes)

| Offset | Size | Field | Rule |
|--------|------|-------|------|
| 0 | 8 | magic | `89 4C 41 53 0D 0A 1A 0A` (`\x89LAS\r\n\x1a\n`) |
| 8 | 2 | major | 1 |
| 10 | 2 | minor | any (the writer emits 2 since feature 007, which added the `DECO` and `DANN` sections; it emitted 1 after feature 006 added `VIEW`; 1.0 and 1.1 files remain valid) |
| 12 | 4 | reserved flags | 0 |
| 16 | 8 | file length | total bytes of the file, equal to the real size |
| 24 | 4 | section count | `2..=64` |
| 28 | 4 | header CRC | CRC of bytes 0..28 |

## Section

| Size | Field | Rule |
|------|-------|------|
| 4 | tag | ASCII `A-Z a-z 0-9` |
| 4 | flags | bit 0 = critical; other bits 0 |
| 8 | payload length | `<=` bytes remaining before the footer |
| n | payload | |
| 4 | CRC | CRC of tag, flags, length and payload (everything of the section before the CRC) |

Order: tag `META` first, tag `CAPT` second, then any optional sections with strictly ascending tags
(unsigned byte comparison). `META` and `CAPT` have the critical bit set. A reader must refuse a file in
which any section it does not understand has the critical bit set, and must keep unknown sections
without the critical bit unchanged when it saves again.

## Footer (16 bytes)

| Offset | Size | Field | Rule |
|--------|------|-------|------|
| 0 | 4 | magic | `4C 41 45 4E` (`LAEN`) |
| 4 | 8 | file length | equal to the header's |
| 12 | 4 | CRC | CRC of footer bytes 0..12 |

The footer must end exactly at the end of the file, and the sections must end exactly where the footer
begins.

## META payload (the capture description)

In order, with no padding; the payload must be consumed exactly.

| Field | Encoding |
|-------|----------|
| flags | `u8`: bit 0 simulated, bit 1 seed present, bit 2 trigger present; other bits 0 |
| seed | `u64`, only if bit 1 |
| device id | string |
| device name | string |
| channels | `u8`, `1..=32` |
| enabled mask | `u32`, non-zero, no bit at or above `channels` |
| requested sample rate | `u64`, `> 0` |
| applied sample rate | `u64`, `> 0` |
| samples before trigger | `u64` |
| trigger | only if bit 2: channel `u8` (an enabled channel), kind `u8` (0 level low, 1 level high, 2 rising, 3 falling, 4 any edge) |
| start seconds | `i64` since 1970-01-01T00:00:00Z |
| start nanoseconds | `u32`, `< 1_000_000_000` |

A string is a varint byte length (at most 4,096) followed by that many bytes of valid UTF-8. The device id
contains a `:` with a non-empty part before it.

## CAPT payload (the capture)

| Field | Encoding |
|-------|----------|
| channels | `u8`, `1..=32` |
| flags | `u8`: bit 0 stopped by limit, bit 1 interrupted; other bits 0 |
| sample rate | `u64`, `> 0` |
| length | `u64`, positions in the capture |
| gap count | varint |
| gaps | per gap: varint `start - previous_end` (the first gap: its start), varint `length >= 1`; after the first gap each start delta is `>= 1` (no adjacent gaps) |
| channel table | per channel: `u8` initial level (0 or 1), varint transition count, varint stream length in bytes |
| channel streams | per channel, in order, `stream length` bytes: `transition count` varints, the first the position of the first transition, each later one the distance to the previous (`>= 1`) |

Constraints: gaps end at or before `length`; every transition position is `>= 1` and `< length`, lies in no gap,
and has a stored sample before it (it is greater than the first stored position: 0, or the end of a gap that
starts at 0); the initial level of a channel without stored samples is 0; each stream decodes to exactly its
declared count within exactly its declared length; the payload ends exactly after the last stream.

## Cross-checks

The channel count of `META` equals that of `CAPT`, and the applied sample rate of `META` equals the sample rate of
`CAPT`.

## Evolution rules

1. A new minor version adds only optional (non-critical) sections or documented values inside new sections.
2. Anything that changes the meaning of an existing field, a reserved bit or the framing is a new major version.
3. Reserved bits are zero in every 1.x file; a reader rejects a non-zero reserved bit.
4. Optional sections defined later (decoder settings `DECO`, annotations, notes) are specified in the
   spec of the feature that adds them; this document defines only the framing they use. The view state
   (channel names, colours, order, visibility, markers, window) is the `VIEW` section, defined by
   `specs/006-ui-shell-signal-view/contracts/view-section.md`, which is why the writer emitted minor 1. The decoder setup and results are the `DECO` and `DANN` sections, defined by
   `specs/007-decoder-framework-annotations/contracts/deco-section.md`, which is why the writer now emits minor 2.

## Errors a reader reports

| Situation | Code |
|-----------|------|
| bad magic, or too short to hold a header | `session.not_a_session` |
| major higher than 1 | `session.unsupported_version` (names found and supported) |
| file shorter than the header's length, or a length that runs past the end | `session.truncated` |
| any CRC mismatch, trailing bytes, reserved bit set, bad tag, order or duplicate violation, non-canonical varint, bad UTF-8 | `session.damaged` |
| structure fine but content inconsistent (rules above, cross-checks) | `session.inconsistent` |

The split between the two: a violation of the encoding or framing (a reserved bit, a stream length or count that does not match its data, a non-canonical varint, a bad checksum, order or text) is `session.damaged`; a violation of a constraint on values that are well encoded (the gap, transition, initial-level, channel-count and sample-rate constraints and the cross-checks) is `session.inconsistent`.
| unknown critical section | `session.unknown_critical_section` |
| a size, count, text or section limit exceeded | `session.too_large` |
| the capture would not fit the memory budget | `session.memory_budget` |
