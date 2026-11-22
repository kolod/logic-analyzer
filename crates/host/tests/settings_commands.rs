//! Theme, scale, memory limit and the window geometry: validated, remembered, and a failing
//! disk reported (spec US5, FR-025).
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::panic
)]

mod common;

use std::io;
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

use common::{TempDir, finished_shell};
use la_host::notices::NoticeContent;
use la_host::settings::{RealSettingsFs, Settings, SettingsFs, Theme, WindowGeometry};
use la_host::{Activity, Command, ConfigEdit, Outcome, Shell, ShellConfig, error};

fn stored(dir: &TempDir) -> Settings {
    Settings::load(&RealSettingsFs, &dir.path().join("settings.json")).settings
}

#[test]
fn theme_and_scale_are_applied_clamped_and_remembered() {
    let dir = TempDir::new("settings-theme");
    let mut shell = Shell::new(ShellConfig::for_tests(dir.path()));
    assert!(matches!(
        shell.apply(Command::SetTheme(Theme::HighContrast)),
        Outcome::Done(_)
    ));
    assert_eq!(shell.state().settings.theme, Theme::HighContrast);
    for (asked, applied) in [(150, 150), (500, 200), (10, 75), (0, 75)] {
        shell.apply(Command::SetUiScale(asked));
        assert_eq!(shell.state().settings.ui_scale_percent, applied, "{asked}");
    }
    shell.apply(Command::SetUiScale(125));
    let on_disk = stored(&dir);
    assert_eq!(on_disk.theme, Theme::HighContrast);
    assert_eq!(on_disk.ui_scale_percent, 125);
    // Another start reads them.
    let again = Shell::new(ShellConfig::for_tests(dir.path()));
    assert_eq!(again.state().settings.ui_scale_percent, 125);
}

#[test]
fn the_memory_limit_is_validated_and_applies_to_the_next_capture_only() {
    let dir = TempDir::new("settings-memory");
    let mut shell = Shell::new(ShellConfig::for_tests(dir.path()));
    for bad in [0, 1, u64::MAX] {
        assert_eq!(
            shell.apply(Command::SetMemoryLimit(bad)),
            Outcome::Ignored(error::INVALID_COMMAND),
            "{bad}"
        );
    }
    let first = 256 * 1024 * 1024;
    assert!(matches!(
        shell.apply(Command::SetMemoryLimit(first)),
        Outcome::Done(_)
    ));
    // A slow capture starts with the first limit.
    for edit in [
        ConfigEdit::Channels(la_devices::ChannelSet::from_u32(0b11)),
        ConfigEdit::SampleRate(20_000),
        ConfigEdit::Length(10_000_000),
        ConfigEdit::Seed(Some(51)),
    ] {
        shell.apply(Command::EditConfig(edit));
    }
    shell.apply(Command::StartCapture);
    let deadline = Instant::now() + Duration::from_secs(20);
    while shell.state().progress.is_none() {
        shell.poll(Instant::now());
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(5));
    }
    let second = 512 * 1024 * 1024;
    assert!(matches!(
        shell.apply(Command::SetMemoryLimit(second)),
        Outcome::Done(_)
    ));
    shell.poll(Instant::now());
    assert_eq!(shell.state().progress.as_ref().unwrap().memory_limit, first);
    shell.apply(Command::StopCapture);
    while shell.state().activity != Activity::Idle {
        shell.poll(Instant::now());
        std::thread::sleep(Duration::from_millis(5));
    }
    // The next capture gets the new limit.
    shell.apply(Command::DismissAllNotices);
    let outcome = shell.apply(Command::StartCapture);
    // The unsaved capture asks first; discarding it starts the next one.
    if let Outcome::NeedsDecision(_) = outcome {
        shell.apply(Command::Decide(la_host::Decision::Discard));
    }
    shell.poll(Instant::now());
    let deadline = Instant::now() + Duration::from_secs(20);
    while shell.state().progress.is_none() {
        shell.poll(Instant::now());
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(5));
    }
    assert_eq!(
        shell.state().progress.as_ref().unwrap().memory_limit,
        second
    );
    shell.apply(Command::StopCapture);
    assert_eq!(stored(&dir).memory_limit_bytes, second);
}

#[test]
fn the_window_geometry_is_kept_and_written_when_asked() {
    let dir = TempDir::new("settings-window");
    let mut shell = Shell::new(ShellConfig::for_tests(dir.path()));
    let good = WindowGeometry {
        x: 40,
        y: 30,
        width: 1200,
        height: 800,
        maximized: false,
    };
    assert!(matches!(
        shell.apply(Command::SetWindowGeometry(good)),
        Outcome::Done(_)
    ));
    assert_eq!(shell.state().settings.window, Some(good));
    // Not written on every move; written when the application saves its settings (at exit).
    assert_eq!(stored(&dir).window, None);
    shell.save_settings();
    assert_eq!(stored(&dir).window, Some(good));
    // Nonsense is refused.
    let bad = WindowGeometry { width: 0, ..good };
    assert_eq!(
        shell.apply(Command::SetWindowGeometry(bad)),
        Outcome::Ignored(error::INVALID_COMMAND)
    );
    assert_eq!(shell.state().settings.window, Some(good));
}

/// A settings file system that cannot write.
struct ReadOnly;

impl SettingsFs for ReadOnly {
    fn read(&self, path: &Path, max_bytes: u64) -> io::Result<Option<Vec<u8>>> {
        RealSettingsFs.read(path, max_bytes)
    }
    fn create_dir_all(&self, dir: &Path) -> io::Result<()> {
        RealSettingsFs.create_dir_all(dir)
    }
    fn write_new(&self, _path: &Path, _bytes: &[u8]) -> io::Result<()> {
        Err(io::Error::new(io::ErrorKind::PermissionDenied, "read-only"))
    }
    fn rename(&self, from: &Path, to: &Path) -> io::Result<()> {
        RealSettingsFs.rename(from, to)
    }
    fn remove(&self, path: &Path) -> io::Result<()> {
        RealSettingsFs.remove(path)
    }
}

#[test]
fn a_settings_file_that_cannot_be_written_is_reported_and_the_choice_still_applies() {
    let dir = TempDir::new("settings-readonly");
    let mut shell = Shell::new(ShellConfig::for_tests(dir.path()));
    shell.set_settings_fs(Arc::new(ReadOnly));
    assert!(matches!(
        shell.apply(Command::SetTheme(Theme::Dark)),
        Outcome::Done(_)
    ));
    assert_eq!(shell.state().settings.theme, Theme::Dark);
    let codes: Vec<String> = shell
        .state()
        .notices
        .iter()
        .filter_map(|n| match &n.content {
            NoticeContent::Error { code } => Some(code.clone()),
            NoticeContent::Info { .. } => None,
        })
        .collect();
    assert_eq!(codes, [error::SETTINGS_WRITE]);
    assert!(!dir.path().join("settings.json").exists());
}

#[test]
fn a_damaged_settings_file_gives_the_defaults_and_a_warning_and_is_replaced_by_the_next_save() {
    let dir = TempDir::new("settings-damaged");
    std::fs::write(dir.path().join("settings.json"), b"{ not json at all").unwrap();
    let mut shell = Shell::new(ShellConfig::for_tests(dir.path()));
    assert_eq!(shell.state().settings, Settings::default());
    assert_eq!(shell.state().notices.len(), 1);
    shell.apply(Command::SetUiScale(110));
    assert_eq!(stored(&dir).ui_scale_percent, 110);
}

#[test]
fn the_choices_survive_a_capture_being_replaced() {
    let dir = TempDir::new("settings-survive");
    let mut shell = finished_shell(&dir, 0b11, 10_000, 52);
    shell.apply(Command::SetTheme(Theme::Light));
    shell.apply(Command::NewCapture);
    shell.apply(Command::Decide(la_host::Decision::Discard));
    assert_eq!(shell.state().settings.theme, Theme::Light);
}
