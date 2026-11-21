# Contract: the `VIEW` session section (format 1.1)

**Status**: extends [feature 003's file format](../../003-session-file-format/contracts/file-format.md)
as its evolution rules allow (rule 1: a new minor version adds only optional sections; rule 4:
the spec of the feature that adds a section defines it). Implementing this feature changes the
writer to emit minor version `1`; readers already accept any minor of major 1.

## Framing

An ordinary optional section: tag `VIEW`, non-critical, payload below, CRC as for every section. By
the ordering rule it follows `META` and `CAPT` and any optional section whose tag sorts before it
(`DECO`, `NOTE`, …). A reader that does not know it preserves it byte for byte when saving.

At most one `VIEW` section per file. The writer emits it only when the state differs from the defaults
(a capture with default names, colours, order, no markers and no remembered window has no `VIEW`
section, so such files stay exactly as feature 003 wrote them except for the minor number).

> **Superseded in part by feature 015**: the marker records (`marker_count` to `selected_second`)
> below describe the one kind of marker of this feature. Feature 015 (`015-markers-rework`) replaces
> them with records for single and pair markers, their colours, comments and numbering, and adds the
> time origin; its plan defines the new layout. The rest of the payload is unchanged.

## Payload (all integers little-endian varints in the canonical form of feature 003; `u8` is one byte)

```text
version            u8          = 1
channel_count      varint      0..=32, equals the CAPT channel count or is 0 (no presentation stored)
repeat channel_count times, in display order:
    index          varint      < CAPT channel count, unique
    flags          u8          bit 0 visible; bit 1 custom colour; bits 2..7 must be 0
    colour         u8          palette index 0..=15   (if bit 1 is clear)
                   u8,u8,u8    red, green, blue       (if bit 1 is set)
    name_len       varint      0..=256 (bytes of UTF-8)
    name           bytes
marker_count       varint      0..=64
repeat marker_count times:
    id             varint      unique
    position       varint      < capture length in samples (or 0 if the capture is empty)
    name_len       varint      0..=256
    name           bytes       UTF-8
selected_first     varint      marker id + 1, or 0 for none
selected_second    varint      marker id + 1, or 0 for none
window_start       varint      \  both 0 means "no remembered window"
window_length      varint      /  otherwise window_length >= 1 and start + length <= capture length
follow_latest      u8          0 or 1
```

Total payload ≤ 64 KiB by construction (32 channels × 262 bytes + 64 markers × ~275 bytes).

## Validation (reader)

The loader of `la-host` (not the format library, which only frames the section) rejects the section
as `host.view_invalid` when: the version is not 1, a varint is not canonical, a field is out of the
ranges above, an index or id repeats, a reserved bit is set, a name is not valid UTF-8, a marker or
window lies outside the capture, or bytes remain after `follow_latest`. Rejection never fails the
open: the capture is shown with default presentation and no markers, a `Warning` notice with the
code `host.view_invalid` is added, and **the invalid section is dropped** from the in-memory session
(it is not written back, so a bad section cannot propagate).

A newer minor of this section (version byte greater than 1) is not understood: it is preserved
untouched, not interpreted, and the capture opens with defaults and an `Info` notice rather than the
warning.

## Writer

Deterministic: the same state gives identical bytes (no map iteration, no timestamps). Names are
written after sanitization (research R2: control characters and bidi controls removed). The writer
never writes more than the limits above; the host enforces the limits on input so a legal state
always fits.

## Tests (defined by this contract)

- round trip for default, typical, and maximal states;
- reader against a hand-written byte vector from this document (independent of the writer);
- every validation rule has a failing-fixture test;
- a mutation fuzzer over valid payloads (every single-byte change, truncation, extension) never
  panics, and any accepted result is a state the writer can write back identically;
- an old reader (feature 003 tests with an extra `VIEW` section) preserves the section.
