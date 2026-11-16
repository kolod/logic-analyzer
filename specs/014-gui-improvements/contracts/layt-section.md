# Contract: the `LAYT` session section

**Status**: extends [feature 003's file format](../../003-session-file-format/contracts/file-format.md)
as its evolution rules allow: an optional section, written by this feature, that a reader which does not
know it keeps byte for byte. The minor number of the format is not changed (nobody has to read the
section to open the file).

## Framing

An ordinary optional section: tag `LAYT`, non-critical, payload below, CRC as for every section. At most
one per file. The writer emits it only when at least one channel has a height other than automatic, so
a file of a user who never resized a signal is exactly what it was before. A reader that does not know
the tag (a build without this feature) preserves it byte for byte when saving.

## Payload (all integers little-endian varints in the canonical form of feature 003; `u8` is one byte)

```text
version            u8          = 1
count              varint      0..=32
repeat count times, in ascending channel order:
    channel        varint      < the channel count of the capture, unique, ascending
    height         varint      24..=240
```

Only channels with a custom height are listed; every other channel is automatic. At most
`1 + 1 + 32 * 3` bytes.

## Validation (reader)

The loader of `la-host` rejects the section as `host.layout_invalid` when the version is not 1, a varint
is not canonical, `count` is over 32, a channel is not below the channel count, repeats or is not
ascending, a height is outside `24..=240`, or bytes remain. Rejection never fails the open: the capture
is shown with automatic heights, a `Warning` notice with the code `host.layout_invalid` is added, and
the invalid section is dropped (not written back).

A newer version of the section (a version byte greater than 1) is preserved untouched and not
interpreted; the capture opens with automatic heights and an `Info` notice.

## Writer

Deterministic: the same heights give the same bytes. The section is built when the session is saved and
removed from the extras when no height is custom.

## Tests

A byte-exact payload for a hand-written case; the canonical-form rejections; every field out of range;
round trip through save and open; an old file without the section opens with automatic heights; a file
with a `LAYT` opens in a build that does not know it (the extras test of feature 003, with a `LAYT`
payload) and is written back unchanged; a random-bytes property test (nothing panics, an accepted payload
re-encodes to the same bytes).
