# Contract: Decoder lanes, overlays, panel and dialog (UI)

**Status**: extends [feature 006's shell layout](../../006-ui-shell-signal-view/contracts/shell-layout.md)
in the places it reserved (the decoder lane area, the side panel, the menu). Layout rules there (minimum
window 800 × 560, long German text, Japanese, scaling, keyboard, no processor use when nothing changes)
apply unchanged.

## Where things go

```text
+-----------------------------------------------------------------------------------+
| menu: File  Capture  View  Markers  Decoders  Help                                |
| toolbar (device, rate, length, seed, signal, real time, Start)                    |
+-------------------------------------------------------------+---------------------+
| time axis                                                   | Markers panel       |
| channel lanes ... with overlays (dots, arrows) on top       |---------------------|
|                                                             | Channels panel      |
|-------------------------------------------------------------|---------------------|
| decoder lanes: one per visible instance, in stack order,    | Decoders panel      |
| each with its name in the label column                      | (instances, state,  |
|                                                             |  selected annotation)|
+-------------------------------------------------------------+---------------------+
| status line                                                                       |
+-----------------------------------------------------------------------------------+
```

- The **decoder lanes** are drawn by the same canvas as the channel lanes, below them, sharing the time
  axis. The region takes **no space** when there are no visible decoder lanes (as in 006). With many
  channels and lanes the lane area scrolls together with the channel lanes (the label column scrolls with
  them; the axis stays fixed).
- A lane has up to 4 rows (R5); its label column shows the instance name (user text, shown as typed) and the
  state: a thin progress bar while running, a "!" mark for incomplete, a "faded" look for outdated or
  loaded. A failed instance shows its error code's message in the panel, not in the lane.
- **Overlays** are drawn on the channel lane they target, over the waveform: dots, arrows on edges and
  dense marks, in the instance's colour. They never change the waveform's cost or cache (the waveform layer
  is not invalidated by annotation changes).
- The **Decoders panel** is the third panel of the right-hand column. It lists the instances (name,
  decoder, state, progress, counts), has **Add decoder…**, and per instance: configure, run, cancel, show
  or hide the lane and remove (lanes are in stack order: an instance after the one it reads from, so
  they are not moved by hand). Below the list it
  shows the **selected annotation** (kind, text, start, end, length, error).
- The **Add decoder** dialog is built from the catalogue (data): a list of decoders with their
  descriptions. The **configuration form** is built from a descriptor: a channel chooser per signal
  (names as set in the Channels panel), a control per option (a chooser for choices, a number field with its
  range for integers, a check box for booleans), the stack source (a chooser of instances or the
  capture), an instance name, and, when the device is simulated and the decoder has a generator, the
  generator options. Invalid choices are refused with the message of `decoder.invalid_setup` under the
  field. Nothing in the dialog is specific to one decoder (SC-012).

## Interaction

| Action | Pointer | Keyboard |
|--------|---------|----------|
| Hover an annotation | a readout near the pointer with the text and the time | |
| Select an annotation | click it | Next and previous annotation of the selected instance |
| Jump to the selection | double-click it, or the "go to" button of the panel | a key for "go to selected annotation" |
| Add, configure, run, cancel, remove | the panel and the **Decoders** menu | every entry has an accelerator or is reachable with the menu keys |
| Hide or show a lane | the eye control in the lane label | via the panel |

## Drawing rules (what is guaranteed on screen)

1. Nothing is dropped silently: where several annotations share a pixel column a **dense mark** with the
   count is drawn; an error annotation is drawn in the error style and a dense mark that contains errors
   says so (FR-027).
2. A range's text is clipped to its width with an ellipsis; if the range is narrower than three characters
   no text is drawn and the range is a bar; the full text is in the readout.
3. Colours come from a fixed set of 8 per theme (light, dark, high contrast) chosen so that overlays are
   legible over the waveforms; shape and position carry the same information as colour (no information by
   colour alone).
4. A lane never overlaps its neighbours; rows do not overlap; text never overlaps text of a neighbouring
   cell (the later one is shortened).
5. Redraws are driven by changes (new results, zoom, pan, resize, theme, language): a finished decode with
   no change on screen costs no processor time.

## Texts

Decoder names, descriptions, signal and option labels, the standard annotation texts of built-in
decoders, state names, panel and dialog texts, the menu and the messages of the `decoder` error codes are
messages in all six languages (static keys; the descriptor lists the decoder's own keys). Instance names
and literal annotation text are user or outside text: shown as they are, sanitized, never translated. The
text budget test (feature 006) covers the new menu, button, label and title messages.
