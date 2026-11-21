# Feature Specification: UI Shell and Signal View

**Feature Branch**: `006-ui-shell-signal-view`

**Created**: 2026-11-14

**Status**: Draft

**Input**: User description: "006 (roadmap item 006, `006-ui-shell-signal-view`): UI toolkit choice, window shell, signal view (zoom, pan, markers), capture control, CJK and long German layout, user-facing error handling."

## Overview

Until now the product is a set of headless libraries: signal storage, the session file format, the
device abstraction with its virtual device, and the localization foundation. This feature gives them
a face. It delivers the first runnable application: a window in which the user picks a device,
captures signals, looks at them, measures them, saves and reopens them, and does all of it in one of
six languages, with every failure explained in plain words and no failure ever taking the
application down.

It is also the point where three open decisions of the constitution close: the choice of the UI
toolkit (made in the plan, with fast signal rendering in mind), the first use of the localization
foundation by a real interface, and the location of the host API contract through which the UI talks
to the core.

The "users" are the person who analyzes digital signals (a hardware developer or hobbyist who must
be able to trust the tool with a rare, unrepeatable capture) and the developers of later features
(decoders, export, drivers), who add their panels and commands to this shell. Protocol decoding,
annotations, decoded-data export and hardware drivers are separate roadmap items; this shell must be
ready to receive them but does not contain them.

Terms used in this document:

- **Shell**: the application window with its menu, toolbar, device and capture controls, status
  area, dialogs and settings.
- **Signal view**: the central area that draws the channels of a capture as digital waveforms
  against a time axis.
- **Capture**: the signal data of one acquisition (or of one opened file) shown in the view.
- **Visible window**: the span of time currently shown; **zoom** changes its length, **pan** moves
  it.
- **Marker**: a dotted vertical line the user places at a point in time. There are two kinds, a
  single marker and a pair marker (an interval); what they are and do is defined by feature 015
  (`015-markers-rework`), which replaced the one kind this feature built.
- **Dense activity**: a pixel column in which more than one transition falls; it must look different
  from an empty or single-transition column so that glitches are never hidden.
- **Simulated**: data produced by the virtual device; it is labelled as such everywhere it is shown
  or saved.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Capture signals and watch them appear (Priority: P1)

The user starts the application, sees the list of available devices (always including the virtual
device), selects one, adjusts its settings (channels, sample rate, capture length, an optional
trigger, and for the virtual device a seed), and presses Start. The signals appear in the signal
view while the capture runs, a progress area shows elapsed time and samples captured, and Stop ends
the capture at any moment. If the device fails or is unplugged, everything captured up to that
point stays on screen and the user is told what happened and what to do.

**Why this priority**: this is the product. Without a way to capture and look at signals nothing
else in the application can be tried, and the virtual device lets this be shown without hardware.

**Independent Test**: start the application with no hardware attached, choose the virtual device,
start a capture with a fixed seed and a known length, watch it finish, and compare the displayed
waveform and sample count with the expected data of that seed; repeat with a scripted device failure
in the middle and check that the data captured before the failure is still displayed.

**Acceptance Scenarios**:

1. **Given** the application has just started, **When** the user opens the device list, **Then** the
   virtual device is listed and selectable, marked as simulated, and the three supported hardware
   devices appear when attached.
2. **Given** a selected device with a valid configuration, **When** the user presses Start, **Then**
   the capture begins, the Start control becomes Stop, the signals grow in the view while the capture
   runs, and the interface stays responsive.
3. **Given** a running capture, **When** the user presses Stop, **Then** the capture ends within one
   second, everything captured so far is kept and shown, and the end reason ("stopped by you") is
   visible.
4. **Given** a configuration the device cannot satisfy (a rate or trigger it does not offer), **When**
   the user tries to start, **Then** the controls prevent or reject it before anything reaches the
   device and say which setting is wrong and what the device offers.
5. **Given** a running capture whose device disconnects or fails, **When** the failure happens,
   **Then** the capture ends, the data captured up to then stays in the view, and a message names
   what failed and what the user can do.
6. **Given** the capture reaches its length or the memory limit, **When** it ends, **Then** the end
   reason is shown ("length reached", "memory limit reached") and saving is offered.
7. **Given** data from the virtual device is on screen, **When** the user looks at the window, **Then**
   a visible label says it is simulated.

---

### User Story 2 - Explore a large capture by zooming and panning (Priority: P1)

The user zooms in on a few nanoseconds and zooms out to the whole capture, pans along the timeline,
and jumps to the start or end or back to "fit everything". Channels have names the user can edit,
can be reordered, hidden and shown, and have their own colours. Zoomed out, a pixel that contains
many transitions is drawn so the user can see that activity is hiding there, and zooming into it
reveals every transition, including a one-sample glitch. Large captures stay smooth.

**Why this priority**: exploring a capture is the second half of the product, and the constitution
makes rendering speed on large captures a requirement of the design (Principle VIII).

**Independent Test**: generate a capture of one hundred million samples on 16 channels with the
virtual device (with a one-sample glitch planted in a long quiet stretch), zoom and pan by script and by hand, measure the time to produce each frame, and check that
the glitch is visible at every zoom level as either a transition or a dense-activity mark.

**Acceptance Scenarios**:

1. **Given** a capture on screen, **When** the user zooms with the wheel, the keyboard or a
   control, **Then** the time under the pointer (or the view centre for keyboard zoom) stays where it
   is and the time axis re-labels itself with sensible units and round tick marks.
2. **Given** any zoom level, **When** the user pans by dragging, the wheel with a modifier, the
   keyboard or a scroll bar, **Then** the view moves smoothly and cannot move beyond the data (a
   small margin is allowed) or lose track of it.
3. **Given** a zoomed-out view where a pixel column holds several transitions, **When** it is
   drawn, **Then** the column is visibly different from a quiet column and from a column holding a
   single transition, and zooming into it shows each transition separately.
4. **Given** a capture with a gap (samples lost during acquisition), **When** the view shows the gap,
   **Then** it is drawn as a gap and never as flat signal, and the capture is flagged as incomplete.
5. **Given** a capture of one hundred million samples, **When** the user zooms or pans continuously,
   **Then** every frame is produced within the time budget in the success criteria, independently of
   the capture length.
6. **Given** the user edits a channel name, order, visibility or colour, **When** the capture is
   saved and reopened, **Then** those choices come back (see User Story 4).
7. **Given** the view is zoomed in as far as one sample being many pixels wide, **When** the user
   keeps zooming, **Then** zoom stops at a sensible limit rather than overflowing or going blank.

---

### User Story 3 - Measure time with markers (Priority: P2)

*Superseded by feature 015 (`015-markers-rework`).* Feature 006 built one kind of marker, two of which
could be chosen for a measurement. Feature 015 replaced it with single markers and pair markers; the
user stories, scenarios and requirements of markers (placing, dragging, snapping, naming, removing,
going to a marker, measuring) are defined there. What this feature still owns is the signal view
that draws the markers and the session file section that saves them (FR-018).

---

### User Story 4 - Save, open and never lose a capture (Priority: P2)

The user saves the capture to a session file, opens a session file later, and is never asked to
choose between losing data and doing what they wanted. Saving and opening show progress and can be
cancelled; a failed save leaves the previous file intact; closing the window or starting a new
capture while there are unsaved data first asks what to do; and a damaged file is refused with a
clear message instead of a crash. Channel names, order, visibility and colours, the capture
description, and the "simulated" flag travel with the file.

**Why this priority**: the constitution makes not losing data the highest goal of the product; the
session format exists (feature 003) and this story is the only place a user can use it. It comes
after the view because a capture must be displayed before it is worth saving.

**Independent Test**: capture from the virtual device, save, close, reopen the file, and compare the
displayed waveform, names, colours and flags with what was saved; then corrupt the file and check that
opening reports a clear error and leaves the application usable; then try to close with unsaved data
and check the prompt.

**Acceptance Scenarios**:

1. **Given** an unsaved capture, **When** the user chooses Save, **Then** a file location is
   requested, saving runs in the background with visible progress and a Cancel control, and on
   completion the capture is marked as saved.
2. **Given** a saved session file, **When** the user opens it, **Then** the capture is shown exactly
   as saved, including channel names, order, visibility and colours, the description and the
   simulated label.
3. **Given** a save that fails (disk full, no permission, cancelled), **When** it ends, **Then** the
   previous file is untouched, the capture stays open and unsaved, and a message says what failed and
   what to do.
4. **Given** a damaged, truncated, too large or foreign file, **When** the user opens it, **Then** it
   is refused with a localized message that names the problem and the application and the capture
   already open are unaffected.
5. **Given** a capture that is unsaved (or still running), **When** the user closes the window, opens
   a file, or starts a new capture, **Then** the application asks whether to save, discard or cancel
   the action, and a running capture is stopped first with its data kept.
6. **Given** a file written by a newer version of the application that this version cannot read,
   **When** it is opened, **Then** the message says that a newer version is needed.
7. **Given** a capture that is simulated, **When** it is saved and reopened, **Then** the simulated
   label is still shown.

---

### User Story 5 - Use the application in my own language (Priority: P2)

The user chooses English, German, French, Spanish, Japanese or Ukrainian in the settings, and the
whole interface changes at once, without restarting and without regard to the system language.
Japanese text is rendered correctly, long German words fit without being cut off or overlapping
other controls, numbers, units and dates follow the language, and text the user or a device supplied
(channel names, descriptions) is shown as it is.

**Why this priority**: required by the constitution for every feature; ranked below the core flows
only because the foundation (feature 005) already exists and the story is mostly about using it
correctly.

**Independent Test**: switch through all six languages with a capture open and check every visible
text; run the layout check with the longest German strings and with Japanese at the smallest allowed
window size and look for clipping, overlap and missing glyphs; compare number and unit formats with
the expected ones.

**Acceptance Scenarios**:

1. **Given** the application is running, **When** the user selects another language in the
   settings, **Then** every menu, label, tooltip, dialog and message is shown in that language
   immediately, without a restart, and the choice is remembered for the next start.
2. **Given** the first start, **When** the application chooses the language, **Then** it uses the
   system language if it is one of the six and English otherwise, and the user's explicit choice
   always wins afterwards.
3. **Given** Japanese is selected, **When** any text is shown (including channel names typed by the
   user in Japanese), **Then** all characters are drawn with a suitable font and none appears as a
   missing-glyph box.
4. **Given** German is selected at the smallest supported window size, **When** every screen and
   dialog is shown, **Then** no text is cut off, overlaps another element or forces a control out of
   reach; long text wraps or the control grows, and where space truly runs out the full text is
   available on hover or focus.
5. **Given** a capture with channel names, a description and a pair marker's duration, **When** the
   language changes, **Then** the names and description stay exactly as the user typed them while
   units, numbers and surrounding text change.
6. **Given** an interface text for which a translation is missing, **When** it is shown, **Then** the
   English text appears and the gap is logged for developers; a raw key is never shown.

---

### User Story 6 - Failures are explained and never stop the application (Priority: P1)

Whatever goes wrong — a device error, a damaged file, a full disk, an internal fault in a
background task — the user sees a message in their language that says what failed and what they can
do, the data on screen stays intact, the application keeps responding, and the technical detail goes
to the log rather than to the dialog. The message is the localized text of the error's stable code
(feature 005), with an option to open the log location for the details.

**Why this priority**: the product's first promise is that it does not crash and does not lose data;
this story is how the promise reaches the user. It is P1 because every other story depends on it.

**Independent Test**: inject every error code the product defines (device, storage, session) and a
panic in a background task, one at a time, while a capture is on screen, and check that each produces
one localized message, no crash, no loss of the displayed data, and a log entry with the code.

**Acceptance Scenarios**:

1. **Given** any stable error code, **When** the operation behind it fails, **Then** the user sees
   the localized message for that code, not the code, not the technical detail, and not a blank.
2. **Given** a background operation that panics or stops unexpectedly, **When** the failure is
   detected, **Then** the application reports it, the capture on screen and the unsaved-data
   protection are intact, and the user can continue working.
3. **Given** several failures in quick succession, **When** they occur, **Then** the messages are
   shown without flooding the user (repeats of the same message are grouped) and each is available in
   a list of recent messages.
4. **Given** the user wants the details for a bug report, **When** they follow the hint in the
   message, **Then** the location of the log file is shown, and the log contains the code, the
   technical detail and the time, but never signal data.
5. **Given** a failure in the drawing of the view itself, **When** it happens, **Then** the view shows
   a clear "cannot draw" state for that region, the rest of the application stays usable, and saving
   still works.

---

### Edge Cases

- **No devices and no virtual device selected**: the Start control is disabled with a visible reason;
  the virtual device is always present, so the list is never empty.
- **Device removed while the device list is open or while editing its settings**: the list refreshes,
  the selection moves to a sensible default and the user is told.
- **Capture of zero samples** (stopped immediately, or a trigger that never fires): shown as an empty
  capture with the end reason; saving it is allowed; the view does not divide by zero or show garbage.
- **Capture of a single sample, or all channels constant**: drawn as flat lines with a correct time
  axis.
- **Maximum channels (32) and a single channel**: the layout adapts; many channels scroll vertically
  with the time axis and channel names kept visible.
- **Window resized very small, maximized, moved to another monitor with a different scale factor**:
  the layout stays usable at the minimum size, redraws sharply after a scale change and keeps the time
  under the view centre.
- **Very long or hostile channel names** (hundreds of characters, right-to-left text, control
  characters, emoji): shown safely, truncated visually with the full text on hover, never breaking the
  layout or the bidirectional order of the surrounding text.
- **Rapid user input** (holding the zoom key, spinning the wheel, resizing while a capture runs, mashing
  Start and Stop): input is coalesced, state stays consistent, and the capture never starts twice.
- **Unsaved capture and the memory limit**: when the limit stops a capture the data stays and saving is
  offered; the user is never left with an unsavable capture because of the limit.
- **Opening a file while a capture runs**: the user is asked first; the capture is stopped and kept
  unless the user cancels.
- **Same file opened twice or a file replaced on disk while open**: the open capture is independent of
  the file after loading; saving to the same path replaces it atomically.
- **Settings file missing, unreadable or corrupt**: defaults are used, the user is told once, and the
  application starts.
- **System clock changes or a time zone with a half-hour offset**: displayed absolute dates use the
  offset in force and never move the signal timeline, which is relative to the capture start.
- **High contrast or dark system theme**: the view and the controls stay legible; the choice of
  colours never relies on a colour difference alone to show a signal state.
- **Capture started on a device in a locked or busy state**: reported with the device-busy message and
  the controls return to Start-ready.
- **A second instance of the application**: allowed; each has its own capture, and saving never lets
  one instance corrupt the other's file.

## Requirements *(mandatory)*

### Functional Requirements

**Shell and device control**

- **FR-001**: The application MUST open a main window on both supported operating systems (Windows
  10/11 and Debian 13) with a menu, a toolbar for the frequent actions, the signal view, a status
  area and a place for messages; the same functions MUST be available on both.
- **FR-002**: The application MUST list the available capture devices through the device abstraction
  only, always including the virtual device, update the list as devices come and go, and show for each
  its name, kind and whether it is simulated; the UI MUST NOT contain device-specific logic.
- **FR-003**: The user MUST be able to edit the configuration of the selected device within what its
  declared capabilities allow (enabled channels, sample rate, capture length, trigger, and for the
  virtual device its seed and signal profile); an invalid choice MUST be prevented or rejected before
  anything reaches the device, with a message that says which setting is wrong and what is allowed.
- **FR-004**: The user MUST be able to start a capture, see its progress (elapsed time, samples
  captured, memory in use against the limit), and stop it at any moment; the capture MUST run in the
  background so the interface never blocks, and Stop MUST take effect within one second.
- **FR-005**: The application MUST show the end reason of every capture (completed, stopped by the
  user, memory limit, device disconnected, device error, samples lost) and MUST keep all data captured
  up to the end in every case.
- **FR-006**: Data from the virtual device MUST be labelled as simulated wherever it is displayed and
  in every file the application saves; the label MUST NOT be removable by the user.

**Signal view**

- **FR-007**: The signal view MUST draw every visible channel of the capture as a digital waveform on a
  shared time axis, with channel names, a legible time axis with units and round ticks, and the
  time under the pointer shown while the pointer is over the view.
- **FR-008**: The user MUST be able to zoom (pointer-anchored for the mouse, centre-anchored for the
  keyboard and controls) and pan (drag, wheel, keyboard, scroll bar), jump to the start, the end and
  "fit all", and zoom to a selected range; every action MUST also be reachable from the keyboard.
- **FR-009**: Rendering the visible part of a capture MUST obey the cost bound of constitution
  Principle VIII — it MUST NOT depend on the length of the capture or on the number of stored
  transitions outside the window — and MUST use the storage's indexed window lookup and its
  multi-resolution summaries rather than reading samples.
- **FR-010**: A pixel column that holds more than one transition MUST be drawn differently from an
  empty column and from a single-transition column (dense-activity indication); zooming in MUST reveal
  every transition, including a single-sample glitch; no information may be dropped silently at any
  zoom level.
- **FR-011**: A gap in a capture (lost samples) MUST be drawn as a gap, and a capture that contains a
  gap or ended abnormally MUST be shown as incomplete; it MUST never be presented as a clean
  measurement.
- **FR-012**: A capture that is still running MUST be shown live: the view MAY follow the newest data
  when the user asks for it, MUST NOT jump while the user is exploring, and MUST NOT make capture
  slower or lose samples.
- **FR-013**: The user MUST be able to rename, reorder, hide and show channels and to choose their
  colours; the application MUST provide a default palette that distinguishes channels without relying
  on colour alone (for example by lane position and labels) and stays legible on light, dark and high
  contrast themes.
- **FR-014**: Zoom MUST be bounded at both ends (the whole capture with a small margin at one end, a
  sensible number of pixels per sample at the other) so no operation can produce an empty, overflowing
  or undefined view.
- **FR-015**: Redraws MUST be driven by changes (new data, zoom, pan, resize, scale factor change) and
  MUST NOT consume processor time while nothing changes.

**Markers and measurement**

- **FR-016** *(superseded by feature 015, FR-001 to FR-021)*: Markers of two kinds, single and pair,
  placed, dragged, snapped, coloured, commented, removed and acted upon as defined by feature 015; each
  marker MUST stay attached to its time while the user zooms and pans.
- **FR-017** *(superseded by feature 015, FR-014)*: The duration and frequency of a pair marker are
  shown as feature 015 defines.
- **FR-018** *(content amended by feature 015, FR-022 to FR-024)*: Markers MUST be saved with the
  capture in the session file, and a file written without markers MUST still open.

**Sessions**

- **FR-019**: The user MUST be able to save a capture to a session file and open one, using the
  session format's operations; both MUST run in the background with visible progress and a Cancel
  control, and a cancelled or failed save MUST leave any existing file untouched.
- **FR-020**: The session file MUST carry the channel names, order, visibility and colours, the
  markers, the capture description and the simulated flag, using only what the session format allows
  (unknown extra information in a file MUST be preserved when the application saves it back).
- **FR-021**: The application MUST protect unsaved data: closing the window, opening a file, starting
  a new capture or quitting while a capture is unsaved or running MUST first ask the user whether to
  save, discard or cancel, and a running capture MUST be stopped with its data kept before any such
  choice takes effect; the application MUST also show clearly whether the current capture has unsaved
  changes.
- **FR-022**: Opening a damaged, truncated, oversized, foreign or newer-version file MUST be refused
  with a localized message naming the problem; the application and the capture already open MUST be
  unaffected.

**Language and layout**

- **FR-023**: Every piece of user-facing text MUST come from the localization foundation: none in
  code, no key shown to the user, English fallback with the gap logged, and the six languages
  complete for every string this feature adds, drafts being acceptable but marked.
- **FR-024**: The user MUST be able to change the language in the settings with immediate effect
  everywhere and without a restart; the choice MUST be remembered; with no choice the system language
  is used if supported and English otherwise.
- **FR-025**: Japanese MUST render correctly and long German text MUST fit at the smallest supported
  window size without truncation, clipping or overlap on any screen or dialog (long text wraps, the
  control grows, or the full text is available on focus or hover); right-to-left or mixed-direction
  user text MUST not disturb the layout of its surroundings.
- **FR-026**: Text supplied by the user or a device (channel names, descriptions, marker names) MUST be
  shown as typed, never translated, and MUST be length-limited and sanitized for display so it cannot
  break the layout or the surrounding text.
- **FR-027**: Numbers, frequencies, sizes, durations and dates shown in the interface MUST follow the
  selected language through the localization foundation, while the time axis of the signal timeline is
  relative to the capture start and independent of the clock and time zone.

**Errors, stability and diagnostics**

- **FR-028**: Every user-visible failure MUST be shown as the localized message of its stable error
  code (what failed and what the user can do), never as the code, a raw internal error or a blank; an
  error code without a message MUST show the generic localized message and be reported as a defect to
  developers.
- **FR-029**: A failure or panic in any background task (acquisition, loading, saving, drawing) MUST be
  contained at its boundary, reported to the user and logged, MUST NOT take down the application, and
  MUST NOT discard the displayed capture or its unsaved-data protection.
- **FR-030**: Messages MUST not flood the user: repeats are grouped, the most recent are kept in a
  message list, and the user can find the log file location from the message.
- **FR-031**: The log MUST record user actions at the level of commands (open, save, start, stop,
  language change), device and capture lifecycle events, and errors with their codes and technical
  detail, with bounded size, and MUST NEVER contain signal data.
- **FR-032**: The application MUST NOT block its interface on any operation that can take longer than
  100 milliseconds, MUST NOT crash on any sequence of user actions, and MUST bound its memory use by the
  limit the user can see and change in the settings.

**Architecture and conventions**

- **FR-033**: The UI MUST talk to the core only through a documented, versioned host API contract,
  English-documented, whose location is decided in the plan; domain logic MUST stay in UI-free crates,
  and everything the shell does MUST be testable without a display (commands and state as plain data).
- **FR-034**: The UI toolkit MUST be chosen in the plan against these requirements (speed of custom
  drawing of signals, text shaping for CJK and bidirectional text, accessibility basics, licence
  compatible with GPL-3.0, maintenance status, and parity on Windows and Debian), the choice recorded
  with the alternatives considered.
- **FR-035**: The visual design MUST be original (constitution Principle XII): no Saleae or PulseView
  code, assets, icons, names, colours or layouts are used; ideas are re-implemented independently.
- **FR-036**: The application MUST be keyboard-operable for every command, expose labels for assistive
  technology on its standard controls to the extent the chosen toolkit supports, and scale with the
  operating system's display scale factor and text size settings.
- **FR-037**: The shell MUST provide a place where later features add panels, commands and decoder
  lanes (a decoder lane area below the channels, a side panel, menu entries) without changes to the
  shell's own code beyond registration; this feature adds no decoder.

### Key Entities *(include if feature involves data)*

- **Shell state**: the single description of what the window shows — selected device and its
  configuration, capture status, the capture, channel presentation, markers, view window, messages,
  settings, unsaved flag — held as plain data so it can be tested and saved.
- **Command**: a named user action (start capture, stop, zoom in, place marker, save, change language)
  that the shell executes; every menu entry, button and shortcut maps to exactly one command.
- **Capture view model**: the current capture plus how it is displayed (visible window, channel order,
  names, colours, visibility, follow-latest on or off).
- **Channel presentation**: per channel, the user's name, colour, order position and visibility.
- **Marker** *(superseded by feature 015)*: a single marker (one time position) or a pair marker (two,
  yielding a duration and a frequency), each with a number, a colour and a comment.
- **Message**: a notice for the user made of a stable code or key, its arguments and its severity;
  the shell groups repeats and keeps the recent ones.
- **Settings**: the user's persistent choices — language, memory limit, theme, window geometry, recent
  files — stored in a small file in the user's profile, with defaults if it is missing or damaged.
- **Host API contract**: the versioned description of the commands, queries and events between the UI
  and the core, written in English.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: A user with no hardware starts the application, captures from the virtual device and sees
  the waveform in under 30 seconds from launch, without reading any documentation, in each of the six
  languages.
- **SC-002**: On a capture of one hundred million samples over 16 channels, every frame during
  continuous zoom and pan is produced in under 16 ms at the 95th percentile and under 50 ms at the
  worst case, and the frame time for the same view of a ten times longer capture differs by less than
  a factor of two (it does not grow with capture length).
- **SC-003**: In an automated check over a planted one-sample glitch, 100 % of zoom levels from the
  whole capture to one sample per many pixels show the glitch either as a transition or as a
  dense-activity mark.
- **SC-004**: The interface never stops responding for more than 100 ms during capture, save, open,
  zoom, pan and language change on a capture of one hundred million samples, measured by a
  responsiveness check run on both supported operating systems.
- **SC-005**: A randomized sequence of at least 100,000 user actions (start, stop, zoom, pan, markers,
  save, open, language change, resize, invalid input) produces no crash, no hang and no loss of the
  captured data, with scripted device failures mixed in.
- **SC-006**: Every stable error code defined in the product, injected one at a time, produces exactly
  one localized message in each of the six languages with no raw code, key or technical detail visible,
  and a log entry carrying the code.
- **SC-007**: A save followed by an open reproduces the capture exactly (every transition, gap, channel
  name, colour, order, visibility, marker, description and simulated flag), and a save interrupted at
  any point leaves the previous file readable, over at least 100 interruption points.
- **SC-008**: In the layout check with the longest German strings and Japanese text at the smallest
  supported window size, zero elements are clipped, overlap or lose their text, and zero characters
  are drawn as missing glyphs, on both operating systems.
- **SC-009**: Switching the language with a capture open takes under 200 ms to repaint the whole
  interface and leaves every user-supplied text unchanged.
- **SC-010**: With no change on screen and no capture running, the application's processor use stays
  under 1 % of one core (it does not redraw continuously).
- **SC-011**: Closing, opening a file or starting a new capture while data is unsaved asks the user in
  100 % of the scripted cases and never discards data without an explicit choice.

## Assumptions

- This is the first feature with a graphical interface, and the toolkit is chosen in the plan; the
  specification therefore avoids naming one. Fast signal drawing may use a custom canvas inside the
  toolkit, as the roadmap suggests.
- The application is a desktop program for Windows 10/11 and Debian 13 on 64-bit PCs; mobile, web and
  other operating systems are out of scope.
- Only the virtual device and (when attached later) hardware drivers are used; the three hardware
  drivers are separate roadmap items (011–013), so everything here is verified with the virtual device
  and with scripted device failures.
- Protocol decoding, annotations, decoded-data export, raw-data export and sigrok `.sr` import/export
  are out of scope (roadmap items 007–010 and 014+); the shell only reserves a place for decoder lanes
  and panels (FR-037).
- Triggers are exposed to the extent the device abstraction declares them; building new trigger
  engines is out of scope.
- Unsaved-data protection is provided by the explicit prompt on close and on destructive actions; a
  periodic or streaming autosave of running captures is not part of this feature and is recorded as a
  possible later addition.
- Themes: the application follows the system light/dark setting and offers a manual override; custom
  user themes are out of scope.
- Full screen-reader support is out of scope for this feature; keyboard operation, scale factor and
  text size support are in scope (FR-036), and assistive labels are provided as far as the chosen
  toolkit allows.
- Printing, screenshots-to-file and report generation are out of scope.
- The language foundation (feature 005), the session format (003), the device abstraction and virtual
  device (004) and the signal storage with its window queries (002) are available and used as
  documented; the shell adds no new error codes except those for its own settings file and commands,
  which come with messages in all six languages.
- Settings live in a small per-user file in the user's profile directory; the exact location and
  format are decided in the plan.
- The default memory limit and the capture lengths offered come from the existing storage and device
  limits; this feature only exposes them.
