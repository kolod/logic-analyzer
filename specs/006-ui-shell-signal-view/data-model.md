# Data Model: UI Shell and Signal View

**Feature**: [spec.md](spec.md) | **Plan**: [plan.md](plan.md)

All types below live in `la-host` unless stated. They are plain data (no toolkit types), cheap to
clone where noted, and are what the host API contract exposes.

## Shell state (`State`)

The single description of what the window shows. Replaced as a whole on commit of a command
(see "Command execution" below); the UI reads it and never mutates it.

| Field | Type | Notes |
|-------|------|-------|
| `devices` | `Vec<DeviceEntry>` | descriptor (from the registry), `simulated`, `available` |
| `selected_device` | `Option<DeviceId>` | defaults to the virtual device |
| `config` | `ConfigDraft` | what the user edits; validated through `la_devices::validate` into a `ResolvedConfig` on every change |
| `config_problems` | `Vec<ConfigProblem>` | per setting, a stable error code plus the allowed values; empty means Start is enabled |
| `capture` | `Option<CaptureView>` | the capture shown (running, finished or loaded) |
| `activity` | `Activity` | `Idle`, `Capturing`, `Saving`, `Loading`; at most one at a time |
| `progress` | `Option<ProgressView>` | elapsed, samples, memory used / limit, or bytes done / total |
| `unsaved` | `Unsaved` | `Clean`, `Dirty`, `Running` (see guard below) |
| `pending_decision` | `Option<Decision>` | a deferred destructive command waiting for the user (`Save`/`Discard`/`Cancel`) |
| `window` | `ViewWindow` | visible time span |
| `follow_latest` | `bool` | live follow while capturing |
| `presentation` | `Presentation` | per-channel name, colour, order, visibility |
| `markers` | `Vec<Marker>` | at most `MAX_MARKERS` (64) |
| `snap` | `bool` | marker snapping on or off |
| `notices` | `NoticeList` | grouped recent messages, bounded |
| `settings` | `Settings` | persistent user choices |
| `language` | `Language` | mirrors `settings.language`; the localizer follows it |
| `version` | `u64` | increases on every committed change; the UI keys its caches with it |
| `version_parts` | `Versions` | separate counters for `capture`, `window`, `presentation`, `markers`, `theme` so the UI invalidates only what changed |

### `CaptureView`

| Field | Notes |
|-------|-------|
| `snapshot` | `la_storage::Snapshot` (cheap clone, immutable) |
| `description` | `la_devices::CaptureDescription` |
| `origin` | `Acquired` (this run) or `File(PathBuf)` |
| `end` | `None` while running, else `EndReason` (completed, stopped by user, memory limit, device disconnected, device error, samples lost, loaded) |
| `incomplete` | true if the snapshot has a gap or ended abnormally (FR-011) |
| `simulated` | from the description; cannot be changed (FR-006) |
| `length_samples`, `sample_rate_hz` | copied for convenient maths |

### `Unsaved` and the guard

```text
Clean --capture starts--> Running --capture ends--> Dirty --save succeeds--> Clean
Clean --capture loaded from file--> Clean
Dirty --any edit of presentation/markers--> Dirty
Clean --edit of presentation/markers--> Dirty   (cosmetic edits count; the user is asked on close)
```

Commands `NewCapture`, `StartCapture` (when a capture is on screen), `OpenSession`, `Quit` and the
window close request are *guarded*: if `unsaved != Clean`, `apply` stops a running capture (data
kept), sets `pending_decision`, and returns `Outcome::NeedsDecision`. `Command::Decide(Save |
Discard | Cancel)` completes or drops the deferred command; `Save` chains into the save operation and
runs the deferred command only after it succeeds. The UI cannot skip the guard because the guarded
command does nothing until decided.

## View window and zoom

`ViewWindow { start: u64, length: u64 }` in samples. Invariants (checked after every operation and
asserted by the action fuzzer):

- `length >= min_length(W)` where `min_length = max(1, ceil(W / 64))` (at most 64 pixels per sample);
- `length <= capture_length` (when `capture_length > 0`); an empty capture uses a nominal window of
  `max(min_length, 1)` samples;
- `start + length <= capture_length` (no arithmetic overflow; computed with `u128` or checked ops).

Operations (all total, none can fail): `zoom(factor, anchor_sample, anchor_fraction)`,
`pan_samples(delta)`, `pan_pixels(delta_px, W)`, `fit_all`, `go_to_start`, `go_to_end`,
`zoom_to(range)`, `reveal(sample)`. The anchor sample stays at the same fractional position of the
window across a zoom, so the time under the pointer does not move.

## Frame plan (`FramePlan`)

Computed from `(CaptureView.snapshot, window, W, presentation, markers)`; contains no toolkit types.

| Part | Content |
|------|---------|
| `window`, `pixels` (W) | the inputs, echoed so the UI can verify freshness |
| `channels` | one `ChannelPlan` per *visible* channel in presentation order |
| `axis` | ticks: `Vec<Tick { x: f32, major: bool, label_seconds: f64 }>` plus the unit chosen for the labels |
| `markers` | for each marker with its sample inside the window: x position, name, time |
| `measurement` | for two selected markers: distance in samples, as seconds, as hertz |
| `stats` | probes, segment count, build time (for the benchmark and the frame log) |
| `state` | `Ok`, `NoCapture`, `Empty` (zero samples), or `CannotDraw(code)` |

`ChannelPlan { channel, name, colour, segments }` where `Segment` is one of:

| Variant | Meaning |
|---------|---------|
| `Level { from, to, high }` | constant level for pixel columns `[from, to)` |
| `Edge { x, rising }` | exactly one transition, `x` the sub-pixel position from the stored edge |
| `Dense { from, to, count }` | columns holding more than one transition (`count` capped like the storage cap) |
| `Gap { from, to }` | lost samples: drawn as a gap, never as a level |
| `Beyond { from }` | columns after the end of the capture |

Rules: segments of one channel are contiguous, sorted, non-overlapping and cover `[0, W)`; adjacent
`Level` segments with the same level are merged; the number of segments is O(W) worst case and 1 for
a quiet channel (property tested).

## Channel presentation

`Presentation { channels: Vec<ChannelStyle>, version: u64 }` with `ChannelStyle { index: u8,
name: String, colour: Colour, visible: bool }` where the vector order is the display order.

- `name`: user text; sanitized for display (control characters removed, bidi controls removed and
  the text isolated like `la-i18n` does for arguments, length limit `MAX_NAME_CHARS = 64` characters,
  the full sanitized text kept). An empty name shows the default label (localized "Channel N").
- `colour`: an index into the built-in palette (16 entries, distinguishable in light, dark and high
  contrast themes) or a custom `Rgb` the user picked.
- Only channels the capture has are listed; a capture of 8 channels has 8 styles.
- Defaults: names empty, colours cycled through the palette, all visible, order by index.

## Marker

> **Superseded by feature 015** (`015-markers-rework`): markers are single or pair markers with a
> number, a colour and a comment, and there is no chosen pair for a measurement. The text below is
> what feature 006 built.

`Marker { id: u32, position: u64, name: String }` (samples; name sanitized as above, may be empty).
Positions are clamped to the capture (`position < length`, or `0` for empty captures). Ids are
unique and never reused within a session file. Measurement uses the two markers the user selects
(by default the two most recently placed).

## Message (`Notice` and `NoticeList`)

| Field | Notes |
|-------|-------|
| `id` | increasing |
| `severity` | `Info`, `Warning`, `Error` |
| `content` | `Error(code)` (rendered through the error catalogue), or `Info(Key, Args)` |
| `count` | repeats of an identical `(severity, content)` while still displayed are grouped |
| `first_at`, `last_at` | wall clock seconds, for display only |

`NoticeList` keeps at most 50 entries (oldest dropped) and shows the newest in a banner area that the
user can dismiss. The text is produced at display time from the current language, so a language
change updates existing messages. Technical detail (the `detail` of the error) is logged, never shown.

## Settings

`Settings` (persisted as `settings.json`, version 1):

| Field | Default | Validation |
|-------|---------|-----------|
| `language` | none (follow system → supported or English) | one of the six tags, else ignored |
| `theme` | `System` | `System`, `Light`, `Dark` |
| `ui_scale_percent` | 100 | clamped to 75–200 |
| `memory_limit_bytes` | the storage default | within the storage `Limits` bounds, else default |
| `snap_markers` | true | |
| `follow_latest` | true | |
| `recent_files` | empty | at most 10, existing paths only are offered |
| `window` | none | `{x, y, width, height, maximized}`; ignored if not on a visible monitor |
| `last_device`, `last_config` | none | used to prefill; validated again against the device |
| `unknown` | empty | fields this version does not know are kept when saving |

## Persisted view section (`VIEW`)

Binary layout in [contracts/view-section.md](contracts/view-section.md): channel styles, markers, the
last window and the follow flag. Maximum sizes and the "invalid means defaults plus a warning" rule
are defined there.

## Commands and events

Listed in [contracts/host-api.md](contracts/host-api.md). Each user-visible action is exactly one
`Command`; the table of commands, menu paths and shortcuts is in
[contracts/shell-layout.md](contracts/shell-layout.md).

## Command execution

1. `apply(command)` clones the parts of `State` it may change (cheap: collections are small, the
   snapshot is an `Arc`) into a working copy.
2. The command runs inside a panic boundary against the working copy and the owned handles (registry,
   acquisition, operations).
3. On success the working copy is committed, `version` and the affected `version_parts` increase,
   `Outcome` says what changed; on a typed error a `Notice` is added and nothing else changes; on a
   panic the working copy is dropped, the panic is logged with the command name, and a `host.internal`
   notice is added.

## New error codes

All have messages in all six languages (`errors.ftl`), following the rules of feature 005.

| Code | When |
|------|------|
| `host.settings_unreadable` | the settings file exists but cannot be read; defaults are used |
| `host.settings_invalid` | the settings file is not valid or has an unsupported version; defaults are used |
| `host.settings_write` | the settings file cannot be saved |
| `host.view_invalid` | the `VIEW` section of an opened file is invalid; the capture opens with default presentation |
| `host.render_failed` | the frame plan could not be built; the view shows the "cannot draw" state |
| `host.internal` | a command failed unexpectedly (panic contained) |
| `host.invalid_command` | a command is not valid in the current state (for example Stop with no capture); normally prevented by disabled controls |
