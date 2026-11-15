# Contract: the `DECO` and `DANN` session sections (format 1.2)

**Status**: extends [feature 003's file format](../../003-session-file-format/contracts/file-format.md)
as its evolution rules allow (rule 1: a new minor version adds only optional sections; rule 4: the
spec of the feature that adds a section defines it) and follows the pattern of
[feature 006's `VIEW` section](../../006-ui-shell-signal-view/contracts/view-section.md). Implementing
this feature changes the writer to emit minor version `2`; readers already accept any minor of major 1.
Files of format 1.0 and 1.1 still load unchanged.

## Framing

Two ordinary optional sections, non-critical, with a CRC as for every section, preserved byte for byte by
a reader that does not know them:

- **`DECO`** (the setup): at most one per file; written only when the capture has decoder instances.
- **`DANN`** (the results): at most one per file; written only when the user chose to save the results
  and at least one instance has any.

By the ordering rule of the format they follow `META` and `CAPT` and sort by tag with the other optional
sections (`DANN`, `DECO`, ..., `VIEW`). The loader of `la-host` (through `la-decoders`), not the format
library, validates the payloads.

## Conventions

All integers are canonical little-endian LEB128 varints as in feature 003 (a reader rejects a
non-canonical or overlong varint); `u8` is one byte; a *string* is a varint length followed by that many
bytes of UTF-8 (bounded below); a *record* is a varint length followed by that many bytes. Lengths are
checked against the bytes that remain before anything is allocated.

## `DECO` payload

```text
version            u8          = 1
instance_count     varint      0..=8
repeat instance_count times, a record whose content is:
    id             varint      unique, < 2^32
    decoder        string      descriptor id, 1..=48 bytes of [a-z0-9._]
    decoder_ver    varint      the decoder version the options were saved with
    name           string      0..=256 bytes, sanitized form
    source         varint      0 = the capture, otherwise instance id + 1; must name an earlier record
    flags          u8          bit 0 lane visible; bits 1..7 must be 0
    channel_count  varint      0..=16
    repeat channel_count times:
        role       string      1..=48 bytes
        channel    varint      0..=31, or 32 meaning "none"
    option_count   varint      0..=16
    repeat option_count times:
        id         string      1..=48 bytes
        kind       u8          0 integer (i64 as zigzag varint), 1 boolean (u8), 2 choice (string)
        value      by kind
    generator      record      the generator's options, same option encoding, may be empty
```

A record whose `decoder` this version does not register, or whose `decoder_ver` is newer than the
registered decoder's, is **not interpreted**: its bytes are kept and written back untouched, the instance is
not created, and the user is told (`decoder.unknown_decoder`). An option this version does not know is
kept in the same way for write-back and reported once (`decoder.options_skipped`); a missing option takes
its default. The order of the records is the order of creation and also the stack order (a source refers
only to an earlier record).

## `DANN` payload

```text
version            u8          = 1
string_table       varint count (0..=4096) then that many strings (message keys, 1..=96 bytes each)
block_count        varint      0..=8
repeat block_count times, a record whose content is:
    instance       varint      id of an instance in DECO
    setup_version  varint
    covered        varint      position up to which the instance had decoded
    flags          u8          bit 0 incomplete; bits 1..7 must be 0
    annotation_count varint    0..=5,000,000
    items_count    varint      0..=5,000,000
    annotations    annotation_count entries, sorted by start, each:
        kind       u8          0 range, 1 point, 2 edge (rising), 3 edge (falling)
        start      varint      delta from the previous start
        length     varint      for a range; 0 otherwise
        target     varint      0 = lane, otherwise channel + 1 (1..=32)
        style      u8          low 3 bits colour 0..7, next 3 bits shape 0..4, bit 6 error, bit 7 reserved 0
        text       u8          0 literal, 1 message
        text_data  literal: string 0..=256 bytes; message: varint table index, varint arg_count 0..=4, args as strings 0..=64
    items          items_count entries, ordered by end, each:
        kind       string      1..=48 bytes
        start      varint      delta from the previous end
        length     varint
        flags      u8          bit 0 error
        payload    u8 kind (0 integer varint, 1 bytes string 0..=64, 2 text string 0..=64) and its value
```

## Validation (reader)

The loader rejects a section as `decoder.section_invalid` when: the version is not 1; a varint is not
canonical; a count or length is beyond its limit or beyond the bytes that remain; an id repeats; a
reserved bit is set; a string is not valid UTF-8 or is not in its sanitized form; a `source` does not
name an earlier record or would make a cycle or a chain deeper than 4; a channel is beyond the capture's
channel count (a value of 32 means none); an option value is outside its descriptor's range or of the wrong
type; an annotation lies beyond the capture length, has an end before its start, is not in start order, or
refers to a string-table index that does not exist; the block names an instance that `DECO` does not
define; bytes remain after the last entry. Total counts are checked against the annotation and memory
limits **before** anything is allocated; the section as a whole counts against the memory budget of the
load like every optional section.

Rejection never fails the open: the capture opens, a `Warning` notice with `decoder.section_invalid` is
added, and **the invalid section is dropped** from the in-memory session (it is not written back, so a bad
section cannot propagate). If only `DANN` is invalid, the setup is kept and the decoders run again. A
newer version byte (greater than 1) is not understood: the section is kept untouched, not interpreted, and
an `Info` notice is shown instead of the warning.

## Writer

Deterministic: the same state gives identical bytes (no map iteration, no timestamps). Names and literal
text are written after sanitization. The writer never writes more than the limits above; the host enforces
them on input so a legal state always fits. Unknown records of `DECO` that were kept on load are written
back byte for byte, in their original positions among the instances.

## Tests (defined by this contract)

- round trips for an empty, a typical (two instances in a stack, options of all kinds) and a maximal state
  (8 instances, long names, 5,000,000 annotations for the size check by construction, not by allocating);
- the readers against hand-written byte vectors from this document, independent of the writer;
- every validation rule with a failing fixture;
- a mutation fuzzer over valid payloads (every single-byte change, truncation and extension; a randomized
  million in the nightly workflow): nothing panics or allocates beyond the limits, and any accepted payload
  re-encodes to identical bytes;
- unknown-decoder and unknown-option records survive a load and a save byte for byte;
- the session library's tests with extra `DECO` and `DANN` sections (they round-trip unchanged), the
  fixtures of the new minor version, and files of format 1.0 and 1.1 still loading.
