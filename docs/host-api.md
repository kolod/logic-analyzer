# Host API

Version: 1.0

This is the authoritative description of the API between a user interface and the core. The crate
is `la-host`; its constant `la_host::HOST_API_VERSION` and the version line above must agree (a test
checks it, together with the command table below).

The host API is the only way a user interface talks to the core (constitution Principle IV). It is
independent of any UI toolkit, synchronous and made of plain data. The first consumer is `la-app`
(iced); a different shell would use the same API. The design that led here is
`specs/006-ui-shell-signal-view/contracts/host-api.md`; this document replaces it as the source of
truth.

## Rules

1. **Commands in, state and plans out.** A shell sends `Command`s and reads `State` and
   `FramePlan`s. It never calls the storage, device or session crates directly.
2. **No blocking.** `apply` and `poll` return at once regardless of capture size; anything slow
   (saving, loading, a capture) runs in the background and its progress appears in `State`.
3. **No panics cross the boundary.** A panic inside a command or a poll is contained, logged and
   reported as the `host.internal` notice; the previous state stays intact.
4. **Errors are values.** A failure is a `Notice` carrying a stable error code; a command that is
   merely not allowed now returns `Outcome::Ignored(..)` and changes nothing.
5. **Text is localized by the host.** The shell asks for text (`text`, `notice_text`,
   `status_line`, `measurement_text`) and never formats numbers, units or messages itself.
6. **Versioned.** A change to commands, state fields or semantics changes the version (minor for
   additions, major for breaks) and lands with this document, the implementation and every
   consuming shell in one merge.

## Entry points

```text
Shell::new(config: ShellConfig) -> Shell
    // loads the settings, builds the registry with the virtual device and the localizer

Shell::apply(&mut self, command: Command) -> Outcome
Shell::poll(&mut self, now: Instant) -> Changes
Shell::state(&self) -> &State
Shell::frame_plan(&self, size: ViewSize) -> FramePlan
Shell::text(&self, key: &Key, args: &Args) -> String
Shell::notice_text(&self, notice: &Notice) -> String
Shell::status_line(&self) -> StatusLine
Shell::measurement_text(&self) -> Option<MeasurementText>   // distance and frequency of two markers
Shell::shortcut_table(&self) -> Vec<ShortcutEntry>          // data: menu path, accelerator, action
Shell::wants_ticks(&self) -> Tick                           // Fast (50 ms) while something runs, else Slow (500 ms)
Shell::recent_files(&self) -> Vec<PathBuf>                  // the ones that still exist
Shell::operation_running(&self) -> bool                     // a save or load
Shell::save_settings(&mut self)                             // before exit: writes what is only in memory
Shell::log_location(&self) -> Option<&Path>
```

`ViewSize { pixels, scale }` is the width of the signal area in physical pixels (the host keeps it
within 1 to 16,384) and the display scale factor.

`Outcome` is one of

- `Done(Changes)`: the command ran;
- `NeedsDecision(Guarded)`: unsaved data would be lost; the command waits for `Decide`;
- `Ignored(reason)`: not valid now, nothing changed. The reason is `host.invalid_command`, or
  `needs_path` when `SaveSession` or `Decide(Save)` was asked for a capture that has no file yet
  (the shell then asks for a name and sends `SaveSessionAs`);
- `Failed(NoticeId)`: it failed and the notice says why.

`Changes` is a set of flags: `DEVICES`, `CONFIG`, `CAPTURE`, `PROGRESS`, `WINDOW`, `PRESENTATION`,
`MARKERS`, `NOTICES`, `SETTINGS`, `ACTIVITY`, `DECISION`, `LANGUAGE`, `THEME`, `QUIT`. A shell
redraws only the parts whose flag is set. `QUIT` means the shell may close the window.

## Commands

| Group | Command | Effect |
|-------|---------|--------|
| Devices | `RefreshDevices` | re-lists devices through the registry |
| | `SelectDevice(DeviceId)` | selects it; resets the configuration draft to the device defaults and validates |
| | `EditConfig(ConfigEdit)` | `Channels`, `SampleRate`, `Length`, `Trigger`, `Seed`, `Profile`, `Realtime`, `AcceptBudgetRisk`; validates after each edit; problems appear in `config_problems` |
| Capture | `StartCapture` | guarded when an unsaved capture is on screen; ignored while one runs or the configuration has problems |
| | `StopCapture` | stops the running capture; the data is kept |
| | `NewCapture` | guarded; clears the capture and returns to Idle |
| View | `SetViewSize(ViewSize)` | tells the host the width so window limits use it |
| | `Zoom { factor, anchor }` | `anchor` is a pixel column (the pointer) or `Centre` |
| | `ZoomStep(ZoomDirection)` | `In` or `Out`, factor 2 around the centre |
| | `Pan { pixels }` / `PanStep(PanStep)` | moves the window, clamped; a manual move turns following off |
| | `FitAll` | shows the whole capture (and follows its growth) |
| | `GoToStart` | the window moves to the start |
| | `GoToEnd` | the window moves to the end |
| | `ZoomToRange { start, end }` | shows that range |
| | `SetFollowLatest(bool)` | live follow while capturing |
| Channels | `RenameChannel { channel, name }` | the name is sanitized and limited to 64 characters |
| | `SetChannelColour { channel, colour }` | a palette entry or a chosen colour |
| | `SetChannelVisible { channel, visible }` | shows or hides a channel |
| | `MoveChannel { from, to }` | changes the order |
| | `ResetPresentation` | defaults for names, colours, order and visibility |
| Markers | `PlaceMarker { position, channel }` | at most 64; snaps to the nearest transition of `channel` when snapping is on |
| | `MoveMarker { id, position, channel }` | snaps like a new marker |
| | `RenameMarker { id, name }` | sanitized like a channel name |
| | `RemoveMarker(u32)` | removes it and clears it from the measured pair |
| | `RemoveAllMarkers` | |
| | `SelectMarkers { first, second }` | the pair a measurement uses |
| | `GoToMarker(u32)` | moves the window so the marker is visible, zoom unchanged |
| | `SetSnap(bool)` | snapping of markers to transitions |
| Sessions | `SaveSession` | to the current file; `Ignored(needs_path)` when there is none |
| | `SaveSessionAs(PathBuf)` | background save; atomic; the old file survives any failure |
| | `OpenSession(PathBuf)` | background load; guarded; a failure leaves the capture on screen unchanged |
| | `CancelOperation` | cancels a running save or load |
| | `Decide(Decision)` | `Save`, `Discard` or `Cancel` for a command that waits; `Save` runs the command only after the save succeeded |
| Settings | `SetLanguage(Language)` | effective at once everywhere; remembered |
| | `SetTheme(Theme)` | `System`, `Light`, `Dark` or `HighContrast` |
| | `SetUiScale(u16)` | percent, clamped to 75 to 200 |
| | `SetMemoryLimit(u64)` | bytes; validated against the storage limits; applies to the next capture only |
| | `SetWindowGeometry(WindowGeometry)` | kept in memory; written by `save_settings` |
| Notices | `DismissNotice(NoticeId)` | |
| | `DismissAllNotices` | |
| App | `RequestClose` | the window close button; guarded |
| | `Quit` | guarded; the shell closes only when `Done` carries `QUIT` |

While a save or load runs, the commands that change what is being saved or replace what is being
loaded (starting or clearing a capture, edits of names, colours, order, visibility and markers,
opening and saving again, `Decide(Save)` and `Decide(Discard)`) are ignored; `CancelOperation`,
`Decide(Cancel)` and everything about the view still work.

Every command a menu entry, toolbar button or shortcut triggers is one of these; a test
(`shortcut_coverage`) checks that each is reachable from the keyboard.

## Unsaved data and the guard

`StartCapture` over an unsaved capture, `NewCapture`, `OpenSession`, `Quit` and `RequestClose`
return `NeedsDecision` when the capture is unsaved or still running. A running capture is stopped
first and its data kept. The answer comes with `Decide`:

- `Cancel` drops the command and keeps everything.
- `Discard` runs the command; the data on screen is replaced (or, for `Quit`, thrown away).
- `Save` saves to the current file and runs the command only when that save succeeded. Without a
  file it returns `Ignored(needs_path)`; the shell asks for a name and sends `SaveSessionAs`, and
  that save carries the waiting command along. If the save fails the command stays pending and the
  capture stays. If the waiting command then fails, the capture on screen stays as it was.

No sequence of commands discards unsaved data without `Decide(Discard)`; a test walks 20,000
random commands to check it.

## Polling

`poll` drains, in this order: device hot-plug events; the progress and end of a save or load; the
progress, events and end of the running capture. It updates `State`, adds `Notice`s for failures
and returns `Changes`. The shell calls it on a timer governed by `wants_ticks()`. With nothing
running the answer is `Slow` and `poll` reports no changes when nothing happened, so the idle
application does not redraw.

Save and load completion update the unsaved state, the recent-files list and the file name for the
window title in the same `poll`. Settings are written at once, atomically, by the command that
changes them; a failure becomes a `host.settings_write` notice and the choice stays in effect.

## Frame plan

`frame_plan(size)` is a function of `State` and `size`. Its cost is
O(log N + W·(1 + log(1 + N_w/W))) per visible channel (constitution Principle VIII) and it
allocates O(W); it never reads samples. Its output is contiguous, sorted and merged segments; dense
columns are never dropped and gaps are never drawn as levels. A one-sample glitch is visible at
every zoom level (a test sweeps the zoom). A failure returns a plan whose `state` is
`CannotDraw(host.render_failed)` instead of panicking. The plan includes the markers inside the
window.

## Text and localization

`text` and `notice_text` go through `la-i18n` with the current language and the English fallback.
Numbers, frequencies, durations, sizes and dates are formatted by the host for the language in use.
User-supplied text (channel and marker names, the description) is sanitized (control, invisible
and bidirectional characters removed, trimmed, limited) and otherwise returned unchanged; it is
never translated.

## The `VIEW` section of a session file

The presentation, markers and window are stored in the optional `VIEW` section of a session file
(format 1.1): `specs/006-ui-shell-signal-view/contracts/view-section.md`. It is written only when
the look differs from the defaults. An invalid section opens the capture with defaults and a
`host.view_invalid` warning and is dropped on the next save; a newer one is kept untouched.

## Logging

The host logs, through `tracing`: each command at `info` (its name, never signal data or user
text), capture and operation lifecycle, and every failure with its code and technical detail at
`warn` or `error`. Panics are logged with the command name. Neither the log nor a notice contains
signal data, channel names or marker names.

## Compatibility

Version 1.0 is the first. Additions of commands or `State` fields that existing shells can ignore
are minor versions. Removing or changing the meaning of a command, a state field or an invariant is
a major version and requires every shell in the repository to be updated in the same merge.
