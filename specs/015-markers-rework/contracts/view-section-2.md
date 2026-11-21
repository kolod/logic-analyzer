# Contract: the `VIEW` session section, payload version 2

**Status**: replaces the payload of [feature 006's contract](../../006-ui-shell-signal-view/contracts/view-section.md)
(version 1). The framing, the section tag (`VIEW`), the "at most one", the "written only when the state
differs from the defaults" rule, the canonical varints and the preservation of a newer version are as
there and unchanged. The session file format (feature 003) is not changed: no new section, no new minor
version.

Version 1 payloads are **not read** (no release has been made; spec FR-024). They are treated as any invalid
payload (see Validation).

## Payload (little-endian canonical varints as in feature 003; `u8` is one byte)

```text
version            u8          = 2
-- channels: exactly as version 1 --
channel_count      varint      0..=32, equals the CAPT channel count or is 0 (no presentation stored)
repeat channel_count times, in display order:
    index          varint      < CAPT channel count, unique
    flags          u8          bit 0 visible; bit 1 custom colour; bits 2..7 must be 0
    colour         u8          palette index 0..=15   (if bit 1 is clear)
                   u8,u8,u8    red, green, blue       (if bit 1 is set)
    name_len       varint      0..=256
    name           bytes       UTF-8
-- markers: new --
marker_count       varint      0..=64
repeat marker_count times, in creation order:
    id             varint      unique, < 2^32
    flags          u8          bit 0: 0 single, 1 pair; bits 1..7 must be 0
    number         varint      >= 1, unique within the kind
    colour         u8          0..=7
    position_a     varint      single: the position; pair: the start
    position_b     varint      present only for a pair: the end, >= position_a
    comment_len    varint      0..=320 (bytes of UTF-8, at most 80 characters)
    comment        bytes       UTF-8, already in sanitized form
last_id            varint      >= every id above, < 2^32; the highest id ever given (0 when none)
next_single        varint      > every single number above, >= 1: the number the next single marker gets
next_pair          varint      > every pair number above, >= 1: the number the next pair marker gets
origin             varint      < capture length in samples (or 0 if the capture is empty)
selected           varint      marker id + 1, or 0 for none
-- window: exactly as version 1 --
window_start       varint      both 0 means "no remembered window"
window_length      varint      otherwise window_length >= 1 and start + length <= capture length
follow_latest      u8          0 or 1
```

Total payload ≤ 64 KiB by construction (32 channels × 262 bytes + 64 markers × ~335 bytes).

## Validation (reader)

The loader of `la-host` rejects the payload as `host.view_invalid` when: the version byte is 0 or 1 (older
payloads are not read); a varint is not canonical or does not fit; a field is out of the ranges above; an
id repeats; a number repeats within its kind; a reserved bit is set; a position is not below the capture
length (or 0 for an empty one); `position_b < position_a`; a comment is not valid UTF-8 or not in its
sanitized form; `last_id` is below an id in use, or a `next_*` is 0 or not above the numbers of its kind in use; `selected` names no marker; the
window is outside the capture; or bytes remain after `follow_latest`. Rejection never fails the open: the
capture is shown with default presentation and no markers, a `Warning` notice `host.view_invalid` is added,
and the invalid section is dropped from the in-memory session (it is not written back).

A payload with a version byte greater than 2 is not understood: it is preserved as it is and written back
unchanged (the rule of version 1, kept).

## Writer

Deterministic: the same state gives the same bytes. The section is written only when the presentation is not
the default, or there is a marker, or the origin is not 0, or a window is remembered. A reader that accepts a
payload re-encodes it to identical bytes (the property the existing tests check; they are extended to the
marker part).
