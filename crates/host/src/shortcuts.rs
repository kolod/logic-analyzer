//! The table of commands, menu paths and keyboard shortcuts.
//!
//! One table drives the menu bar, the tooltips and the keyboard handling of a user interface, so
//! every action has exactly one place where it is defined and a test can check that every command
//! is reachable and that no accelerator is used twice (contract `shell-layout.md`).

use crate::command::Command;
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
    /// Open the dialog that asks for a file name, then send `SaveSessionAs`.
    SaveFileAs,
    /// Open the settings dialog.
    Settings,
    /// Show where the log files are.
    ShowLog,
    /// Show the about dialog.
    About,
}

/// When an entry can be used.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EnabledWhen {
    /// Always.
    Always,
    /// Nothing is running.
    Idle,
    /// A capture is running.
    Capturing,
    /// A capture is on screen.
    CaptureExists,
    /// Idle, a device is selected and its configuration has no problem.
    CanStart,
}

impl EnabledWhen {
    /// Whether the entry is enabled in `state`.
    #[must_use]
    pub fn is_enabled(self, state: &State) -> bool {
        match self {
            EnabledWhen::Always => true,
            EnabledWhen::Idle => state.activity == Activity::Idle,
            EnabledWhen::Capturing => state.activity == Activity::Capturing,
            EnabledWhen::CaptureExists => state.capture.is_some(),
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
    vec![
        ShortcutEntry::new(
            Action::Command(Command::NewCapture),
            ["menu-file", "menu-file-new"],
            Some(Accelerator::ctrl("N")),
            EnabledWhen::Idle,
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
    ]
}
