//! Logging configuration with validation and clamping.

use std::path::PathBuf;

use tracing::Level;

use crate::paths::default_log_dir;

/// Smallest accepted total log size: a smaller bound could not hold useful history.
pub const MIN_TOTAL_BYTES: u64 = 64 * 1024;
/// Default total log size per process.
pub const DEFAULT_TOTAL_BYTES: u64 = 8 * 1024 * 1024;
/// Allowed number of segment files per process.
pub const SEGMENTS_RANGE: (u8, u8) = (2, 16);
/// Allowed number of retained instances (processes) in the log directory.
pub const RETAINED_RANGE: (u8, u8) = (1, 50);

/// Name of the environment variable that overrides the log level.
pub const LEVEL_ENV_VAR: &str = "LA_LOG";

/// Where and how much to log.
///
/// Values outside the allowed ranges are clamped by [`LogConfig::normalized`], which
/// logging initialization always applies, so a bad value can never disable logging
/// or let it grow without bound.
#[derive(Debug, Clone)]
pub struct LogConfig {
    /// Directory that receives the log files; created if missing.
    pub directory: PathBuf,
    /// Upper bound on the total size of this process's log files, in bytes.
    pub max_total_bytes: u64,
    /// Number of segment files the bound is divided into (2..=16).
    pub segments: u8,
    /// Number of most recent instances whose logs are kept at start-up (1..=50).
    pub retained_instances: u8,
    /// Most verbose level that is written.
    pub level: Level,
}

impl Default for LogConfig {
    fn default() -> Self {
        let default_level = if cfg!(debug_assertions) {
            Level::DEBUG
        } else {
            Level::INFO
        };
        let level = std::env::var(LEVEL_ENV_VAR)
            .ok()
            .and_then(|v| parse_level(&v))
            .unwrap_or(default_level);
        Self {
            directory: default_log_dir(),
            max_total_bytes: DEFAULT_TOTAL_BYTES,
            segments: 4,
            retained_instances: 5,
            level,
        }
    }
}

impl LogConfig {
    /// Returns a copy with every value clamped into its allowed range.
    #[must_use]
    pub fn normalized(mut self) -> Self {
        self.max_total_bytes = self.max_total_bytes.max(MIN_TOTAL_BYTES);
        self.segments = self.segments.clamp(SEGMENTS_RANGE.0, SEGMENTS_RANGE.1);
        self.retained_instances = self
            .retained_instances
            .clamp(RETAINED_RANGE.0, RETAINED_RANGE.1);
        self
    }
}

/// Parses a log level name (`error`, `warn`, `info`, `debug`, `trace`), ignoring case
/// and surrounding whitespace.
#[must_use]
pub fn parse_level(text: &str) -> Option<Level> {
    match text.trim().to_ascii_lowercase().as_str() {
        "error" => Some(Level::ERROR),
        "warn" | "warning" => Some(Level::WARN),
        "info" => Some(Level::INFO),
        "debug" => Some(Level::DEBUG),
        "trace" => Some(Level::TRACE),
        _ => None,
    }
}
