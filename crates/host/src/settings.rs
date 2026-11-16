//! The persistent user settings (`settings.json` in the per-user configuration directory).
//!
//! Reading never fails the application: a missing file gives the defaults silently, any other
//! problem gives the defaults plus the error to report. Writing is atomic (temporary file, then
//! rename) so a crash never leaves a half-written file. Fields this version does not know are
//! kept when the file is saved again.

use std::collections::BTreeMap;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use la_core::Error;
use serde::{Deserialize, Serialize};

use crate::error;

/// The settings format version this program reads and writes.
pub const SETTINGS_VERSION: u32 = 1;
/// The largest settings file that is read (a larger one is treated as damaged).
pub const MAX_SETTINGS_BYTES: u64 = 1024 * 1024;
/// The most recent files that are remembered.
pub const MAX_RECENT_FILES: usize = 10;
/// The range of the interface scale, in percent.
pub const UI_SCALE_RANGE: (u16, u16) = (75, 200);
/// The largest memory limit that is accepted (1 TiB); a larger value falls back to the default.
pub const MAX_MEMORY_LIMIT_BYTES: u64 = 1 << 40;
/// The range of a usable window size in logical pixels.
const WINDOW_SIZE_RANGE: (u32, u32) = (200, 20_000);

/// The colour theme.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Theme {
    /// Follow the operating system.
    System,
    /// Always light.
    Light,
    /// Always dark.
    Dark,
    /// Maximum contrast: black and white with bright signal colours.
    HighContrast,
}

/// Where the window was.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct WindowGeometry {
    /// Left edge.
    pub x: i32,
    /// Top edge.
    pub y: i32,
    /// Width in logical pixels.
    pub width: u32,
    /// Height in logical pixels.
    pub height: u32,
    /// Whether the window was maximized.
    pub maximized: bool,
}

impl WindowGeometry {
    /// Whether the size is one a window can have.
    #[must_use]
    pub fn is_usable(&self) -> bool {
        let (low, high) = WINDOW_SIZE_RANGE;
        (low..=high).contains(&self.width) && (low..=high).contains(&self.height)
    }
}

/// The user's persistent choices.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    /// Format version (always [`SETTINGS_VERSION`] in memory).
    pub version: u32,
    /// The chosen language tag; `None` follows the operating system.
    pub language: Option<String>,
    /// The colour theme.
    pub theme: Theme,
    /// The interface scale in percent (75–200).
    pub ui_scale_percent: u16,
    /// The memory limit of a capture, in bytes.
    pub memory_limit_bytes: u64,
    /// The memory limit of the results of all decoders together, in bytes.
    pub decoder_memory_limit_bytes: u64,
    /// Whether the results of the decoders are saved with the session.
    pub save_decoder_results: bool,
    /// Whether markers snap to transitions.
    pub snap_markers: bool,
    /// Whether the view follows the newest data of a running capture.
    pub follow_latest: bool,
    /// The most recently saved or opened files, newest first.
    pub recent_files: Vec<String>,
    /// The window position and size.
    pub window: Option<WindowGeometry>,
    /// The device selected last time.
    pub last_device: Option<String>,
    /// The device configuration used last time (validated again before use).
    pub last_config: Option<serde_json::Value>,
    /// The view of the panel at the right edge: `capture`, `decoders`, `markers` or `hidden`.
    pub side_panel: Option<String>,
    /// Fields written by a newer version, kept as they are.
    #[serde(flatten)]
    pub unknown: BTreeMap<String, serde_json::Value>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            version: SETTINGS_VERSION,
            language: None,
            theme: Theme::System,
            ui_scale_percent: 100,
            memory_limit_bytes: default_memory_limit(),
            decoder_memory_limit_bytes: la_decoders::store::DEFAULT_MEMORY_LIMIT,
            save_decoder_results: false,
            snap_markers: true,
            follow_latest: true,
            recent_files: Vec::new(),
            window: None,
            last_device: None,
            last_config: None,
            side_panel: None,
            unknown: BTreeMap::new(),
        }
    }
}

fn default_memory_limit() -> u64 {
    la_storage::Limits::default().memory_budget_bytes
}

/// The smallest and the largest memory limit a capture accepts, in bytes.
#[must_use]
pub fn memory_limit_bounds() -> (u64, u64) {
    let mut low = 1_u64 << 20;
    while !memory_limit_is_valid(low) && low < MAX_MEMORY_LIMIT_BYTES {
        low = low.saturating_mul(2);
    }
    (low, MAX_MEMORY_LIMIT_BYTES)
}

/// The smallest and the largest memory limit of decoder results, in bytes (16 MiB to 4 GiB).
pub const DECODER_MEMORY_LIMIT_RANGE: (u64, u64) = (16 << 20, 4 << 30);

/// Whether `bytes` is an acceptable memory limit for decoder results.
#[must_use]
pub fn decoder_memory_limit_is_valid(bytes: u64) -> bool {
    (DECODER_MEMORY_LIMIT_RANGE.0..=DECODER_MEMORY_LIMIT_RANGE.1).contains(&bytes)
}

/// Whether `bytes` is an acceptable memory limit for a capture.
pub fn memory_limit_is_valid(bytes: u64) -> bool {
    bytes <= MAX_MEMORY_LIMIT_BYTES
        && la_storage::Limits {
            memory_budget_bytes: bytes,
            ..la_storage::Limits::default()
        }
        .validate()
        .is_ok()
}

impl Settings {
    /// The settings with every value forced into its allowed range.
    #[must_use]
    pub fn normalized(mut self) -> Self {
        self.version = SETTINGS_VERSION;
        self.ui_scale_percent = self
            .ui_scale_percent
            .clamp(UI_SCALE_RANGE.0, UI_SCALE_RANGE.1);
        if !memory_limit_is_valid(self.memory_limit_bytes) {
            self.memory_limit_bytes = default_memory_limit();
        }
        if !decoder_memory_limit_is_valid(self.decoder_memory_limit_bytes) {
            self.decoder_memory_limit_bytes = la_decoders::store::DEFAULT_MEMORY_LIMIT;
        }
        self.recent_files.truncate(MAX_RECENT_FILES);
        if self.window.is_some_and(|w| !w.is_usable()) {
            self.window = None;
        }
        self
    }

    /// Reads the settings. Never fails: see [`Loaded::problem`].
    #[must_use]
    pub fn load(fs: &dyn SettingsFs, path: &Path) -> Loaded {
        let defaults = |problem: Option<Error>| Loaded {
            settings: Settings::default(),
            problem,
        };
        let bytes = match fs.read(path, MAX_SETTINGS_BYTES) {
            Ok(None) => return defaults(None),
            Ok(Some(bytes)) => bytes,
            Err(e) => {
                return defaults(Some(
                    error::settings_unreadable(format!("cannot read the settings file: {e}"))
                        .with_source(e),
                ));
            }
        };
        let invalid = |why: &str| defaults(Some(error::settings_invalid(why)));
        if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > MAX_SETTINGS_BYTES {
            return invalid("the settings file is larger than the allowed maximum");
        }
        let Ok(text) = std::str::from_utf8(&bytes) else {
            return invalid("the settings file is not valid UTF-8");
        };
        let Ok(value) = serde_json::from_str::<serde_json::Value>(text) else {
            return invalid("the settings file is not valid JSON");
        };
        match value.get("version").and_then(serde_json::Value::as_u64) {
            Some(v) if v == u64::from(SETTINGS_VERSION) => {}
            Some(_) => return invalid("the settings file has an unsupported version"),
            None => return invalid("the settings file has no version"),
        }
        match serde_json::from_value::<Settings>(value) {
            Ok(settings) => Loaded {
                settings: settings.normalized(),
                problem: None,
            },
            Err(_) => invalid("the settings file has a field of the wrong type or value"),
        }
    }

    /// Writes the settings atomically.
    ///
    /// # Errors
    ///
    /// `host.settings_write` if any step fails; the previous file is then untouched.
    pub fn save(&self, fs: &dyn SettingsFs, path: &Path) -> Result<(), Error> {
        let bytes = serde_json::to_vec_pretty(self)
            .map_err(|e| error::settings_write(format!("cannot encode the settings: {e}")))?;
        let write_error = |what: &str, e: io::Error| {
            error::settings_write(format!("cannot {what} the settings file: {e}")).with_source(e)
        };
        let directory = path.parent().unwrap_or_else(|| Path::new("."));
        fs.create_dir_all(directory)
            .map_err(|e| write_error("create the directory of", e))?;
        let temp = temporary_name(directory);
        if let Err(e) = fs.write_new(&temp, &bytes) {
            let _ = fs.remove(&temp);
            return Err(write_error("write", e));
        }
        if let Err(e) = fs.rename(&temp, path) {
            let _ = fs.remove(&temp);
            return Err(write_error("replace", e));
        }
        Ok(())
    }
}

/// A unique name for the temporary file next to the settings file.
fn temporary_name(directory: &Path) -> PathBuf {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let unique = COUNTER.fetch_add(1, Ordering::Relaxed);
    directory.join(format!(".settings.{}.{unique}.tmp", std::process::id()))
}

/// The result of reading the settings.
#[derive(Debug)]
pub struct Loaded {
    /// The settings, the defaults when the file was missing or unusable.
    pub settings: Settings,
    /// Why the file could not be used (`None` for a missing or good file).
    pub problem: Option<Error>,
}

/// The file operations of the settings, behind a seam so tests can inject failures.
pub trait SettingsFs: Send + Sync {
    /// Reads at most `max_bytes + 1` bytes of the file; `None` if it does not exist.
    ///
    /// # Errors
    ///
    /// Any I/O error other than "not found".
    fn read(&self, path: &Path, max_bytes: u64) -> io::Result<Option<Vec<u8>>>;

    /// Creates the directory and its parents.
    ///
    /// # Errors
    ///
    /// The I/O error of the creation.
    fn create_dir_all(&self, dir: &Path) -> io::Result<()>;

    /// Creates a new file with this content, flushed to stable storage; fails if it exists.
    ///
    /// # Errors
    ///
    /// The I/O error of any step.
    fn write_new(&self, path: &Path, bytes: &[u8]) -> io::Result<()>;

    /// Atomically moves `from` over `to`.
    ///
    /// # Errors
    ///
    /// The I/O error of the rename.
    fn rename(&self, from: &Path, to: &Path) -> io::Result<()>;

    /// Removes a file (a missing file is not an error).
    ///
    /// # Errors
    ///
    /// The I/O error of the removal.
    fn remove(&self, path: &Path) -> io::Result<()>;
}

/// The real file system.
#[derive(Debug, Default, Clone, Copy)]
pub struct RealSettingsFs;

impl SettingsFs for RealSettingsFs {
    fn read(&self, path: &Path, max_bytes: u64) -> io::Result<Option<Vec<u8>>> {
        let file = match std::fs::File::open(path) {
            Ok(file) => file,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(e),
        };
        let mut bytes = Vec::new();
        file.take(max_bytes.saturating_add(1))
            .read_to_end(&mut bytes)?;
        Ok(Some(bytes))
    }

    fn create_dir_all(&self, dir: &Path) -> io::Result<()> {
        std::fs::create_dir_all(dir)
    }

    fn write_new(&self, path: &Path, bytes: &[u8]) -> io::Result<()> {
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)?;
        file.write_all(bytes)?;
        file.flush()?;
        file.sync_all()
    }

    fn rename(&self, from: &Path, to: &Path) -> io::Result<()> {
        std::fs::rename(from, to)
    }

    fn remove(&self, path: &Path) -> io::Result<()> {
        match std::fs::remove_file(path) {
            Err(e) if e.kind() != io::ErrorKind::NotFound => Err(e),
            _ => Ok(()),
        }
    }
}
