//! The persistent user settings (data-model "Settings", research R8).
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::manual_range_contains
)]

use std::collections::BTreeMap;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use la_host::error;
use la_host::settings::{
    MAX_RECENT_FILES, MAX_SETTINGS_BYTES, Settings, SettingsFs, Theme, WindowGeometry,
};

/// An in-memory file system that can fail at a chosen mutating step.
#[derive(Default)]
struct TestFs {
    files: Mutex<BTreeMap<PathBuf, Vec<u8>>>,
    steps: Mutex<usize>,
    fail_at: Mutex<Option<usize>>,
    read_error: Mutex<Option<io::ErrorKind>>,
}

impl TestFs {
    fn with_file(path: &Path, bytes: &[u8]) -> Self {
        let fs = TestFs::default();
        fs.files
            .lock()
            .unwrap()
            .insert(path.to_path_buf(), bytes.to_vec());
        fs
    }

    fn step(&self) -> io::Result<()> {
        let mut steps = self.steps.lock().unwrap();
        *steps += 1;
        if *self.fail_at.lock().unwrap() == Some(*steps) {
            return Err(io::Error::other("injected failure"));
        }
        Ok(())
    }

    fn contents(&self, path: &Path) -> Option<Vec<u8>> {
        self.files.lock().unwrap().get(path).cloned()
    }

    fn file_count(&self) -> usize {
        self.files.lock().unwrap().len()
    }
}

impl SettingsFs for TestFs {
    fn read(&self, path: &Path, max_bytes: u64) -> io::Result<Option<Vec<u8>>> {
        if let Some(kind) = *self.read_error.lock().unwrap() {
            return Err(io::Error::from(kind));
        }
        let files = self.files.lock().unwrap();
        Ok(files.get(path).map(|bytes| {
            let take = usize::try_from(max_bytes.saturating_add(1)).unwrap();
            bytes.iter().copied().take(take).collect()
        }))
    }

    fn create_dir_all(&self, _dir: &Path) -> io::Result<()> {
        self.step()
    }

    fn write_new(&self, path: &Path, bytes: &[u8]) -> io::Result<()> {
        self.step()?;
        self.files
            .lock()
            .unwrap()
            .insert(path.to_path_buf(), bytes.to_vec());
        Ok(())
    }

    fn rename(&self, from: &Path, to: &Path) -> io::Result<()> {
        self.step()?;
        let mut files = self.files.lock().unwrap();
        let bytes = files
            .remove(from)
            .ok_or_else(|| io::Error::from(io::ErrorKind::NotFound))?;
        files.insert(to.to_path_buf(), bytes);
        Ok(())
    }

    fn remove(&self, path: &Path) -> io::Result<()> {
        self.files.lock().unwrap().remove(path);
        Ok(())
    }
}

fn path() -> PathBuf {
    PathBuf::from("/config/logic-analyzer/settings.json")
}

fn load(bytes: &[u8]) -> la_host::settings::Loaded {
    Settings::load(&TestFs::with_file(&path(), bytes), &path())
}

#[test]
fn defaults_are_sensible() {
    let s = Settings::default();
    assert_eq!(s.language, None);
    assert_eq!(s.theme, Theme::System);
    assert_eq!(s.ui_scale_percent, 100);
    assert_eq!(
        s.memory_limit_bytes,
        la_storage::Limits::default().memory_budget_bytes
    );
    assert!(s.snap_markers);
    assert!(s.follow_latest);
    assert!(s.recent_files.is_empty());
    assert_eq!(s.window, None);
}

#[test]
fn a_missing_file_gives_the_defaults_and_no_problem() {
    let loaded = Settings::load(&TestFs::default(), &path());
    assert_eq!(loaded.settings, Settings::default());
    assert!(loaded.problem.is_none());
}

#[test]
fn settings_survive_a_round_trip() {
    let fs = TestFs::default();
    let settings = Settings {
        language: Some("uk".into()),
        theme: Theme::Dark,
        ui_scale_percent: 150,
        memory_limit_bytes: 4 * 1024 * 1024 * 1024,
        snap_markers: false,
        follow_latest: false,
        recent_files: vec!["/a.lasession".into(), "/b.lasession".into()],
        window: Some(WindowGeometry {
            x: 10,
            y: 20,
            width: 1280,
            height: 800,
            maximized: true,
        }),
        last_device: Some("virtual".into()),
        ..Settings::default()
    };
    settings.save(&fs, &path()).unwrap();
    let loaded = Settings::load(&fs, &path());
    assert!(loaded.problem.is_none());
    assert_eq!(loaded.settings, settings);
    // Only the settings file remains: no temporary file is left behind.
    assert_eq!(fs.file_count(), 1);
}

#[test]
fn values_are_clamped_or_replaced_by_defaults() {
    let loaded = load(br#"{"version":1,"ui_scale_percent":10}"#);
    assert_eq!(loaded.settings.ui_scale_percent, 75);
    let loaded = load(br#"{"version":1,"ui_scale_percent":999}"#);
    assert_eq!(loaded.settings.ui_scale_percent, 200);
    let default_limit = Settings::default().memory_limit_bytes;
    let loaded = load(br#"{"version":1,"memory_limit_bytes":0}"#);
    assert_eq!(loaded.settings.memory_limit_bytes, default_limit);
    let loaded = load(br#"{"version":1,"memory_limit_bytes":1125899906842624}"#);
    assert_eq!(loaded.settings.memory_limit_bytes, default_limit);
    let loaded = load(br#"{"version":1,"memory_limit_bytes":5368709120}"#);
    assert_eq!(loaded.settings.memory_limit_bytes, 5_368_709_120);
    assert!(loaded.problem.is_none());
}

#[test]
fn the_recent_files_list_is_bounded() {
    let names: Vec<String> = (0..25).map(|i| format!("\"/f{i}.lasession\"")).collect();
    let json = format!(r#"{{"version":1,"recent_files":[{}]}}"#, names.join(","));
    let loaded = load(json.as_bytes());
    assert_eq!(MAX_RECENT_FILES, 15);
    assert_eq!(loaded.settings.recent_files.len(), 15);
    assert_eq!(loaded.settings.recent_files[0], "/f0.lasession");
}

#[test]
fn an_unusable_window_geometry_is_ignored() {
    let loaded =
        load(br#"{"version":1,"window":{"x":0,"y":0,"width":0,"height":600,"maximized":false}}"#);
    assert_eq!(loaded.settings.window, None);
    let loaded = load(
        br#"{"version":1,"window":{"x":0,"y":0,"width":100000,"height":600,"maximized":false}}"#,
    );
    assert_eq!(loaded.settings.window, None);
    let loaded = load(
        br#"{"version":1,"window":{"x":-50,"y":10,"width":1000,"height":600,"maximized":false}}"#,
    );
    assert!(loaded.settings.window.is_some());
}

#[test]
fn an_unsupported_language_tag_is_kept_for_the_localizer_to_judge() {
    // The settings file only stores the text; the localizer reports an unknown tag.
    let loaded = load(br#"{"version":1,"language":"zz"}"#);
    assert_eq!(loaded.settings.language.as_deref(), Some("zz"));
}

#[test]
fn unknown_fields_are_preserved_when_saving() {
    let fs = TestFs::with_file(
        &path(),
        br#"{"version":1,"theme":"light","future_field":{"a":[1,2,3]},"another":"x"}"#,
    );
    let loaded = Settings::load(&fs, &path());
    assert_eq!(loaded.settings.theme, Theme::Light);
    loaded.settings.save(&fs, &path()).unwrap();
    let text = String::from_utf8(fs.contents(&path()).unwrap()).unwrap();
    assert!(text.contains("future_field"), "{text}");
    assert!(text.contains("\"another\""), "{text}");
}

#[test]
fn a_file_that_cannot_be_read_gives_defaults_and_the_unreadable_code() {
    let fs = TestFs::with_file(&path(), b"{}");
    *fs.read_error.lock().unwrap() = Some(io::ErrorKind::PermissionDenied);
    let loaded = Settings::load(&fs, &path());
    assert_eq!(loaded.settings, Settings::default());
    assert_eq!(loaded.problem.unwrap().code(), error::SETTINGS_UNREADABLE);
}

#[test]
fn damaged_files_give_defaults_and_the_invalid_code() {
    let too_large = vec![b' '; usize::try_from(MAX_SETTINGS_BYTES).unwrap() + 1];
    let cases: Vec<(&str, Vec<u8>)> = vec![
        ("not utf-8", vec![0xff, 0xfe, 0x00, 0x80]),
        ("not json", b"theme = dark".to_vec()),
        ("empty", Vec::new()),
        ("json array", b"[1,2,3]".to_vec()),
        ("wrong version", br#"{"version":2}"#.to_vec()),
        ("no version", br#"{"theme":"dark"}"#.to_vec()),
        (
            "wrong type",
            br#"{"version":1,"ui_scale_percent":"big"}"#.to_vec(),
        ),
        ("bad theme", br#"{"version":1,"theme":"purple"}"#.to_vec()),
        ("too large", too_large),
    ];
    for (name, bytes) in cases {
        let loaded = load(&bytes);
        assert_eq!(loaded.settings, Settings::default(), "{name}");
        assert_eq!(
            loaded.problem.unwrap().code(),
            error::SETTINGS_INVALID,
            "{name}"
        );
    }
}

#[test]
fn a_failure_at_any_step_of_the_write_leaves_the_old_file_readable() {
    let old = Settings {
        theme: Theme::Light,
        ..Settings::default()
    };
    let new = Settings {
        theme: Theme::Dark,
        ..Settings::default()
    };
    // Steps: create directory, write the temporary file, rename.
    for failing_step in 1..=3 {
        let fs = TestFs::default();
        old.save(&fs, &path()).unwrap();
        *fs.steps.lock().unwrap() = 0;
        *fs.fail_at.lock().unwrap() = Some(failing_step);
        let err = new.save(&fs, &path()).unwrap_err();
        assert_eq!(err.code(), error::SETTINGS_WRITE, "step {failing_step}");
        *fs.fail_at.lock().unwrap() = None;
        let loaded = Settings::load(&fs, &path());
        assert!(loaded.problem.is_none(), "step {failing_step}");
        assert_eq!(loaded.settings.theme, Theme::Light, "step {failing_step}");
        assert_eq!(
            fs.file_count(),
            1,
            "no temporary file after step {failing_step}"
        );
    }
}
