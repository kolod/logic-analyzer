# Feature Specification: Decoder Framework and Annotations

**Feature Branch**: `007-decoder-framework-annotations`

**Created**: 2026-11-15

**Status**: Draft

**Input**: User description: "Decoder framework and annotations (roadmap item 007, branch 007-decoder-framework-annotations). Add a protocol-decoder framework to the logic analyzer: an annotation model with two presentations — decoder lanes below the channels (ranges with text, points) and overlays drawn on the signal itself (points, edge arrows, ranges on the channel that carries the event); decoder stacking (one decoder consuming the output of another); fault isolation (a failing or panicking decoder is contained, reported with a stable error code and never loses or corrupts the capture or other decoders); background, cancellable, progress-reporting execution so the interface never blocks, also while the capture is still growing; indexed lookup of annotations for the visible window so drawing cost does not depend on the capture length (constitution Principle VIII); persistence of the decoder setup and optionally its results with the session file through the extensible session format (the optional DECO section reserved by feature 003); a place in the UI of feature 006 for the decoder lane area, the decoder side panel and menu entries, following the extension points that feature 006 left open (FR-037) and closing its gaps where needed; and one simple sample decoder with its own signal generator for the virtual device (to validate the framework end to end; real protocol decoders such as I2C come in feature 008). All user-visible text in the six languages, no panics on any input, tests runnable without hardware."

## Overview

A logic analyzer is used to answer questions about protocols: not "where did the line go high" but
"which byte was sent, and was it acknowledged". Until now the product shows waveforms and measures
time (feature 006) but it cannot read what the signals mean. This feature adds the machinery that
does: a framework in which **decoders** read channels of a capture and produce **annotations**, and
a way to show those annotations next to the signals. It delivers no real protocol decoder (I2C is
feature 008); it delivers the framework, proven end to end by one small sample decoder with its own
signal generator.

What the user gets:

- **Add a decoder to a capture**: choose a decoder, say which channel carries which of its signals,
  set its options, and the decoder reads the capture in the background.
- **Two ways to see the result**: a **decoder lane** below the channels that shows what was decoded
  as labelled ranges and points (for example "byte 0x5A"), and **overlays on the signal itself**, drawn
  exactly where the event happens (an arrow on the clock edge at which a bit was sampled, a coloured
  dot where a frame starts).
- **Stacking**: a decoder can consume what another decoder produced, so a higher-level protocol can
  be read on top of a lower-level one.
- **Safety**: decoders are treated as untrusted-quality code (constitution Principles I and IX). A
  decoder that fails, panics, hangs or uses too much memory is stopped, reported in plain words
  with a stable error code, and cannot damage the capture or any other decoder.
- **Smoothness**: decoding runs in the background and can be cancelled; the window never freezes,
  also while the capture is still growing; and drawing annotations of a capture of one hundred
  million samples is as fast as drawing a small one.
- **Persistence**: the decoder setup is saved with the session and comes back when the file is
  opened; the decoded results can be saved as well, if the user wants them.

This feature also fills the places that feature 006 reserved for it (the decoder lane area, a side
panel and menu entries, FR-037 of that feature), so later decoders add themselves without changes to
the application shell.

Out of scope: real protocol decoders (feature 008 onward), exporting the decoded log (feature 009),
decoders written by users as scripts or plugins, a decoder marketplace, importing PulseView or
sigrok decoders, and the gaps of feature 006 that decoding does not need (a scroll bar, selecting a
time range with the mouse, labels for assistive technology).

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Decode a signal and see what it says (Priority: P1)

A user captures from the virtual device (or opens a capture), adds the sample decoder, tells it which
channel carries the clock and which the data, and watches the result appear: a lane below the
channels with the decoded bytes as labelled ranges, arrows on the clock edges where each bit was
read, and a coloured dot where a byte starts. Zooming in on a byte shows its bits and the arrows on
the exact edges; zooming out keeps the lane readable.

**Why this priority**: this is the point of the whole feature. Everything else (stacking, saving,
speed, safety) makes sense only once a user can decode something.

**Independent Test**: with no hardware, choose the virtual device, configure it to generate for the
sample decoder (User Story 6), capture, add the sample decoder with the right channels, and check
that the decoded bytes equal the bytes the generator produced, that each appears as a range in the
decoder lane with its value, and that each bit has an arrow on the clock edge it was sampled on.

**Acceptance Scenarios**:

1. **Given** a capture with a clocked bit stream on two channels, **When** the user adds the sample
   decoder and assigns the clock and data channels, **Then** annotations appear: one range per byte
   in the decoder lane, one point per bit, and one edge arrow on the clock channel per sampled bit.
2. **Given** a decoder with annotations, **When** the user zooms in on one byte, **Then** the bits,
   their values and the arrows on the sampling edges are visible and line up with the waveform.
3. **Given** a decoder with annotations, **When** the user zooms out so that many annotations share
   a pixel, **Then** the lane shows that there is activity (a dense mark) instead of hiding
   annotations, and zooming in reveals every one of them.
4. **Given** the decoder dialog, **When** the user assigns a channel that does not exist, assigns the
   same channel to two signals the decoder needs different, or leaves a required signal empty,
   **Then** the choice is refused before decoding starts, with a message that says which signal is
   wrong and what is allowed.
5. **Given** a decoder with annotations, **When** the user clicks an annotation, **Then** its details
   (kind, value, start, end and length in time) are shown and the view can jump to it.
6. **Given** a decoder, **When** the user removes it, **Then** its lane, overlays and results
   disappear and the capture is unchanged.

---

### User Story 2 - A broken decoder never harms the session (Priority: P1)

Decoders are untrusted-quality code. When one fails (an error, a panic, an endless loop, a runaway
memory use, or output that breaks the rules of the annotation model) the user sees one clear message
that names the decoder and what went wrong, the annotations produced before the fault are kept and
marked as incomplete, every other decoder keeps working, and the capture is exactly as it was.

**Why this priority**: the constitution makes this non-negotiable (Principles I and IX): a fault in
one subsystem must never take down the application or discard the session. A framework that cannot
promise it is not allowed to ship.

**Independent Test**: install test decoders that fail in every way listed in the constitution (error
result, panic, no response to cancellation, too much memory, invalid annotations) next to a healthy
decoder; run them all on the same capture, and check the capture bytes, the healthy decoder's
results, the messages and the log.

**Acceptance Scenarios**:

1. **Given** a decoder that panics half way, **When** it runs, **Then** the application keeps
   running, one localized message with a stable error code appears, the annotations before the
   panic stay (marked incomplete), and nothing else changes.
2. **Given** a decoder that never returns, **When** it runs, **Then** it is cancelled and abandoned
   within a bounded time, reported as not responding, its results are discarded or kept as
   incomplete, and the interface never froze.
3. **Given** a decoder that produces too many annotations or uses too much memory, **When** it
   passes its budget, **Then** it is stopped with a message that says the limit was reached; what it
   produced up to then is kept and marked incomplete.
4. **Given** a decoder that produces an invalid annotation (an end before its start, a position
   outside the capture, a channel that does not exist, text that is too long), **When** the
   framework receives it, **Then** the annotation is refused and counted, the decoder is reported
   once, and the rest of its valid output is kept.
5. **Given** two decoders on the same capture and one of them fails, **When** the user looks at the
   other, **Then** its results are complete and unchanged.
6. **Given** any decoder fault, **When** the user saves the capture, **Then** the saved capture data
   is identical to what it would have been without any decoder.

---

### User Story 3 - A very large capture stays smooth, also while it grows (Priority: P1)

Decoding a capture of one hundred million samples does not freeze the window: the user can zoom,
pan, place markers, save or cancel while the decoder works, sees how far it has come, and sees the
results appear progressively. Once decoded, drawing the annotations costs the same however long the
capture is. While a capture is still running, the decoder follows it and annotates what has arrived.

**Why this priority**: constitution Principles IV, VIII and IX. A decoder that makes the interface
stutter on a big capture would be unusable exactly when decoding is most wanted.

**Independent Test**: generate a capture of one hundred million samples on 16 channels with a
protocol-valid signal; add the decoder; measure the longest time the interface is unresponsive
during decoding, cancelling and zooming, and measure frame times while panning with the full
annotation set; repeat with a ten times longer capture.

**Acceptance Scenarios**:

1. **Given** a large capture, **When** the decoder runs, **Then** progress is shown, the window stays
   responsive, and annotations appear progressively from the start of the capture.
2. **Given** a decoder that is running, **When** the user presses Cancel, **Then** it stops within
   one second, the annotations produced so far are kept and marked incomplete, and the user can start
   it again.
3. **Given** a finished decode of a large capture, **When** the user zooms and pans continuously,
   **Then** frame times meet the same budget as drawing the waveforms alone and do not grow with the
   length of the capture.
4. **Given** a capture that is still running, **When** a decoder is added or already present,
   **Then** it decodes the data that has arrived, extends its annotations as more arrives, and the
   capture loses no samples because of it.
5. **Given** annotations that overlap, nest, or sit on the same position, **When** the view is
   drawn, **Then** none is lost and none hides another permanently (for example nested ranges are
   shown on separate rows).
6. **Given** the user changes the decoder's channels or options while it runs, **When** the change
   is accepted, **Then** the running pass is cancelled and a new pass starts; old and new
   annotations are never mixed.

---

### User Story 4 - Stack decoders (Priority: P2)

A higher-level decoder can take the output of a lower-level one as its input: the user adds the
lower decoder, then adds a decoder on top and chooses the lower one as its source. The upper decoder
shows its own lane and its own annotations, and it updates when the lower one does. The framework
enforces sensible rules: a decoder can only be stacked on one that produces what it needs, there are
no cycles, and a fault in the lower decoder marks the upper one as based on incomplete data without
damaging it.

**Why this priority**: constitution Principle IX requires stackable decoders and feature 010 builds
the first real stack on this. It ranks below the single-decoder flow because with one shipped decoder
there is nothing to stack yet; it is verified with test decoders and ready for 008 and 010.

**Independent Test**: install two test decoders, a lower one that produces bytes and an upper one
that consumes bytes; stack them; check that the upper one sees exactly what the lower one produced,
that removing or changing the lower one updates or removes the upper one, and that a cycle or a
mismatched stack is refused.

**Acceptance Scenarios**:

1. **Given** a lower decoder with output, **When** the user stacks an upper decoder that accepts that
   kind of output, **Then** the upper decoder receives the lower decoder's output and produces its
   own annotations.
2. **Given** a stack, **When** the lower decoder is changed, re-run or removed, **Then** the upper
   decoder is re-run or removed accordingly, never left showing stale results.
3. **Given** a stack, **When** the lower decoder fails, **Then** the upper decoder processes what the
   lower produced before the fault, its results are marked as based on incomplete data, and a
   message says which one failed.
4. **Given** the stacking choices, **When** the user tries to stack a decoder on a source that
   produces something it cannot read, or to create a loop, **Then** the choice is refused with a
   message that says why.
5. **Given** a stack, **When** the user looks at the lanes, **Then** each decoder has its own lane,
   in stack order, with its name, and can be hidden or removed without affecting the others
   (removing a lower decoder asks about the upper ones).

---

### User Story 5 - Save and open a capture with its decoders (Priority: P2)

The decoder setup (which decoders, their channels, options and stacking) is saved in the session file
and restored when the file is opened. The user can also choose to save the decoded results, so a very
large capture shows its annotations at once when reopened instead of decoding again. Files from
before this feature open as they always did, and files with a decoder section written by a newer
version open with their decoder setup preserved untouched.

**Why this priority**: part of not losing the user's work (Principle I). It ranks below decoding
itself because a decode can always be repeated.

**Independent Test**: configure a stack of decoders on a capture, save, close, reopen and compare the
setup; repeat with results saved; open a file from before this feature; open a file with a damaged
decoder section and one written by a newer version.

**Acceptance Scenarios**:

1. **Given** a capture with decoders, **When** the user saves and reopens it, **Then** the same
   decoders are there with the same channels, options and stacking, and decoding starts again in the
   background.
2. **Given** the option to save results, **When** the user saves and reopens, **Then** the
   annotations are shown at once and are identical to those that were saved; they are marked as
   loaded from the file until a new decode replaces them.
3. **Given** a session file written before this feature, **When** it is opened, **Then** it opens
   exactly as before, with no decoders.
4. **Given** a decoder section that is damaged, invalid, or names a decoder this version does not
   have, **When** the file is opened, **Then** the capture opens, the rest of the decoder setup that
   is valid is kept, a message explains what was skipped, and the invalid part is not written back
   as if it were valid (an unknown decoder's setup is kept untouched so that a newer version can
   read it).
5. **Given** a save that is interrupted at any point, **When** the previous file is opened, **Then**
   it is intact, as for every save in this product.

---

### User Story 6 - Try decoding without hardware, with a generator (Priority: P2)

The virtual device can generate signals that are valid for the sample decoder, so everything in this
feature can be tried without hardware. With the sample decoder configured, the generated signals
decode back to exactly the content that was generated; the user can supply a seed to repeat a
capture, and can ask for deliberate errors to see how the decoder reports them.

**Why this priority**: constitution Principle XI requires a generator for every decoder that can be
used with the virtual device, with round-trip tests. It is P2 because it is needed to demonstrate and
test the framework, not as a goal by itself.

**Independent Test**: for many seeds, generate content, encode it to channel signals, decode it, and
compare; do the same with deliberate errors and check that the errors are reported where they were
planted.

**Acceptance Scenarios**:

1. **Given** the virtual device with the sample decoder's generator selected and a seed, **When** a
   capture runs, **Then** the signals are valid for the sample decoder and decoding them yields
   exactly the generated bytes.
2. **Given** the same seed and configuration, **When** the capture runs twice, **Then** the signals
   are identical.
3. **Given** deliberate errors switched on, **When** the capture is decoded, **Then** the planted
   errors are reported as error annotations at the right places and the valid content around them is
   still decoded.
4. **Given** the generator is running, **When** the user stops the capture, **Then** it stops at once
   without blocking the interface.
5. **Given** no decoder is selected for the generator, **When** the virtual device runs, **Then** it
   behaves as before (generic random signals).
6. **Given** a capture from the generator, **When** it is shown or saved, **Then** it is labelled as
   simulated everywhere, as every virtual-device capture is.

---

### User Story 7 - Read the annotations comfortably and in my language (Priority: P3)

Annotations are legible at every zoom level, in all six languages, and at the smallest window size.
Names of decoders and their options, the messages about them, and the standard annotation texts of
built-in decoders follow the language chosen in the settings and change at once when it changes;
text that comes from outside the application (a decoder's literal text) is shown as it is.

**Why this priority**: required by the constitution for every feature; it ranks last because the
flows above work in English first.

**Independent Test**: switch through all six languages with decoders and annotations on screen, at the
minimum window size, and check every decoder-related text; run the layout check for the decoder
dialog, lane and panel.

**Acceptance Scenarios**:

1. **Given** decoders on screen, **When** the user changes the language, **Then** decoder names,
   option labels, panel texts, messages and the standard texts of built-in annotations change at
   once; literal text supplied by a decoder is unchanged.
2. **Given** the smallest supported window with the longest German texts and Japanese text, **When**
   the decoder dialog, lane labels and panel are shown, **Then** nothing is clipped, overlaps or
   lacks glyphs.
3. **Given** a long annotation text and a narrow range, **When** the range is too small for its text,
   **Then** the text is shortened with an ellipsis, the full text is available when the pointer or the
   focus is on it, and neighbouring annotations are not overlapped.

---

### Edge Cases

- A decoder is added to an empty capture, or to a channel that has no transitions: it finishes at
  once with no annotations and no error.
- The capture has gaps (lost samples) or ended abnormally: decoders see the gaps as regions without
  data, report "incomplete here" annotations instead of guessing, and resume after the gap.
- The capture grows while a decoder is running or after it finished: the decoder continues from where
  it stopped; the user never sees the same annotation twice or one that was removed.
- A decoder's channel is hidden (feature 006's flag) or not recorded: the decoder still reads the
  data when it exists; for a channel that has no data in the capture the decoder reports that it has
  nothing to read.
- Channels are renamed or reordered: the decoder setup follows the channel number, not the name or
  position; the setup shows the current name.
- The user adds a decoder while another decode of the same capture is running, or removes it during a
  save: the operations are ordered safely, nothing is lost, and a save writes a consistent state.
- Millions of annotations: memory stays within the decoder results limit; when it is reached, the
  decoder stops with a message and keeps what it has; the limit is visible and changeable in the
  settings.
- An annotation is longer than the capture, ends exactly at the end, or has zero length: refused,
  accepted as a point, or shown at the edge, as specified for each kind.
- Annotation text contains control characters, bidirectional characters or is extremely long: it is
  sanitized and limited for display like every outside text.
- A decoder name or option in a session file refers to a version that changed its options: unknown
  options are kept untouched, missing options take their defaults, and a message is shown.
- The decoder lane area has no decoders: it takes no space, as before this feature.
- Closing the window or starting a new capture while decoders are running: the unsaved-data question
  of feature 006 applies; decoding is cancelled cleanly and never blocks closing.
- A decoder is added to a capture opened from a file that was saved with results: the new decode
  replaces the loaded results only when it finishes, and the old ones stay visible until then.
- The user has no decoders: nothing about this feature appears except the menu entry to add one.

## Requirements *(mandatory)*

### Functional Requirements

**Decoders and their setup**

- **FR-001**: The application MUST provide a decoder framework in which a decoder declares what it is
  (a stable identifier, a name, a description), which signals it needs (each required or optional,
  with a role such as "clock" or "data"), which options it has (each with a type, allowed values and a
  default), which kinds of input it accepts (channels of the capture, or the output of another
  decoder), and which kinds of output it produces.
- **FR-002**: The user MUST be able to add a decoder to the capture on screen, assign a channel to each
  of its signals, set its options, rename its instance, hide or show its lane, and remove it; every
  choice MUST be validated before decoding starts (an unknown channel, a channel used for two roles
  that need to differ, a missing required signal, an option out of range) and a refusal MUST say
  which setting is wrong and what is allowed.
- **FR-003**: The application MUST allow several decoder instances on one capture (the same decoder
  more than once on different channels, and different decoders), up to a limit that is stated and
  enforced with a message when reached.
- **FR-004**: Decoders MUST be defined by this project: the decoder interface MUST be documented in
  English as a versioned contract (constitution Principle IX) and MUST NOT depend on the format of
  PulseView, sigrok or Logic 2; the framework MUST NOT contain any code of those projects.
- **FR-005**: The framework MUST be independent of the user interface toolkit and testable without a
  display: decoders, the annotation store, the execution and the setup are plain data and logic in the
  core, reached by the interface only through the host API contract (constitution Principle IV).

**Annotations**

- **FR-006**: The annotation model MUST support at least: a range in time, a point in time, an edge
  marker with a direction (rising or falling), a target (the decoder's own lane, or a channel of the
  capture for an overlay), a style (colour and shape chosen from a defined set), and text; and it MUST
  be independent of any drawing technology.
- **FR-007**: A decoder MUST be able to place an annotation in its own lane (ranges with text, points,
  packets) and, independently, as an overlay on a channel (for example a dot on one channel at a
  frame start, an arrow on the clock edge at which a bit was sampled), and one event MAY produce both.
- **FR-008**: An annotation's text MUST be either a localizable message (a key from the application's
  resources with arguments), used by built-in decoders so that it follows the user's language, or
  literal text, used by anything else and shown as it is, sanitized and length-limited like every text
  from outside the application; error annotations MUST be marked as errors so they can be styled and
  counted.
- **FR-009**: Every annotation MUST be validated when it enters the store: positions inside the
  capture, an end not before its start, a target that exists, a style from the defined set, text within
  its length limit; an invalid annotation MUST be refused and counted, never stored and never allowed to
  break the store or the drawing.
- **FR-010**: Annotations MUST be stored per decoder instance in an indexed form that gives, for a
  time window, the annotations that intersect it in time logarithmic in their number plus the number
  returned, without scanning the others (constitution Principle IX, "indexed lookup"); the index MUST
  support ranges that overlap, nest and share positions.
- **FR-011**: The store MUST keep its memory within an explicit limit, visible and changeable in the
  settings; reaching the limit MUST stop the decoder with a handled message and keep what was
  produced, marked incomplete (constitution Principle IV).

**Execution**

- **FR-012**: A decoder MUST run in the background and MUST be cancellable at any moment; cancelling
  MUST take effect within one second, MUST keep the annotations produced so far (marked incomplete),
  and MUST leave the decoder ready to run again.
- **FR-013**: Decoding MUST report progress (how much of the capture has been processed) and MUST NOT
  make the interface unresponsive for more than 100 milliseconds at any moment, on a capture of one
  hundred million samples (continuing feature 006's FR-032).
- **FR-014**: Decoding MUST read the capture only through the storage's indexed, snapshot-based access
  (constitution Principle VIII's storage), MUST NOT copy the whole capture, and MUST NOT be able to
  change the capture; the decoder's view of the data is read-only by construction.
- **FR-015**: A decoder MUST handle a capture that is still growing: it decodes what has arrived,
  continues from where it stopped as more arrives, and the capture's ingestion MUST NOT lose samples or
  slow down because of it (the capture's priority over decoding is absolute).
- **FR-016**: Decoders MUST see gaps (lost samples) and an abnormal end of the capture as explicit
  regions without data; they MUST NOT guess across a gap, and the framework MUST mark the results
  around a gap as incomplete.
- **FR-017**: A change that makes results invalid (different channels, different options, a changed
  lower decoder) MUST cancel the running pass and start a new one, and results of the old and the new
  setup MUST never be mixed; the old results stay visible until the new ones replace them or are
  removed, and are marked as outdated meanwhile.

**Stacking**

- **FR-018**: A decoder MUST be able to take the output of another decoder instance as its input;
  the framework MUST check that the lower decoder produces the kind of output the upper one accepts,
  MUST refuse cycles and a stack deeper than a stated limit, and MUST give each decoder in a stack its
  own lane and annotations.
- **FR-019**: A change to a lower decoder (new results, a new pass, removal, a fault) MUST propagate to
  the decoders above it: they re-run, are removed after confirmation, or are marked as based on
  incomplete data; the upper decoders MUST never present results that no longer match their source
  without marking them as outdated.

**Fault isolation**

- **FR-020**: A decoder's error result, panic, failure to respond, excessive memory use, excessive
  annotation count and invalid output MUST each be contained at the decoder's boundary: reported to
  the user as one message with a stable error code naming the decoder and what happened, logged with the
  code and technical detail, and MUST NOT affect the capture, the unsaved-data protection of feature
  006, or any other decoder (constitution Principles I, VI and IX).
- **FR-021**: A decoder that does not stop when asked MUST be abandoned after a bounded time: its late
  output MUST be ignored, the decoder instance MUST be marked as failed, and the interface MUST never
  wait for it.
- **FR-022**: The results produced before a fault MUST be kept and marked incomplete, so partial
  information is not thrown away (constitution Principle I), and a failed decoder MUST be restartable by
  the user.
- **FR-023**: Messages about decoders MUST NOT flood the user: repeats of the same fault are grouped as
  in feature 006, and the log MUST record decoder lifecycle and every fault with its code and technical
  detail, never signal data and never the text of annotations or user-supplied names.

**Interface**

- **FR-024**: The decoder lane area of feature 006 MUST show one lane per visible decoder instance,
  below the channels, in stack order, with the decoder's name and a way to hide, show and remove it; the
  area MUST take no space when there are no decoders and MUST scroll together with the channels
  according to the layout contract of feature 006.
- **FR-025**: Overlay annotations MUST be drawn on the channel they target, on top of the waveform,
  without hiding the waveform and without changing the cost of drawing it; overlays of a hidden channel
  are hidden with it.
- **FR-026**: The side area of feature 006 MUST get a decoder panel that lists the instances with their
  state (waiting, running with progress, finished, incomplete, failed, outdated), and offers adding,
  configuring, cancelling, re-running and removing; the menu MUST get the matching entries, each
  reachable from the keyboard (continuing feature 006's FR-036).
- **FR-027**: Zoomed-out annotations MUST NOT be silently dropped: where several annotations share a
  pixel the drawing MUST show that there is more than one (a dense-activity mark), error annotations MUST
  stay visibly distinct, and zooming in MUST reveal every annotation (constitution Principle VIII).
- **FR-028**: The user MUST be able to point at an annotation to see its details, click it to select it,
  and jump the view to it (centred, with a zoom that shows it whole); annotations MUST be reachable from
  the keyboard (next and previous annotation of a decoder).
- **FR-029**: Registering a decoder with the application MUST be enough for it to appear in the "add
  decoder" list, its dialog and its lane; the application shell MUST NOT need a change to show a new
  decoder (closing feature 006's FR-037 for decoders).
- **FR-030**: Every user-facing text of this feature MUST come from the localization foundation in all
  six languages (drafts acceptable, marked), with no key shown to the user; decoder names, descriptions,
  option labels and the standard annotation texts of built-in decoders MUST be localized, while literal
  text from a decoder is not (constitution Principle VII and Localization Requirements).

**Sessions**

- **FR-031**: The decoder setup (instances, decoder identifiers and versions, channel assignments,
  options, names, stacking, lane visibility) MUST be saved in the session file in an optional, versioned
  section of the session format (the `DECO` section reserved by feature 003), written only when there
  are decoders, and read back when the file is opened.
- **FR-032**: The user MUST be able to choose whether decoded results are saved with the setup; saved
  results MUST be shown at once on opening, be identical to what was saved, be marked as loaded from
  the file, and be replaced when a new decode of the same instance finishes; when results are not saved
  the decoders run again on opening.
- **FR-033**: Opening a file MUST NOT fail because of the decoder section: a damaged or invalid section,
  a decoder this version does not have, or options it does not understand MUST open the capture with the
  valid part of the setup, a message that says what was skipped, and MUST keep the parts it did not
  understand untouched when the file is saved again (so that a newer version can read them); a file
  without the section MUST open as before.
- **FR-034**: Files MUST remain atomic and validated as before; loading the decoder section MUST treat
  every byte as untrusted (bounded sizes, no panic, typed errors) and MUST be covered by tests with
  malformed, truncated and mutated input (constitution Principles I and III).

**Sample decoder and generator**

- **FR-035**: The feature MUST ship one built-in sample decoder that reads a clock channel and a data
  channel and decodes the data bits sampled on a chosen clock edge into bytes, with options for the
  clock edge, the bit order and the number of bits per byte, producing: a range per byte in its lane, a
  point per bit, an arrow on the clock edge of each sampled bit, a dot at the start of each byte, and
  error annotations for an incomplete byte and for samples that arrive during a gap. Its purpose is to
  prove the framework end to end; it is not a protocol that is meant to be used in the field.
- **FR-036**: The sample decoder MUST have a signal generator for the virtual device as its counterpart
  (constitution Principle XI): it produces random but valid content from a seed, encodes it to the
  clock and data channels, can deliberately plant the errors the decoder reports (incomplete bytes), is
  reproducible (the same seed and configuration give identical signals), does not block the interface,
  is cancellable, and the virtual device keeps behaving as before when no decoder is selected for it.
- **FR-037**: A round-trip test MUST decode generated content and compare it with the content that was
  generated, for many seeds and with planted errors (constitution Principle XI); the decoder MUST also
  have tests with recorded signals including malformed and truncated input (constitution Principle IX).
- **FR-038**: Captures from the generator MUST be labelled as simulated in the interface and in saved
  files, as every virtual-device capture, and decoder results MUST NOT remove or hide that label.

**Architecture and conventions**

- **FR-039**: The decoder contract MUST be documented in English and versioned together with the host
  API (new commands, state and events for decoders extend the host API contract of feature 006 and its
  version); a change to either MUST land with the implementation and the consuming interface in one
  merge, and a test MUST check that the documents and the code agree.
- **FR-040**: The feature MUST add no network access, no telemetry and no execution of code that comes
  from a file or from the user: decoders are built into the application in this feature.
- **FR-041**: Any code, algorithm or test data taken from another project MUST be recorded and handled
  as the constitution (Principle XII) requires; the default is a fresh implementation from the
  protocol knowledge.
- **FR-042**: The decoder panel, lane and dialog MUST follow the layout rules of feature 006 (minimum
  window size, long German text, Japanese text, scaling, keyboard operation) and MUST add no
  processor use while nothing changes on screen.

### Key Entities *(include if feature involves data)*

- **Decoder**: a built-in definition of how to read one kind of signal pattern: identifier, version,
  name, description, signals it needs (role, required or optional), options (type, allowed values,
  default), accepted input, produced output. Defined by this project; independent of other products.
- **Decoder instance**: one use of a decoder on a capture: which decoder, its name, channel assignments,
  option values, its source when stacked, whether its lane is shown, and its state (waiting, running,
  finished, incomplete, failed, outdated, loaded).
- **Annotation**: one result of a decoder: a kind (range, point, edge marker), a position or span in
  time, a target (the decoder's lane or a channel), a direction for edge markers, a style, text (a
  localizable message or literal text), an optional error mark, and its decoder instance.
- **Annotation store**: all annotations of one instance, indexed by time for logarithmic window lookup
  and bounded in memory; knows which regions are incomplete.
- **Decode pass**: one run of an instance over the capture: progress, the part of the capture it has
  covered, cancellation, and how it ended.
- **Decoder stack**: the relationships between instances (an upper instance reads the output of a lower
  one); acyclic, with a depth limit.
- **Decoder fault**: an error, panic, non-response, memory or count overrun, or invalid output of one
  instance, with its stable error code and technical detail; contained at the instance.
- **Decoder setup**: the saved part of the instances (identifiers, versions, channels, options, names,
  stacking, visibility) and, optionally, their results, as stored in the session file.
- **Generator**: the counterpart of a decoder for the virtual device: produces valid, seeded, optionally
  deliberately faulty signals for the channels the decoder reads.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: A user with no hardware and no documentation adds the sample decoder to a virtual-device
  capture and sees decoded bytes, bit points and clock-edge arrows in under 60 seconds from the start of
  the application, in each of the six languages.
- **SC-002**: With a finished decode of a capture of one hundred million samples over 16 channels and at
  least one million annotations, every frame during continuous zoom and pan is produced in under 16 ms at
  the 95th percentile and under 50 ms in the worst case, and the frame time for the same view of a ten
  times longer capture differs by less than a factor of two.
- **SC-003**: In an automated check over planted annotations, 100 % of zoom levels from the whole
  capture to one sample per many pixels show every planted annotation either on its own or as part of a
  dense-activity mark, including a single one-sample annotation among millions.
- **SC-004**: The interface never stops responding for more than 100 ms during decoding, cancelling,
  zooming, panning, saving and a language change on a capture of one hundred million samples, on both
  supported operating systems; cancelling a running decode takes effect within one second.
- **SC-005**: For every kind of decoder fault (error result, panic, no response to cancellation, memory
  overrun, annotation count overrun, invalid output), injected one at a time, 100 % of the runs end with
  the capture byte-identical to the original, every other decoder's results unchanged, exactly one
  localized message with a stable error code, and a log entry carrying the code.
- **SC-006**: A round trip (generate, encode to signals, decode, compare) succeeds for at least 10,000
  seeds, with and without planted errors, and every planted error is reported at the position where it
  was planted.
- **SC-007**: A randomized sequence of at least 100,000 user actions that includes adding, configuring,
  stacking, cancelling, re-running and removing decoders, saving, opening (including damaged decoder
  sections), capture growth and language changes produces no crash, no hang, no lost capture data and no
  mixed old and new results.
- **SC-008**: A save followed by an open reproduces the decoder setup exactly (and the results exactly,
  when saved) over at least 100 interruption points of the save, and opening a file from before this
  feature, a file with a damaged decoder section and a file written by a newer version each succeed
  without losing the capture.
- **SC-009**: While a capture is running at the highest sample rate of the virtual device, the decoder
  shows annotations for data that arrived within one second of its arrival, and the capture loses no
  samples compared with the same capture without a decoder.
- **SC-010**: In the layout check with the longest German strings and Japanese text at the smallest
  supported window size, zero elements of the decoder dialog, lane and panel are clipped, overlap or lose
  their text, and zero characters are drawn as missing glyphs.
- **SC-011**: With decoders configured and finished, and no change on screen, the application's processor
  use stays under 1 % of one core.
- **SC-012**: A reviewer who did not write the decoder contract adds a new test decoder from the
  documentation alone, and the application shows it in the "add decoder" list with its dialog and lane
  without any change to the application shell.

## Assumptions

- The sample decoder is deliberately simple and generic ("clocked bits": data bits sampled on a clock
  edge and grouped into bytes). It is not a standard protocol so that it does not compete with the real
  decoders that follow (I2C in 008, higher-level decoders in 010, SPI, UART and others later); its
  generator, annotations and errors exist to exercise every part of the framework.
- Only one decoder is user-visible in this feature, so decoder stacking is verified with test decoders
  in the framework's tests; the stacking interface is complete and ready for 008 and 010.
- Decoders are built into the application. Decoders written by users as scripts or plugins are a
  separate, later decision and are outside this feature (and need their own isolation design, such as a
  separate process).
- Isolation is by containment in the application (a worker that is watched, with a time and memory budget
  and cancellation checks), consistent with the existing contained workers of the product; a stronger
  sandbox is outside this feature.
- A decode reads the capture through the read-only, snapshot-based access of the signal storage and the
  channels of the capture; it never writes to it. Decoding of a growing capture works on the part that
  has arrived, and the capture's ingestion has priority.
- The memory limit for decoder results is a separate, visible setting with a sensible default and the
  same kind of validation as the capture's memory limit; the number of instances (default limit: 8) and
  the stack depth (default limit: 4) are stated and enforced.
- Decoded results are saved only if the user chooses it (default: off), because they can be recomputed;
  the decoder setup is always saved.
- The window and interface of feature 006 are the base. This feature fills the decoder lane area, the
  side panel and the menu entries that 006 reserved (its FR-037) and adds only the interaction decoders
  need (selecting an annotation, jumping to it). The other gaps of 006 (a scroll bar, selecting a range
  with the mouse, labels for assistive technology) are outside this feature.
- Standard annotation texts of built-in decoders are localized; text from any other source is literal
  and shown as it is, sanitized and length-limited, as the constitution requires.
- The decoder contract is documented in English next to the host API contract and versioned with it;
  compatibility with PulseView, sigrok or Logic 2 decoders is not required (constitution Principle IX).
- The roadmap order holds: this feature depends on 002 (storage), 004 (devices and the virtual device),
  005 (localization) and 006 (the application), all merged; decoded log export (009) and the real
  decoders (008, 010) follow and build on it.
