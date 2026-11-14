# Contract: Position, Level and Viewport Semantics (version 1)

This document fixes the exact meaning of positions, levels, transitions and pixel
columns so that tests, the future signal view, decoders and export agree.

## Positions and levels

- A **position** is a `u64` sample index; the first sample is position 0. A capture of
  length `L` has samples at positions `0..L`. Positions inside a gap count towards `L`
  but hold no samples.
- The **level at position `x`**:
  - `Missing` if `x >= L` or `x` lies inside a gap;
  - otherwise `initial_level XOR (rank(x) mod 2)`, where `rank(x)` is the number of
    stored transitions with position `<= x`.
- A **transition at position `p`** (`1 <= p <= L - 1`) means level(`p - 1`) differs from
  level(`p`); it is *rising* when level(`p`) is high. Sample 0 has no transition.
- **Resync transition**: the first sample after a gap is not compared with the sample
  before the gap. If its level differs from the last level before the gap, a transition
  is stored at the gap's end position (the true time is unknown, lost in the gap). Such
  a transition is always adjacent to a gap and is reported together with
  `missing_data = true` on its pixel column.

## Windows

- A window is `[start, end)` with `start < end`; both are `u64`. It may extend beyond
  the capture length (a live view may look past the published data); parts outside the
  data are `Missing`/`beyond_end`, never invented. `start >= end` is
  `storage.invalid_window`.
- The pixel count `W` must satisfy `1 <= W <= limits.max_pixels`, else
  `storage.invalid_pixels`.

## Pixel columns

For `L_w = end - start`:

```text
a_i = start + floor(i * L_w / W)      for i = 0 ..= W      (computed in u128)
column i covers samples [a_i, a_{i+1}); a_0 = start, a_W = end
```

Columns can have zero width (when `L_w < W`); consecutive columns share the boundary
`a_{i+1}`.

For column `i` the result is:

| Field | Definition |
|-------|------------|
| `start_level` | level at position `a_i` |
| `end_level` | level at position `a_{i+1}` (so `end_level(i) == start_level(i+1)`) |
| `count` | number of transitions with `a_i < p <= a_{i+1}` = `rank(a_{i+1}) - rank(a_i)` |
| `activity` | `None` if `count == 0`; `One(edge)` if `count == 1`; `Many{count}` otherwise |
| `missing_data` | some gap `[g0, g1)` satisfies `a_i < g1` and `a_{i+1} >= g0` |
| `beyond_end` | `a_i >= L` |

Consequences (tested):

1. The columns partition the transition range `(a_0, a_W]`: every transition in the
   window is counted in exactly one column, so no glitch can be dropped.
2. `Many` is never reported for a column with fewer than two transitions and `None`
   is never reported for a column containing one.
3. Without gaps and inside the data,
   `start_level XOR (count mod 2) == end_level`.
4. Zero-width columns have `count == 0` and equal start and end levels.
5. A transition exactly at `a_0` belongs to the window before it, a transition exactly
   at `a_W` belongs to the last column.

## Edge iteration

`transitions(channel, start, end)` yields every stored transition with
`start < p <= end` in increasing position order, each with its direction; it yields each
transition exactly once even across block and chunk boundaries. Cost is
O(log N + k) for `k` yielded transitions.

## Cost contract

For one channel the cost of `viewport` is
`O(log N + W * (1 + log(1 + N_w / W)))` block-index and offset probes, where `N` is the
number of stored transitions and `N_w` the number inside the window. It never depends
linearly on `N` or `N_w` (research R2). `QueryStats::probes` reports the count.
