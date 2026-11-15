//! The table of commands, menu paths and keyboard shortcuts.
//!
//! One table drives the menu bar, the tooltips and the keyboard handling of a user interface, so
//! every action has exactly one place where it is defined and a test can check that every command
//! is reachable and that no accelerator is used twice (contract `shell-layout.md`).

use crate::command::{Command, PanStep, ZoomDirection};
use crate::state::{Activity, State};

/// A keyboard shortcut. `key` is a name from a small fixed vocabulary: a letter or digit, `F1`..
/// `F12`, `Left`, `Right`, `Home`, `End`, `Delete`, `Escape`, or one of `+ - = [ ] ,`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Accelerator {
    /// Whether Ctrl (Command is not used) must be held.
    pub ctrl: bool,
    /// Whether Shift must be held.
    pub shift: bool,
    /// Whether Alt must be held.
    pub alt: bool,
    /// The key.
    pub key: &'static str,
}

impl Accelerator {
    /// The key alone.
    #[must_use]
    pub const fn key(key: &'static str) -> Self {
        Self {
            ctrl: false,
            shift: false,
            alt: false,
            key,
        }
    }

    /// Ctrl and the key.
    #[must_use]
    pub const fn ctrl(key: &'static str) -> Self {
        Self {
            ctrl: true,
            shift: false,
            alt: false,
            key,
        }
    }

    /// Ctrl, Shift and the key.
    #[must_use]
    pub const fn ctrl_shift(key: &'static str) -> Self {
        Self {
            ctrl: true,
            shift: true,
            alt: false,
            key,
        }
    }

    /// Shift and the key.
    #[must_use]
    pub const fn shift(key: &'static str) -> Self {
        Self {
            ctrl: false,
            shift: true,
            alt: false,
            key,
        }
    }
}

/// What an entry does.
#[derive(Debug, Clone, PartialEq)]
pub enum Action {
    /// Send this command to the host.
    Command(Command),
    /// Open the dialog that asks for a file to open, then send `OpenSession`.
    OpenFile,
    /// Save to the current file, or ask for a file name when there is none yet.
    SaveFile,
    /// Open the dialog that asks for a file name, then send `SaveSessionAs`.
    SaveFileAs,
    /// Open the settings dialog.
    Settings,
    /// Show where the log files are.
    ShowLog,
    /// Show the about dialog.
    About,
    /// Make the interface larger.
    UiScaleUp,
    /// Make the interface smaller.
    UiScaleDown,
    /// Turn following the newest data of a running capture on or off.
    ToggleFollowLatest,
    /// Place a marker at the pointer (or the centre of the view).
    PlaceMarker,
    /// Go to the previous marker.
    PreviousMarker,
    /// Go to the next marker.
    NextMarker,
    /// Remove the selected marker.
    RemoveSelectedMarker,
    /// Turn snapping of markers on or off.
    ToggleSnap,
}

/// When an entry can be used.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EnabledWhen {
    /// Always.
    Always,
    /// Nothing is running (no capture, no save, no load).
    Idle,
    /// No save or load is running.
    NoOperation,
    /// A capture is on screen and nothing is running.
    CanSave,
    /// A capture is running.
    Capturing,
    /// A capture is on screen.
    CaptureExists,
    /// There is at least one marker.
    MarkersExist,
    /// Idle, a device is selected and its configuration has no problem.
    CanStart,
}

impl EnabledWhen {
    /// Whether the entry is enabled in `state`.
    #[must_use]
    pub fn is_enabled(self, state: &State) -> bool {
        match self {
            EnabledWhen::Always => true,
            EnabledWhen::Idle => state.activity == Activity::Idle && state.operation.is_none(),
            EnabledWhen::NoOperation => state.operation.is_none(),
            EnabledWhen::CanSave => {
                state.capture.is_some()
                    && state.activity == Activity::Idle
                    && state.operation.is_none()
            }
            EnabledWhen::Capturing => state.activity == Activity::Capturing,
            EnabledWhen::CaptureExists => state.capture.is_some(),
            EnabledWhen::MarkersExist => !state.markers.list.is_empty(),
            EnabledWhen::CanStart => {
                state.activity == Activity::Idle
                    && state.selected_device.is_some()
                    && state.config_problems.is_empty()
            }
        }
    }
}

/// One row of the table.
#[derive(Debug, Clone, PartialEq)]
pub struct ShortcutEntry {
    /// What it does.
    pub action: Action,
    /// The menu path as message keys: the menu, then the entry.
    pub menu: [&'static str; 2],
    /// The keyboard shortcut, if any.
    pub accelerator: Option<Accelerator>,
    /// When it is enabled.
    pub enabled_when: EnabledWhen,
}

impl ShortcutEntry {
    fn new(
        action: Action,
        menu: [&'static str; 2],
        accelerator: Option<Accelerator>,
        enabled_when: EnabledWhen,
    ) -> Self {
        Self {
            action,
            menu,
            accelerator,
            enabled_when,
        }
    }
}

/// The table: every action of the application with its menu path and shortcut.
#[must_use]
pub fn table() -> Vec<ShortcutEntry> {
    let mut entries = vec![
        ShortcutEntry::new(
            Action::Command(Command::NewCapture),
            ["menu-file", "menu-file-new"],
            Some(Accelerator::ctrl("N")),
            EnabledWhen::Idle,
        ),
        ShortcutEntry::new(
            Action::OpenFile,
            ["menu-file", "menu-file-open"],
            Some(Accelerator::ctrl("O")),
            EnabledWhen::NoOperation,
        ),
        ShortcutEntry::new(
            Action::SaveFile,
            ["menu-file", "menu-file-save"],
            Some(Accelerator::ctrl("S")),
            EnabledWhen::CanSave,
        ),
        ShortcutEntry::new(
            Action::SaveFileAs,
            ["menu-file", "menu-file-save-as"],
            Some(Accelerator::ctrl_shift("S")),
            EnabledWhen::CanSave,
        ),
        ShortcutEntry::new(
            Action::Settings,
            ["menu-file", "menu-file-settings"],
            Some(Accelerator::ctrl(",")),
            EnabledWhen::Always,
        ),
        ShortcutEntry::new(
            Action::Command(Command::Quit),
            ["menu-file", "menu-file-quit"],
            Some(Accelerator::ctrl("Q")),
            EnabledWhen::Always,
        ),
        ShortcutEntry::new(
            Action::Command(Command::StartCapture),
            ["menu-capture", "menu-capture-start"],
            Some(Accelerator::key("F5")),
            EnabledWhen::CanStart,
        ),
        ShortcutEntry::new(
            Action::Command(Command::StopCapture),
            ["menu-capture", "menu-capture-stop"],
            Some(Accelerator::shift("F5")),
            EnabledWhen::Capturing,
        ),
        ShortcutEntry::new(
            Action::PlaceMarker,
            ["menu-markers", "menu-markers-place"],
            Some(Accelerator::key("M")),
            EnabledWhen::CaptureExists,
        ),
        ShortcutEntry::new(
            Action::PreviousMarker,
            ["menu-markers", "menu-markers-previous"],
            Some(Accelerator::key("[")),
            EnabledWhen::MarkersExist,
        ),
        ShortcutEntry::new(
            Action::NextMarker,
            ["menu-markers", "menu-markers-next"],
            Some(Accelerator::key("]")),
            EnabledWhen::MarkersExist,
        ),
        ShortcutEntry::new(
            Action::RemoveSelectedMarker,
            ["menu-markers", "menu-markers-remove"],
            Some(Accelerator::key("Delete")),
            EnabledWhen::MarkersExist,
        ),
        ShortcutEntry::new(
            Action::Command(Command::RemoveAllMarkers),
            ["menu-markers", "menu-markers-remove-all"],
            None,
            EnabledWhen::MarkersExist,
        ),
        ShortcutEntry::new(
            Action::ToggleSnap,
            ["menu-markers", "menu-markers-snap"],
            Some(Accelerator::key("S")),
            EnabledWhen::CaptureExists,
        ),
        ShortcutEntry::new(
            Action::UiScaleUp,
            ["menu-view", "menu-view-scale-up"],
            Some(Accelerator::ctrl("=")),
            EnabledWhen::Always,
        ),
        ShortcutEntry::new(
            Action::UiScaleDown,
            ["menu-view", "menu-view-scale-down"],
            Some(Accelerator::ctrl("-")),
            EnabledWhen::Always,
        ),
        ShortcutEntry::new(
            Action::About,
            ["menu-help", "menu-help-about"],
            Some(Accelerator::key("F1")),
            EnabledWhen::Always,
        ),
        ShortcutEntry::new(
            Action::ShowLog,
            ["menu-help", "menu-help-log"],
            None,
            EnabledWhen::Always,
        ),
        ShortcutEntry::new(
            Action::ToggleFollowLatest,
            ["menu-capture", "menu-capture-follow"],
            Some(Accelerator::key("F6")),
            EnabledWhen::Capturing,
        ),
        ShortcutEntry::new(
            Action::Command(Command::ZoomStep(ZoomDirection::In)),
            ["menu-view", "menu-view-zoom-in"],
            Some(Accelerator::key("+")),
            EnabledWhen::CaptureExists,
        ),
        ShortcutEntry::new(
            Action::Command(Command::ZoomStep(ZoomDirection::Out)),
            ["menu-view", "menu-view-zoom-out"],
            Some(Accelerator::key("-")),
            EnabledWhen::CaptureExists,
        ),
        ShortcutEntry::new(
            Action::Command(Command::FitAll),
            ["menu-view", "menu-view-fit"],
            Some(Accelerator::ctrl("0")),
            EnabledWhen::CaptureExists,
        ),
        ShortcutEntry::new(
            Action::Command(Command::GoToStart),
            ["menu-view", "menu-view-start"],
            Some(Accelerator::key("Home")),
            EnabledWhen::CaptureExists,
        ),
        ShortcutEntry::new(
            Action::Command(Command::GoToEnd),
            ["menu-view", "menu-view-end"],
            Some(Accelerator::key("End")),
            EnabledWhen::CaptureExists,
        ),
        ShortcutEntry::new(
            Action::Command(Command::PanStep(PanStep::Left)),
            ["menu-view", "menu-view-pan-left"],
            Some(Accelerator::key("Left")),
            EnabledWhen::CaptureExists,
        ),
        ShortcutEntry::new(
            Action::Command(Command::PanStep(PanStep::Right)),
            ["menu-view", "menu-view-pan-right"],
            Some(Accelerator::key("Right")),
            EnabledWhen::CaptureExists,
        ),
        ShortcutEntry::new(
            Action::Command(Command::PanStep(PanStep::PageLeft)),
            ["menu-view", "menu-view-page-left"],
            Some(Accelerator::shift("Left")),
            EnabledWhen::CaptureExists,
        ),
        ShortcutEntry::new(
            Action::Command(Command::PanStep(PanStep::PageRight)),
            ["menu-view", "menu-view-page-right"],
            Some(Accelerator::shift("Right")),
            EnabledWhen::CaptureExists,
        ),
    ];
    // The menus appear in this order; entries keep their order inside a menu.
    let rank = |entry: &ShortcutEntry| match entry.menu[0] {
        "menu-file" => 0,
        "menu-capture" => 1,
        "menu-view" => 2,
        "menu-markers" => 3,
        "menu-help" => 4,
        _ => 5,
    };
    entries.sort_by_key(rank);
    entries
}
