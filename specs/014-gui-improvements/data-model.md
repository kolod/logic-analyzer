# Data Model: GUI Improvements

New and changed data. Nothing about the capture, the description, `VIEW`, `DECO` or `DANN` changes.

## Host state and presentation

| Entity | Field | Type and rule |
|--------|-------|---------------|
| `ChannelStyle` | `height` | `u16`; `0` means automatic, otherwise `24..=240` logical pixels; default 0 |
| `State` | `side_panel` | `SidePanel`: `Capture` (the default on a first start), `Decoders`, `Markers`, `Hidden` |
| `Settings` | `side_panel` | optional string `"capture"`, `"decoders"`, `"markers"` or `"hidden"`; an unknown value reads as the default; kept by builds that do not know it |
| `Settings` | `recent_files` | up to `MAX_RECENT_FILES` = 15 entries, newest first |
| `RecentFile` | `path`, `exists` | what the File menu shows; `exists` is looked up when the menu is built |
| `ChannelPlan` | `edge_before`, `edge_after` | `bool`: the channel has a real edge beyond the window on that side |

## Commands (host API 1.2)

| Command | Rule |
|---------|------|
| `SetChannelHeight { channel, height }` | refused (`host.invalid_command`) for an unknown channel; `height` 0 or clamped into `24..=240`; allowed at any time, also while a capture runs |
| `SetSidePanel(SidePanel)` | always allowed; saved in the settings |
| `ScrollToEdge { channel, direction }` | `direction` is `Previous` or `Next`; ignored with `host.invalid_command` when there is no capture, the channel is not shown or no edge exists on that side; otherwise moves the window as in contracts/host-api-1.2.md and turns following the newest data off |
| `RemoveRecentFile(PathBuf)` | removes the entry (and ignores a path that is not listed) |
| `ClearRecentFiles` | empties the list |
| `ResetPresentation` | also sets every height to automatic |

## Pure functions (host)

| Function | Rule |
|----------|------|
| `samples_for_seconds(rate_hz, text) -> Result<u64, SecondsError>` | decimal text, at most 9 decimals, no sign, no exponent; `rate_hz * seconds` in `u128`, rounded half up to a whole sample; refused: empty, not a number, zero or less than one sample, too large for `u64`, rate zero |
| `seconds_for_samples(rate_hz, samples) -> String` | the exact decimal value with the digits the numbers resolve (no trailing zeros), `"0"` for none; never uses floating point |

## Session

| Section | Tag | Content |
|---------|-----|---------|
| channel layout | `LAYT` (optional, not critical) | the heights, contracts/layt-section.md |

## Application (not stored)

| Entity | Content |
|--------|---------|
| lane layout | for each lane: its rectangle (top, height) after scrolling, and in the legend its zones: colour bar, name, border |
| drag state | none, a lane being moved (from index, pointer offset, landing index), or a border being dragged (lane index, start height, start y) |
| editor | which channel is being renamed and the text in it |
| edge buttons | not state: derived from the pointer position and `edge_before` / `edge_after` on every frame |
| length text | the seconds as typed, kept when the rate changes |
| vertical offset | how far the lanes are scrolled |

## Validation rules

- A height outside `24..=240` is clamped by the host; `LAYT` with a height outside it is invalid.
- A rename goes through the existing sanitising of names (feature 006).
- The side-panel setting never prevents the application from starting: an unreadable value gives the default.

## State transitions

- The side panel: `Hidden → X` by the icon of X; `X → Y` by the icon of Y; `X → Hidden` by the icon of X.
  No other transition.
- A lane drag: idle → dragging (movement over 4 px from a press on a name) → idle with `MoveChannel` on release
  (or nothing when released where it started or outside the strip with Escape).
