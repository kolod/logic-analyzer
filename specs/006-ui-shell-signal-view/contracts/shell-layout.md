# Contract: Shell layout, commands and shortcuts

The visual design is original (constitution Principle XII); the arrangement below is a functional
description, not a pixel layout. It is verified by the headless checks and the manual checklist in
[../quickstart.md](../quickstart.md).

## Regions

```text
+--------------------------------------------------------------------+
| Menu bar                                                           |
| Toolbar  [Start/Stop] [Device ▾] [Rate ▾] [Length ▾]   [Zoom] [Fit] |
+--------------------------------------------------------------------+
| Notice banner (only when there is a message)                       |
+----------+---------------------------------------------+-----------+
| Channel  | Time axis                                   | Side      |
| labels   |---------------------------------------------| panel     |
| (name,   | Signal view (one lane per visible channel)  | (device   |
| colour,  |                                             | settings, |
| visible) |---------------------------------------------| markers,  |
|          | Decoder lanes (reserved, empty in 006)      | capture   |
|          |                                             | info)     |
+----------+---------------------------------------------+-----------+
| Status: state · samples · memory/limit · simulated · unsaved · rate |
+--------------------------------------------------------------------+
```

- The **channel label column** scrolls vertically with the lanes (never separately), so a name always
  sits next to its lane; with 32 channels the lanes scroll and the axis stays fixed.
- The **decoder lanes** region is reserved for roadmap item 007: it has a registration point that
  adds lanes and panels, and in this feature it is empty and takes no space (FR-037).
- The **side panel** is collapsible; its sections (Device, Markers, Capture) are registered the same
  way later features register theirs.
- **Simulated** is shown in the status area and as a badge in the view corner whenever the capture is
  simulated; it is not dismissible (FR-006).
- **Minimum window size**: 800 × 560 logical pixels. At this size and with the longest German and
  Japanese resources no text may clip, overlap or lose its control (SC-008).

## Layout rules for text (FR-025, FR-026)

1. Containers size from the measured text; fixed widths are used only for icons.
2. Labels may wrap to a second line; buttons grow; the toolbar wraps into two rows before it hides
   a control.
3. A single-line field that truncates (channel name in the label column) shows an ellipsis and the
   full text on hover and focus.
4. User text is isolated for bidirectional order and has a visible length limit (64 characters) in
   edit fields.
5. Numbers and units never share a fixed-width box with surrounding words.

## Commands, menu paths and shortcuts

`ShortcutEntry { command, menu_path (message keys), accelerator, enabled_when }` is data in
`la-host`, so the menu, tooltips and the keyboard test read one table. Accelerators are defaults;
none conflicts with the operating system on Windows or Debian.

| Command | Menu (key under `menu-*`) | Shortcut | Enabled when |
|---------|---------------------------|----------|--------------|
| `NewCapture` | File ▸ New capture | Ctrl+N | Idle |
| `OpenSession` | File ▸ Open… | Ctrl+O | Idle or Capturing (asks first) |
| `SaveSession` | File ▸ Save | Ctrl+S | a capture exists |
| `SaveSessionAs` | File ▸ Save as… | Ctrl+Shift+S | a capture exists |
| `Quit` | File ▸ Quit | Ctrl+Q | always |
| `StartCapture` | Capture ▸ Start | F5 | device selected, no config problems, Idle |
| `StopCapture` | Capture ▸ Stop | Shift+F5 | Capturing |
| `SetFollowLatest` | Capture ▸ Follow latest | F6 | Capturing |
| `ZoomStep(In)` / `(Out)` | View ▸ Zoom in / out | `+` / `-` (and Ctrl+wheel) | a capture exists |
| `PanStep` | View ▸ Pan | ←/→, Shift+←/→ for pages | a capture exists |
| `FitAll` | View ▸ Fit all | Ctrl+0 | a capture exists |
| `GoToStart` / `GoToEnd` | View ▸ Start / End | Home / End | a capture exists |
| `PlaceMarker` | Markers ▸ Place at cursor | M | a capture exists |
| `GoToMarker` | Markers ▸ Previous / Next | `[` / `]` | markers exist |
| `RemoveMarker` | Markers ▸ Remove | Delete | a marker is selected |
| `RemoveAllMarkers` | Markers ▸ Remove all | | markers exist |
| `SetSnap` | Markers ▸ Snap to transitions | S | a capture exists |
| `SetUiScale` | View ▸ Interface scale | Ctrl+= / Ctrl+- | always |
| (settings dialog) | Tools ▸ Settings… | Ctrl+, | always |
| `DismissAllNotices` | View ▸ Dismiss messages | Esc (when a banner is shown) | notices exist |
| (log location) | Help ▸ Show log location | | always |
| (about) | Help ▸ About | | always |

Mouse: wheel zooms around the pointer, Shift+wheel pans, drag pans, drag on the time axis selects a
range (then `ZoomToRange`), click on a lane with Alt held places a marker, drag a marker to move it.
All mouse actions have a keyboard or menu equivalent in the table (FR-036).

## Dialogs

| Dialog | Purpose | Notes |
|--------|---------|-------|
| Unsaved changes | Save / Discard / Cancel for a deferred guarded command | shown for close, open, new, quit; also when a capture is running (it is stopped first) |
| Settings | language, theme, interface scale, memory limit | language applies on selection; the dialog's own text switches too |
| Save/Open | native file dialogs | filter `.lasession` (extension chosen in implementation, recorded in `docs/host-api.md`); recent files in the File menu |
| About | name, version, licence (GPL-3.0), third-party notices (fonts, libraries) | text from resources except licence names |
| Error details | the localized message plus "Show log location" | never shows the technical detail |

## Status area content

Left to right: activity (Idle / Capturing / Saving 42 % / Loading), samples captured (localized
count with plural form), memory used / limit (localized size), end reason of the last capture,
`Simulated`, `Unsaved changes`, sample rate of the capture. Each part appears only when relevant.

## Messages (resource keys)

All shell text lives in `resources/i18n/<lang>/ui.ftl` with keys prefixed by area
(`menu-`, `toolbar-`, `status-`, `dialog-`, `settings-`, `capture-`, `marker-`, `channel-`,
`measure-`, `view-`, `end-reason-`). Counted text (`count-samples`, `count-channels`, …) reuses the
shared messages of feature 005. The parity check of feature 005 covers the new file without change
other than registering `ui.ftl` in the embedded resource list.
