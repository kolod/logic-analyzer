# Feature Specification: GUI Improvements

**Feature Branch**: `014-gui-improvements`

**Created**: 2026-11-16

**Status**: Draft

**Input**: User description: "014-gui-improvements: rework of the interface, as listed in the roadmap (item 014): recent files as a submenu of the 15 latest; an application icon and icons for session files; a right side panel in which Decoders, Markers and the capture settings are views, one visible at a time, switched by an icon strip (like the left bar of VS Code); channels renamed, recoloured, resized and reordered directly in the legend strip of the signals, so the Channels panel goes away; capture settings (device, sample rate, length in seconds, channels to capture as buttons eight in a row, and the rest) in the new panel; a button that appears when the mouse nears the left or right end of a signal and scrolls to the next edge without changing the zoom, putting the edge a quarter of the view from the edge it came from; and the Decoders menu entry and the D key, which do nothing since a second decoder was added."

## Overview

The interface of feature 006 works, but it was built to be correct first. Using it for a while shows
where it gets in the way: the channels are edited in a panel far from the signals they describe, the
capture is set up in a toolbar that grows with every setting, two panels share the right edge and both
are always open, a long stretch of a flat signal cannot be left without zooming out, the recent files
are a flat list, the application has no icon of its own, and since the I2C decoder was added the menu
entry "Add decoder" and its shortcut do nothing.

This feature reworks those places without changing what the application does or what it stores:

- **Channels are edited where they are shown.** The strip to the left of the signals (the legend) is
  the place to rename a channel, change its colour, change the height of its signal and move it. The
  Channels panel is no longer needed and goes away.
- **One panel on the right, one view at a time.** A strip of icons switches between Capture, Decoders
  and Markers, as the bar on the left of VS Code switches between its views. The signals get the room
  the second panel used to take.
- **Capture setup in that panel.** Start (Stop while a capture runs), the device, the sample rate, the
  length in seconds, which channels to record (as buttons, eight in a row) and the other settings are
  in the Capture view; there is no toolbar any more, the status line keeps the state of the capture.
  The width of the panel is changed with a splitter, and a cog at the foot of the icon strip opens the
  Settings.
- **Quick scroll to the next edge.** When the pointer nears the left or the right end of a signal, a
  button with an arrow appears; it scrolls to the next edge of that signal without changing the zoom,
  so a long flat stretch can be crossed in one click.
- **Recent files as a submenu**, the 15 latest.
- **Icons**: one for the application and one for session files.
- **Add decoder works** from the menu and the keyboard whatever the number of decoders.

Out of scope: new analysis features, new file formats, a colour picker (a colour is chosen from the
palette as before), themes, rearranging the panels freely (docking), the installer and the registration
of the file type with the operating system (that is packaging, a later item; this feature delivers the
icons and the declarations the installer will use), keyboard navigation of the new controls beyond what
the existing ones have, and assistive-technology labels (a gap of feature 006 that stays).

## Clarifications

Changes asked for by the owner while the first implementation was being used (2026-11-16 and
2026-11-17). They were built and are described here so that the specification matches the application;
the requirements they touch are marked "amended" below.

- Q: Where do Start and Stop live? -> A: At the top of the Capture view; the toolbar is gone (FR-011).
- Q: How wide is the panel, and can the user change it? -> A: 280 px at first; a splitter at its left
  edge changes it between 220 and 480 px, kept in the settings (FR-010).
- Q: What do the channel buttons show? -> A: Eight to a row sharing the width of the panel, 28 px high,
  with the number of the channel (the name is too long for the button); **All** and **None** beside
  **Reset** set the whole recording mask (FR-013).
- Q: How is a typed length with a comma or a point read? -> A: Both separate the fraction, in every
  language, with at most nine digits after it (FR-012).
- Q: What does dragging a channel look like? -> A: As in Logic 2: the whole lane (background, bands and
  signal) is lifted with the pointer and the other lanes slide aside, smoothly, to open the place where
  it will land (FR-004).
- Q: What does a legend row show? -> A: The name as the main text and, under it, small, the number of the
  analyzer input, which stays when the row moves (FR-027).
- Q: How is a decoder shown in the panel? -> A: By its name (no editable name field), an arrow folds its
  properties, a bin removes it, and the header buttons have no tooltips (FR-009).
- Q: Can the answer to "unsaved capture: Save, Discard or Cancel" be remembered? -> A: Save and Discard
  can, with a box in the question and in the Settings; Cancel never can, and a remembered Cancel never
  stops the application from quitting or a window from closing (FR-028).
- Q: Where are the Settings? -> A: Also under a cog at the foot of the icon strip (FR-029).

Asked for on 2026-11-17 (specified here first, then built; see `verification.md`):

- Q: How wide is the Start button? -> A: The width of the panel, like the other controls of the Capture
  view (FR-030).
- Q: The status line jumps while a capture runs. -> A: The numbers in it must have a fixed width, so the
  text does not move as the digits change (FR-031).
- Q: Where is the time under the pointer shown? -> A: Attached to the pointer, below and to the right
  of the crosshair, or below and to the left when there is not enough room on the right; not in the top
  right corner, where it merges with the time axis (FR-032).
- Q: What does the pointer look like while the view is moved sideways with the mouse? -> A: A hand
  (FR-033).

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Edit a channel where it is shown (Priority: P1)

A user looks at the signals and wants to name them, give them colours, make the interesting ones taller
and put them in the order that suits the circuit. In the legend strip left of the signals: a click on
the coloured bar left of a channel's name changes its colour; a click on the name lets the user type a
new one; dragging the border between two signals changes the height of the signal above it; dragging
a signal by its legend moves it up or down among the others. There is no Channels panel to open.

**Why this priority**: the channels are what the user looks at all the time, and this is the change
that makes the screen simpler (one panel less) and the work faster (no trip to a panel and back).

**Independent Test**: with a capture on screen, rename a channel, change its colour twice, make one
signal taller and another lower, move the first to the bottom and check that everything shows at once
in the signals, the legend and, after saving and opening, in the file.

**Acceptance Scenarios**:

1. **Given** a capture on screen, **When** the user clicks the coloured bar next to "Channel 0", **Then**
   the channel takes the next colour of the palette in its bar, its name, its signal and its overlays,
   and another click takes the one after.
2. **Given** the same, **When** the user clicks the name, **Then** a text field with the name opens in
   place; Enter or a click elsewhere accepts it, Escape keeps the old name, and an empty or hostile
   name is handled by the rules the application already has for names.
3. **Given** two signals one above the other, **When** the user drags the border between them down or
   up, **Then** the upper signal grows or shrinks within a minimum and a maximum height and the rest
   moves along.
4. **Given** a list of signals, **When** the user drags a signal by its legend to another place, **Then**
   the whole lane is lifted with the pointer, the other lanes slide aside to open the place where it
   will land, the signal lands there on release, and the order is kept for the next capture and saved in
   the file.
5. **Given** a capture is running, **When** the user renames, recolours, resizes or moves a channel,
   **Then** it works (these never change what is recorded).
6. **Given** the application starts, **When** no capture exists yet, **Then** the channels of the device
   are listed in the legend strip and can be edited in the same way, and the edits stay for the first
   capture.
7. **Given** a legend row, **Then** the name is its main text and, under it in small type, the number of
   the analyzer input (`D0`, `D1`, ...); the number does not change when the row is moved.

---

### User Story 2 - One panel on the right, one view at a time (Priority: P1)

A user opens the application and sees the signals and, on the right, a single panel with a narrow strip
of icons at its edge: one for the capture settings, one for the decoders, one for the markers. Clicking
an icon shows that view; clicking the icon of the view that is already shown hides the panel and gives
its room back to the signals. Which view was shown stays for the next start.

**Why this priority**: it frees the screen and gives the settings of the capture and of the decoders and
markers one predictable place; user stories 3 and 7 depend on it.

**Independent Test**: click each icon in turn and check that exactly one view is visible each time, that
clicking the active icon hides the panel, and that the choice returns after restarting.

**Acceptance Scenarios**:

1. **Given** the application has started for the first time, **When** the window opens, **Then** the panel
   shows the Capture view.
2. **Given** a view is shown, **When** the user clicks the icon of another, **Then** that view replaces it
   and nothing of the first stays on screen.
3. **Given** a view is shown, **When** the user clicks its own icon, **Then** the panel is hidden and the
   signals use the width; a second click shows it again.
4. **Given** the user added a marker or a decoder while another view was shown, **When** they open the
   matching view, **Then** it shows the current state (nothing is lost by switching).
5. **Given** the window is made narrow, **When** the panel is shown, **Then** it keeps a usable minimum
   width and the signals shrink first.
6. **Given** the panel is shown, **When** the user drags the splitter at its left edge, **Then** the panel
   becomes narrower or wider between 220 and 480 px, and the width is kept for the next start.
7. **Given** the icon strip, **Then** a cog at its foot opens the Settings; it is not a view and does not
   hide or replace the one that is shown.

---

### User Story 3 - Set up a capture in the Capture view (Priority: P2)

A user opens the Capture view and sets everything about the next capture in one place: Start (Stop while
a capture runs), the device, the sample rate, the length **in seconds**, which channels to record, and
the other settings (for the simulated device the seed and the density of the signal; it always runs in real time). The
channels are buttons, eight in a row sharing the width of the panel, each in the colour of its channel:
a channel that is off shows its number in the channel's colour inside a border of that colour; a channel
that is on is filled with the channel's colour. **All** and **None** beside **Reset** switch every
channel at once. A setting the
device cannot do is explained next to it, and Start stays unavailable until it is fixed.

**Why this priority**: it removes the toolbar's growth and gives the capture settings room to be
explained; it is P2 because capturing already works.

**Independent Test**: set the length to 2.5 seconds at 1 MHz and check that the capture is 2,500,000
samples long; turn channels on and off with the buttons and check which are recorded.

**Acceptance Scenarios**:

1. **Given** a sample rate of 1 MHz, **When** the user types 2.5 as the length in seconds, **Then** the
   view shows the number of samples it makes (2,500,000), and the capture has that length.
2. **Given** a length that the device or the memory limit does not allow, **When** it is entered, **Then**
   the reason is shown next to the field and Start stays unavailable.
3. **Given** a device with 16 channels, **When** the view is open, **Then** the channels are two rows of
   eight buttons; with 32 channels, four rows.
4. **Given** a channel button, **When** it is off, **Then** its number and border have the colour of the
   channel and its inside has the colour of the panel; **When** the user clicks it, **Then** the border
   and the inside take the colour of the channel (the text takes a colour that can be read on it) and
   the channel is recorded in the next capture.
5. **Given** a capture is running, **When** the user looks at the channel buttons, **Then** they are
   locked, as the flags were, because what that capture records cannot change.
6. **Given** no channel is on, **Then** Start stays unavailable and the view says why.
7. **Given** the user changes the sample rate with a length in seconds set, **Then** the length in
   seconds is kept and the number of samples follows (and the memory estimate with it).
8. **Given** the channel buttons, **When** the user presses All or None, **Then** every channel is on or
   off at once (inert while a capture runs).
9. **Given** the length field, **When** the user types `2,5` or `2.5`, **Then** both mean two and a half
   seconds, whatever the language.

---

### User Story 4 - Quick scroll to the next edge (Priority: P2)

A user has zoomed in on a fast signal and the interesting change is far off to the right in a flat stretch.
They move the pointer to the right end of that signal: a small button with an arrow appears. One click
scrolls the view so that the next edge of that signal is a quarter of the view from the left side (so
that three quarters of what follows are in view); the zoom does not change. At the left end the button
scrolls back to the previous edge, which is put a quarter of the view from the right side.

**Why this priority**: it removes the zoom out, find, zoom in routine that long flat stretches force, and
it is a favourite of every logic analyzer user; P2 because the existing pan keys still work.

**Independent Test**: on a capture with a long flat stretch, click the button and check that the next
edge of the chosen signal is at a quarter of the view from the left and the scale is the same; do the
same to the left.

**Acceptance Scenarios**:

1. **Given** a signal with edges only at positions 1,000 and 90,000 and a view of 400 samples at 0,
   **When** the pointer nears the right end of the signal and the user clicks the arrow, **Then** the
   view moves so that the edge at 1,000 is at 100 samples from its left side, and the zoom is the same.
2. **Given** the view shows the edge at 1,000 at a quarter, **When** the user clicks again, **Then** the
   next edge (90,000) is put a quarter from the left.
3. **Given** the pointer near the left end, **When** the user clicks the arrow, **Then** the previous edge
   is put a quarter of the view from the right side.
4. **Given** the signal has no edge beyond the view on that side, **Then** the button is not offered on
   that side (or is shown disabled).
5. **Given** the pointer is away from both ends of the signal, **Then** no button is shown; it appears
   and disappears without moving anything.
6. **Given** a stretch with many edges close together (a busy or a hatched region at wide zoom),
   **When** the user clicks, **Then** the view goes to the first edge beyond the visible area and nothing
   is skipped without being shown on the way.
7. **Given** a gap in the capture (lost samples), **Then** the button treats the gap as it is drawn: it
   is not an edge, and the button goes on to the next edge after it.
8. **Given** a capture is running with "Follow latest data" on, **When** the user uses the button,
   **Then** following stops, as it does when the view is moved by hand.

---

### User Story 5 - Recent files in a submenu (Priority: P3)

A user opens the File menu and finds **Open recent** as a submenu holding the 15 files opened or saved
most recently, newest first, each by its file name with the folder as a hint; choosing one opens it. A
file that no longer exists is shown as unavailable and can be removed from the list.

**Why this priority**: a convenience; the list exists already, as a flat one.

**Independent Test**: open or save 16 different files and check that the submenu holds the last 15,
newest first, and that choosing one opens it.

**Acceptance Scenarios**:

1. **Given** 16 files were used, **When** the user opens File and Open recent, **Then** 15 entries show,
   the newest on top.
2. **Given** an entry, **When** the user chooses it, **Then** the file opens with the question about
   unsaved data first if there is any (the rules of feature 006).
3. **Given** a listed file was deleted, **When** the submenu opens, **Then** its entry is marked
   unavailable and cannot be chosen; a command removes it (and a command clears the list).
4. **Given** no file was used yet, **Then** the submenu shows that it is empty.

---

### User Story 6 - Icons (Priority: P3)

The window, the task bar and the application file show an icon of the application; session files show
an icon of their own in the file manager. Both are original artwork (constitution Principle XII).

**Why this priority**: identity and polish; nothing depends on it.

**Independent Test**: build the application and check that the window shows the icon, and that the
repository holds the file-type icon and the declarations (a desktop entry and a MIME type for Debian,
the icon resource for Windows) that the installer feature will use.

**Acceptance Scenarios**:

1. **Given** the application is started on Windows or Debian, **Then** the window and the task bar show the
   application icon, which stays legible at 16, 32 and 256 pixels.
2. **Given** the source tree, **Then** it holds the application icon and the session-file icon in the
   formats each system needs, and a desktop entry and a MIME type declaration for the session format.
3. **Given** the icon files, **Then** they are original works (no Saleae, PulseView or other brand's
   artwork), recorded with their origin in the documentation.

---

### User Story 7 - Add a decoder from the menu and the keyboard (Priority: P2)

A user chooses **Decoders → Add decoder** or presses **D** and is offered the decoders that exist, each by
name; choosing one adds it. This works whatever the number of decoders (today it does nothing since the
second one was added).

**Why this priority**: a defect introduced by feature 008 that the user can run into at once.

**Independent Test**: with the two built-in decoders, choose each from the menu and check that it is
added; press D and do the same.

**Acceptance Scenarios**:

1. **Given** a capture on screen, **When** the user opens Decoders → Add decoder, **Then** every decoder is
   listed by name and choosing one adds it with nothing assigned yet.
2. **Given** the same, **When** the user presses D, **Then** the same choice is offered (a list that is
   opened, or the Decoders view with its list open).
3. **Given** a decoder was registered by a test or later feature, **Then** it appears in the choice with no
   change to the interface (the promise of feature 007).
4. **Given** no capture on screen, **Then** the entry is unavailable and says why.

---

### User Story 8 - A steadier Capture view, status line and signal view (Priority: P3)

Four small defects of the first implementation, found by using it: the Start button is narrower than the
panel it sits in; the status line shakes while a capture runs because the digits of a proportional font
have different widths; the time under the pointer is drawn in the top right corner of the signal view,
where it merges with the time axis and is far from what the user is looking at; and nothing tells the
user that dragging moves the view.

**Why this priority**: none of it blocks a task; each one makes the application feel unfinished.

**Independent Test**: start a capture of the simulated device in real time and watch the status line; move
the pointer over the signals near the right and the bottom edges and read the time; press and drag the
view sideways.

**Acceptance Scenarios**:

1. **Given** the Capture view, **Then** the Start button (Stop while a capture runs) is as wide as the
   panel's content, as the device chooser and the fields below it are, at every width of the panel.
2. **Given** a capture is running, **When** the numbers in the status line change (samples, time, memory),
   **Then** the text does not move sideways and the parts after the numbers stay where they are.
3. **Given** the pointer is over the signals, **Then** the time of the sample under it is shown next to
   the pointer, below and to the right of the crosshair; **When** there is not enough room to the right
   of the pointer, **Then** it is shown below and to the left of the crosshair instead; it is not drawn
   over the time axis.
4. **Given** the pointer is not over the signals, **Then** no time is shown.
5. **Given** the user presses a mouse button in the signal area and moves the pointer sideways to move
   the view, **Then** the pointer is a hand while the button is held and is the crosshair again when it
   is released.

---

### Edge Cases

- What happens when a name typed in the legend is empty, too long, or contains control or invisible
  characters? The rules of feature 006 apply (the text is sanitised and limited; empty gives back the
  default name).
- What happens when the user drags a signal and releases it where it came from? Nothing changes and
  nothing is recorded as a change.
- What happens when a signal is dragged near the edge of the visible area with more channels than fit?
  The list scrolls while dragging.
- What happens when the height of a signal is made smaller than its text? There is a minimum height at
  which the name still fits; the border cannot be dragged below it.
- What happens at 32 channels with the Capture view open? The channel buttons are four rows and the view
  scrolls if the panel is too short.
- What happens when the language is changed? The panel, the views, the menu and the buttons follow it at
  once, and long German or Japanese texts do not clip (the rules of the constitution).
- What happens when the pointer is near the bottom of the signal view, so there is no room for the time
  below the crosshair? The time is drawn above it (the specification adds this; the request named only
  the right and the left side).
- What happens to a decoder lane or an overlay when the channels are reordered or resized? They follow
  their channel.
- What happens when the length in seconds does not make a whole number of samples? It is rounded to the
  nearest sample and the number of samples is shown.
- What happens when the window is very small? The panel and the strip keep their minimum sizes and the
  signals shrink first; below the smallest usable size the window does not shrink further.
- What happens to the stored layout of an older session file or settings file? It opens as before: nothing
  of this feature is required in a file, and what is new (the panel's view, the heights) is optional.

## Requirements *(mandatory)*

### Functional Requirements

**Channels in the legend strip**

- **FR-001**: A click on the coloured bar next to a channel's name MUST change the channel to the next
  colour of the palette; the colour MUST apply to the bar, the name, the signal and the marks drawn on
  the channel.
- **FR-002**: A click on a channel's name MUST open an editor in place; accepting it renames the channel
  under the rules of feature 006 for names; cancelling keeps the old name.
- **FR-003**: Dragging the border between two signals MUST change the height of the signal above it
  between a minimum and a maximum, and the height MUST be kept for the next capture and saved with the
  session.
- **FR-004** *(amended)*: Dragging a signal by its legend MUST move it to another place in the order: the
  whole lane (background, bands and signal) is lifted with the pointer and the other lanes slide aside,
  smoothly, to open the place where it will land; the order MUST be kept for the next capture and saved
  with the session.
- **FR-005**: The Channels panel MUST be removed, and everything it did MUST be possible in the legend
  strip and the Capture view: names, colours, order, heights (new), the flags (in the Capture view), and
  the reset of the defaults (the button **Reset** of the Capture view).
- **FR-006**: Renaming, recolouring, resizing and moving MUST work at any time, with or without a capture
  and while one runs, and MUST NOT change what is recorded.

**The right panel**

- **FR-007** *(amended)*: The right side MUST be one panel with a strip of icons, one for each of the views
  Capture, Decoders and Markers; exactly one view MUST be visible at a time, or none when the panel is
  hidden. A cog at the foot of the strip MUST open the Settings (it is not a view; see FR-029).
- **FR-008**: A click on the icon of the view that is shown MUST hide the panel; a click on another icon
  MUST show that view. The state of the panel (hidden or the view shown) MUST be kept for the next
  start.
- **FR-009** *(amended)*: The content of the Decoders view and of the Markers view MUST be what the panels
  of feature 006 and feature 007 held, with these changes of presentation: a decoder is shown by its
  name (the editable name field is gone; the host command `RenameDecoder` stays), an arrow folds its
  properties away and unfolds them, a bin removes it, and the header buttons have no tooltips.
- **FR-010** *(amended)*: The panel MUST keep a usable minimum width; the signals give way first. Its width
  MUST be changeable with a splitter at its left edge between 220 and 480 px (280 px at first) and be
  kept in the settings.

**The Capture view**

- **FR-011** *(amended)*: The Capture view MUST hold, from the top, Start (Stop while a capture runs) and
  what keeps it from working, the device, the sample rate, the length, the choice of channels to record
  and the other settings of the capture (for the simulated device: the seed and the density; it always runs in real time).
  There is no toolbar; the status line keeps the state of the capture.
- **FR-012** *(amended)*: The length MUST be entered in seconds, with the number of samples it makes shown
  beside it; a comma and a point MUST both separate the fraction, in every language, with at most nine
  digits after it; a length that is not a whole number of samples MUST be rounded to the nearest sample;
  the length MUST keep its value in seconds when the sample rate changes.
- **FR-013** *(amended)*: The channels to record MUST be buttons, eight in a row sharing the width of the
  panel, 28 px high, with the number of the channel in the middle. A channel that is off MUST show its
  number and its border in the colour of the channel with the inside in the colour of the panel; a
  channel that is on MUST show its border and its inside in the colour of the channel, with a number
  that can be read on it. The buttons **All** and **None** beside **Reset** MUST set the whole mask at
  once; they are inert while a capture runs.
- **FR-014**: The buttons MUST be locked while a capture runs and Start MUST stay unavailable while no
  channel is on; the reason MUST be shown. A setting the device cannot do MUST be explained next to it,
  as it is now under the toolbar.
- **FR-015**: A channel that is off MUST not be shown as a signal in the capture on screen if it has
  no data, and the flags MUST keep the meaning of feature 006 (what is shown and what is recorded) unless
  the plan finds a reason to separate them and records it.

**Quick scroll**

- **FR-016**: When the pointer is near the left or the right end of a signal, a button with an arrow MUST
  appear on that end; when the pointer leaves, it MUST disappear; neither MUST move anything.
- **FR-017**: A click on the right button MUST scroll the view, without changing the zoom, so that the
  first edge of that signal beyond the right end of the view is a quarter of the view from the left
  side; a click on the left button MUST do the same to the left, putting the first edge before the view
  a quarter of the view from the right side.
- **FR-018**: The button MUST NOT be offered (or MUST be disabled) on a side where the signal has no
  further edge; a gap in the capture MUST NOT count as an edge.
- **FR-019**: Using the button MUST stop "Follow latest data", as moving the view by hand does.
- **FR-020**: The search for the next edge MUST be fast on a capture of any length (it must not depend on
  the number of edges that are skipped), as the rendering of the signal is (constitution Principle
  VIII).

**Menus and icons**

- **FR-021**: File → Open recent MUST be a submenu of the 15 latest files, newest first; a file that no
  longer exists MUST be shown as unavailable; there MUST be commands to remove such an entry and to clear
  the list.
- **FR-022**: The application MUST have an icon shown by the window and the task bar; session files MUST
  have an icon of their own; the source tree MUST hold both in the formats the supported systems need,
  and a desktop entry and a MIME type declaration for the session format. All artwork MUST be original
  and its origin recorded.
- **FR-023**: Decoders → Add decoder and the key D MUST offer every registered decoder by name, whatever
  their number, and adding one MUST work; the entry MUST be unavailable, with the reason, when there is
  no capture.

**All of it**

- **FR-024**: Every new text MUST be localised in the six languages and follow the layout rules of the
  constitution (long German texts and Japanese must not clip).
- **FR-025**: Nothing of this feature MAY change what is captured, decoded or stored except the optional
  layout data (the panel's view, the heights of the signals); files saved before this feature MUST open
  as before and files saved by it MUST open in the version before it with the layout data ignored.
- **FR-027**: A legend row MUST show the name of the channel as its main text and, under it, the number of
  the analyzer input; the number MUST NOT change when the row is moved.
- **FR-028**: The question about an unsaved capture (Save, Discard, Cancel; feature 006) MUST offer a box
  "Remember my choice" that applies to Save and to Discard, and the same two answers MUST be in the
  Settings, where they can be taken back. Cancel MUST NOT be rememberable, and a remembered answer
  MUST NOT make quitting or closing the window impossible. A remembered Save still asks for a name when
  the capture has no file yet. The answer is kept in the settings.
- **FR-029**: The Settings MUST be reachable from the cog at the foot of the icon strip as well as from
  the menu.
- **FR-030**: The Start button (Stop while a capture runs) MUST be as wide as the content of the panel.
- **FR-031**: The status line MUST NOT shift while a capture runs: every number in it MUST be shown in a
  field of fixed width (digits of equal width), so that its text and what follows it stay in place as the
  digits change.
- **FR-032**: The time under the pointer MUST be drawn next to the pointer, below and to the right of the
  crosshair, or below and to the left when there is not enough room on the right (and above when there is
  not enough room below); it MUST NOT be drawn in a fixed corner or over the time axis.
- **FR-033**: While the view is being moved sideways with the mouse (the button held), the pointer MUST be
  a hand; when the button is released it MUST return to what it was.
- **FR-026**: The behaviour of every control above MUST be covered by tests that run without a display
  where the interface allows it (the state and the commands of the host), and by the headless rendering
  tests of the interface for what is drawn.

### Key Entities

- **Legend strip**: the column left of the signals; per channel a colour bar, a name, and the border
  below it that is dragged to resize.
- **Channel layout**: per channel the name, the colour, the position in the order and the height of
  its signal; kept for the next capture and saved with the session.
- **Side panel**: the area at the right edge with its icon strip and the one view shown (or none).
- **Capture settings**: device, sample rate, length (seconds and the samples they make), the channels to
  record, and the settings of the device.
- **Edge button**: the arrow that appears at an end of a signal and moves the view to the next edge.
- **Recent files list**: up to 15 file locations, newest first.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: A user can rename a channel, change its colour, change its signal's height and move it
  without opening any panel, and each takes at most two interactions (a click and typing, a click, a
  drag, a drag).
- **SC-002**: With the application just started, the signals take at least 25 % more of the window width
  than with the two panels of feature 006 open (one panel at a time, or none).
- **SC-003**: A user sets up a capture (device, rate, length in seconds, the channels) and starts it
  without leaving the Capture view, and the capture has the length asked for to within one sample.
- **SC-004**: On a capture with a flat stretch of 100 million samples, one click on the edge button
  brings the next edge into view in under 100 ms and the zoom is unchanged; the same click on a capture
  of one thousand samples is as fast.
- **SC-005**: After the click, the edge is within one pixel of a quarter of the view from the side it
  came from, for every zoom level.
- **SC-006**: The submenu holds exactly the 15 latest files, newest first, and a deleted file is
  never opened by a click on it.
- **SC-007**: With every number of registered decoders from one to eight, Decoders → Add decoder and D
  add the decoder the user chooses (zero cases in the tests where they do nothing).
- **SC-008**: Every new text exists in the six languages and the parity check passes; no text is clipped
  at the minimum window size in any language (the headless rendering test of feature 006 extended to the
  new screens).
- **SC-009**: Files saved by the version before this feature open unchanged, and files saved by this one
  open in the version before it with nothing lost but the layout.

## Assumptions

- The feature is about the interface; the decoders, the storage and the session format are unchanged
  except for optional layout data. Whether the host contract grows (heights, the view of the panel, the
  length in seconds) is decided in the plan; if it does, it is versioned and documented as the
  constitution requires.
- The colour of a channel is chosen from the palette, one step with every click, as the colour square of
  feature 006 does now; a colour picker is out of scope. The palette is the one that exists.
- The flag of a channel keeps its two meanings (shown and recorded) of feature 006; the channel buttons
  of the Capture view are that flag. A way to hide a recorded channel without removing it from the next
  capture is not part of this feature.
- "The next edge" of the quick scroll is the nearest edge of that signal outside the visible area on
  that side, so that every click brings something new into view; the quarter is a quarter of the width
  of the visible area.
- The length stays a number of samples inside the capture and the session; seconds are how it is entered
  and shown, derived from the sample rate. For a device whose rate is fixed or changes, the seconds are
  kept and the samples recomputed.
- The panel's view and the heights are stored in the settings (the layout of the window), not in the
  capture, except the channel layout (names, colours, order, heights), which is saved with the session
  as the names, colours and order already are.
- Icons: the application icon is drawn for this project, as vector artwork with raster exports; the
  session-file icon shares its motif. Registering the file type with the operating system (Windows
  registry, installation of the desktop entry and the MIME type) belongs to packaging (roadmap item 015+);
  this feature supplies the files.
- The toolbar of feature 006 is removed; what it held moved into the Capture view and the status line
  (see the Clarifications).
- The keyboard shortcuts of feature 006 keep their meaning, with D the one that is fixed. New controls
  are mouse-driven; their keyboard operation and labels for assistive technology are not part of this
  feature (gaps of feature 006 that stay).
- Translations are machine drafts reviewed by the owner before release, as for earlier features.
