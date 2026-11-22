# Implementation Plan: I2C Lane as Blocks

**Branch**: `016-i2c-lane-blocks` | **Date**: 2026-11-22 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `specs/016-i2c-lane-blocks/spec.md`

## Summary

The I2C lane becomes two sub-bands of solid rectangles: the parts of a packet above (start, address,
ack/nak, data, ack/nak, repeated start, stop, placed on the bit cells of the bus period) and the whole
packet below (green or red, grey if unfinished). The decoder changes what it writes into its lane (new
places, one annotation per part, no points); the framework gets one small, optional addition (decoder API
1.1 → 1.2): two block shapes that tell which band an annotation belongs to, and a descriptor field that
says how many rows the lane has. The drawing of a block (solid fill, a text that is shown whole or not at
all) is done by the interface. What the decoder reads, its items for stacking, its arrows and dots, the
generator and the session file format do not change.

## Technical Context

**Language/Version**: Rust, stable; MSRV 1.85 for `la-host` and `la-storage` (no new language features).

**Primary Dependencies**: none new (`la-decoders`, `la-host`, `la-app`, `la-i18n`).

**Storage**: results are saved in the `DANN` section of feature 007. The style byte of an annotation has
room for the two new shapes (3 bits for the shape, 5 of 8 used), so the section's version stays 1 and no
file written before this feature is refused; a file written by this version and opened by an older one is
refused by the older one's shape check (there has been no public release).

**Testing**: `cargo test`: unit tests of the block geometry on hand-built signals, a round trip over the
generator's seeds (blocks derived from the expected items), planner tests for the bands, the interface's
drawing helpers (fit of text, colours distinguishable on every theme), the existing I2C suite unchanged.

**Target Platform**: Windows 10/11 and Debian 13.

**Project Type**: desktop-app (UI-independent core crates + UI shell). Changes: `la-decoders` (decoder,
framework addition), `la-host` (rows of a lane from the descriptor), `la-app` (drawing of blocks).

**Performance Goals**: unchanged: drawing cost bounded by the visible window (planner of 007); the number
of annotations per byte goes from about 11 to about 12 in the lane (a block per part and for the packet,
no points), which the existing budgets allow.

**Constraints**: no panic on any input; the decoder keeps its checked arithmetic; blocks never overlap in
a band; no new dependency; no new message keys (the texts exist: `ann-i2c-start`, `-repeated-start`, `-stop`,
`-ack`, `-nak`, `-address-*`, `-data`, `-transaction-*`).

**Scale/Scope**: one decoder file set (`bus.rs`, `text.rs`), one planner function, one drawing function,
one descriptor field, two shapes, two documents.

## Constitution Check

*GATE: passed before Phase 0 research and re-checked after Phase 1 design.*

- [x] **I. Stability & Data Integrity**: positions are computed with saturating arithmetic from the
  bus period; a block is validated by the sink like any annotation (inside the capture, ordered); the
  random-signal test of 008 runs on the new output. Nothing writes files except the existing results
  section, whose format is unchanged.
- [x] **II. Hardware**: not affected.
- [x] **III. Test-First**: the geometry, colour and band tests are written first and fail on the present
  lane (tasks order them so); every bug found gets a regression test.
- [x] **IV. Architecture**: the logic of what a block is lives in `la-decoders`; the host only passes the
  number of rows; the interface only draws. Decoder API 1.1 → 1.2 is an additive minor version
  (optional descriptor field with a default, two new shapes that existing decoders ignore), documented in
  `docs/decoder-api.md`, implemented and tested in the same merge. Host API: no change.
- [x] **V. Simplicity**: no dependency; the band is a property of the shape, not a new annotation field,
  which keeps the file format and the 44 construction sites of `Style` as they are.
- [x] **VI. Observability**: nothing new is logged.
- [x] **VII. Localization**: no new keys; the existing keys are reused with their present texts, so the
  parity, budget and font checks are unchanged; documents in English.
- [x] **VIII. Rendering**: the planner places a block in a row by its shape in O(1); the cost per frame
  stays O(log N + W); a planner test with dense traffic checks it as for 007.
- [x] **IX. Decoding**: lane (blocks) and overlays (dots, arrows) both still produced; faults contained.
- [x] **X. Export**: not affected; items are unchanged.
- [x] **XI. Virtual device**: the generator is unchanged and is the source of the round trip.
- [x] **XII. Original design**: colours are the application's own palette; no outside code.
- [x] **Platforms**: nothing platform-specific.

## Project Structure

### Documentation (this feature)

```text
specs/016-i2c-lane-blocks/
├── spec.md
├── plan.md                  # this file
├── research.md              # R1 bands, R2 geometry, R3 colours, R4 drawing, R5 errors, R6 tests
├── data-model.md            # blocks, bands, the new shapes and descriptor field
├── quickstart.md            # how to see and check it
├── contracts/
│   ├── i2c-lane.md          # what the I2C decoder writes into its lane
│   └── decoder-api-1.2.md   # the framework addition
├── checklists/requirements.md
└── tasks.md                 # by /speckit-tasks
```

### Source Code (repository root)

```text
crates/decoders/src/
├── model.rs                       # Shape::Part, Shape::Whole
├── descriptor.rs                  # Descriptor::lane_rows (default 3)
├── lanes.rs                       # band routing of Part / Whole shapes in plan()
├── persist/dann.rs                # shape codes for the two shapes
├── decoders/i2c/{bus.rs,text.rs}  # blocks instead of ranges and points
└── lib.rs / API version           # DECODER_API_VERSION 1.2
crates/host/src/decoders.rs        # lane plan with the decoder's rows
crates/app/src/decoder_view.rs     # solid blocks, whole-or-nothing text, contrast text colour
docs/decoder-api.md, docs/decoders/i2c.md, docs/user-guide.md
crates/decoders/tests/             # i2c_blocks.rs (new), lanes.rs (bands), decoder_api_doc.rs
```

**Structure Decision**: as feature 008: everything about the protocol in `la-decoders`; the shell learns
only to draw two new shapes and to ask the descriptor for the rows of the lane.

## Complexity Tracking

| Violation | Why needed | Simpler alternative rejected because |
|-----------|------------|--------------------------------------|
| A change to the decoder contract (API 1.2: two shapes, `lane_rows`) although feature 008 aimed at adding the decoder by registration only | The spec asks for fixed sub-bands (parts above, packet below); the planner packs by overlap, so it would put the packet block, which starts first and covers the parts, in the upper row | Relying on the order of insertion (fragile, differs between a growing and a finished capture); a field on `Annotation` (changes the file format and every construction site); two lanes for one decoder (not what was asked) |
