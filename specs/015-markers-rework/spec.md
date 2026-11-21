# Feature Specification: Single and Pair Markers

**Feature Branch**: `015-markers-rework`

**Created**: 2026-11-18

**Status**: Draft

**Input**: User description: "015-markers-rework: replace the one kind of marker (two of which could be measured against each other) with two kinds, a Single Marker and a Pair Marker. A single marker is a pentagon-arrow pointing down on a dotted vertical line; a pair marker has a square mark to the left of its start line and to the right of its end line. Single markers can: make the time count from them, delete the data to their left or right, scroll to them (centred). Pair markers can: trim the data to the pair, fit the view to the pair (with room for the marks), convert to two single markers; the interval and its frequency are computed. All markers: change colour (a few choices), carry a short comment, be deleted. Two single markers can be converted to a pair. Two buttons on top of the Markers panel add the two kinds; deleting is a bin icon as in the Decoders panel. Numbering is per kind: prefix T for single, P for pair. No support for files of earlier versions. Marker text is removed from or moved out of the other specifications."

## Overview

Feature 006 gave the application one kind of marker: a vertical line at a point in time, any two of
which could be chosen as the ends of a measurement. In use this is awkward. The measurement depends
on two "buttons" chosen among many markers, a marker that is only a bookmark looks the same as the
ends of an interval, and none of the things one wants to do *with* a point in time (count the time
from it, throw away what came before it, centre the view on it) or *with* an interval (keep only it,
zoom to it) is offered.

This feature replaces that one kind with two, each made for what it is used for:

- **Single marker** (named **T1**, **T2**, ...): one point in time. A vertical line with a
  coloured pentagon, pointing down, whose lower tip sits on the line at the top of the signals. It is
  a place to look at, to count the time from, or to cut the capture at.
- **Pair marker** (named **P1**, **P2**, ...): an interval. Two vertical lines; a coloured
  square to the left of the start line and one to the right of the end line, so the pair reads as a
  bracket around the interval. The application computes its duration and the frequency of that
  duration.

Both kinds are put in place with the mouse: the add command only starts a placement, in which a
dotted line in the marker's colour follows the pointer (and snaps to the transitions of the signal
under it) until a click fixes it, and then the line is solid.

Both kinds have a colour (from a few choices), an optional short comment, and can be deleted. The
Markers view gets two buttons at its top, one for each kind; a click on a marker's row opens its actions,
which include deleting.

Out of scope: more than two kinds of marker, free colour choice (a palette only), measurements among
more than two points, markers on decoder annotations, exporting marker data to a file other than the
session file, an undo history for the destructive actions (they ask for confirmation instead), and
reading files written before this feature (there has been no public release, so no code for
them is written, see FR-024).

## Clarifications

### Session 2026-11-18

- Q: Is a new marker put at a default place, or by the user? -> A: By the user with the mouse. The
  command to add a marker (a button, a shortcut, a menu entry) only starts a placement; nothing is
  added until the user clicks (FR-007, FR-027).
- Q: How is a marker drawn while it is not yet fixed, and after? -> A: Not fixed: a dotted vertical
  line in the marker's colour (the colour of its mark) that follows the pointer. Fixed: a solid line.
  This replaces the dotted line of the first version of this specification (FR-002, FR-003, FR-028).
- Q: How is a pair placed? -> A: The first click fixes the first half, the second click the second
  half; then the half that is further left becomes the start and the other the end (FR-029).
- Q: Which side is the mark of a half that is already fixed on? -> A: The side that fits the role it
  would have for the current pointer position: the start's square is always left of its line, the
  end's right of its line, and the mark jumps when the pointer crosses the line. Before the pointer has
  moved, the first half is the left one, the start (FR-030).
- Q: Where can a line be fixed? -> A: Only at whole sample positions, the points where the signal was
  measured, never between them (FR-031).
- Q: How does snapping work while placing? -> A: When the pointer is near a transition, the line
  snaps to it and the transition is highlighted, so that a transition can be hit exactly even where one
  pixel covers many samples; the pointer must be over the lane (the band) of the signal that has the
  transition: above or below that band nothing snaps, however close the transition is in time
  (FR-032, FR-033, user story 9).
- Q: What if many transitions lie in the snapping zone, so that the one wanted cannot be told from its
  neighbours? -> A: The pointer is held still for about two seconds, then the view zooms in smoothly
  around it until the transitions near it can be told apart; when the marker is fixed or the placement
  is cancelled, the zoom and position from before are restored (FR-034). This replaces stepping with the
  arrow keys, which the first plan had proposed.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Mark a point in time with a single marker (Priority: P1)

The user puts a single marker on a point of the signals with the mouse, to point at it, read its time
and come back to it. It stays on that point while the view is zoomed and panned.

**Why this priority**: it is the base of the feature, the smallest thing that replaces what feature
006 offered, and the other stories build on it.

**Independent Test**: place a single marker on a virtual-device capture, zoom and pan, drag it with
snapping on, and check its mark, its line, its time and its name **T1**; then add another and check
that it is **T2**.

**Acceptance Scenarios**:

1. **Given** a capture on screen, **When** the user presses the **Single marker** button at the top of
   the Markers view (or uses its shortcut or menu entry), **Then** no marker is added yet; the
   application is in placement mode, and a dotted vertical line in the colour the marker will have
   follows the pointer across the signals, with its pentagon at the top.
2. **Given** placement mode, **When** the user clicks the left mouse button, **Then** the marker **T1**
   is fixed at that point: its line is now solid, across the signals, with the pentagon filled with the
   marker's colour, pointing down, its lower tip on the line; placement mode ends.
3. **Given** a single marker, **When** the user zooms and pans, **Then** it stays at the same point
   of the signal and shows the time of that point.
4. **Given** snapping is on and the pointer is over the lane of a signal near a transition of it,
   **When** the user fixes the marker, **Then** it is at the exact time of that transition (see
   story 9 for the rules of snapping).
5. **Given** placement mode, **When** the user presses Escape or the right mouse button, or presses the
   same add button again, **Then** placement ends and nothing is added.
6. **Given** two single markers have been added, **When** the first is deleted and a new one is
   added, **Then** the new one is **T3**; a number is not given twice in one capture.

---

### User Story 2 - Measure an interval with a pair marker (Priority: P1)

The user adds a pair marker, puts its two ends on two points of the signals and reads the duration
between them and the frequency of that duration.

**Why this priority**: measuring pulse widths and periods is what the markers of feature 006 were
mainly used for; the pair marker is where that goes.

**Independent Test**: place a pair marker on the two edges of a pulse of known width in a
virtual-device capture and check the displayed duration and frequency against the known values in
several languages.

**Acceptance Scenarios**:

1. **Given** a capture on screen, **When** the user presses the **Pair marker** button, **Then** no
   marker is added yet; in placement mode a dotted vertical line in the marker's colour follows the
   pointer, with a square at its side.
2. **Given** that placement mode, **When** the user clicks, **Then** the first half is fixed (its line
   becomes solid) and the dotted line of the second half follows the pointer; the second click
   fixes the second half and ends placement, and the marker **P1** exists, with a coloured square
   to the **left** of its start line and one to the **right** of its end line.
3. **Given** the first half is fixed, **When** the pointer is to the right of it, **Then** the fixed
   half is the start (its square to the left of its line) and the half following the pointer is the
   end (its square to the right of its line); **When** the pointer moves to the left of it, **Then**
   the squares jump: the half following the pointer is the start and the fixed half is the end, with
   its square to the right of its line.
4. **Given** the second click is made on the same sample as the first, **Then** the pair exists with a
   duration of zero.
6. **Given** a pair marker, **When** the user drags either end, **Then** only that end moves (with
   snapping when it is on); if it is dragged past the other end the two swap roles, so the start is
   always the earlier one.
7. **Given** a pair marker, **When** the user reads it, **Then** the panel shows the time of the start
   and of the end, the **duration** between them and the **frequency** (the reciprocal of the
   duration), formatted with the number, unit and decimal conventions of the selected language.
8. **Given** the two ends are at the same point, **When** the user reads the pair, **Then** the
   duration is zero and no frequency is shown (not infinity).
9. **Given** a pair marker, **When** the user zooms and pans, **Then** both ends stay on their points
   of the signal.

---

### User Story 3 - Colour, comment, remove (Priority: P2)

For every marker, of either kind, the user can choose its colour from a few, write a short comment
that says what it marks, and delete it.

**Why this priority**: with several markers on screen, telling them apart and remembering why each is
there is what makes them usable; it does not block the measuring itself.

**Independent Test**: on a capture with a single and a pair marker, change both colours, write a
comment on each, delete one from its action list, save, reopen and compare.

**Acceptance Scenarios**:

1. **Given** a marker, **When** the user picks a colour, **Then** its mark, its line(s) and its swatch
   in the panel take that colour at once, and the choices are the same few colours for both kinds.
2. **Given** a marker, **When** the user types a comment, **Then** it is shown in its row of the panel
   and as a tooltip on its mark, is limited to a short length, and is shown exactly as typed (control
   and invisible characters removed), never translated.
3. **Given** a marker, **When** the user chooses **Delete** in its action list, **Then** it disappears from the view and the panel at once, a
   pair together with both its ends.
4. **Given** a marker's row has been deleted, **When** the user adds a marker of the same kind,
   **Then** it does not take the deleted one's number (see FR-006).

---

### User Story 4 - Work with a point: time reference, cut, go to (Priority: P2)

From a single marker the user can: count the time from it, delete the data to its left, delete the
data to its right, or scroll to it.

**Why this priority**: they are what a single marker is for beyond looking at it; all but the cuts
are harmless to the data, and the cuts are confirmed.

**Independent Test**: on a capture of known content with a single marker, set the marker as the time
reference and check the axis and the other markers' times; cut left, then (on another copy) right,
and check the remaining length and the positions of the remaining markers; scroll to it from far
away and check it is at the centre.

**Acceptance Scenarios**:

1. **Given** a single marker, **When** the user chooses "count time from here", **Then** the time at
   that marker is zero: the axis labels and the times of all markers are counted from it, earlier
   points having negative times; no data changes. A way to put the origin back to the start of the
   capture is offered while another origin is set.
2. **Given** a single marker, **When** the user chooses "delete data to the left" and confirms,
   **Then** the capture starts at the marker, the markers to the left of it are removed (the marker
   itself stays, now at the start), the time reference is kept if its point remains and goes back to
   the start otherwise, the view shows the same time span as far as it still exists, and the capture
   is marked unsaved.
3. **Given** a single marker, **When** the user chooses "delete data to the right" and confirms,
   **Then** the capture ends at the marker, markers to the right of it are removed, and the capture is
   marked unsaved.
4. **Given** the user chooses either deletion, **When** the confirmation asks, **Then** it says
   which side and how much time will be lost, and Cancel changes nothing.
5. **Given** a single marker out of the visible window, **When** the user chooses "scroll to it",
   **Then** the view moves, with the zoom unchanged, so that the marker is at the horizontal centre
   (as near it as the window allows when the marker is within half a window of an end of the capture:
   the window never extends beyond the capture, as feature 006 requires).
6. **Given** a capture is running, **When** the user looks at the actions, **Then** the two deletions
   are unavailable (the data is still being recorded).

---

### User Story 5 - Work with an interval: trim, fit, split (Priority: P2)

From a pair marker the user can: keep only the data between its ends, fit the view to it, or turn it
into two single markers.

**Why this priority**: zooming to the interval being measured and discarding the rest are the natural
next steps after measuring; they do not block the measuring itself.

**Independent Test**: on a capture with a pair marker, fit the view and check that both ends and
their squares are inside the ruler with a margin; trim and check that the capture is the interval
and that the pair now sits at its ends; split and check the two singles at the ends.

**Acceptance Scenarios**:

1. **Given** a pair marker, **When** the user chooses "fit the view to the pair", **Then** the zoom
   and position change so that the interval fills the signal area with an equal margin on the left and
   on the right, large enough for both squares to lie fully inside the time ruler (except on a side
   where the pair lies at the end of the capture and the window cannot extend further; see Edge Cases).
2. **Given** a pair marker, **When** the user chooses "trim to the pair" and confirms, **Then** the
   capture holds only the data from the start to the end of that pair (both included as ends), all
   markers outside it are removed, the pair stays at the two ends of the capture, and the capture is
   marked unsaved.
3. **Given** a pair marker, **When** the user chooses "convert to two single markers", **Then** the
   pair is replaced by two single markers at its start and its end, with new numbers of the single
   kind, the pair's colour and (on the start one) its comment; the number of the pair is not reused.
4. **Given** a capture is running, **When** the user looks at the actions, **Then** "trim to the
   pair" is unavailable.

---

### User Story 6 - Make a pair out of two single markers (Priority: P3)

The user who has put two single markers, and then decides they delimit an interval, can turn them
into one pair marker.

**Why this priority**: a convenience; the same result is reached by adding a pair and dragging.

**Independent Test**: with two single markers, convert them and check that one pair exists with
ends at the two positions and the singles are gone.

**Acceptance Scenarios**:

1. **Given** at least two single markers, **When** the user chooses "make a pair" in the action list
   of one of them and then clicks another single marker (its row in the panel, or its mark or line in
   the signal view), **Then** they are replaced by one pair marker whose start is the earlier and whose
   end the later of the two, with a new number of the pair kind, the colour and comment of the earlier one.
2. **Given** "make a pair" is waiting for the second marker, **When** the user looks at the markers,
   **Then** pairs and the first marker are shaded (in the panel and in the signal view) and ignore clicks,
   and Escape cancels. **Given** there is only one single marker, **Then** the action is unavailable.

---

### User Story 7 - The Markers view (Priority: P2)

The Markers view of the side panel lists the markers of both kinds and offers every action.

**Why this priority**: it is where the markers are managed; stories 1-6 use it.

**Independent Test**: add several markers of both kinds, and check the list, the order, the buttons
and the actions of every row, in two languages and at the smallest supported panel width.

**Acceptance Scenarios**:

1. **Given** the Markers view is shown, **When** the user looks at its top, **Then** two buttons,
   **Single marker** and **Pair marker**, each with its mark (the pentagon, and the two squares), start
   the placement of a marker of that kind (stories 1, 2 and 9).
2. **Given** markers of both kinds exist, **When** the user looks at the list, **Then** it holds one
   row per marker in the order of the time of its (start) position, each with its name (**T3**,
   **P1**), its colour swatch, its comment, its time or its two times with the duration and the
   frequency (pairs); there is no separate button or bin on the row.
3. **Given** a row, **When** the user clicks it (or presses the marker's mark in the signal view with the
   right button), **Then** it offers the actions of its kind (stories
   4 and 5) and, for every kind, the colour and the comment.
4. **Given** a mark in the signal view, **When** the user selects it or its row, **Then** both are
   shown as selected, and the Delete key removes the selected marker.
5. **Given** the limit of 64 markers is reached, **When** the user adds one, **Then** nothing is
   added and the user is told why; a pair counts as one.

---

### User Story 8 - Saving and opening (Priority: P2)

Markers of both kinds are saved with the capture and come back as they were.

**Why this priority**: markers are part of what the user spends time on; losing them on save would
make them unusable for real work.

**Independent Test**: save a capture with markers of both kinds (colours, comments, a time
reference), reopen it, and compare everything.

**Acceptance Scenarios**:

1. **Given** markers of both kinds with colours and comments, and a time reference, **When** the
   capture is saved and opened, **Then** they all come back, with the same numbers.
2. **Given** a file whose marker information is invalid, **When** it is opened, **Then** the capture
   opens with no markers and a warning, as feature 006 requires for its view section.

---

### User Story 9 - Place a marker exactly, by snapping to a transition (Priority: P1)

While placing a marker (or dragging one), the user brings the pointer near a transition and the line
snaps to it, the transition being highlighted, even at a zoom where one pixel holds many samples.

**Why this priority**: reading a pulse width exactly is the reason for the markers; without exact
placement at crowded zooms the measurement is guesswork.

**Independent Test**: on a capture with transitions a few samples apart, zoom out until they share one
pixel and place a pair on two chosen transitions; check the duration against the known one; then
move the pointer above and below the lane of the signal and check that nothing snaps.

**Acceptance Scenarios**:

1. **Given** placement mode and the pointer over the lane of a signal, within the snapping distance of
   a transition of that signal, **When** the user looks at the view, **Then** that transition is
   highlighted and the dotted line is at its exact time.
2. **Given** a zoom where several transitions fall in one pixel, **When** the pointer is over that
   pixel in the lane, **Then** the line snaps to the transition nearest to the pointer's time, and the
   other transitions of that pixel can be reached by the automatic zoom of scenarios 7 to 9; none is
   out of reach at any zoom.
3. **Given** the pointer is near a transition in time but below or above the lane of its signal (over
   another lane, the ruler or empty space), **When** the user fixes the marker, **Then** it is not
   snapped to that transition; it is at the sample under the pointer, unless a transition of the lane
   the pointer is actually over is within reach.
4. **Given** the pointer is over a lane whose signal has no transition within reach, **When** the user
   fixes the marker, **Then** it is at the sample under the pointer.
5. **Given** snapping is turned off, **Then** no transition is highlighted and the marker is fixed at
   the sample under the pointer.
6. **Given** a marker (or a half of a pair) is dragged, **Then** the same snapping, highlighting and
   automatic zoom apply.
7. **Given** placement mode, snapping on and the pointer over a lane with more than one transition of
   its signal within the snapping distance, **When** the pointer stays still (moves less than 3 px)
   for 2 seconds, **Then** the view zooms in smoothly (about 0.4 s) around the pointer, the point under
   it staying where it is, until the transitions nearest to it lie at least two snapping distances
   apart (each has a zone of its own), or until the zoom limit is reached.
8. **Given** the view has zoomed in automatically, **When** the user moves the pointer to the wanted
   transition (it is highlighted) and clicks, **Then** the marker is fixed on it exactly and the zoom
   and position the view had before the automatic zoom are restored at once.
9. **Given** the view has zoomed in automatically, **When** the user cancels (Escape, the right button, the
   add button again, or any command that ends placement), **Then** the same zoom and position are
   restored and nothing is added. **When** the user zooms or pans by hand while it is zoomed in
   automatically, **Then** the view is theirs and is not restored. For a pair, the first click restores
   the view and the second half may trigger the automatic zoom again.
10. **Given** snapping is off, the pointer is outside a lane, or the zone holds one transition or none,
    **When** the pointer stays still, **Then** the view does not zoom.

---

### Edge Cases

- The capture is empty (no samples): a marker can be placed only at position zero; the actions that
  change the data are unavailable.
- Placement mode and the limit of 64 markers: the add commands are refused with the reason before
  placement mode starts.
- The user zooms, pans or scrolls while placing: allowed; the dotted line stays under the pointer.
  Starting any other command (opening a file, starting a capture, closing) ends placement; an
  unfinished pair is not added.
- The capture grows (a running capture) or changes while the view is zoomed in automatically: the saved
  zoom and position are restored as far as the capture allows (clamped to its length).
- The pointer leaves the signal area while zoomed in automatically: the view stays zoomed in until the
  marker is fixed or the placement ends (the user may come back to the lane); only then it is restored.
- The pointer leaves the signal area during placement: the dotted line is hidden and a click outside
  the area does not fix a marker; placement stays active.
- The pointer is over the ruler, the legend or a decoder lane during placement: the line follows the
  pointer's time and a click fixes it at that sample, with no snapping (the decoder lanes have no
  transitions of their own).
- A capture is running while a marker is placed: allowed, the position is the sample under the pointer
  at the click.
- The two ends of a pair are at the same point: duration zero, no frequency; the squares overlap and
  both stay visible and draggable (the one dragged last is on top).
- A single marker at the first or the last sample: "delete data to the left/right" would leave the
  capture of one sample or nothing to delete; the action is unavailable when it would delete nothing,
  and a capture is never emptied by it.
- A pair at an end of the capture: the window never extends beyond the capture (feature 006), so "fit to
  the pair" gives that side as much margin as the capture has (possibly none) and the square on that
  side may be cut off by the edge of the signal area; the action still works and the other side keeps
  its full margin. The same holds for "scroll to" near an end.
- Trimming or cutting while decoders are shown: the decoders run again on the shortened capture, as
  they do for any other change of the data; their results before the cut are gone with the data.
- A pair of which only one end would remain after a cut or a trim is removed as a whole (a half is never
  left); the user is told how many pairs went (an information notice, not a dialog).
- Cutting or trimming moves positions: markers that remain keep their points of the signal and are
  shown at their new positions; the time reference keeps its point if it remains.
- A marker dragged outside the capture is held at the nearest end of it.
- A comment is longer than allowed, or has only spaces or control characters: it is shortened or
  reads as empty; nothing breaks the layout (also in the longest German text and in Japanese).
- A capture is running: markers can be added, moved and deleted; only the data-changing actions are
  unavailable.
- Two markers of different kinds at the same point: both are drawn, their labels do not overlap
  unreadably (they are staggered), and each can still be picked.

## Requirements *(mandatory)*

### Functional Requirements

**The two kinds**

- **FR-001**: The application MUST have exactly two kinds of marker: a **single marker** (one point in
  time) and a **pair marker** (an interval with a start and an end). The old kind, one marker of which
  could be chosen as the first or the second of a measurement, MUST no longer exist, and with it the
  choosing of two markers for a measurement.
- **FR-002**: A single marker MUST be drawn as a solid vertical line across the signal area and, at
  its top, a mark in the shape of a pentagon-arrow pointing down (a rectangle with a pointed lower end),
  filled with the marker's colour; the lowest point of the mark MUST coincide with the line.
- **FR-003**: A pair marker MUST be drawn as two solid vertical lines; at the top, a square filled
  with the marker's colour MUST lie to the **left** of the start line (touching it) and one to the
  **right** of the end line (touching it).
- **FR-004**: Marks MUST stay legible on light, dark and high-contrast themes and MUST not be
  distinguished by colour alone (the shapes differ between the kinds, the name is shown near the mark).
- **FR-005**: Markers MUST be attached to a sample position and stay at the same point of the signal
  while the user zooms and pans; each MUST show the time of its position (a pair: of both ends), with
  the conventions of the selected language.

**Names**

- **FR-006**: Each marker MUST have a name made of a prefix and a number: **T** and a number for a
  single marker, **P** and a number for a pair marker. The numbering MUST be separate for each kind,
  start at 1, and a number MUST NOT be given to a second marker in the same capture (including after
  the first has been deleted or converted). Names are not translated and MUST be the same in every
  language.

**Adding and moving**

- **FR-007**: Two buttons at the top of the Markers view, and for each of them a menu entry and a
  shortcut, MUST start the placement of a single marker and
  of a pair marker. Starting a placement MUST NOT add a marker; the marker exists only when the user
  fixes it with a click (FR-027 to FR-033).
- **FR-008**: The user MUST be able to drag a marker (a single marker; either end of a pair) along
  the time axis, with the snapping and the highlighting of FR-032 and FR-033 and the dotted line of
  FR-028 while dragging. A pair's ends that are dragged past each other MUST swap roles.
- **FR-009**: The limit of markers MUST stay 64 in all (singles and pairs together, a pair counting as
  one); at the limit nothing is added and the user is told why.
- **FR-010**: Positions MUST stay inside the capture; a position that would fall outside is held at
  the nearest end.

**Placing a marker**

- **FR-027**: While a placement is active (placement mode) the line of FR-028 MUST follow the pointer;
  a left click fixes the marker (for a pair, its next half); Escape, a right click or the same add
  command again MUST cancel it, adding nothing (for a pair, also a first half already fixed, which
  is dropped).
- **FR-028**: While a single marker or a half of a pair is not fixed (being placed or dragged), its
  vertical line MUST be drawn dotted and in the colour of the mark of that marker, instead of the
  usual thin line, and MUST follow the pointer; once it is fixed, the line MUST be drawn solid.
- **FR-029**: Placing a pair takes two clicks: the first fixes the first half, the second the other.
  After the second, the half that lies to the left becomes the start and the other the end, whichever
  was fixed first; two halves on the same sample make a pair of zero duration.
- **FR-030**: While the first half of a pair is fixed, the mark of each half MUST be on the side that
  fits the role it would have: the start's square to the left of its line, the end's to the right of
  its line. The fixed half is the start when the pointer is to the right of it and the end when the
  pointer is to the left of it (the start before the pointer has moved), and its square MUST jump to
  the other side of the line when that changes.
- **FR-031**: A fixed line (also when dragged) MUST lie only on a whole sample position, a point where
  the signal was measured.
- **FR-032**: When the pointer is within the snapping distance of a transition of the signal whose
  lane it is over, the line MUST snap to the exact time of that transition and the transition MUST be
  highlighted in the lane. The snapping MUST work at every zoom, including those where one pixel holds
  several transitions, so that every transition can be given to a marker exactly.
- **FR-033**: Snapping MUST consider only the transitions of the signal of the lane the pointer is
  over; over another lane, the ruler, the legend or empty space, a transition that is close in time
  MUST NOT attract the line. The snapping setting of feature 006 (default on) stays: with it off,
  nothing snaps and nothing is highlighted.
- **FR-034**: While a marker is placed or dragged, snapping is on, and the zone of the pointer (the
  snapping distance on each side, on the lane under it) holds more than one transition, holding the
  pointer still (moving less than 3 px) for 2 seconds MUST zoom the view in smoothly (about 0.4 s)
  around the pointer, keeping the point under it fixed, until the transitions nearest to the pointer lie
  at least two snapping distances apart or the zoom limit is reached. When the marker is fixed or
  the placement is cancelled or ended in any way, the zoom and position from before the automatic
  zoom MUST be restored at once, unless the user zoomed or panned by hand in between. A pair restores after each click. The feature has no setting and no control of its own; it shows only a status line text, and only while the view is zoomed in automatically.

**Actions of a single marker**

- **FR-011**: A single marker MUST offer **count time from here**: the time axis and the times of all
  markers are then counted from the marker's point, which has the time zero; earlier points have
  negative times. It changes no data. While the origin is not the start of the capture, an action to
  put it back MUST be offered. At most one point is the origin, and the origin MUST be saved in the
  session file.
- **FR-012**: A single marker MUST offer **delete data to the left** and **delete data to the right**.
  Each MUST ask for confirmation naming the side and the span of time that will be lost, change the
  capture so that it starts (or ends) at the marker, remove the markers that fall in the deleted part,
  mark the capture as unsaved and let decoders run again on the result. A pair that would be left with
  only one end is removed with an information notice. They MUST be unavailable
  while a capture is running and when they would delete nothing; no capture MUST be left empty by them.
- **FR-013**: A single marker MUST offer **scroll to it**: the view moves, zoom unchanged, so that
  the marker is at the horizontal centre of the signal area, or as near to it as the window allows
  near the ends of the capture.

**Actions of a pair marker**

- **FR-014**: A pair marker MUST show its **duration** (the distance in time from start to end) and
  the **frequency** (the reciprocal of the duration), formatted for the selected language through the
  localization foundation and at a precision that never claims more than the sample period resolves
  (the rule of feature 006 is kept). A duration of zero has no frequency.
- **FR-015**: A pair marker MUST offer **trim to the pair**: after confirmation (naming the span of
  time kept and lost) the capture holds only the data between its start and its end; markers outside
  are removed, those inside are kept; the pair then sits at the two ends. It MUST be unavailable
  while a capture is running; it marks the capture as unsaved and lets decoders run again.
- **FR-016**: A pair marker MUST offer **fit the view to the pair**: zoom and position are set so that
  the interval fills the signal area, with an equal margin on the left and the right at least as wide
  as a mark plus a small gap, so that both squares are fully inside the time ruler. It changes no
  data, and the zoom limits of feature 006 still hold.
- **FR-017**: A pair marker MUST offer **convert to two single markers**: the pair is replaced by two
  single markers at its start and its end, with new single-kind numbers (FR-006), the pair's colour,
  and its comment on the start one.

**Actions of every marker**

- **FR-018**: Every marker MUST offer **colour** (a choice among a small fixed set of colours, eight
  by default, the same for both kinds, distinguishable on every theme; each new marker takes the next
  one in turn, and the first one again after the last), **comment** (short text, at most 80 characters, sanitized like names of channels and never
  translated) and **delete** (in the action list, with no confirmation). The comment MUST be shown in the row and as a tooltip of the mark.
- **FR-019**: The user MUST be able to turn **two single markers into one pair marker**: selecting
  "make a pair" in the action list of a single marker and a click on another single marker (in the panel or in
  the signal view) joins the two; the pair's start is the earlier of
  the two and its end the later, with a new pair-kind number, the colour and the comment of the earlier.

**The panel**

- **FR-020**: The Markers view MUST have, at its top, two buttons (one per kind) and below them one
  row per marker, ordered by the time of its (start) position; a row MUST show the name, the colour,
  the comment, the time (a single) or the two times, the duration and the frequency (a pair). A click on
  a row opens the action list under it. Every action of FR-011 to FR-019 MUST be reachable from the row and from the context menu
  of the marker's mark in the signal view. The text of the view MUST be localized in the six languages
  and fit at the smallest panel width in the longest German text and in Japanese.
- **FR-021**: Selecting a marker (its mark or its row) MUST select both (the row by a stronger outline); the Delete key removes the
  selected marker; the previous/next marker shortcuts of feature 006 MUST jump to the previous and
  next marker position (for a pair, its start).

**Saving**

- **FR-022**: Markers of both kinds, with their numbers, colours, comments and positions, and the time
  origin MUST be saved in the session file and restored by opening it, within the `VIEW` section
  defined by feature 006, whose layout MUST be redefined by this feature.
- **FR-023**: Marker information that is invalid on opening MUST NOT fail the open: the capture
  opens with no markers and a warning, as for any invalid `VIEW` section.
- **FR-024**: No code to read files written before this feature MUST be added (the application has
  had no public release). Such a file MUST still open without a crash; its `VIEW` section may be
  dropped with the warning of FR-023.

**Interface contract and checks**

- **FR-025**: The host API MUST offer commands for every action above (add each kind, move, set
  colour, set comment, delete, count time from, cut left, cut right, scroll to, trim, fit, convert to
  singles, make a pair, reset the origin), and the host API document and its version MUST be updated
  with them; the commands of the old kind MUST be removed. Every command MUST be reachable from the
  interface, and each destructive one MUST be refused with a notice while a capture runs.
- **FR-026**: The marker rules (numbering, ordering, limits, positions after cuts, the time origin,
  conversions) MUST be testable without the interface, as plain data and commands; a randomized
  sequence of actions including the new ones MUST leave no marker outside the capture, no duplicate
  number and no crash.

### Key Entities

- **Single marker**: a sample position, a colour, a comment, a number (kind T).
- **Pair marker**: a start and an end sample position (start never after end), a colour, a comment, a
  number (kind P); its duration and frequency are derived, never stored.
- **Marker colour**: one of a small fixed set.
- **Time origin**: the sample position that counts as time zero for the axis and the marker times; the
  start of the capture unless the user sets it from a single marker; saved with the session.
- **Placement**: the state between the command that adds a marker and the click that fixes it (for a pair,
  between the first click and the second); the dotted line follows the pointer in it.
- **Marker numbering**: per kind, next number to give, never reused in one capture; saved with the
  session so that numbers stay stable.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: A user can add a single marker with two actions (start the placement, click) and a pair
  with three (start, click, click), and read its time (a pair: its duration and frequency) without
  choosing anything else.
- **SC-002**: On a capture whose pulse width and period are known by construction, the duration and
  frequency shown for a pair on its edges are exact to the sample period in all six languages.
- **SC-003**: Each of the ten actions (count time from, cut left, cut right, scroll to, trim, fit,
  convert to singles, make a pair, colour, comment) gives its specified result in an automated check,
  and no action other than the four data-changing ones changes the capture data.
- **SC-004**: After "scroll to", the marker is within one pixel of the centre of the signal area whenever
  the capture extends at least half a window on both sides of it; after "fit", both squares are fully
  inside the ruler whenever the capture extends far enough beyond the pair for the margin.
- **SC-005**: A randomized sequence of at least 100,000 marker and view actions, with cuts, trims and
  conversions mixed in, leaves every marker inside the capture, every number unique in its kind, and
  ends without a crash or a hang.
- **SC-006**: A save followed by an open reproduces every marker (kind, positions, colour, comment,
  number), the numbering state and the time origin exactly.
- **SC-008**: On a capture with transitions one to three samples apart, at a zoom where one pixel holds
  at least ten samples, holding the pointer still for 2 seconds over any of them zooms in until it can be
  told from its neighbours, and a marker fixed on it is exact: in a check of at least 1,000 random
  transitions, no placement is off by even one sample, and after every fix and every cancel the zoom and
  position from before are back exactly.
- **SC-009**: With the pointer outside the lane of a signal, no transition of that signal attracts the
  line, in the same 1,000 checks.
- **SC-007**: The Markers view, with markers of both kinds, shows no clipped or overlapping text at
  the smallest panel width in the longest German strings and in Japanese.

## Assumptions

- The feature builds on features 006 (the signal view, marker snapping, the Markers view of
  the side panel of feature 014) and 007 (decoders that run again on changed data).
- The text of the old behaviour is replaced in the earlier specifications as listed under
  "Changes to other specifications" below; the earlier specifications keep describing everything else.
- There is no undo history in the application; therefore the actions that remove data ask for
  confirmation, and the other actions are cheap to reverse by hand.
- "Count time from here" is a view setting saved in the session file; it never changes sample data.
- A comment is at most 80 characters and the palette has eight colours; the exact colours are chosen
  in planning, with the checks for legibility of feature 006 (FR-013).
- Keyboard shortcuts of feature 006 keep their keys: **M** starts the placement of a single marker,
  **[** and **]** jump, **S** toggles snapping (on by default), Delete removes the selected marker; a
  pair gets a shortcut of its own (**Shift+M**), chosen in planning if it clashes.
- The snapping distance stays what feature 006 had (about twelve pixels along the time axis); how
  every transition is reached at a crowded zoom is the automatic zoom of FR-034.
- Only a left click inside the signal area fixes a marker; the right button and Escape cancel.
- The wait before the automatic zoom is 2 seconds and the stillness limit 3 px; the zoom takes about
  0.4 s; the restore is immediate. They are fixed values, not settings. The user's own zoom (the wheel)
  during placement keeps working and takes over from the automatic one.
- Single markers become pairs only by the explicit action of FR-019; the application never pairs them
  by itself.
- The host API changes incompatibly (commands removed and added); this is acceptable because no
  release has been made, and the host API document is versioned as feature 014 did.

## Changes to other specifications

What was written about markers in earlier specifications, and what became of it, decided so that the
earlier text no longer conflicts with this feature (where it does, this feature wins):

| Where | What it said | What became of it |
|-------|--------------|-------------------|
| 006 spec, definition **Marker** | a vertical line to read the time of a point and the distance between two | rewritten: a single or a pair marker, see 015 |
| 006 spec, User Story 3, scenarios 1-5 | place, measure between two, snap, remove, go to | replaced by a pointer to 015 (stories 1-5, 7); snapping stays as in 015 FR-008 |
| 006 spec, FR-016, FR-017 | place/drag/snap/rename/remove/jump; time of two markers, distance, frequency | superseded by 015 FR-005 to FR-021 |
| 006 spec, FR-018 | markers saved in the session file; a file without markers opens | kept; the content is defined by 015 FR-022 to FR-024 |
| 006 spec, Key Entity **Marker** | a time position, a name and an identity; a pair yields a measurement | superseded by 015 Key Entities |
| 006 `data-model.md`, `contracts/view-section.md` | layout of the old marker records and of the measured pair | marked superseded; the new layout is defined in the plan of 015 |
| 014 spec, FR-009 | the Markers view holds what feature 006 had | amended: the content of the Markers view is defined by 015 |
| `roadmap.md` | item 015+ "as needed" | item 015 added; the open-ended item moves to 016+ |
| 014 spec, plan, research, tasks | packaging "belongs to roadmap item 015+" | renumbered to 016+ (no other change) |

Not changed: the remarks of feature 005 (a "marker" of a missing message), 007 and 008 (annotation
"edge markers" and a mention of placing markers while a decoder works), and the history of tasks and
verification files of 006 and 014, which record what was done then. The user guide and the host API
document describe the application as it is; they are updated when this feature is implemented.
