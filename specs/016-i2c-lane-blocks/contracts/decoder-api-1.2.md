# Contract: decoder API 1.2

Additive minor version over 1.1 (feature 007, 008). A decoder that does not use the additions behaves as it
did; every built-in decoder compiles after adding the new field to its descriptor with the default.

## Additions

```text
model::Shape::Part     // a range drawn as a solid block in the upper band of the lane
model::Shape::Whole    // a range drawn as a solid block in the lower band of the lane
Descriptor::lane_rows  // u8, 1..=3, the rows of the lane's spans and marks (default LANE_ROWS = 3)
DECODER_API_VERSION    // "1.2"
```

## Rules

1. `Part` and `Whole` are accepted for ranges only; a point or an edge with such a shape is an invalid
   record (dropped and counted, `decoder.invalid_output`).
2. `lanes::plan(snapshot, window, rows)` is called with the descriptor's `lane_rows` for a lane. A `Part`
   goes to row 0, a `Whole` to row `min(1, rows - 1)`. They are not packed: a cell whose place in its row is
   taken becomes a dense cell (counted, with its error count). Annotations of other shapes are packed into
   the rows as before.
3. The interface draws a `Part` or `Whole` as a solid rectangle in the palette colour of the style, whatever
   the `error` flag says: the flag adds a heavier outline and is counted, it does not replace the colour. Its
   text is drawn only if all of it fits.
4. A descriptor with `lane_rows` outside 1..=3 is refused when the registry is built (as other descriptor
   faults are).
5. Saved results: the style byte keeps its layout; the shape codes 5 (`Part`) and 6 (`Whole`) are new.
   A reader that does not know them rejects the section as invalid.

## Documentation

`docs/decoder-api.md`: the shapes in "Sink", `lane_rows` in "Descriptor", rows in "Results, lanes and
overlays", and "Versioning" (1.2). The test that compares the document with the code
(`decoder_api_doc.rs`) is extended to the new names.
