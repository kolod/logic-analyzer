# Contract: host API 1.1 → 1.2

Additions only (a minor version, `docs/host-api.md` rules): a shell that ignores them keeps working.
`HOST_API_VERSION` and the version line of `docs/host-api.md` become `1.2` in the same merge as the
implementation, and the command table of the document gains the new commands (a test compares the table
with the code).

## State

```text
State.side_panel: SidePanel            // Capture (default), Decoders, Markers, Hidden
ChannelStyle.height: u16               // 0 automatic, else 24..=240
ChannelPlan.edge_before: bool          // a real edge of this channel lies before the window
ChannelPlan.edge_after: bool           // a real edge lies after the window
Shell::recent_files() -> Vec<RecentFile { path: PathBuf, exists: bool }>   // up to 15, newest first
```

`edge_before` and `edge_after` are computed with the storage's indexed `previous_transition` and
`next_transition` (O(log N) each), only for the channels shown, once per frame plan.

## Commands

```text
SetChannelHeight { channel: u8, height: u16 }
SetSidePanel(SidePanel)
ScrollToEdge { channel: u8, direction: EdgeDirection }      // EdgeDirection: Previous | Next
RemoveRecentFile(PathBuf)
ClearRecentFiles
```

### ScrollToEdge, exactly

Let the window be `[start, start + length)` over a capture of `capture_length` samples.

- `Next`: the edge is the first real transition of `channel` at a position `>= start + length` (an edge
  at position `p` is the change between `p - 1` and `p`). The new start is `edge - length / 4`, clamped to
  `[0, capture_length - length]` (or 0 when the window is longer than the capture).
- `Previous`: the edge is the last real transition at a position `< start`. The new start is
  `edge - (3 * length) / 4` (integer division, so the edge is a quarter of the window from the right
  side), clamped as above.
- A transition next to a gap (one side of it has no data) is not a real edge and is skipped.
- The zoom (`length`) is unchanged; "follow latest data" is turned off.
- No capture, an unknown or hidden channel, or no edge on that side: `Outcome::Ignored(host.invalid_command)`
  and nothing changes.

The edge lands within one sample of exact (integer division of the length); for a window of at least 4
samples it is within one column of a quarter of the way for every width.

### SetChannelHeight

A height of 0 means automatic; any other is clamped into `24..=240`. Allowed at any time. `Changes::PRESENTATION`.
Heights are saved in `LAYT` (contracts/layt-section.md) and reset by `ResetPresentation`.

### SetSidePanel

Sets `State.side_panel` and saves the settings. `Changes::SETTINGS`. No capture is needed.

## Pure functions

```text
la_host::config::samples_for_seconds(rate_hz: u64, text: &str) -> Result<u64, SecondsError>
la_host::config::seconds_for_samples(rate_hz: u64, samples: u64) -> String
```

Rules in the data model. The property test: for every rate and every sample count, parsing what
`seconds_for_samples` printed gives the same sample count back whenever the printed value is exact
(at most 9 decimals); for other rates the printed value is rounded to 9 decimals and the round trip is
within one sample, which the test states.

## Settings

`Settings.side_panel: Option<String>` (optional, default absent, kept by builds that do not know it).
`MAX_RECENT_FILES = 15`: a settings file with more entries is truncated on load.

## Storage (la-storage)

```text
Snapshot::next_transition(channel: u8, from: u64) -> Result<Option<Transition>>       // first with position >= from
Snapshot::previous_transition(channel: u8, before: u64) -> Result<Option<Transition>> // last with position < before
```

O(log N); tested against a brute-force scan on random captures, including empty channels, edges at
position 0 and at the last sample, and captures with gaps.
