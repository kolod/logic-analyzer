# Quickstart: validating single and pair markers

How to see that the feature works, end to end. Details of the types and commands are in
[data-model.md](data-model.md) and [contracts/](contracts/); the steps below do not repeat them.

## Prerequisites

- Rust stable (MSRV 1.85 for the core crates; the application crate needs the version in its manifest).
- Windows 10/11 or Debian 13. No hardware: the virtual device is used.
- From the repository root: `cargo build --workspace`.

## 1. Automated checks

```text
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo bench -p la-host --bench frame_plan -- --test      # compares with the recorded baseline
```

Expected: all pass. The tests that matter for this feature:

| Test | What it proves |
|------|----------------|
| `crates/storage/tests/crop.rs` | a cropped copy has, at every position, the level of the source shifted; gaps are clipped; the status flags and the sample rate stay; a range over the budget is refused before copying; cancel leaves nothing |
| `crates/host/tests/markers.rs` (rewritten) | numbering per kind and never reused, ordering, limit 64, conversions, remap after cuts, origin, selection |
| `crates/host/tests/placement.rs` (new) | the state machine of placing a single and a pair; which half is the start; cancel by every route; refusal at the limit and while saving/loading/cropping |
| `crates/host/tests/snapping.rs` (new) | `resolve_pointer`: lane gating (above, below, over another lane, over the ruler), radius, tie to the earlier, gaps skipped, `crowded` detection, `crowded_zoom_target` (the transitions near the pointer end up two snapping distances apart, the point under the pointer stays under it, the zoom limit is respected), 1,000 random transitions each picked exactly after the automatic zoom (SC-008, SC-009), cost does not grow with the distance (no scan) |
| `crates/host/tests/placement_zoom.rs` (new) | `PlacementZoomTo` stores the window once; fix, cancel, begin, end-of-drag and every ending command restore it (clamped, also when the capture grew); a hand zoom, pan or scroll clears it without restoring; a pair restores after each click |
| `crates/host/tests/marker_actions.rs` (new) | time origin and the axis labels, cut left/right, trim, scroll-to centring and clamping, fit, split, make a pair |
| `crates/host/tests/view_section.rs` (extended) | version 2 round-trip, canonical re-encoding, every rejection rule, a version 1 payload is rejected (not read), a newer version is kept |
| `crates/host/tests/random_walk.rs` (extended) | 100,000 actions including placement, drags, cuts, trims, conversions, save/open leave every invariant of `Markers::check` true and never panic |
| `crates/host/tests/host_api_doc.rs` | the version line and the command table of `docs/host-api.md` match the code |
| `crates/app` tests (headless) | the marks and lines are drawn (single: pentagon on a solid line; pair: squares left of the start and right of the end), the dotted line and the jump of the mark while placing, the highlight, the panel in six languages at 220 px |

## 2. By hand (the simulated device)

1. Start the application, choose the **Markers** view, start a capture with the simulated device and wait
   for it to finish.
2. Press **Single marker**. Nothing appears yet; move the pointer over the signals: a dotted line in the
   marker's colour follows it. Move it over a lane next to an edge: the edge is highlighted and the line
   sits exactly on it. Move it above the lane (the ruler): no snapping. Click: the line becomes solid with
   a pentagon pointing down; the row **T1** appears.
3. Zoom out until one pixel holds many samples, start another single marker and rest the pointer over a
   lane at a crowded spot: after 2 seconds the view zooms in smoothly around the pointer, the edges
   separate; move to the wanted edge (highlighted) and click: the marker sits exactly on it and the view
   returns to the zoom and position it had. Repeat, and press Esc during the zoomed-in view: same
   return, nothing added. Repeat and turn the wheel while zoomed in: the view stays where you put it.
4. Press **Pair marker**, click on a rising edge, move the pointer left of the click: the square of the
   fixed half moves to its right (it is now the end); move right of it: back to the left. Click on a falling
   edge. **P1** appears with its duration and the frequency in your language (switch the language in the
   Settings and check the formatting).
5. Click the row **T1**: *Count time from here* — the axis shows negative times to the left, the markers'
   times shift; *Time counts from the start* puts it back. *Scroll to it* from far away: it is at the
   centre. *Delete data to the left* → the confirmation names the lost time → Delete: the capture
   shortens, the unsaved mark appears, decoders (if any) run again; Cancel during a large cut leaves it as
   it was.
6. Click the row **P1**: *Fit the view to the pair* — the interval fills the signals with room for both
   squares; *Trim to the pair*; *Convert to two single markers* gives **T3** and **T4** (not T1/T2).
7. Add two singles, open the actions of one, press **Make a pair**, then click the other (its row or its mark): one **P2** (P1's number is not reused). Pairs and the first marker are shaded meanwhile; Esc cancels.
8. Change a colour, write a comment, press **Delete** in the list: the marker is gone without a question. New markers took the colours in turn.
9. Save, close and open the file: every marker, colour, comment, number and the time origin are back.
   Open `crates/host/tests/fixtures/` files made by the version before this feature: they open, with no
   markers and the `view_invalid` warning (that is the specified behaviour, not a bug).

## 3. Performance and responsiveness

- `LA_FRAME_STATS=1` (the frame log of feature 014) with 64 markers on screen: no frame over the budget of
  feature 014; moving the pointer in placement mode on a capture of 100 million samples keeps the frame log
  quiet (the resolution is O(log N)).
- A cut on a capture of 100 million samples shows progress and Cancel in the status line and the window
  keeps responding.
