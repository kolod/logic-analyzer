### status: draft
### Interface text of the application window: menus, toolbar, status line, dialogs and settings.
### Keys start with the area they belong to (`menu-`, `toolbar-`, `status-`, `dialog-`, `settings-`,
### `capture-`, `marker-`, `channel-`, `measure-`, `view-`, `end-reason-`).

### Application

# The window title and the product name. It stays the same in every language.
app-title = Logic Analyzer

### Status and capture results

# The application is doing nothing.
status-idle = Idle
# A capture is running.
status-capturing = Capturing
# A session file is being saved.
status-saving = Saving
# A session file is being opened.
status-loading = Loading
# Memory in use over the memory limit; both values are already formatted sizes.
status-memory = { $used } / { $limit }
# The capture reached its configured length.
end-reason-completed = Capture completed
# The user stopped the capture.
end-reason-stopped = Stopped by you
# The capture stopped because it reached the memory limit.
end-reason-memory-limit = Memory limit reached
# The device was disconnected during the capture.
end-reason-disconnected = Device disconnected
# The device or its connection failed during the capture.
end-reason-device-error = Device error
# The capture finished but some samples were lost.
end-reason-samples-lost = Completed with lost samples
# The capture was loaded from a session file.
end-reason-loaded = Loaded from file
# The selected device disappeared; the argument is its identifier.
notice-device-removed = The selected device "{ $text }" is no longer available, so the default device was selected.

### Menus

# The File menu.
menu-file = File
# Start a new, empty capture.
menu-file-new = New capture
# Quit the application.
menu-file-quit = Quit
# The Capture menu.
menu-capture = Capture
# Start a capture.
menu-capture-start = Start
# Stop the running capture.
menu-capture-stop = Stop

### Toolbar, view and dialogs

# Label of the device chooser.
toolbar-device = Device
# Label of the sample rate field (the value is in hertz).
toolbar-rate = Sample rate (Hz)
# Label of the capture length field (the value is a number of samples).
toolbar-length = Length (samples)
# Label of the seed of the simulated signals.
toolbar-seed = Seed
# Placeholder of the seed field: a new seed is chosen at every start.
toolbar-seed-random = random
# Label of the choice of how busy the simulated signals are.
toolbar-profile = Signal
# Simulated signals with long quiet stretches.
profile-sparse = Sparse
# Simulated signals with moderate activity.
profile-normal = Normal
# Simulated signals with a transition every few samples.
profile-dense = Dense
# Checkbox: the simulated device runs at the real sample rate.
toolbar-realtime = Real time
# Button: start a capture.
toolbar-start = Start
# Button: stop the running capture.
toolbar-stop = Stop
# Label: the data on screen is simulated, not a measurement.
status-simulated = Simulated
# Label: samples were lost or the capture ended abnormally.
status-incomplete = Incomplete
# Label: there are changes that are not saved.
status-unsaved = Unsaved changes
# Shown in the signal view before the first capture.
view-no-capture = No capture yet. Choose a device and press Start.
# Shown in the signal view when the capture has no samples.
view-empty = The capture has no samples.
# Shown in the signal view when the signals could not be drawn.
view-cannot-draw = The signals cannot be drawn.
# The default name of a channel; the argument is its number, counted from 0.
channel-default = Channel { $number }
# Title of the question about a capture that is not saved.
dialog-unsaved-title = Unsaved capture
# Question about a capture that is not saved.
dialog-unsaved-body = The capture has not been saved. What do you want to do with it?
# Button: save the capture first.
dialog-save = Save
# Button: throw the unsaved capture away.
dialog-discard = Discard
# Button: do not carry on.
dialog-cancel = Cancel

### View and channels

# The View menu.
menu-view = View
# Show less time in the view.
menu-view-zoom-in = Zoom in
# Show more time in the view.
menu-view-zoom-out = Zoom out
# Show the whole capture.
menu-view-fit = Fit all
# Move the view to the start of the capture.
menu-view-start = Go to start
# Move the view to the end of the capture.
menu-view-end = Go to end
# Move the view a little towards the start.
menu-view-pan-left = Pan left
# Move the view a little towards the end.
menu-view-pan-right = Pan right
# Move the view by one whole window towards the start.
menu-view-page-left = Page left
# Move the view by one whole window towards the end.
menu-view-page-right = Page right
# Keep the view at the newest data of a running capture.
menu-capture-follow = Follow latest data
# Title of the panel that lists the channels.
panel-channels = Channels
# Tooltip of the button that moves a channel up in the order.
channel-move-up = Move up
# Tooltip of the button that moves a channel down in the order.
channel-move-down = Move down
# Tooltip of the button that changes the colour of a channel.
channel-colour = Colour
# Button: restore the default names, colours, order and visibility.
channel-reset = Reset channels

### Messages

# Button: remove the message.
notice-dismiss = Dismiss
# Button: remove every message.
notice-dismiss-all = Dismiss all
# Button: open the folder with the log files, for a bug report.
notice-log = Show log location
# Button: show the recent messages.
notice-list = Messages
# The Help menu.
menu-help = Help
# Open the folder with the log files.
menu-help-log = Show log location

### Markers and measurements

# The Markers menu.
menu-markers = Markers
# Place a marker at the pointer, or at the centre of the view.
menu-markers-place = Place marker
# Go to the previous marker.
menu-markers-previous = Previous marker
# Go to the next marker.
menu-markers-next = Next marker
# Remove the selected marker.
menu-markers-remove = Remove marker
# Remove every marker.
menu-markers-remove-all = Remove all markers
# Turn snapping of markers to signal transitions on or off.
menu-markers-snap = Snap to transitions
# Title of the panel that lists the markers.
panel-markers = Markers
# The default name of a marker; the argument is its number.
marker-default = Marker { $number }
# Tooltip of the button that moves the view to a marker.
marker-go = Go to marker
# Tooltip of the button that removes a marker.
marker-remove = Remove
# Tooltip of the button that makes a marker the first one of the measurement.
marker-measure-first = First of the measurement
# Tooltip of the button that makes a marker the second one of the measurement.
marker-measure-second = Second of the measurement
# Label of the time between two markers.
measure-distance = Distance
# Label of the frequency of the time between two markers.
measure-frequency = Frequency
# Hint shown when fewer than two markers are chosen for a measurement.
measure-pick = Choose two markers to measure the time between them.

### Session files

# Open a saved session file.
menu-file-open = Open…
# Save the capture to its session file.
menu-file-save = Save
# Save the capture to a new session file.
menu-file-save-as = Save as…
# Heading of the list of recently used session files in the File menu.
menu-file-recent = Recent files
# Shown while a session file is being written.
dialog-saving = Saving the session…
# Shown while a session file is being read.
dialog-opening = Opening the session…
# Title of the file chooser that opens a session.
dialog-open-title = Open a session
# Title of the file chooser that saves a session.
dialog-save-title = Save the session
# Name of the file type filter for session files (.lasession).
dialog-filter-session = Logic analyzer sessions
# The window title of a capture that has no file yet.
title-untitled = Untitled capture
# Information: the file stores view settings written by a newer version.
notice-view-newer = The view settings in this file come from a newer version and were not applied. They stay in the file as they are.

### Settings and About

# Open the settings dialog.
menu-file-settings = Settings…
# Make the whole interface larger.
menu-view-scale-up = Larger interface
# Make the whole interface smaller.
menu-view-scale-down = Smaller interface
# Open the about dialog.
menu-help-about = About
# Title of the settings dialog.
settings-title = Settings
# Label of the language choice.
settings-language = Language
# Label of the colour theme choice.
settings-theme = Theme
# Theme choice: follow the operating system.
theme-system = Follow the system
# Theme choice: light colours.
theme-light = Light
# Theme choice: dark colours.
theme-dark = Dark
# Theme choice: maximum contrast.
theme-high-contrast = High contrast
# Label of the interface scale choice (a percentage follows).
settings-scale = Interface scale
# Label of the memory limit of one capture.
settings-memory = Memory limit of a capture
# The unit after the memory limit field (mebibytes).
settings-memory-unit = MiB
# Note under the memory limit: when it takes effect.
settings-memory-note = Applies to the next capture.
# Validation message of the memory limit; the arguments are the smallest and the largest value in MiB.
settings-memory-invalid = Enter a whole number of MiB from { $min } to { $max }.
# Button: close a dialog.
dialog-close = Close
# Title of the about dialog.
about-title = About
# Text of the about dialog; the argument is the version number.
about-body = Logic Analyzer { $version }. Records digital signals and shows them on a timeline; a simulated device lets you try it without hardware.
# License line of the about dialog.
about-license = Free software under the GNU General Public License, version 3.

### Decoders: the sample decoder
# The sample decoder (decoders describe themselves with these keys).
decoder-sample-clocked-bits-name = Clocked bits
decoder-sample-clocked-bits-description = Reads bits from a data line on the edges of a clock line and groups them into bytes. A sample decoder that shows what decoders can do.
decoder-sample-clocked-bits-signal-clock = Clock
decoder-sample-clocked-bits-signal-data = Data
decoder-sample-clocked-bits-option-clock_edge = Clock edge
decoder-sample-clocked-bits-choice-clock_edge-rising = Rising
decoder-sample-clocked-bits-choice-clock_edge-falling = Falling
decoder-sample-clocked-bits-option-bit_order = Bit order
decoder-sample-clocked-bits-choice-bit_order-msb = Most significant bit first
decoder-sample-clocked-bits-choice-bit_order-lsb = Least significant bit first
decoder-sample-clocked-bits-option-bits = Bits per byte
decoder-sample-clocked-bits-option-idle = Idle limit (samples)
# Annotation: a byte. $value is the value in hexadecimal, such as 0x5A.
ann-sample-clocked-bits-byte = Byte { $value }
# Annotation: one sampled bit. $value is 0 or 1.
ann-sample-clocked-bits-bit = Bit { $value }
# Annotation (error): a byte that ended early. $value is the number of bits it had.
ann-sample-clocked-bits-incomplete = Incomplete byte: { $value } bits
# Annotation (error): positions of the capture were lost here.
ann-sample-clocked-bits-gap = Samples lost here

### Decoders: menu, panel and the selected annotation
menu-decoders = Decoders
menu-decoders-add = Add decoder
menu-decoders-previous = Previous annotation
menu-decoders-next = Next annotation
menu-decoders-goto = Go to selected annotation
panel-decoders = Decoders
decoders-add = Add decoder
decoders-empty = No decoders yet. Add one to see what the signals say.
decoders-need-capture = Capture or open signals first.
decoder-state-waiting = Waiting
decoder-state-running = Running
decoder-state-finished = Finished
decoder-state-cancelled = Cancelled
decoder-state-failed = Failed
decoder-needs-channels = Choose the channels to start decoding.
decoder-channel-none = (none)
decoder-lane-show = Show the lane
decoder-remove = Remove decoder
decoder-annotations = Annotations: { $number }
decoder-incomplete = Incomplete results
decoder-selected = Selected annotation
decoder-detail-length = Length: { $value }
decoder-detail-error = Reports a problem in the signal
decoder-detail-position = Position: { $first } to { $second }

### Decoders: the simulation of signals that are valid for a decoder
decoder-generator-errors = Plant deliberate errors in the simulated signals
decoder-generator-rate = About one byte in this many is cut short

### Decoders: running, cancelling and the memory limit
decoder-run = Run again
decoder-cancel = Cancel
settings-decoder-memory = Memory limit of decoder results
settings-decoder-memory-note = Shared by all decoders. Applies to the next pass.
