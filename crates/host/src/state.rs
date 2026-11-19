//! The shell state: the single description of what the window shows.
//!
//! A user interface reads it and never changes it; only [`Shell`](crate::Shell) does, by
//! executing commands on a working copy and committing the copy when the command succeeds.

use la_devices::{DeviceDescriptor, DeviceId};
use la_i18n::Language;

use crate::capture::{CaptureView, ProgressView};
use crate::command::{Changes, Guarded};
use crate::decoders::DecoderState;
use crate::devices::{ConfigDraft, ConfigProblem};
use crate::markers::{End, Markers, Placement, SavedView};
use crate::notices::NoticeList;
use crate::presentation::Presentation;
use crate::settings::Settings;
use crate::window::{ViewSize, ViewWindow};
use std::path::PathBuf;

/// A save or a load.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OperationKind {
    /// Writing a session file.
    Save,
    /// Reading a session file.
    Load,
    /// Building the cropped copy of the capture after a cut or a trim.
    Crop,
}

/// A save or load that is running.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OperationView {
    /// Which one.
    pub kind: OperationKind,
    /// Bytes done so far.
    pub done_bytes: u64,
    /// The size of the file (zero until known).
    pub total_bytes: u64,
}

/// What the application is busy with (at most one thing at a time).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Activity {
    /// Nothing.
    Idle,
    /// A capture is running.
    Capturing,
    /// A session is being saved.
    Saving,
    /// A session is being loaded.
    Loading,
    /// A cut or a trim of the capture is running (a cropped copy is being built).
    Cropping,
}

/// Whether data would be lost by discarding the current capture.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unsaved {
    /// Nothing to lose.
    Clean,
    /// The capture or its presentation has changes that are not saved.
    Dirty,
    /// A capture is running (and unsaved).
    Running,
}

/// Which view the panel at the right edge shows (`docs/host-api.md`, 1.2).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum SidePanel {
    /// The settings of the capture.
    #[default]
    Capture,
    /// The decoders.
    Decoders,
    /// The markers and the measurement.
    Markers,
    /// No panel: the signals take the width.
    Hidden,
}

impl SidePanel {
    /// The name kept in the settings file.
    #[must_use]
    pub fn as_setting(self) -> &'static str {
        match self {
            SidePanel::Capture => "capture",
            SidePanel::Decoders => "decoders",
            SidePanel::Markers => "markers",
            SidePanel::Hidden => "hidden",
        }
    }

    /// The view named in the settings file; anything else is the default.
    #[must_use]
    pub fn from_setting(text: &str) -> Self {
        match text {
            "decoders" => SidePanel::Decoders,
            "markers" => SidePanel::Markers,
            "hidden" => SidePanel::Hidden,
            _ => SidePanel::Capture,
        }
    }
}

/// The side of a signal on which `ScrollToEdge` looks for an edge.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EdgeDirection {
    /// The last edge before the visible window.
    Previous,
    /// The first edge after the visible window.
    Next,
}

/// Counters that increase when a part of the state changes, so a user interface can invalidate
/// exactly the caches that depend on it.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Versions {
    /// The capture shown.
    pub capture: u64,
    /// The visible window.
    pub window: u64,
    /// Channel presentation.
    pub presentation: u64,
    /// Markers.
    pub markers: u64,
    /// Theme and interface scale.
    pub theme: u64,
    /// The notices.
    pub notices: u64,
    /// Decoder instances, their state and the annotation selection.
    pub decoders: u64,
}

impl Versions {
    /// Increases the counters of the parts in `changes`.
    pub fn bump(&mut self, changes: Changes) {
        let parts = [
            (Changes::CAPTURE, &mut self.capture),
            (Changes::WINDOW, &mut self.window),
            (Changes::PRESENTATION, &mut self.presentation),
            (Changes::MARKERS, &mut self.markers),
            (Changes::THEME, &mut self.theme),
            (Changes::NOTICES, &mut self.notices),
            (Changes::DECODERS, &mut self.decoders),
        ];
        for (part, counter) in parts {
            if changes.contains(part) {
                *counter = counter.saturating_add(1);
            }
        }
    }
}

/// What the window shows.
#[derive(Debug, Clone)]
pub struct State {
    /// Increases with every successful command that changed something.
    pub version: u64,
    /// Per-part counters.
    pub versions: Versions,
    /// What the application is busy with.
    pub activity: Activity,
    /// Whether discarding the capture would lose data.
    pub unsaved: Unsaved,
    /// The size of the signal area the host was told about.
    pub view_size: ViewSize,
    /// The visible window.
    pub window: ViewWindow,
    /// Whether the window shows the whole capture (until the user zooms or pans).
    pub auto_fit: bool,
    /// The devices the registry lists.
    pub devices: Vec<DeviceDescriptor>,
    /// The selected device.
    pub selected_device: Option<DeviceId>,
    /// The configuration being edited.
    pub config: ConfigDraft,
    /// What the device would refuse in `config` (empty: Start is possible).
    pub config_problems: Vec<ConfigProblem>,
    /// The capture on screen.
    pub capture: Option<CaptureView>,
    /// Progress of the running capture, frozen when it ends.
    pub progress: Option<ProgressView>,
    /// How the channels of the capture are shown.
    pub presentation: Presentation,
    /// The markers of the capture.
    pub markers: Markers,
    /// The marker being placed with the mouse, if any.
    pub placement: Option<Placement>,
    /// The window an automatic zoom of a crowded snapping zone replaced; put back when the
    /// marker is fixed or the placement ends.
    pub snap_zoom: Option<SavedView>,
    /// The marker end being dragged (after a pair's ends crossed, the role it has now).
    pub drag: Option<(u32, End)>,
    /// The session file the capture was saved to or opened from.
    pub session_path: Option<PathBuf>,
    /// The save or load that is running.
    pub operation: Option<OperationView>,
    /// Optional sections found in the opened file that this version keeps as they are.
    pub(crate) session_extras: Vec<la_session::Section>,
    /// Whether markers snap to the nearest transition.
    pub snap: bool,
    /// A command waiting for the user to decide what happens to unsaved data.
    pub pending_decision: Option<Guarded>,
    /// Whether the view follows the newest data of a running capture.
    pub follow_latest: bool,
    /// The messages for the user.
    pub notices: NoticeList,
    /// The persistent user choices.
    pub settings: Settings,
    /// The language in use (mirrors the settings choice or the system default).
    pub language: Language,
    /// The decoder instances and what is selected in their results.
    pub decoders: DecoderState,
    /// The view of the panel at the right edge.
    pub side_panel: SidePanel,
}
