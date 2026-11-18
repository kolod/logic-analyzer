# Research: GUI Improvements

Decisions taken against the code of features 006 to 008 (`crates/app`, `crates/host`,
`docs/host-api.md`) and the spec. No `NEEDS CLARIFICATION` was left in the Technical Context.

## R1. Where the logic lives

**Decision.** Everything that can be decided without a screen goes into `la-host` (the host API,
Principle IV): the heights of the channels, the view of the side panel, the recent-file rules, the
conversion between seconds and samples, the search for the next edge, and what a button of the
capture view means. `la-app` keeps what is geometry and drawing: the lane layout (a pure module with
unit tests), the hit zones of the legend, the drawing of the edge buttons, the panel and the menus.

**Why.** The constitution requires the core to be testable headlessly and a second shell to be
possible; the tests of feature 006 already work this way (host tests for state, headless rendering
tests for what is drawn).

## R2. The host API grows to 1.2

Additions, all optional for a shell that ignores them (a minor version, as for 1.1):

| Addition | Why |
|----------|-----|
| `Command::SetChannelHeight { channel, height }` and `ChannelStyle.height` | FR-003 |
| `Command::SetSidePanel(SidePanel)` and `State.side_panel` (`Hidden`, `Capture`, `Decoders`, `Markers`), kept in the settings | FR-007, FR-008 |
| `Command::ScrollToEdge { channel, direction }` (and, in `la-storage`, `Snapshot::next_transition` and `previous_transition`) | FR-017, FR-019 |
| `ChannelPlan.edge_before` and `ChannelPlan.edge_after` (whether the signal has an edge beyond the window on that side) | FR-016, FR-018 |
| `Command::RemoveRecentFile(PathBuf)`, `Command::ClearRecentFiles`, `MAX_RECENT_FILES` 10 → 15, `Shell::recent_files()` returning whether each file exists | FR-021 |
| `la_host::config::{samples_for_seconds, seconds_for_samples}` (pure functions) | FR-012 |

`MoveChannel`, `RenameChannel`, `SetChannelColour` and `SetChannelVisible` exist and are used as they
are. `docs/host-api.md` and `HOST_API_VERSION` change in the same merge; the test that compares the
document's command table with the code covers the new commands.

## R3. Heights of the signals

**Today** every lane has the same height, computed from the window: `clamp((area - axis) / lanes, 30,
72)`, and what does not fit is cut off. **Decision:** a channel has an optional height in logical
pixels, `0` meaning "automatic" (today's rule), otherwise `24..=240`. Dragging a border sets the height
of the channel above it. A lane of a decoder keeps the automatic rule.

When the lanes are taller than the area the signal area scrolls vertically: with the wheel while the
pointer is in the legend strip (the wheel over the waveforms keeps zooming), and a thin bar at the right
edge of the strip. The cut-off of today (a 32-channel capture in a short window) is thereby fixed too.

The heights are kept for the next capture and saved with the session in a **new optional section
`LAYT`** (contracts/layt-section.md), not in `VIEW`: a build without this feature keeps an unknown
section untouched (feature 003) and opens everything else, including names, colours and order, as it
did, which `VIEW` version 2 would not give (an older reader treats a newer `VIEW` as unreadable). This
is FR-025.

**Alternatives rejected.** Heights as shares of the area (no scrolling needed, but a tall signal then
shrinks the others and 32 channels still do not fit); changing `VIEW` to version 2.

## R4. The legend strip: one layout for drawing, hit tests and editors

**Decision.** A pure module `crates/app/src/lane_layout.rs` computes, from the plan (the channels with
their heights, the decoder lanes), the area height and the vertical offset, the rectangle of every lane
and, inside the legend strip, the zones of a lane: the **colour bar** (the first 8 pixels), the **name**
(the rest of the strip, minus a margin), and the **border** below the lane (a band of 7 pixels centred
on the line, for dragging). Drawing, the hit tests and the place of the in-place editor all use it, so
they cannot disagree. It has unit tests.

Interaction (the canvas widget decides, the application executes through the host):

| Gesture | Result |
|---------|--------|
| press and release on the colour bar | `SetChannelColour(next palette entry)` |
| press and release on the name, movement under 4 px | the editor opens over the name |
| press on the name, move more than 4 px | a drag of the lane begins; a mark shows the landing place; release sends `MoveChannel` |
| press on a border, move | the height of the lane above follows the pointer (live, `SetChannelHeight` on release; the drawing follows the pointer meanwhile) |
| double click on a border | back to the automatic height |

**The in-place editor** is an ordinary `text_input` widget placed over the name by the application, on
a layer above the canvas, at the rectangle the layout gives; Enter or losing focus accepts and sends
`RenameChannel` (the host sanitises, an empty name gives back the default), Escape cancels. A canvas
cannot host a text field, hence the overlay.

## R5. The side panel

**Decision.** `view()` becomes `row![signal, panel?, strip]` with the strip of three icon buttons at
the right edge and the panel (280 px, minimum 220) between it and the signals when a view is shown. The
views are the three existing panel builders (`markers_panel`, `decoders_panel`, and a new
`capture_panel`); `channel_panel` is deleted. Icons are drawn with the canvas (simple vector shapes:
a circle with a dot for capture, a stack of boxes for decoders, a flag for markers) so that no icon font
or image dependency is needed, and they follow the theme. The current view is
`State.side_panel`, saved in the settings, so it comes back after a restart. The strip shows the active
view with a bar beside it. Clicking the active icon sends `SetSidePanel(Hidden)`.

The width of the panel is not a user setting in this feature (no dragging of the panel edge): out of
scope.

## R6. The capture view

**Decision.** `toolbar()` keeps Start/Stop, the state and, below, the explanations of problems that are
not about channels; the controls move into `capture_panel()`: device, sample rate, **length in seconds**,
seed, density, and the channel buttons.

**Seconds.** The configuration still counts samples (the device configuration and the session
description are unchanged). `samples_for_seconds(rate_hz, text)` parses a decimal number (a dot or the
locale's separator is not needed: the field is text the user types, parsed with the application's
number rules; at most 9 decimals), multiplies exactly in integer arithmetic (`u128`), rounds to the
nearest sample, and refuses a value that is zero, negative, not a number or too large for `u64`.
`seconds_for_samples(rate_hz, samples)` is the inverse for display, with only as many digits as the
number resolves (as the marker frequency does). The seconds the user typed are kept in the application
state; when the rate changes the samples are recomputed from them (FR-012), and the view shows the
samples beside the field.

**Channel buttons.** Eight in a row (`row` of eight, wrapped), each a `button` with a style function
that takes the channel colour from the theme variant (the same mapping as the legend): off, the text and
the border in the colour and the background of the panel; on, border and background in the colour and
the text in whichever of black or white has the better contrast on it (a function in `theme.rs`,
unit-tested for all 16 palette entries in both themes). Pressing sends `SetChannelVisible` (the flag keeps its two
meanings, an assumption of the spec). While a capture runs the buttons have no `on_press`.

## R7. Quick scroll to the next edge

**Decision.** The host does the search and the placement:

- `ScrollToEdge { channel, direction }` finds, in the snapshot, the first real edge of that channel
  **beyond** the visible window on that side. The storage offers `transitions(channel, start, end)`,
  forward only; this feature adds to `la-storage` two small queries on the same index,
  `Snapshot::next_transition(channel, from)` and `Snapshot::previous_transition(channel, before)`,
  each O(log N), tested against a brute-force scan on random captures. Edges next to a gap are not
  edges (as in the decoder reader) and are skipped; the number of skips is bounded by the number of
  gaps, which the storage limits.
- The window then starts at `edge - length / 4` (rightwards) or `edge - 3 * length / 4` (leftwards),
  clamped to the capture, with the length unchanged. It stops "follow latest data".
- The plan carries `edge_before` and `edge_after` for each channel, computed with the same search (two
  lookups of cost O(log N) per channel per frame, so Principle VIII holds), so the interface knows which
  buttons to offer. While a capture grows, the value is that of the snapshot of the frame.

**The buttons** are drawn in the cheap overlay layer of the canvas when the pointer is within 28 px of a
lane's left or right end (inside the lane, not on the legend), as a rounded square with an arrow; they
never invalidate the cached layer. A click on one is handled by the canvas before the pan or the marker
rules, and sends the command.

**Alternatives rejected.** "The next edge after the pointer" (a different meaning from the description,
which speaks of the view); doing the search in the interface (it would need the storage and break the
layering).

## R8. Recent files

`MAX_RECENT_FILES` becomes 15. `Shell::recent_files()` returns `Vec<RecentFile { path, exists }>`
(existence is checked when the menu opens, not per frame). The File menu gets an **Open recent**
entry that opens a submenu next to it (a second column in the menu overlay: the menu code of
`app.rs` is generalised to entries that have children, which R9 also uses). Entries that do not exist are shown
disabled with a marker; the submenu ends with "Remove unavailable entries" and "Clear the list"
(`RemoveRecentFile`, `ClearRecentFiles`). The folder is shown as a tooltip-like secondary text.

## R9. Add decoder from the menu and the keyboard

`Action::AddDecoder` has no child list today, hence "the menu cannot choose for you". **Decision:** the
Decoders menu entry becomes a submenu listing `catalogue` by name (the same generalised menu entries as
R8); choosing one sends `AddDecoder`. The D key opens the Decoders menu with that submenu expanded, so
the same choice is made with the keyboard; the entry is disabled with its reason when there is no
capture (the table's `enabled_when` as now). The branch `AddDecoderMenu` with one decoder is removed.
The panel's list of decoders stays as a second way.

## R10. Icons

- **Artwork**: an original design drawn for the project, in two SVG files kept as the source:
  the application (a rounded square with a stylised digital waveform in two colours) and the session file
  (a page with a folded corner carrying the same waveform). Drawn by hand as plain paths: no stock art,
  no font, nothing taken from Saleae, PulseView or sigrok. A script `tools/make_icons.py` (Python with
  Pillow, documented, not part of the build) renders them to PNG at 16, 32, 48, 64, 128, 256 pixels, to
  a Windows `.ico`, and to the raw RGBA used by the window; the outputs are committed.
- **Window icon**: `iced::window::Settings.icon` from raw RGBA (`icon::from_rgba`), a 64x64 array
  committed as a binary asset and embedded with `include_bytes!`. No decoding library is needed.
- **Windows executable icon**: the icon shown by Explorer and the task bar for the file itself is a
  resource in the executable. It is added by a build script with the `winresource` crate (MIT, a build
  dependency only on Windows, small, maintained; justified in the plan). If `cargo deny` or the MSRV job
  objects, the fallback is to leave the resource to packaging (015+) and keep the runtime window icon.
- **Declarations** in `packaging/`: `linux/logic-analyzer.desktop`, `linux/logic-analyzer-session.xml`
  (the MIME type for the session file, with its glob and the icon name) and `windows/README.md` saying
  what the installer must register (the extension, the icon from the `.ico`). A test checks that the
  files are well formed and that the icon names they mention exist.
- **Origin** of the artwork is recorded in `docs/third-party-notices.md` (it is ours, GPL-3.0 with the
  project) so nobody has to guess.

## R11. Compatibility

- Settings: new field `side_panel` is optional with a default; a build without the feature keeps it as
  an unknown field (the settings already flatten unknown fields).
- Session: `LAYT` is an optional section; unreadable or newer `LAYT` is ignored with an Info notice
  and the capture opens with automatic heights (as `DECO` and `DANN` are handled).
- Nothing in `VIEW`, `DECO`, `DANN`, the capture or the description changes.

## R12. Localization

New keys (all with six translations, no plural needed except where noted): the names of the three views
and their icons' tooltips, "Open recent" and its submenu entries (remove unavailable entries, clear the
list, empty, unavailable), the labels of the capture view (length in seconds, samples it makes, channels
to record, the reason Start is unavailable when no channel is on), the tooltips of the edge buttons (to the
next edge, to the previous edge), the legend editor (rename hint), "Add decoder" submenu title. A
sample count beside the seconds field uses the locale formatting of numbers. The in-place editor
and the buttons must not clip long German or Japanese texts at the minimum window size; the
headless rendering test of feature 006 is extended to the new screens in every language.

## R13. Tests

- Host tests (no display): heights (set, clamp, reset, saved and restored with the session in `LAYT`,
  absent in an old file, ignored in a damaged one, kept by a build that does not know it); side panel
  state and its settings; `ScrollToEdge` on constructed captures (edges, gaps, none beyond, both
  directions, the quarter placement to the sample, follow turned off); `edge_before/after` in plans;
  recent files (15, order, missing files, remove, clear); seconds conversion (property test of the round
  trip, rounding, overflow, hostile text); the host-API document test; the random walk learns the new
  commands.
- Application unit tests (no display): `lane_layout` (rectangles, zones, scrolling, drag landing),
  colour contrast for the channel buttons.
- Headless rendering (iced_test): the new screens in six languages at the minimum size; the legend
  with the editor open; the capture view with 8, 16 and 32 channels; the edge buttons visible
  (hover simulated); the side panel in each state.
- Benchmark: the frame-plan benchmark gets the two edge lookups; baseline recorded; no regression of
  the complexity class (Principle VIII).

## R14. Third-party code

None. The only new dependency is `winresource` as a Windows build dependency (R10), recorded in the
plan with purpose, maintenance and license.
