# Quickstart: validating the GUI improvements

Run from the repository root; the same commands on Windows 10/11 and Debian 13. The interface crate
needs Rust 1.92 or newer (the MSRV job builds everything else with 1.85).

## 1. The core (automated, no display)

```text
cargo test -p la-storage -p la-host
```

Expected: all pass, among them
- `next_transition` and `previous_transition` against a brute-force scan on random captures (empty
  channels, position 0, the last sample, gaps);
- heights: set, clamp, reset, saved in `LAYT`, restored, absent in an old file, rejected when damaged,
  kept by a build that does not know the section;
- `SetSidePanel` and its settings, including an unknown value;
- `ScrollToEdge`: both directions, the quarter placement to the sample, gaps, no edge beyond, follow
  turned off, refusal without a capture or for a hidden channel; `edge_before` and `edge_after` of the plan;
- recent files: 15, order, missing files, remove, clear, a settings file with 30 entries;
- seconds and samples: examples by hand, the round-trip property test, overflow, hostile text;
- the host-API document test (version 1.2, the command table), the random walk with the new commands.

## 2. The interface (automated)

```text
cargo test -p la-app
```

Expected: the lane layout tests (rectangles, zones, scrolling, landing of a drag), the contrast of the
channel buttons for the 16 palette entries in both themes, and the headless rendering tests: every new
screen in the six languages at the minimum window size without clipping, the legend with the editor
open, the Capture view with 8, 16 and 32 channels, the edge buttons with the pointer near an end, the
panel in each of its four states.

## 3. Compatibility (automated)

`cargo test -p la-host --test sessions_compat` opens the files of earlier versions kept under
`crates/host/tests/fixtures/` (a session of feature 007 and of feature 008) and checks that nothing is
lost, and that a file saved by this feature with `LAYT` is written back unchanged by the extras rules.

## 4. Look at it (manual, on both systems)

1. Start the application. The Capture view is on the right, the strip of icons at the edge. Click the
   Decoders icon, then the Markers icon: one view at a time. Click the shown one: the panel goes away
   and the signals widen. Restart: the same state comes back.
2. In the Capture view: choose the virtual device, type 2.5 as the length in seconds at 1 MHz (the
   field beside it says 2,500,000 samples), turn some channels off with the buttons (the border and the
   number keep the colour of the channel, the inside is empty) and on again (filled), press Start.
3. Click the colour bar of a channel: its colour steps. Click its name, type, press Enter. Drag the
   border between two channels. Drag a channel to another place. Save the session, close, open it:
   names, colours, order and heights are back.
4. Zoom in on a long flat stretch of a channel and put the mouse near its right end: a button with an
   arrow appears; click it: the next edge is a quarter of the view from the left, the zoom is the same.
   The same at the left end.
5. File → Open recent: a submenu of up to 15 files; delete one of the files on disk and open the menu
   again: it is disabled.
6. Decoders → Add decoder: a submenu with I2C and Clocked bits; press D: the same choice.
7. The window, the task bar and (on Windows) the file in Explorer show the application icon; on Debian
   install `packaging/linux/` by hand and check the icon of a session file in the file manager.
8. Switch the language to German and Japanese and look at the panel, the buttons and the menus at the
   smallest window size: nothing is cut.

## 5. Performance (automated)

```text
cargo bench -p la-host --bench frame_plan
```

Expected: within the recorded baseline; the two edge lookups per channel do not change the complexity
class (the cost does not grow with the capture length).

## What a failure looks like

- A host test names the command and the state that differ.
- A layout test prints the lane and the zone that were hit against the one expected.
- A rendering test writes the picture when `LA_SNAPSHOT_DIR` is set, so a person can look at it.
