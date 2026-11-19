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
# Data of the capture is being deleted (a cut or a trim of the capture).
status-cropping = Deleting data
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
toolbar-rate-choice = Sample rate
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
# Button: restore the default names, colours, order and visibility.
channel-reset = Reset

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
# Start placing a single marker: the next click on the signals fixes it.
menu-markers-place-single = Place single marker
# Start placing a pair marker: two clicks on the signals fix its two ends.
menu-markers-place-pair = Place pair marker
# Stop placing a marker; nothing is added.
menu-markers-cancel = Cancel placement
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
# Button that starts placing a single marker (a point in time).
marker-single-add = Single marker
# Button that starts placing a pair marker (an interval with a start and an end).
marker-pair-add = Pair marker
# Shown in the Markers view when there is no marker.
marker-empty = No markers. Use the buttons above to add one.
# Status line while a single marker is being placed.
marker-placing-single = Click to place the marker. Esc cancels.
# Status line while the first end of a pair marker is being placed.
marker-placing-pair-first = Click to place the first end of the pair. Esc cancels.
# Status line while the second end of a pair marker is being placed.
marker-placing-pair-second = Click to place the second end. Esc cancels.
# Status line while the view has zoomed in by itself so that an edge can be found.
marker-zoomed-in = Zoomed in to find the edge. The view returns when you click or press Esc.
# Shown in the Markers view when no more markers can be added.
marker-limit = The limit of 64 markers is reached. Delete one to add another.
# Tooltip of the button that opens the actions of a marker.
marker-actions = Actions
# Button that deletes a marker.
marker-delete = Delete
# Placeholder of the field for the comment of a marker.
marker-comment = Comment
# Button that turns the two checked single markers into a pair marker.
marker-make-pair = Make a pair
# Action of a single marker: time is counted from it.
marker-action-origin = Count time from here
# Action of the single marker that time is counted from: count from the start again.
marker-action-origin-reset = Time counts from the start
# Action of a single marker: delete the data to its left.
marker-action-cut-left = Delete data to the left
# Action of a single marker: delete the data to its right.
marker-action-cut-right = Delete data to the right
# Action of a single marker: scroll the view to it.
marker-action-scroll = Scroll to it
# Action of a pair marker: keep only the data between its ends.
marker-action-trim = Trim to the pair
# Action of a pair marker: zoom and scroll so that the pair fills the view.
marker-action-fit = Fit the view to the pair
# Action of a pair marker: replace it by two single markers.
marker-action-split = Convert to two single markers
# Why an action that deletes data is not available: a capture is running.
marker-cut-disabled-running = Not possible while a capture is running
# Why an action that deletes data is not available: there is nothing to delete.
marker-cut-disabled-nothing = There is nothing to delete on that side
# Notice: pairs that would have been left with one end were removed; the text is how many.
marker-cut-pair-removed = Pairs left with only one end were removed: { $text }.
# Title of the question before data of the capture is deleted.
dialog-crop-title = Delete data?
# Question: the data to the left of a single marker is deleted; lost and kept are durations.
dialog-crop-left-body = The data to the left of the marker will be deleted: { $lost } ({ $percent }%). { $kept } will remain. This cannot be undone.
# Question: the data to the right of a single marker is deleted.
dialog-crop-right-body = The data to the right of the marker will be deleted: { $lost } ({ $percent }%). { $kept } will remain. This cannot be undone.
# Question: only the data between the ends of a pair marker is kept.
dialog-crop-pair-body = Only the data between the ends of the pair will be kept: { $kept }. { $lost } ({ $percent }%) will be deleted. This cannot be undone.
# Button that confirms the deletion of data.
dialog-crop-confirm = Delete
# Title of the progress dialog while data of the capture is being deleted.
dialog-cropping = Deleting data
# Names of the eight colours of a marker, for the tooltips of the swatches.
marker-colour-0 = Red
marker-colour-1 = Amber
marker-colour-2 = Green
marker-colour-3 = Teal
marker-colour-4 = Blue
marker-colour-5 = Violet
marker-colour-6 = Pink
marker-colour-7 = Grey
panel-capture = Capture
panel-view-capture = Capture settings
panel-view-decoders = Decoders
panel-view-markers = Markers and measuring
capture-length-seconds = Length (seconds)
capture-length-samples = Samples: { $count }
capture-length-invalid = Enter the length as a number of seconds, such as 2.5.
capture-length-decimals = Use at most nine digits after the point.
capture-length-small = That is less than one sample at this sample rate.
capture-length-large = That is longer than a capture can be.
capture-length-no-rate = Set the sample rate first.
capture-channels = Channels to record
capture-channels-all = All
capture-channels-none = None
capture-no-channel = No channel is chosen: choose at least one to start.

### Session files

# Open a saved session file.
menu-file-open = Open…
# Save the capture to its session file.
menu-file-save = Save
# Save the capture to a new session file.
menu-file-save-as = Save as…
# Heading of the list of recently used session files in the File menu.
menu-file-recent = Recent files
menu-file-recent-empty = No recent files
menu-file-recent-missing = (missing)
menu-file-recent-remove-missing = Remove unavailable entries
menu-file-recent-clear = Clear the list
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
settings-unsaved-title = Unsaved capture
settings-remember-choice = Remember my choice
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

### Decoders: I2C
# The I2C decoder (decoders describe themselves with these keys).
decoder-i2c-name = I2C
decoder-i2c-description = Reads the I2C bus (two wires, clock and data): the address, the bytes and the acknowledgements of every transaction.
decoder-i2c-signal-scl = Clock (SCL)
decoder-i2c-signal-sda = Data (SDA)
decoder-i2c-option-address = Address notation
decoder-i2c-choice-address-addr7 = 7-bit address (without the direction bit)
decoder-i2c-choice-address-addr8 = 8-bit value (with the direction bit)
decoder-i2c-option-radix = Number base
decoder-i2c-choice-radix-hex = Hexadecimal
decoder-i2c-choice-radix-dec = Decimal
decoder-i2c-choice-radix-bin = Binary
# Annotation: a start condition (the data line falls while the clock is high).
ann-i2c-start = Start
# Annotation: a repeated start condition (a start without a stop before it).
ann-i2c-repeated-start = Repeated start
# Annotation: a stop condition (the data line rises while the clock is high).
ann-i2c-stop = Stop
# Annotation: a whole transaction that writes. $value is the address of the target, such as 0x50.
ann-i2c-transaction-write = Write to { $value }
# Annotation: a whole transaction that reads. $value is the address of the target, such as 0x50.
ann-i2c-transaction-read = Read from { $value }
# Annotation: the address byte of a write. $value is the address, such as 0x50.
ann-i2c-address-write = Address { $value } (write)
# Annotation: the address byte of a read. $value is the address, such as 0x50.
ann-i2c-address-read = Address { $value } (read)
# Annotation: a data byte. $value is its value, such as 0x12.
ann-i2c-data = Data { $value }
# Annotation: the receiver acknowledged the byte.
ann-i2c-ack = ACK
# Annotation: the receiver did not acknowledge the byte.
ann-i2c-nak = NAK
# Annotation: a transaction that was not finished (the capture ended or samples were lost).
ann-i2c-incomplete = Incomplete transaction
# Annotation (error): a byte that ended early. $value is the number of bits that were read.
ann-i2c-error-byte-cut = Byte cut short after { $value } bits
# Annotation (error): the address was not acknowledged. $value is the address, such as 0x50.
ann-i2c-error-address-nak = Address { $value } not acknowledged
# Annotation (error): a byte written to a target was not acknowledged. $value is the byte, such as 0x12.
ann-i2c-error-data-nak = Data { $value } not acknowledged
# Annotation (error): a start condition followed by a stop condition with no clock pulse between them.
ann-i2c-error-empty = Empty transaction: no address
# Annotation (error): positions of the capture were lost here.
ann-i2c-error-gap = Samples lost here

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

### Decoders: stacking
decoder-source = Reads from
decoder-source-capture = The capture
dialog-remove-decoder-title = Remove decoder
dialog-remove-decoder-body = Other decoders read from this one. Removing it removes them too. The capture is not affected.
dialog-remove-decoder-confirm = Remove them all

### Decoders: saving and opening
notice-decoders-newer = This file has decoders from a newer version of the application. They are kept as they are and not shown; the capture opens normally.
notice-layout-newer = This file holds signal heights saved by a newer version of the application. They are kept as they are and not shown; the capture opens with automatic heights.
settings-save-decoder-results = Save decoded results with the session
settings-save-decoder-results-note = Makes the file larger. Without it the decoders run again when the file is opened.
