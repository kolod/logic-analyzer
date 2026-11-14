//! The shell state: the single description of what the window shows.
//!
//! A user interface reads it and never changes it; only [`Shell`](crate::Shell) does, by
//! executing commands on a working copy and committing the copy when the command succeeds.

use la_i18n::Language;

use crate::command::Changes;
use crate::notices::NoticeList;
use crate::settings::Settings;
use crate::window::{ViewSize, ViewWindow};

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
    /// Whether the view follows the newest data of a running capture.
    pub follow_latest: bool,
    /// The messages for the user.
    pub notices: NoticeList,
    /// The persistent user choices.
    pub settings: Settings,
    /// The language in use (mirrors the settings choice or the system default).
    pub language: Language,
}
