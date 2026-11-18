# Contract: the interaction of the reworked interface

What the user can do and what it sends to the host. Geometry is in logical pixels (before the interface
scale). `lane_layout.rs` is the one source of the rectangles; drawing, hit tests and the in-place editor
all read it.

## The legend strip (the first 112 px of the signal area, `GUTTER`)

For a lane with its top `y` and height `h`, after the vertical offset:

| Zone | Rectangle | Press and release | Press and move |
|------|-----------|-------------------|----------------|
| colour bar | `x 0..8`, `y..y+h` | `SetChannelColour(Palette((i + 1) % 16))` (a custom colour goes to palette 0) | nothing |
| name | `x 8..112`, `y..y+h-4` | the editor opens over the name | after 4 px: the lane is dragged |
| border | `y+h-3 .. y+h+4` across the whole strip | double click: `SetChannelHeight(0)` | the lane above is resized live with `SetChannelHeight(h)` |

- The lane of a decoder shows no colour bar, no editor and no border (its height is automatic); it can
  be neither dragged nor resized.
- A dragged lane is lifted with the pointer as a whole (background, bands and signal) and the other lanes
  slide aside, smoothly, to open the place where it will land; release sends `MoveChannel { from, to }`;
  Escape or release outside the strip cancels. Dragging near the top or the bottom of the area scrolls
  it.
- A legend row shows the name as its main text and, under it in small type, the number of the analyzer
  input (`D0`, `D1`, ...), which stays with the channel when the row is moved; a very low row shows the
  number at its right edge instead.
- Resizing is limited to `24..=240` while dragging (the border stops at the limits); the height is sent
  with `SetChannelHeight` as the border moves, so the lane follows the pointer.
- The legend scrolls vertically with the wheel while the pointer is over it, and has a thin bar at its
  right edge when the lanes are taller than the area; the wheel over the waveforms keeps zooming.
- The editor is a one-line text field with the current name selected; Enter accepts; Escape cancels; a
  click elsewhere accepts when the text changed and cancels when it did not. An empty text gives back
  the default name ("Channel N") because the host does.

## The side panel

```text
| signals ...................... | splitter (5 px) | panel (220 to 480 px, 280 at first) | icons (36 px) |
```

- The strip of icons is always at the right edge: Capture, Decoders, Markers, top to bottom, each a 28 px
  button with a 3 px bar on its left when it is the shown view, and a cog at the bottom that opens the
  Settings (it is not a view: it opens the Settings dialog and changes nothing of the panel; it is a message of the application, not a host command).
- Click on an icon: `SetSidePanel(that view)`; on the icon of the shown view: `SetSidePanel(Hidden)`.
- The width of the panel is changed by dragging the splitter at its left edge (`SetPanelWidth`, kept in
  the settings, limited to 220 to 480 px); the signals take the rest, also at the minimum window width.
  The pointer is followed over the whole window while the button is held.
- The Capture view, top to bottom: Start (Stop while a capture runs) and what keeps it from working, device, sample rate, length in seconds (with the samples beside it),
  seed, density (the last two only for the simulated device, which always runs in real time), the channels to record, the
  explanations of problems.
- **Channel buttons**: 8 per row sharing the width of the panel (28 px high, 3 px gap), centred number, the number of the channel inside (the name is too long);
  off: border and number in the channel's colour on the panel background; on: border and background in the
  channel's colour and the number in black or white, whichever contrasts better. They send
  `SetChannelVisible`; **All** and **None** beside **Reset** set the whole recording mask at once; they are inert while a capture runs; the tooltip is the channel's name.

## The edge buttons

- In a lane, when the pointer is inside the waveform area within 28 px of its left or right end, a 24 px
  rounded square with an arrow is drawn at that end, vertically centred, in the overlay layer.
- The right one is drawn only if the plan's `edge_after` for the channel is true; the left one only if
  `edge_before` is true.
- A press on the button sends `ScrollToEdge { channel, direction }` and is not also a pan, a marker
  placement or an annotation click. Leaving the zone removes the button; nothing moves.
- The buttons are not drawn for decoder lanes or while the pointer is over the legend or the axis.

## Menus

- The menu code supports entries that open a submenu to the right of the menu.
- **File → Open recent**: up to 15 entries (file name, the folder as secondary text); a missing file is
  disabled and shows a marker; at the end "Remove unavailable entries" and "Clear the list" (each only
  when there is something to remove). An empty list shows one disabled entry.
- **Decoders → Add decoder**: one entry per registered decoder by name; disabled with the reason when
  there is no capture. The key **D** opens the Decoders menu with the submenu expanded.

## What does not change

All keyboard shortcuts except the behaviour of D, the zoom and pan gestures over the waveforms, the
marker interactions, the annotation selection and hover, the status line, the dialogs.
