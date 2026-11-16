# User guide

Logic Analyzer records digital signals and shows them on a timeline you can zoom and measure. This
guide covers what the application does today. It works with the simulated device that is always
available; drivers for real hardware come in later features.

Supported systems: Windows 10 and 11, and Debian 13.

## Starting

Run `logic-analyzer` (after `cargo build --release -p la-app` the program is
`target/release/logic-analyzer`). The window opens with the signal area in the middle, a panel and a
strip of icons at the right edge, and a status line at the bottom. The window and the executable carry the application icon, and session files have an icon
of their own once an installer registers them (`packaging/`). The first
start uses your system language when it is one of English, German, French, Spanish, Japanese or
Ukrainian, and English otherwise; you can change it at any time in the settings.

## Devices and capturing

The **Device** list shows every device the application can use. **Virtual device (Simulated)** is
always there: it produces repeatable pseudo-random signals, so you can try everything without
hardware. A simulated capture is marked as such in the status line and in saved files, and the mark
cannot be removed.

Set up a capture in the **Capture** view of the panel at the right edge (see below):

| Control | Meaning |
|---------|---------|
| Sample rate (Hz) | samples per second |
| Length (seconds) | when the capture stops by itself; type seconds such as `2.5` or `0.001` (a point or a comma, whatever the language; at most nine digits after it) and the number of samples is shown beside it. Changing the sample rate keeps the seconds and recalculates the samples |
| Seed | the simulated device repeats exactly the same signal for the same seed; empty means random |
| Signal | how busy the simulated signal is: Sparse, Normal or Dense |
| Real time | the capture takes as long as the signal would; off fills the capture as fast as possible |

A setting the device cannot do is explained under the settings and **Start** stays unavailable until
it is fixed. **Start** (F5) begins; **Stop** (Shift+F5) ends a running capture early and keeps what
was recorded. The status line shows the state, the length, the memory in use against the limit, how
the capture ended, the sample rate, whether the data is simulated and whether there are unsaved
changes. A capture that lost samples or ended with an error is shown with its data and marked as
incomplete; it is never presented as a clean measurement.

While a capture runs, **Capture → Follow latest data** (F6) keeps the newest data in view; moving
the view by hand turns it off.

## Zoom and pan

| Action | Mouse | Keyboard |
|--------|-------|----------|
| Zoom in or out around the pointer | mouse wheel | `+` and `-` (around the centre) |
| Move the view | drag, or Shift and the wheel | Left and Right (a tenth of the view), Shift+Left and Shift+Right (a whole view) |
| Show everything | | Ctrl+0 |
| Jump to the start or the end | | Home and End |

The time axis shows ticks with units that follow the zoom, from nanoseconds to minutes. Zoomed out,
a stretch with many changes is drawn as a hatched band and a stretch with no data (a gap) as a
dimmed one; neither is ever drawn as a level. A single glitch stays visible at every zoom: if you
see nothing there is nothing, and if you see a band, zoom in and the edges appear.

## The panel at the right edge

A strip of three icons is always at the right edge: **Capture**, **Decoders** and **Markers**. Click
an icon to show that view in the panel next to it; click the icon of the view that is shown to hide the
panel and give the signals the room. Only one view is shown at a time. Your choice is remembered for
the next start.

### The Capture view

It holds **Start** (it becomes **Stop** while a capture runs), the device, the sample rate, the length
in seconds, the settings of the simulated device and the **channels to record**: one button for each
channel, eight to a row, with **All** and **None** beside **Reset channels** to switch them all at once. A button with only a frame
and a number in the channel's colour is **off**; a filled one is **on**. Click one to switch the
channel. Any combination works (for example channels 0, 2 and 5 only); with none on, **Start** stays
unavailable and the view says why. The buttons are locked while a capture runs, because what that
capture records cannot change. A channel that is off has no lane, and the next capture does not record
it.

### Editing a channel in the legend

The legend is the strip at the left of the signals, one row for each channel:

- **Click the coloured bar** at its left edge to step to the next colour of the palette.
- The **name** is the main text of a row and, under it, small, the **number of the analyzer input**
  (`D0`, `D1`, …), which does not change when you move the row, so you can always tell which wire a
  signal is on. A very low row shows the number at its right edge instead.
- **Click the name** to rename the channel in place: Enter accepts, Escape cancels. An empty name
  gives back the default one. Names are yours and are never translated.
- **Drag the name** up or down to move the channel; a line shows where it will land. Release outside
  the legend to cancel.
- **Drag the border** below a row to make the signal taller or shorter (24 to 240 pixels);
  **double-click** the border to give the height back to the program.
- The mouse wheel over the legend scrolls it when the signals do not fit.

Names, colours, order and heights can be changed at any time, before the first capture included, and
they stay for the next capture. They are saved with a capture and come back when you open it; a file
written by an older version opens with automatic heights. **Reset channels** (in the Capture view)
brings back the default names, colours, order and heights.

### Jumping to the next edge

Move the pointer to the left or right end of a signal: when that signal has a change beyond the
window on that side, a small button with an arrow appears. Click it and the view jumps, with the zoom
unchanged, so that the edge is a quarter of the window from the side it came from. Nothing appears
when there is no edge on that side. A change next to a gap (where samples were lost) is not counted
as an edge.

## Markers and measuring

Press **M** (or right-click a lane) to place a marker; at most 64 fit. With **Snap to transitions**
(S) a marker jumps to the nearest signal change of the lane it was placed on, within about twelve
pixels. Drag a marker's line to move it. In the **Markers** panel give each marker a name, choose
which two are measured (the buttons **1** and **2**), jump to one, or remove it (Delete removes the
selected one; **Markers → Remove all markers** clears them). **[** and **]** jump to the previous
and next marker.

With two markers chosen the panel shows the **distance** in time and its **frequency**, formatted for
your language. The frequency shows only as many digits as the distance really resolves.

## Decoders

A decoder reads the signals of the capture on screen and says what they mean. There are two:
**I2C**, which reads the two-wire bus (see "Decoding I2C" below), and the sample decoder **Clocked
bits**, which samples a data channel on every edge of a clock channel and groups the bits into bytes.
The rest of this chapter holds for both.

1. Capture (or open) signals, then choose **Decoders → Add decoder** in the menu (or press **D**, which
   opens that menu with the list of decoders) and pick a decoder by name. The list **Add decoder** in the
   **Decoders** panel does the same.
2. In the **Decoders** panel choose the channel for each signal (here **Clock** and **Data**) and, if
   you like, the options: which clock edge, the bit order, the bits per byte and the idle limit in
   samples. The decoding starts as soon as the channels are chosen, and again whenever a channel or
   an option changes.
3. A **lane** with the decoder's name appears below the channels. A range is a byte (its value in
   hexadecimal), a dot is a bit, and an arrow on the clock channel marks every sampled edge; a dot on
   the data channel marks the first bit of a byte. Annotations drawn in red report a problem in the
   signal, such as a byte that was cut short or samples that were lost. When there are too many
   marks for the width of the view to tell them apart, none of them is drawn (the ranges that are wide
   enough still are); zoom in and they appear.
4. Click an annotation to select it: the panel shows its text, position, length in time and whether it
   is an error. Click it twice to bring it into view. **F8** and **Shift+F8** select the next and the
   previous annotation, and **Ctrl+F8** goes to the selected one.
5. The check box **Show the lane** hides the lane and its overlays; the cross removes the decoder.

**Trying a decoder without hardware.** When the selected device is the simulated one, the virtual
device makes signals that are valid for the decoder on the channels you chose (the other channels stay
random), and the same seed gives the same signals. Add the decoder to a capture on screen, then start
the next capture: the decoder stays and decodes it from the start. Tick **Plant deliberate errors in the
simulated signals** in the decoder's block to cut some bytes short on purpose and look for the red
marks where they were planted.

### Decoding I2C

Choose **I2C** in the list, then choose the channels of **Clock (SCL)** and **Data (SDA)**.

- **The lane** shows every transaction as a range (`Write to 0x50`, `Read from 0x50`) with its
  address (`Address 0x50 (write)`), its data bytes (`Data 0x12`) and an **ACK** or **NAK** point for
  each byte; start, repeated start and stop are flags.
- **On the signals** a dot on the data line marks every start and stop, and an arrow on the clock line
  marks every clock edge at which a bit was read (nine for a byte: eight bits and the acknowledge).
  Zoom in on a byte to see them; at wide zoom they are not drawn, as for any decoder.
- **Address notation.** The **Address notation** option shows the 7-bit address (`0x50`, the usual
  way) or the 8-bit value with the direction bit as many data sheets write it (`0xA0` to write, `0xA1`
  to read). **Number base** shows hexadecimal, decimal or binary. Changing either decodes again; the
  old results stay, marked outdated, until the new ones replace them. A 10-bit address is shown as one
  address.
- **Errors**, in red: an address or a written byte that was not acknowledged (the NAK with which a
  read ends is normal and is not one), a byte cut short by a stop or a start, a start followed by a
  stop with no clock between, and samples lost in the middle of a transaction. A transaction that is
  not finished when the capture ends is drawn to the end with the text *Incomplete transaction*; it is
  not an error.
- **Why nothing is shown at the beginning.** The decoder first finds the speed of the bus from the
  clock pulses (you do not set it), then looks for the first start after the bus was idle for at least
  one clock period. Anything before it, such as a capture that begins in the middle of a byte or
  before the microcontroller has set up its I2C interface, is ignored without a mark. While a capture
  is still running and too few clock pulses have been seen to know the speed, the lane stays empty
  and fills in as soon as they have.
- **Without hardware.** With the simulated device, I2C channels carry valid traffic with random
  addresses, directions and data; tick **Plant deliberate errors in the simulated signals** to get
  unacknowledged bytes and cut-short bytes too. The same seed gives the same traffic.

A decoder that fails never harms anything else: its block says what happened and what to do, the
results it had found before the failure stay (marked incomplete), the other decoders and the capture
are untouched, and **Run again** starts it again. **Cancel** stops a running pass. A decoder that does
not answer in time is given up on after a few seconds; if several are stuck, restart the application.
The memory the results of all decoders may use is set in the settings (16 MiB to 4 GiB).

**Stacking.** A decoder that reads what another decoder produced (a higher-level protocol on top of a
lower one) has a **Reads from** chooser in its block, listing only what it can read. A chain is at most
four decoders deep and a decoder cannot read itself or one that reads it. When you run the lower
decoder again, the ones above show their old results (flagged as outdated) until the new ones arrive;
if the lower one fails, the results above are marked incomplete. Removing a decoder that others read
from asks first, because they go with it. Lanes are in stack order, lowest first.

**Saving.** The decoders you set up are saved with the session and decode again when it is opened. The
setting **Save decoded results with the session** also saves what they found, so that it shows at once
when the file is opened (the file is larger); press **Run again** to decode anew. A file from a newer
version keeps decoders this version does not have, and they are written back unchanged.

Decoding never changes the capture. A dashed line along the bottom of a lane means that its results
are not whole (the decoder was cancelled or stopped).

## Saving and opening

The program saves a capture with its channel names, colours, order, markers and the zoom in a
`.lasession` file. **File → Save** (Ctrl+S) writes to the current file, **Save as…**
(Ctrl+Shift+S) asks for a name, **Open…** (Ctrl+O) opens a file, and **Recent files** in the File
menu opens a submenu with the last fifteen, newest first, each with its folder below its name. A file
that is gone is shown dimmed with a marker and cannot be opened; **Remove unavailable entries** and
**Clear the list** tidy the list (the files themselves are never touched). A progress dialog with **Cancel** shows while a big file is written or
read.

Saving never damages an existing file: it writes a new file and replaces the old one only when the
new one is complete, so a full disk, a crash or a power failure leaves the previous file as it
was. A damaged or truncated file is refused with a message that says what is wrong, and what you
had open stays open. If only the part of the file that stores your channel names and markers is
damaged, the capture still opens with the defaults and a warning.

You are never silently asked to lose data: a new capture, opening a file or closing the window while
the capture is unsaved or still running first asks **Save**, **Discard** or **Cancel**. A running
capture is stopped and its data kept while you decide. The window title shows the file name and a
`*` while there are unsaved changes.

## Language and settings

**File → Settings…** (Ctrl+,) opens the settings:

- **Language**: the whole interface changes at once, with no restart. The choice is remembered. Your
  channel and marker names stay exactly as you typed them.
- **Theme**: follow the system, light, dark or high contrast.
- **Interface scale**: 75 % to 200 %; Ctrl+= and Ctrl+- change it from anywhere.
- **Memory limit of a capture**: in MiB; a capture that reaches it stops and keeps its data. A new
  limit applies to the next capture, never to one that is running.

The settings, the window size and position and the list of recent files are stored in
`settings.json` in the per-user configuration folder (`%APPDATA%\logic-analyzer\config` on Windows,
`~/.config/logic-analyzer` on Debian). If that file is damaged the defaults are used and you are
told.

## When something goes wrong

Every failure is shown in the red bar under the toolbar in your language, in plain words, with what
you can do about it. Repeated identical failures are counted instead of piling up, and the bar can
list recent messages. The technical details go to the log, never to the screen. **Help → Show log
location** opens the folder with the log files; attach the newest one to a bug report. The log never
contains signal data, channel names or marker names.

## Keyboard summary

| Key | Action |
|-----|--------|
| F5 / Shift+F5 | start / stop |
| F6 | follow latest data |
| Ctrl+N, Ctrl+O, Ctrl+S, Ctrl+Shift+S | new, open, save, save as |
| Ctrl+, | settings |
| F1 | about |
| `+` `-` Ctrl+0 Home End Left Right (with Shift: a whole view) | the view |
| M `[` `]` Delete S | markers |
| D, F8, Shift+F8, Ctrl+F8 | add a decoder, next / previous / selected annotation |
| Ctrl+= Ctrl+- | interface scale |

Every menu entry shows its shortcut, and every command can be reached from the keyboard.

## Notes for Debian

- Japanese is built into the program, so the interface is readable without extra fonts. If you type
  other CJK text (in a channel name, for example) and see empty boxes, install a CJK font:
  `sudo apt install fonts-noto-cjk`.
- The window needs a working graphics driver (Vulkan or OpenGL, for example `mesa-vulkan-drivers`)
  and a desktop session (X11 or Wayland). File dialogs use the desktop portal when there is one
  (`xdg-desktop-portal` with the backend of your desktop).
- *To be confirmed on a clean Debian 13 installation: the exact list of runtime packages.*

## Real hardware

Drivers for the supported devices (Raspberry Pi Pico, Logic16 clone, WeAct) arrive in later
features. Each will bring its own short section here: how to give your user access to the device
(a udev rule on Debian, a driver on Windows) and how to connect it. *Placeholder until the first
driver is added.*
