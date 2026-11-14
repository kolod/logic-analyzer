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
# Label of the number of channels to capture.
toolbar-channels = Channels
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
# Tooltip of the checkbox that shows or hides a channel.
channel-show = Show
# Tooltip of the button that moves a channel up in the order.
channel-move-up = Move up
# Tooltip of the button that moves a channel down in the order.
channel-move-down = Move down
# Tooltip of the button that changes the colour of a channel.
channel-colour = Colour
# Button: restore the default names, colours, order and visibility.
channel-reset = Reset channels
