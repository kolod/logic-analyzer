//! Commands a user interface sends to the host, and what comes back.
//!
//! Every user-visible action — a menu entry, a toolbar button, a shortcut, a mouse gesture — is
//! exactly one [`Command`] (contract `docs/host-api.md`).

use std::ops::BitOr;
use std::path::PathBuf;

use la_devices::{DeviceId, Trigger};
use la_i18n::Language;

use la_decoders::setup::{Setup, Source, Value};

use crate::decoders::AnnotationRef;
use crate::devices::SignalProfile;
use crate::markers::{End, Kind, Pointer};
use crate::notices::NoticeId;
use crate::presentation::Colour;
use crate::settings::Theme;
use crate::window::ViewSize;

/// A change of the device configuration the user is editing.
#[derive(Debug, Clone, PartialEq)]
pub enum ConfigEdit {
    /// The enabled channels as a bit mask (bit `i` is channel `i`).
    Channels(u32),
    /// The sample rate in hertz.
    SampleRate(u64),
    /// The capture length in samples.
    Length(u64),
    /// The trigger, or none.
    Trigger(Option<Trigger>),
    /// The seed of the virtual device, or none for a fresh one.
    Seed(Option<u64>),
    /// The signal profile of the virtual device.
    Profile(SignalProfile),
    /// Whether the virtual device is paced in real time (otherwise it runs as fast as it can).
    /// The application always uses real time; the switch is kept for tests, which need fast captures.
    Realtime(bool),
    /// Whether to start a finite capture although its worst-case storage cost exceeds the
    /// memory limit (the capture then ends cleanly if the limit is really reached).
    AcceptBudgetRisk(bool),
}

/// How much the view zooms: the new length is `length × numerator / denominator`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ZoomFactor {
    /// Numerator.
    pub numerator: u64,
    /// Denominator.
    pub denominator: u64,
}

/// What stays in place while zooming.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ZoomAnchor {
    /// The sample under this pixel column (a pointer position).
    Column(u32),
    /// The centre of the view (keyboard and controls).
    Centre,
}

/// Direction of a zoom step.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ZoomDirection {
    /// Show less time.
    In,
    /// Show more time.
    Out,
}

/// A keyboard pan step.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PanStep {
    /// A tenth of the window towards the start.
    Left,
    /// A tenth of the window towards the end.
    Right,
    /// A whole window towards the start.
    PageLeft,
    /// A whole window towards the end.
    PageRight,
}

/// The user's answer to a question about unsaved data.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Decision {
    /// Save first, then carry on.
    Save,
    /// Throw the unsaved data away and carry on.
    Discard,
    /// Do not carry on.
    Cancel,
}

impl Decision {
    /// The word the settings file keeps for a remembered answer.
    #[must_use]
    pub fn as_setting(self) -> &'static str {
        match self {
            Decision::Save => "save",
            Decision::Discard => "discard",
            Decision::Cancel => "cancel",
        }
    }

    /// The answer a settings word stands for; `None` for a word that is none of the three.
    #[must_use]
    pub fn from_setting(word: &str) -> Option<Self> {
        match word {
            "save" => Some(Decision::Save),
            "discard" => Some(Decision::Discard),
            "cancel" => Some(Decision::Cancel),
            _ => None,
        }
    }
}

/// Which part of the capture a cut or a trim keeps.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CropSide {
    /// The data to the left of a single marker is deleted.
    Left,
    /// The data to the right of a single marker is deleted.
    Right,
    /// Only the data between the ends of a pair marker is kept.
    Pair,
}

/// A cut or a trim of the capture that waits for the user to confirm it: the positions kept are
/// `start..end` of the `total` positions of the capture.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CropPlan {
    /// What is kept.
    pub side: CropSide,
    /// The first position kept.
    pub start: u64,
    /// One past the last position kept.
    pub end: u64,
    /// The length of the capture as it is now.
    pub total: u64,
}

impl CropPlan {
    /// How many positions are kept.
    #[must_use]
    pub fn kept(&self) -> u64 {
        self.end.saturating_sub(self.start)
    }

    /// How many positions are deleted.
    #[must_use]
    pub fn lost(&self) -> u64 {
        self.total.saturating_sub(self.kept())
    }
}

/// A command that waits for a [`Decision`] because unsaved data would be lost, or because the
/// user is asked to confirm it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Guarded {
    /// A cut or a trim of the capture: asked to confirm, since it cannot be undone. The answer
    /// `Discard` is "delete", `Cancel` is "keep everything".
    Crop(CropPlan),
    /// Starting a new, empty capture.
    NewCapture,
    /// Starting a capture while one is on screen.
    StartCapture,
    /// Opening a session file.
    OpenSession(PathBuf),
    /// Removing a decoder that other decoders are stacked on (they go with it).
    RemoveDecoder(u32),
    /// Closing the window.
    Close,
    /// Quitting.
    Quit,
}

/// Which annotation `GoToAnnotation` moves to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnnotationTarget {
    /// The selected one.
    Selected,
    /// The next one by start, in the store of the selected annotation.
    Next,
    /// The previous one by start, in the store of the selected annotation.
    Previous,
}

/// Everything a user interface can ask of the host.
#[derive(Debug, Clone, PartialEq)]
pub enum Command {
    /// Lists the devices again.
    RefreshDevices,
    /// Selects a device.
    SelectDevice(DeviceId),
    /// Edits the configuration of the selected device.
    EditConfig(ConfigEdit),
    /// Starts a capture with the validated configuration.
    StartCapture,
    /// Stops the running capture; everything captured so far is kept.
    StopCapture,
    /// Clears the capture and returns to idle.
    NewCapture,
    /// Tells the host the size of the signal area.
    SetViewSize(ViewSize),
    /// Zooms by a factor around an anchor.
    Zoom {
        /// The factor.
        factor: ZoomFactor,
        /// What stays in place.
        anchor: ZoomAnchor,
    },
    /// Zooms in or out by a factor of two around the centre.
    ZoomStep(ZoomDirection),
    /// Pans by pixels of the current zoom.
    Pan {
        /// Pixels; positive moves towards the end of the capture.
        pixels: i64,
    },
    /// Pans by a keyboard step.
    PanStep(PanStep),
    /// Shows the whole capture.
    FitAll,
    /// Moves the view to the start of the capture.
    GoToStart,
    /// Moves the view to the end of the capture.
    GoToEnd,
    /// Shows the samples from `start` to `end`.
    ZoomToRange {
        /// First sample.
        start: u64,
        /// End sample.
        end: u64,
    },
    /// Turns following the newest data on or off.
    SetFollowLatest(bool),
    /// Renames a channel.
    RenameChannel {
        /// The channel.
        channel: u8,
        /// The new name (sanitized by the host).
        name: String,
    },
    /// Changes the colour of a channel.
    SetChannelColour {
        /// The channel.
        channel: u8,
        /// The colour.
        colour: Colour,
    },
    /// Shows or hides a channel.
    SetChannelVisible {
        /// The channel.
        channel: u8,
        /// Whether it is shown.
        visible: bool,
    },
    /// Moves a channel in the display order.
    MoveChannel {
        /// Current position in the order.
        from: usize,
        /// New position in the order.
        to: usize,
    },
    /// Sets the height of the signal of a channel (0: automatic, otherwise 24 to 240 pixels).
    SetChannelHeight {
        /// The channel.
        channel: u8,
        /// The height in logical pixels.
        height: u16,
    },
    /// Restores the default names, colours, order, heights and visibility.
    ResetPresentation,
    /// Chooses the view of the panel at the right edge (or hides it).
    SetSidePanel(crate::state::SidePanel),
    /// Sets the width of the panel at the right edge in logical pixels, kept within
    /// `settings::PANEL_WIDTH_RANGE`. Saved in the settings.
    SetPanelWidth(u32),
    /// Remembers the answer to the question about an unsaved capture (`Some(Save)` or
    /// `Some(Discard)`: the question is not asked again and the answer is applied) or goes back to
    /// asking (`None`). `Cancel` cannot be remembered and is ignored. Saved in the settings.
    SetUnsavedChoice(Option<Decision>),
    /// Scrolls to the next edge of a channel beyond the visible window, a quarter of the view from
    /// the side it came from; the zoom does not change.
    ScrollToEdge {
        /// The channel.
        channel: u8,
        /// The side to look on.
        direction: crate::state::EdgeDirection,
    },
    /// Removes a file from the list of recent files.
    RemoveRecentFile(PathBuf),
    /// Empties the list of recent files.
    ClearRecentFiles,
    /// Starts placing a marker of a kind with the mouse; nothing is added until a click fixes it
    /// (`FixPlacement`). The same kind again cancels the placement, the other kind switches to it.
    BeginPlacement(Kind),
    /// Ends the placement under way and adds nothing.
    CancelPlacement,
    /// A click while placing: fixes the marker (for a pair, its next half) where the pointer is.
    FixPlacement {
        /// Where the pointer is over the signal area.
        pointer: Pointer,
    },
    /// Moves one end of a marker (a drag); snaps like a placement.
    MoveMarkerEnd {
        /// The marker.
        id: u32,
        /// Which end.
        end: End,
        /// Where the pointer is over the signal area.
        pointer: Pointer,
    },
    /// The button of a drag was released: puts back the view an automatic zoom replaced.
    EndMarkerDrag,
    /// One frame of the automatic zoom of a crowded snapping zone; the window it replaces is
    /// remembered and put back when the marker is fixed or the placement ends.
    PlacementZoomTo {
        /// First visible sample.
        start: u64,
        /// Number of visible samples.
        length: u64,
    },
    /// Sets the colour of a marker (an index below `MARKER_COLOURS`).
    SetMarkerColour {
        /// The marker.
        id: u32,
        /// The colour.
        colour: u8,
    },
    /// Sets the comment of a marker (sanitized and shortened by the host).
    SetMarkerComment {
        /// The marker.
        id: u32,
        /// The comment.
        comment: String,
    },
    /// Selects a marker, or none.
    SelectMarker(Option<u32>),
    /// Makes the position of a single marker the origin of time (time counts from it).
    SetTimeOrigin(u32),
    /// Puts the origin of time back to the start of the capture.
    ResetTimeOrigin,
    /// Deletes the data to the left of a single marker (after the user confirms).
    CutLeft(u32),
    /// Deletes the data to the right of a single marker (after the user confirms).
    CutRight(u32),
    /// Keeps only the data between the ends of a pair marker (after the user confirms).
    TrimToPair(u32),
    /// Moves the view so that a marker is at its centre; the zoom does not change.
    ScrollToMarker(u32),
    /// Sets the zoom and the position so that a pair fills the view with room for its marks.
    FitToPair(u32),
    /// Replaces a pair marker by two single markers at its ends.
    SplitPair(u32),
    /// Replaces two single markers by one pair marker between them.
    MakePair {
        /// One single marker.
        first: u32,
        /// The other single marker.
        second: u32,
    },
    /// Removes one marker.
    RemoveMarker(u32),
    /// Removes every marker.
    RemoveAllMarkers,
    /// Turns snapping of markers to transitions on or off.
    SetSnap(bool),
    /// Adds an instance of a registered decoder with default options and no channels yet.
    AddDecoder {
        /// The decoder id.
        decoder: String,
    },
    /// Replaces the setup of an instance in one step (validated as a whole).
    ConfigureDecoder {
        /// The instance.
        id: u32,
        /// The new setup.
        setup: Setup,
    },
    /// Assigns a channel to a signal role of an instance, or none.
    SetDecoderChannel {
        /// The instance.
        id: u32,
        /// The signal role.
        role: String,
        /// The channel, or none.
        channel: Option<u8>,
    },
    /// Sets one option of an instance.
    SetDecoderOption {
        /// The instance.
        id: u32,
        /// The option.
        option: String,
        /// The value.
        value: Value,
    },
    /// Renames an instance (sanitized, at most 64 characters).
    RenameDecoder {
        /// The instance.
        id: u32,
        /// The new name.
        name: String,
    },
    /// Shows or hides the lane and overlays of an instance.
    ShowDecoderLane {
        /// The instance.
        id: u32,
        /// Whether they are shown.
        visible: bool,
    },
    /// Removes an instance and its results (asks first when other instances read from it).
    RemoveDecoder(u32),
    /// Makes the capture or another instance the input of an instance.
    StackDecoder {
        /// The instance.
        id: u32,
        /// Where it reads from.
        source: Source,
    },
    /// Starts a new pass of an instance that is not running.
    RunDecoder(u32),
    /// Stops the running pass of an instance within a second, keeping its results (incomplete).
    CancelDecoder(u32),
    /// Sets the memory limit of decoder results in bytes (for passes that start afterwards).
    SetDecoderMemoryLimit(u64),
    /// Chooses whether the results of the decoders are saved with the session.
    SetSaveDecoderResults(bool),
    /// Selects an annotation.
    SelectAnnotation {
        /// The instance.
        instance: u32,
        /// The annotation.
        annotation: AnnotationRef,
    },
    /// Moves the view to the selected, next or previous annotation.
    GoToAnnotation(AnnotationTarget),
    /// Clears the annotation selection.
    ClearAnnotationSelection,
    /// Saves to the current path.
    SaveSession,
    /// Saves to a new path.
    SaveSessionAs(PathBuf),
    /// Opens a session file.
    OpenSession(PathBuf),
    /// Cancels a running save or load.
    CancelOperation,
    /// Answers a question about unsaved data.
    Decide(Decision),
    /// Chooses the language.
    SetLanguage(Language),
    /// Chooses the theme.
    SetTheme(Theme),
    /// Sets the interface scale in percent.
    SetUiScale(u16),
    /// Sets the memory limit of captures in bytes.
    SetMemoryLimit(u64),
    /// Remembers where the window is (written to the settings file when the application
    /// saves its settings, not on every move).
    SetWindowGeometry(crate::settings::WindowGeometry),
    /// Removes one notice.
    DismissNotice(NoticeId),
    /// Removes every notice.
    DismissAllNotices,
    /// The window close button.
    RequestClose,
    /// Quits the application.
    Quit,
    /// Provokes a panic inside the command executor (tests and failure checks only).
    #[cfg(feature = "test-support")]
    TestPanic,
    /// Fails with an error of this code (tests and failure checks only).
    #[cfg(feature = "test-support")]
    TestError(&'static str),
}

/// The reason of an ignored save: the capture has no file yet, so a path is needed (the user
/// interface then asks for one and sends `SaveSessionAs`).
pub const NEEDS_PATH: &str = "needs_path";

impl Command {
    /// Whether the command must wait while a save or load runs: it would change what is being
    /// saved or replace what is being loaded.
    #[must_use]
    pub fn waits_for_operation(&self) -> bool {
        matches!(
            self,
            Command::StartCapture
                | Command::NewCapture
                | Command::SaveSession
                | Command::SaveSessionAs(_)
                | Command::OpenSession(_)
                | Command::RenameChannel { .. }
                | Command::SetChannelColour { .. }
                | Command::SetChannelVisible { .. }
                | Command::MoveChannel { .. }
                | Command::SetChannelHeight { .. }
                | Command::ResetPresentation
                | Command::BeginPlacement(_)
                | Command::FixPlacement { .. }
                | Command::MoveMarkerEnd { .. }
                | Command::SetMarkerColour { .. }
                | Command::SetMarkerComment { .. }
                | Command::SetTimeOrigin(_)
                | Command::ResetTimeOrigin
                | Command::CutLeft(_)
                | Command::CutRight(_)
                | Command::TrimToPair(_)
                | Command::SplitPair(_)
                | Command::MakePair { .. }
                | Command::RemoveMarker(_)
                | Command::RemoveAllMarkers
                | Command::AddDecoder { .. }
                | Command::ConfigureDecoder { .. }
                | Command::SetDecoderChannel { .. }
                | Command::SetDecoderOption { .. }
                | Command::RenameDecoder { .. }
                | Command::ShowDecoderLane { .. }
                | Command::RemoveDecoder(_)
                | Command::StackDecoder { .. }
                | Command::RunDecoder(_)
                | Command::Decide(Decision::Save | Decision::Discard)
        )
    }

    /// A short stable name for the log (never contains user text).
    #[must_use]
    pub fn name(&self) -> &'static str {
        match self {
            Command::RefreshDevices => "refresh_devices",
            Command::SelectDevice(_) => "select_device",
            Command::EditConfig(_) => "edit_config",
            Command::StartCapture => "start_capture",
            Command::StopCapture => "stop_capture",
            Command::NewCapture => "new_capture",
            Command::SetViewSize(_) => "set_view_size",
            Command::Zoom { .. } => "zoom",
            Command::ZoomStep(_) => "zoom_step",
            Command::Pan { .. } => "pan",
            Command::PanStep(_) => "pan_step",
            Command::FitAll => "fit_all",
            Command::GoToStart => "go_to_start",
            Command::GoToEnd => "go_to_end",
            Command::ZoomToRange { .. } => "zoom_to_range",
            Command::SetFollowLatest(_) => "set_follow_latest",
            Command::RenameChannel { .. } => "rename_channel",
            Command::SetChannelColour { .. } => "set_channel_colour",
            Command::SetChannelVisible { .. } => "set_channel_visible",
            Command::MoveChannel { .. } => "move_channel",
            Command::ResetPresentation => "reset_presentation",
            Command::SetChannelHeight { .. } => "set_channel_height",
            Command::SetSidePanel(_) => "set_side_panel",
            Command::SetUnsavedChoice(_) => "set_unsaved_choice",
            Command::SetPanelWidth(_) => "set_panel_width",
            Command::ScrollToEdge { .. } => "scroll_to_edge",
            Command::RemoveRecentFile(_) => "remove_recent_file",
            Command::ClearRecentFiles => "clear_recent_files",
            Command::BeginPlacement(_) => "begin_placement",
            Command::CancelPlacement => "cancel_placement",
            Command::FixPlacement { .. } => "fix_placement",
            Command::MoveMarkerEnd { .. } => "move_marker_end",
            Command::EndMarkerDrag => "end_marker_drag",
            Command::PlacementZoomTo { .. } => "placement_zoom_to",
            Command::SetMarkerColour { .. } => "set_marker_colour",
            Command::SetMarkerComment { .. } => "set_marker_comment",
            Command::SelectMarker(_) => "select_marker",
            Command::SetTimeOrigin(_) => "set_time_origin",
            Command::ResetTimeOrigin => "reset_time_origin",
            Command::CutLeft(_) => "cut_left",
            Command::CutRight(_) => "cut_right",
            Command::TrimToPair(_) => "trim_to_pair",
            Command::ScrollToMarker(_) => "scroll_to_marker",
            Command::FitToPair(_) => "fit_to_pair",
            Command::SplitPair(_) => "split_pair",
            Command::MakePair { .. } => "make_pair",
            Command::RemoveMarker(_) => "remove_marker",
            Command::RemoveAllMarkers => "remove_all_markers",
            Command::SetSnap(_) => "set_snap",
            Command::AddDecoder { .. } => "add_decoder",
            Command::ConfigureDecoder { .. } => "configure_decoder",
            Command::SetDecoderChannel { .. } => "set_decoder_channel",
            Command::SetDecoderOption { .. } => "set_decoder_option",
            Command::RenameDecoder { .. } => "rename_decoder",
            Command::ShowDecoderLane { .. } => "show_decoder_lane",
            Command::RemoveDecoder(_) => "remove_decoder",
            Command::StackDecoder { .. } => "stack_decoder",
            Command::RunDecoder(_) => "run_decoder",
            Command::CancelDecoder(_) => "cancel_decoder",
            Command::SetDecoderMemoryLimit(_) => "set_decoder_memory_limit",
            Command::SetSaveDecoderResults(_) => "set_save_decoder_results",
            Command::SelectAnnotation { .. } => "select_annotation",
            Command::GoToAnnotation(_) => "go_to_annotation",
            Command::ClearAnnotationSelection => "clear_annotation_selection",
            Command::SaveSession => "save_session",
            Command::SaveSessionAs(_) => "save_session_as",
            Command::OpenSession(_) => "open_session",
            Command::CancelOperation => "cancel_operation",
            Command::Decide(_) => "decide",
            Command::SetLanguage(_) => "set_language",
            Command::SetTheme(_) => "set_theme",
            Command::SetUiScale(_) => "set_ui_scale",
            Command::SetMemoryLimit(_) => "set_memory_limit",
            Command::SetWindowGeometry(_) => "set_window_geometry",
            Command::DismissNotice(_) => "dismiss_notice",
            Command::DismissAllNotices => "dismiss_all_notices",
            Command::RequestClose => "request_close",
            Command::Quit => "quit",
            #[cfg(feature = "test-support")]
            Command::TestPanic => "test_panic",
            #[cfg(feature = "test-support")]
            Command::TestError(_) => "test_error",
        }
    }
}

/// The parts of the state a command or a poll changed; a shell redraws only those.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Changes(u32);

impl Changes {
    /// Nothing changed.
    pub const NONE: Changes = Changes(0);
    /// The device list or the selection.
    pub const DEVICES: Changes = Changes(1 << 0);
    /// The device configuration draft or its problems.
    pub const CONFIG: Changes = Changes(1 << 1);
    /// The capture shown.
    pub const CAPTURE: Changes = Changes(1 << 2);
    /// Progress of a capture, save or load.
    pub const PROGRESS: Changes = Changes(1 << 3);
    /// The visible window.
    pub const WINDOW: Changes = Changes(1 << 4);
    /// Channel names, colours, order or visibility.
    pub const PRESENTATION: Changes = Changes(1 << 5);
    /// The markers.
    pub const MARKERS: Changes = Changes(1 << 6);
    /// The notices.
    pub const NOTICES: Changes = Changes(1 << 7);
    /// The settings.
    pub const SETTINGS: Changes = Changes(1 << 8);
    /// What the application is busy with.
    pub const ACTIVITY: Changes = Changes(1 << 9);
    /// A question about unsaved data appeared or was answered.
    pub const DECISION: Changes = Changes(1 << 10);
    /// The language.
    pub const LANGUAGE: Changes = Changes(1 << 11);
    /// The theme or the interface scale.
    pub const THEME: Changes = Changes(1 << 12);
    /// The application should close.
    pub const QUIT: Changes = Changes(1 << 13);
    /// Decoder instances, their state, results or the annotation selection.
    pub const DECODERS: Changes = Changes(1 << 14);

    /// Whether nothing changed.
    #[must_use]
    pub fn is_empty(self) -> bool {
        self.0 == 0
    }

    /// Whether every part of `other` is in `self`.
    #[must_use]
    pub fn contains(self, other: Changes) -> bool {
        self.0 & other.0 == other.0
    }

    /// Adds the parts of `other`.
    pub fn insert(&mut self, other: Changes) {
        self.0 |= other.0;
    }
}

impl BitOr for Changes {
    type Output = Changes;

    fn bitor(self, rhs: Changes) -> Changes {
        Changes(self.0 | rhs.0)
    }
}

/// What happened to a command.
#[derive(Debug, Clone, PartialEq)]
pub enum Outcome {
    /// The command ran; these parts of the state changed.
    Done(Changes),
    /// Unsaved data would be lost: the command waits for a [`Decision`].
    NeedsDecision(Guarded),
    /// The command is not valid in the current state; nothing changed.
    Ignored(&'static str),
    /// The command failed; the notice says why.
    Failed(NoticeId),
}

/// How often the shell should call `poll`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tick {
    /// Something runs: about every 50 ms.
    Fast,
    /// Nothing runs: about twice a second (device hot-plug).
    Slow,
}
