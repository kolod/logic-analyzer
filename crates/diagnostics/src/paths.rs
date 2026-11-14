//! Platform-specific log directory resolution.
//!
//! This is the only operating-system-specific code in the crate (constitution
//! Principle IV). On Debian the directory is `$XDG_STATE_HOME/logic-analyzer/logs`
//! (default `~/.local/state/logic-analyzer/logs`); on Windows it is below
//! `%LOCALAPPDATA%\logic-analyzer`.

use std::path::PathBuf;

/// The default directory for log files.
pub(crate) fn default_log_dir() -> PathBuf {
    match directories::ProjectDirs::from("", "", "logic-analyzer") {
        Some(dirs) => dirs
            .state_dir()
            .unwrap_or_else(|| dirs.data_local_dir())
            .join("logs"),
        None => std::env::temp_dir().join("logic-analyzer").join("logs"),
    }
}
