### status: draft
### The user-facing text of every stable error code. The key of a message is `error-` plus the
### error code with the dot written as a hyphen (`session.damaged` is `error-session-damaged`).
### Each message says what happened and what the user can do. The technical detail of an error is
### for the log and never appears here.

# Shown when something fails that has no message of its own (an error code added without text).
# $code is the stable error code, shown so the user can report it.
error-unexpected = Something unexpected went wrong (code { $code }). Please try again; if it keeps happening, report it with the log file.

### Devices

# Starting a capture on a device that another capture is already using.
error-device-busy = This device is already in use by another capture. Stop that capture or choose another device.
# The device was unplugged while it was in use.
error-device-disconnected = The device was disconnected. Everything captured so far is kept. Reconnect the device and start a new capture.
# The driver of the device reported a failure.
error-device-driver_error = The device reported an error and the capture stopped. Everything captured so far is kept. Unplug and reconnect the device, then try again.
# The driver of the device failed in a way it should not (a contained internal failure).
error-device-driver_panic = The driver for this device stopped unexpectedly. Everything captured so far is kept. Reconnect the device and try again; if it keeps happening, report it with the log file.
# The device described its own abilities in a way that makes no sense.
error-device-invalid_capabilities = The device described itself in a way the program cannot use, so it is not available. Reconnect it, and if the problem stays, report it with the log file.
# The requested number of samples cannot be captured by this device.
error-device-invalid_capture_length = The requested capture length is not possible for this device. Choose a shorter length or a different device.
# The selected channels are not channels of this device.
error-device-invalid_channel = The selected channels are not available on this device. Choose channels the device has.
# An action was requested at a moment when it cannot be done.
error-device-invalid_state = That action is not possible right now. Wait for the current operation to finish and try again.
# A capture of the chosen length might not fit in the memory the program may use.
error-device-memory_budget = A capture of this length might not fit in memory. Choose a shorter capture or allow the program to use more memory.
# The chosen device is not in the list of connected devices.
error-device-not_found = The device is not connected or is no longer available. Check the connection and refresh the device list.
# The communication with the device (USB or serial) failed.
error-device-transport = Communication with the device failed. Everything captured so far is kept. Check the cable and the USB port, then try again.
# A trigger was set but its condition never happened in the allowed time.
error-device-trigger_timeout = The trigger condition did not occur in time, so the capture did not start. Check the trigger settings and the signal, or capture without a trigger.
# The device stopped answering.
error-device-unresponsive = The device stopped responding. Everything captured so far is kept. Unplug and reconnect it, then try again.
# The chosen sample rate is not one of the device's rates.
error-device-unsupported_sample_rate = This device cannot capture at the chosen sample rate. Choose one of the rates the device offers.
# The chosen trigger kind is not supported by the device.
error-device-unsupported_trigger = This device does not support the chosen trigger. Choose a trigger the device offers or capture without one.

### Session files

# The user cancelled saving or opening a session.
error-session-cancelled = The operation was cancelled. Nothing was changed.
# A checksum of the session file does not match (the file was damaged on disk or in transfer).
error-session-damaged = The session file is damaged and cannot be opened. If you have a backup copy of the file, open that instead.
# The session file is well formed but its content contradicts itself.
error-session-inconsistent = The session file contains data that does not make sense, so it cannot be opened. If you have a backup copy of the file, open that instead.
# An internal problem while saving or opening a session.
error-session-internal = The program hit an unexpected problem while handling the session file. Please try again; if it keeps happening, report it with the log file.
# The file name, the folder or another input of a save is unusable.
error-session-invalid_input = The file name or location cannot be used. Choose another file name or folder.
# Reading the session file failed (missing, no permission, in use).
error-session-io_read = The session file could not be read. Check that the file exists and that you may open it, then try again.
# Writing the session file failed (disk full, no permission, folder missing).
error-session-io_write = The session file could not be saved. Your existing file was not changed. Check that there is free disk space and that you may write to the folder, or choose another location.
# The session would not fit in the memory the program may use.
error-session-memory_budget = This session is too large to open with the current memory limit. Allow the program to use more memory or open a smaller file.
# The file is not a session file (another format, or random data).
error-session-not_a_session = This file is not a session file from this program. Choose a file that was saved by it.
# A size or a count in the file is larger than the program allows.
error-session-too_large = This session file is larger than the program allows. Open a smaller file.
# The file ends too early (cut off while copying or saving).
error-session-truncated = The session file is incomplete, probably because it was cut off while being copied or saved. If you have a backup copy, open that instead.
# The file contains a required part this version does not know.
error-session-unknown_critical_section = This session file needs a newer version of the program. Update the program to open it.
# The file was written by a newer, incompatible version.
error-session-unsupported_version = This session file was made by a newer version of the program. Update the program to open it.
# The background task of saving or opening failed unexpectedly.
error-session-worker_failed = Saving or opening the session stopped unexpectedly. Your data in the program was not changed. Please try again; if it keeps happening, report it with the log file.

### Captured signal storage

# A channel number that the capture does not have was requested.
error-storage-channel_out_of_range = That channel does not exist in this capture. Choose a channel the capture has.
# A block of data without samples arrived.
error-storage-chunk_empty = The device delivered an empty block of data. Try again; if it keeps happening, report it with the log file.
# A block of data did not continue where the capture ended.
error-storage-chunk_out_of_order = Data arrived out of order, so the capture was stopped. Everything captured so far is kept. Try again; if it keeps happening, report it with the log file.
# A block of data was larger than the program accepts at once.
error-storage-chunk_too_large = The device delivered more data at once than the program accepts. Try again with a lower sample rate.
# Data was added to a capture that has already been finished.
error-storage-closed = The capture is already finished and cannot take more data. Start a new capture.
# An internal problem of the signal storage.
error-storage-internal = The program hit an unexpected problem while storing the capture. Everything captured so far is kept. Please report it with the log file.
# The number of channels is outside what a capture can hold.
error-storage-invalid_channel_count = The number of channels is not supported. Choose between 1 and 32 channels.
# A gap (lost samples) was described with a length that cannot be right.
error-storage-invalid_gap = A gap in the capture was described incorrectly. Everything captured so far is kept. Please report it with the log file.
# The memory or size limits of a capture are outside the allowed range.
error-storage-invalid_limits = The memory or size limits are not valid. Open the settings and enter valid values.
# The view asked for a number of pixel columns that is not allowed.
error-storage-invalid_pixels = The view could not be drawn at that width. Resize the window or change the zoom and try again.
# A stored capture breaks the rules a capture must follow.
error-storage-invalid_restore = The stored capture does not follow the rules for captures, so it cannot be opened. If you have a backup copy of the file, open that instead.
# The sample rate is zero.
error-storage-invalid_sample_rate = The sample rate must be greater than zero. Enter a valid sample rate.
# The part of the capture to show has its end before its start.
error-storage-invalid_window = The part of the capture to show is not valid. Select a range that starts before it ends.
# The capture reached the memory limit.
error-storage-memory_limit = The capture reached the memory limit and stopped. Everything captured so far is kept. Save it, then raise the memory limit or capture less data.
# The capture grew longer than the largest position the program can address.
error-storage-position_overflow = The capture became longer than the program can handle and was stopped. Everything captured so far is kept. Save it and capture in shorter parts.

### Application and settings

# The settings file exists but cannot be read; the defaults are used.
error-host-settings_unreadable = The settings file could not be read, so the default settings are used. Check that you have permission to read the file; your choices will be saved again when you change them.
# The settings file is damaged or from another version; the defaults are used.
error-host-settings_invalid = The settings file is damaged or from a different version, so the default settings are used. Choose your settings again; they will be saved when you change them.
# The settings could not be saved.
error-host-settings_write = Your settings could not be saved and will be lost when you close the program. Check that the settings folder exists and that you can write to it.
# The saved view (channel names, colours, markers) of an opened session is invalid.
error-host-view_invalid = The saved view of this session (channel names, colours and markers) could not be restored, so the defaults are shown. The captured signals are not affected. Choose the names and colours again if you want them.
error-host-layout_invalid = The saved heights of the signals in this session could not be restored, so the automatic heights are shown. The captured signals are not affected. Choose the heights again if you want them.
# The signal view could not be drawn.
error-host-render_failed = The signals could not be drawn. The capture is not affected and can still be saved. Resize the window or change the zoom; if it keeps happening, report it with the log file.
# A command failed unexpectedly (a contained panic or an impossible state).
error-host-internal = An unexpected problem occurred while carrying out your command, and it was not carried out. Your capture is not affected. Please try again; if it keeps happening, report it with the log file.
# An action is not valid in the current state.
error-host-invalid_command = That action is not possible right now. Wait for the current operation to finish and try again.

# Decoders: faults, refused choices and the saved decoder setup.
error-decoder-failed = A decoder stopped with an error. The annotations it produced before that are kept and marked incomplete. Try changing its settings or running it again; the capture is not affected.
error-decoder-panicked = A decoder failed unexpectedly. The annotations it produced before that are kept and marked incomplete. Try running it again; if it keeps happening, report it with the log file. The capture is not affected.
error-decoder-not_responding = A decoder stopped responding and was abandoned. What it produced before is kept and marked incomplete. Try running it again or removing it; the capture is not affected.
error-decoder-memory_limit = The decoder results reached the memory limit, so decoding stopped. Everything found so far is kept and marked incomplete. Try removing other decoders or raising the limit in the settings.
error-decoder-annotation_limit = A decoder reached the limit on the number of annotations and stopped. Everything found so far is kept and marked incomplete. Choose stricter settings or decode a shorter part of the capture.
error-decoder-invalid_output = A decoder produced some results that were not valid. They were skipped; the valid results are shown. If you see this again, report it with the log file.
error-decoder-invalid_setup = That setup choice is not valid for this decoder, so nothing was changed. Choose another channel or value.
error-decoder-stack_cycle = That choice would make decoders depend on each other in a circle, so nothing was changed. Choose a different source.
error-decoder-stack_incompatible = That decoder cannot read the results of the chosen decoder, so nothing was changed. Choose a different source.
error-decoder-stack_too_deep = That chain of decoders would be too long, so nothing was changed. Choose a shorter chain.
error-decoder-too_many_instances = The limit on decoders for one capture is reached, so no decoder was added. Try removing one first.
error-decoder-too_many_stuck = Several decoders stopped responding and are still stuck, so no new decoding is started. Try restarting the application to free them; the capture is not affected.
error-decoder-section_invalid = The decoder part of this session file is damaged. The decoders that could be read are restored; the capture and the rest of the session are not affected. Check the decoders and set up any that are missing again.
error-decoder-unknown_decoder = This session uses a decoder that this version of the application does not have. It was skipped and the rest of the session is restored. Update the application if you need that decoder.
error-decoder-options_skipped = Some saved decoder options were not understood and were skipped, so the defaults are used for them. Check the decoder settings.

### WeAct LogicAnalyzer

# The system does not let this program open the board.
error-weact-access_denied = This program is not allowed to open the WeAct LogicAnalyzer. Check that the access rule (udev) from the documentation is installed, then unplug and reconnect the device.
# Windows: the board is not bound to the WinUSB driver.
error-weact-driver_missing = The WeAct LogicAnalyzer is not using the WinUSB driver, so it cannot be opened. Check the documentation for how to install WinUSB with Zadig, then reconnect the device.
# Another program holds the board.
error-weact-in_use = The WeAct LogicAnalyzer is in use by another program. Stop the other program (for example PulseView) and try again.
# The board refused the firmware or reports a version this program does not know.
error-weact-firmware_rejected = The WeAct LogicAnalyzer did not accept the firmware, or it reports a version this program does not know. Unplug the device, reconnect it and try again.
# The board did not come back after the firmware was loaded.
error-weact-not_renumerated = The WeAct LogicAnalyzer did not come back after its firmware was loaded. Unplug it, wait a few seconds and plug it in again.
# The board is still being prepared (start was asked too early).
error-weact-preparing = The WeAct LogicAnalyzer is still being prepared. Wait a moment and start again.
# Nothing arrived after the capture was started.
error-weact-no_data = The WeAct LogicAnalyzer did not send any data. Check the USB cable and try another USB port, then start the capture again.
# The board stopped the capture because the computer could not keep up.
error-weact-stream_stopped = The capture ended early because the computer could not keep up with the WeAct LogicAnalyzer. Everything captured so far is kept. Try a lower sample rate and avoid USB hubs and docking stations.
# The firmware image that ships with the application is unusable.
error-weact-firmware_invalid = The firmware of the WeAct LogicAnalyzer that came with this program is damaged. Please reinstall the application.
