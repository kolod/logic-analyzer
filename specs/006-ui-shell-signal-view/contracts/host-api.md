# Contract: Host API (version 1.0)

**Status**: the design source. The final, authoritative text is [`docs/host-api.md`](../../../docs/host-api.md)
(research R3), which includes the changes made during implementation: where the two differ, the
document in `docs/` is right. `la_host::HOST_API_VERSION` and that document's version line agree;
a test fails otherwise.

The host API is the only way a user interface talks to the core (constitution Principle IV). It is
UI-toolkit independent, synchronous and made of plain data. The first consumer is `la-app`
(iced); a different shell would use the same API.

## Rules

1. **Commands in, state and events out.** A shell sends `Command`s and reads `State` and
   `FramePlan`s. It never calls the storage, device or session crates directly.
2. **No blocking.** `apply` and `poll` return within 100 ms regardless of capture size; anything
   slower runs as a background operation whose progress appears in `State`.
3. **No panics cross the boundary.** A panic inside a command is contained, logged and reported as
   the `host.internal` notice; the previous state stays intact.
4. **Errors are values.** A failure is a `Notice` carrying a stable error code; a command that is
   merely not allowed now returns `Outcome::Ignored(host.invalid_command)` and changes nothing.
5. **Text is localized by the host.** The shell asks for text (`text`, `notice_text`) and never
   formats numbers, units or messages itself.
6. **Versioned.** A change to commands, state fields, events or semantics changes the version
   (minor for additions, major for breaks) and lands with this document, the implementation and the
   consuming shell in one merge.

## Entry points

```text
Shell::new(config: ShellConfig) -> Shell
    // loads settings, builds the registry with the virtual device, the localizer, logging hooks

Shell::apply(&mut self, command: Command) -> Outcome
Shell::poll(&mut self, now: Instant) -> Changes
Shell::state(&self) -> &State
Shell::frame_plan(&self, size: ViewSize) -> FramePlan
Shell::text(&self, key: &Key, args: &Args) -> String
Shell::notice_text(&self, notice: &Notice) -> String
Shell::shortcut_table(&self) -> &[ShortcutEntry]          // data, see shell-layout.md
Shell::wants_ticks(&self) -> Tick                          // None | Fast (50 ms) | Slow (500 ms)
Shell::log_location(&self) -> Option<PathBuf>
```

`ViewSize { pixels: u32, scale: f32 }`: the width of the signal area in physical pixels (clamped to
the storage maximum) and the display scale factor.

`Outcome` is one of `Done(Changes)`, `NeedsDecision(Decision)` (guarded command deferred),
`Ignored(ErrorCode)`, `Failed(NoticeId)`.

`Changes` is a set of flags: `DEVICES`, `CONFIG`, `CAPTURE`, `PROGRESS`, `WINDOW`, `PRESENTATION`,
`MARKERS`, `NOTICES`, `SETTINGS`, `ACTIVITY`, `DECISION`, `LANGUAGE`, `THEME`. A shell redraws only
the parts whose flag is set.

## Commands

| Group | Command | Effect |
|-------|---------|--------|
| Devices | `RefreshDevices` | re-lists devices through the registry |
| | `SelectDevice(DeviceId)` | selects it; resets the config draft to the device defaults and validates |
| | `EditConfig(ConfigEdit)` | `Channels(mask)`, `SampleRate(hz)`, `Length(samples)`, `Trigger(Option<Trigger>)`, `Seed(Option<u64>)`, `Profile(name)`; validates after each edit; problems appear in `config_problems` |
| Capture | `StartCapture` | guarded; starts the acquisition with the validated config; ignored if there are problems |
| | `StopCapture` | stops the running capture; data kept; end reason "stopped by user" |
| | `NewCapture` | guarded; clears the capture and returns to Idle |
| View | `SetViewSize(ViewSize)` | informs the host of W so window limits use it |
| | `Zoom { factor, anchor }` | `anchor`: pixel column (pointer) or `Centre` |
| | `ZoomStep(In or Out)` | factor 2 around the centre |
| | `Pan { pixels }` / `PanStep(Left, Right, PageLeft, PageRight)` | moves the window, clamped |
| | `FitAll`, `GoToStart`, `GoToEnd`, `ZoomToRange { start, end }` | |
| | `SetFollowLatest(bool)` | live follow while capturing |
| Channels | `RenameChannel { channel, name }`, `SetChannelColour { channel, colour }`, `SetChannelVisible { channel, visible }`, `MoveChannel { from, to }`, `ResetPresentation` | edits presentation; marks `Dirty` |
| Markers | `PlaceMarker { position, channel }` | snaps to the nearest transition of `channel` when snapping is on |
| | `MoveMarker { id, position, channel }`, `RenameMarker { id, name }`, `RemoveMarker(id)`, `RemoveAllMarkers` | |
| | `SelectMarkers { first, second }`, `GoToMarker(id)`, `SetSnap(bool)` | |
| Sessions | `SaveSession` | saves to the current path, or returns `Outcome::Ignored` asking for `SaveSessionAs` when there is none (the shell then shows a dialog) |
| | `SaveSessionAs(PathBuf)`, `OpenSession(PathBuf)` | background operations; `OpenSession` is guarded |
| | `CancelOperation` | cancels a running save or load |
| | `Decide(Decision)` | `Save`, `Discard`, `Cancel` for a deferred guarded command |
| Settings | `SetLanguage(Language)`, `SetTheme(Theme)`, `SetUiScale(percent)`, `SetMemoryLimit(bytes)` | validated, saved atomically in the background, language effective at once |
| Notices | `DismissNotice(id)`, `DismissAllNotices` | |
| App | `RequestClose` | the window close button; guarded |
| | `Quit` | guarded; the shell closes only when `Outcome::Done` carries `QUIT` |

Every command a menu entry, toolbar button or shortcut triggers is one of these (FR-036 check).

## Events and polling

`poll` drains, in this order: device hot-plug events; acquisition progress and events (end reason,
gaps, failures); save/load progress and outcomes; the settings writer's result. It updates `State`,
adds `Notice`s for failures, and returns `Changes`. The shell calls it on a timer governed by
`wants_ticks()`; with nothing running the answer is `Slow`, and `poll` does no work and reports no
changes when nothing happened, so the idle application neither redraws nor uses measurable CPU.

Capture completion, save completion, and load completion each update the `Unsaved` state, the
recent-files list and the window title data in the same `poll`.

## Frame plan contract

`frame_plan(size)` is a pure function of `State` and `size` that costs
O(log N + W·(1 + log(1 + N_w/W))) per visible channel (constitution Principle VIII) and
allocates O(W). It never reads samples. Its output satisfies the invariants of
[data-model.md](../data-model.md) (contiguous, sorted, merged segments; dense columns never
dropped; gaps never drawn as levels). A failure returns a plan whose `state` is
`CannotDraw(host.render_failed)` instead of panicking.

## Text and localization

`text` and `notice_text` go through `la-i18n` with the current language and the English fallback.
Numbers, frequencies, durations, sizes and dates in the plan (`label_seconds`, measurements) are
supplied raw; the host formats them when producing text for the shell (`format_measurement`,
`format_tick_label`). User-supplied text (channel and marker names, description) is returned
sanitized and unchanged otherwise.

## Logging

The host logs, through `tracing`: each command at `info` (name and non-sensitive arguments such as
channel index or path file name, never signal data), capture and operation lifecycle, every notice
with its code and technical detail at `warn`/`error`. Panics are logged with the command name. The
log location is available through `log_location()`.

## Compatibility

Version 1.0 is the first. Additions of commands or `State` fields that existing shells can ignore
are minor versions. Removing or changing the meaning of a command, a state field or an invariant is a
major version and requires every shell in the repository to be updated in the same merge.
