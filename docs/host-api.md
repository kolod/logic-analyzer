# Host API

Version: 1.2

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
Shell::recent_files(&self) -> Vec<RecentFile>               // up to 15, newest first; RecentFile { path, exists }
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
| | `EditConfig(ConfigEdit)` | `Channels`, `SampleRate`, `Length`, `Trigger`, `Seed`, `Profile`, `Realtime`, `AcceptBudgetRisk`; validates after each edit; problems appear in `config_problems`; `Channels` moves the flags of the channels the same way |
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
| Channels | `RenameChannel { channel, name }` | the name is sanitized and limited to 64 characters; the channel edits below work with or without a capture, and what they set stays for the next capture |
| | `SetChannelColour { channel, colour }` | a palette entry or a chosen colour |
| | `SetChannelVisible { channel, visible }` | the one flag of a channel: shows it (a lane is drawn when the capture on screen has data for it) and makes the next capture record it, so it also changes `EditConfig(Channels)`; ignored while a capture runs |
| | `MoveChannel { from, to }` | changes the order |
| | `SetChannelHeight { channel, height }` | the height of the signal of a channel in logical pixels: 0 is automatic, anything else is clamped into 24 to 240; allowed at any time; saved with the session in the optional `LAYT` section |
| | `ResetPresentation` | defaults for names, colours, order, heights and visibility |
| Side panel | `SetSidePanel(SidePanel)` | the view of the panel at the right edge: `Capture` (the default), `Decoders`, `Markers` or `Hidden`; kept in the settings |
| | `ScrollToEdge { channel, direction }` | the window moves, zoom unchanged, so that the first real edge of the channel beyond the window on that side (`Next`: at or after its end; `Previous`: before its start) is a quarter of the window from the side it came from; follow-latest turns off; ignored when there is no such edge |
| Unsaved data | `SetUnsavedChoice(Option<Decision>)` | remembers the answer to the question about an unsaved capture (`Some(Save)` or `Some(Discard)`: the question is no longer asked and the answer is applied as if it had been given; `None`: ask every time; `Some(Cancel)` is ignored, since a remembered Cancel would silently stop quitting and opening); kept in the settings (`unsaved_choice`: `save` or `discard`; any other word asks). A remembered Save of a capture that has no file yet still asks for a name (`needs_path`); the question about removing a decoder is always asked |
| Recent files | `RemoveRecentFile(path)` / `ClearRecentFiles` | edit the list of the 15 most recent files |
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
| Decoders | `AddDecoder { decoder }` | needs a capture on screen; creates an instance of the registered decoder with default options and no channels yet; refused with `decoder.too_many_instances` at eight instances |
| | `ConfigureDecoder { id, setup }` | replaces channels, options, name, lane visibility and generator options in one step; validated as a whole (`decoder.invalid_setup` names the setting in `decoders.refusal`); a valid change that alters results starts a new pass |
| | `SetDecoderChannel { id, role, channel }` | one channel assignment (or none); same validation and effect |
| | `SetDecoderOption { id, option, value }` | one option value; same validation and effect |
| | `RenameDecoder { id, name }` | sanitized, at most 64 characters |
| | `ShowDecoderLane { id, visible }` | shows or hides the lane and its overlays |
| | `RemoveDecoder(id)` | removes the instance, its pass and its results; when instances are stacked on it the answer is `NeedsDecision(Guarded::RemoveDecoder(id))`: `Decide(Discard)` removes it and everything above it, `Decide(Cancel)` keeps all, `Decide(Save)` is ignored; the capture is not touched |
| | `StackDecoder { id, source }` | makes `source` (the capture or another instance) the input of `id`; refused with `decoder.stack_incompatible`, `decoder.stack_cycle` or `decoder.stack_too_deep` (in `decoders.refusal`); a valid change starts a new pass, and the instances stacked on `id` start again after it |
| | `RunDecoder(id)` | starts a new pass of an instance that is not running (finished, cancelled, failed) |
| | `CancelDecoder(id)` | stops the running pass within a second; what it found is kept and marked incomplete; a decoder that does not stop in time is abandoned (`decoder.not_responding`) |
| | `SetSaveDecoderResults(bool)` | whether the results of the decoders are saved with the session (the setup is always saved); remembered |
| | `SetDecoderMemoryLimit(bytes)` | 16 MiB to 4 GiB, remembered; applies to passes that start afterwards, never to a running one |
| Annotations | `SelectAnnotation { instance, annotation }` | selects it and fills `decoders.selection` with its details |
| | `GoToAnnotation(AnnotationTarget)` | `Selected`, `Next` or `Previous` by start in the store of the selected annotation; moves the window so that it is shown whole and centred; costs O(log n) |
| | `ClearAnnotationSelection` | |
| App | `RequestClose` | the window close button; guarded |
| | `Quit` | guarded; the shell closes only when `Done` carries `QUIT` |

While a save or load runs, the commands that change what is being saved or replace what is being
loaded (starting or clearing a capture, edits of names, colours, order, visibility and markers,
adding, changing and removing decoders,
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

Since 1.2 every shown channel's plan has `height` (the height the user set, 0 for automatic),
`edge_before` and `edge_after`: whether a real edge of that channel lies before the window or after it.
They come from the storage's `previous_transition` and `next_transition` (O(log N) each, a few more
for each gap that is skipped), computed once per plan for the shown channels only. An edge is *real*
when the samples on both sides of it exist: the change next to a gap is not an edge.

Since 1.1 it also carries `lanes` (one lane plan per visible decoder instance: rows of spans and marks
and a row of dense cells, at most `4·W` cells) and `overlays` (per shown channel and instance, the marks,
edges and dense cells that instance draws on the channel, at most `2·W`), built with one window query per
store (`la_decoders::lanes`). Every annotation intersecting the window is represented exactly once; an
error is never hidden in a dense cell without its error count. The plan never reads samples and never
fails.

### `ScrollToEdge`, exactly

Let the window be `[start, start + length)` over a capture of `capture_length` samples; an edge at
position `p` is the change between `p - 1` and `p`.

- `Next`: the edge is the first real transition of the channel at a position `>= start + length`; the
  new start is `edge - length / 4`.
- `Previous`: the edge is the last real transition at a position `< start`; the new start is
  `edge - (3 * length) / 4` (integer division).
- The start is clamped to `[0, capture_length - length]` (0 when the window is longer than the
  capture); the length never changes; following the newest data is turned off.
- No capture, an unknown or hidden channel, or no edge on that side: `Ignored(host.invalid_command)`
  and nothing changes.

## Seconds and samples

`la_host::config::samples_for_seconds(rate_hz, text)` turns what the user typed as the length of a
capture into a number of samples, and `seconds_for_samples(rate_hz, samples)` writes the length back.
Both are exact integer arithmetic: the text is a decimal number with at most nine digits after the
point (no sign, no exponent, no separator but the point), the result is rounded half up, and a text
with less than half a sample, more than `u64::MAX` samples or a zero rate is refused with a
`SecondsError`. The configuration of a capture still counts samples; seconds are how it is entered.

## Storage queries

`Snapshot::next_transition(channel, from)` is the first transition with a position greater than
`from`, and `Snapshot::previous_transition(channel, before)` the last one with a position smaller than
`before`; both return `Option<Edge>` in O(log N) and `storage.channel_out_of_range` for a channel that
does not exist.

## Decoders

`State.decoders` holds the catalogue of registered decoders in the current language (name, description,
signals, options with their types and defaults), the instances (setup, state `Waiting`, `Running`,
`Finished`, `Cancelled` or `Failed`, whether the setup is complete, progress, the flags `incomplete`,
`outdated` and `loaded`, the stable code of the last fault, and the counts of annotations and refused
records), the reason the last command was refused (`refusal`: code and the setting), the selected
annotation with its details, and the limits in force. `Versions.decoders` and the change flag
`DECODERS` tell a shell what to redraw; results growing during a pass bump them. A pass runs on a
supervised worker and never blocks `apply` or `poll`; closing the capture, opening a file or starting a
capture removes every instance. The lanes come in stack order (an instance after the one it reads from). The instances stacked on one that runs again start again after it and show their old results, flagged `outdated`, until the new ones arrive; a fault below makes the results above incomplete without failing them. `Host::decoder_sources(id)` lists what an instance could read from.

At most half of the cores (at least one) decode at once; the other instances are `Waiting` and take their
turn. A pass yields after every chunk and follows a capture that is still growing, finishing only when
the capture is closed. A change of the setup that alters results starts a new pass and keeps the old
results, flagged `outdated`, until the new pass publishes its first results, which replace them at
once. The decoder side of the contract is `docs/decoder-api.md`.

## The `DECO` and `DANN` sections of a session file

Saving writes `DECO` (the setup of every instance, in stack order) when there are instances, and `DANN`
(the annotations and items) only when `SetSaveDecoderResults` is on and results exist; the writer emits
format minor 2. The payloads are defined by `specs/007-decoder-framework-annotations/contracts/deco-section.md`
(canonical varints, every value validated, the same state giving the same bytes). Opening restores the
instances and starts their passes again, or, when the file has valid results, shows them at once flagged
`loaded` without starting a pass until the user runs one or changes the setup. A damaged section never
fails the open: the capture opens, a warning with `decoder.section_invalid` is shown and the section is
dropped (a damaged `DANN` keeps the setup, and the decoders run again). A record of a decoder this
version does not know, or an option it does not know, is kept as it was read and written back byte for
byte (`decoder.unknown_decoder`, `decoder.options_skipped`); a section of a newer version is kept
untouched with an information notice. `Unsaved` becomes `Dirty` when a setup changes, not when a pass
finishes.

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

## The `LAYT` section of a session file

The heights of the channels the user set are stored in the optional `LAYT` section
(`specs/014-gui-improvements/contracts/layt-section.md`): a version, a count and (channel, height)
pairs. It is written only when at least one height differs from automatic, so a file of an earlier
version stays byte-compatible. A reader that does not know the section keeps it as an extra and
writes it back unchanged; an invalid payload opens the capture with automatic heights and a
`host.layout_invalid` warning, a newer one is kept untouched with a notice.

## Recent files

The list holds at most 15 paths, newest first, in the settings. `recent_files()` reports each with
whether the file exists, so a shell can show a missing one disabled; the host never drops an entry
because the file is gone. `RemoveRecentFile(path)` removes one entry (the file stays; an entry that is
not listed is ignored) and `ClearRecentFiles` empties the list; both save the settings.

## Logging

The host logs, through `tracing`: each command at `info` (its name, never signal data or user
text), capture and operation lifecycle, and every failure with its code and technical detail at
`warn` or `error`. Panics are logged with the command name. Neither the log nor a notice contains
signal data, channel names or marker names.

## Compatibility

Version 1.0 is the first; 1.1 adds the decoder commands, state and plan parts; 1.2 adds the channel
height, the side panel, the scroll to an edge and the recent files commands, the `LAYT` section and
the edge flags of the plan. Additions of commands or `State` fields that existing shells can ignore
are minor versions. Removing or changing the meaning of a command, a state field or an invariant is
a major version and requires every shell in the repository to be updated in the same merge.
