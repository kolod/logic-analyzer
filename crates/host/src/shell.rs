//! The shell: executes commands on a working copy of the state and commits it on success.

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use la_core::Error;
use la_i18n::{Args, Key, Language, Localizer};

use crate::command::{Changes, Command, Outcome, Tick};
use crate::error;
use crate::notices::{Notice, NoticeContent, NoticeId, Severity, notice_text};
use crate::settings::{RealSettingsFs, Settings, SettingsFs};
use crate::state::{Activity, State, Unsaved, Versions};
use crate::window::{ViewSize, ViewWindow, WindowLimits};

/// How the shell is set up.
#[derive(Debug, Clone)]
pub struct ShellConfig {
    /// Where the settings file is.
    pub settings_path: PathBuf,
    /// The system's preferred language tags, in order (used when the user has not chosen one).
    pub system_languages: Vec<String>,
    /// Where the log files are, if known (shown to the user on request).
    pub log_location: Option<PathBuf>,
}

impl ShellConfig {
    /// The configuration of a real run: the per-user configuration directory and the
    /// system's languages.
    #[must_use]
    pub fn for_user() -> Self {
        let directory = directories::ProjectDirs::from("", "", "logic-analyzer").map_or_else(
            || std::env::temp_dir().join("logic-analyzer"),
            |dirs| dirs.config_dir().to_path_buf(),
        );
        Self {
            settings_path: directory.join("settings.json"),
            system_languages: Language::system_tags(),
            log_location: Some(la_diagnostics_log_dir()),
        }
    }

    /// A configuration for tests: settings in `directory`, English as the system language.
    #[must_use]
    pub fn for_tests(directory: &Path) -> Self {
        Self {
            settings_path: directory.join("settings.json"),
            system_languages: Vec::new(),
            log_location: None,
        }
    }
}

fn la_diagnostics_log_dir() -> PathBuf {
    la_diagnostics::LogConfig::default().directory
}

/// Why a command did not complete.
enum Reject {
    /// Not valid now; nothing to report to the user.
    Ignored(&'static str),
    /// Failed; the user is told.
    Failed(Error),
}

/// The host: state, commands and text.
pub struct Shell {
    state: State,
    localizer: Localizer,
    config: ShellConfig,
    settings_fs: Arc<dyn SettingsFs>,
}

fn now_seconds() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(i64::MAX))
}

impl Shell {
    /// Builds the shell: loads the settings (a problem becomes a notice, never a failure), chooses
    /// the language and starts without a capture.
    #[must_use]
    pub fn new(config: ShellConfig) -> Self {
        let settings_fs: Arc<dyn SettingsFs> = Arc::new(RealSettingsFs);
        let loaded = Settings::load(settings_fs.as_ref(), &config.settings_path);
        let system: Vec<&str> = config.system_languages.iter().map(String::as_str).collect();
        let localizer = Localizer::from_settings(loaded.settings.language.as_deref(), &system);
        let view_size = ViewSize::default();
        let limits = WindowLimits {
            capture_length: 0,
            pixels: view_size.pixels,
        };
        let state = State {
            version: 0,
            versions: Versions::default(),
            activity: Activity::Idle,
            unsaved: Unsaved::Clean,
            view_size,
            window: ViewWindow::fit_all(&limits),
            follow_latest: loaded.settings.follow_latest,
            notices: crate::notices::NoticeList::new(),
            language: localizer.language(),
            settings: loaded.settings,
        };
        let mut shell = Shell {
            state,
            localizer,
            config,
            settings_fs,
        };
        if let Some(problem) = loaded.problem {
            shell.report_with(Severity::Warning, &problem);
        }
        shell
    }

    /// The current state.
    #[must_use]
    pub fn state(&self) -> &State {
        &self.state
    }

    /// The localizer (language, messages and formats).
    #[must_use]
    pub fn localizer(&self) -> &Localizer {
        &self.localizer
    }

    /// The text of a message of `ui.ftl` in the current language.
    #[must_use]
    pub fn text(&self, key: &Key, args: &Args) -> String {
        self.localizer.message(key, args)
    }

    /// The text of a notice in the current language.
    #[must_use]
    pub fn notice_text(&self, notice: &Notice) -> String {
        notice_text(notice, &self.localizer)
    }

    /// How often the shell wants to be polled.
    #[must_use]
    pub fn wants_ticks(&self) -> Tick {
        Tick::Slow
    }

    /// Where the log files are, if known.
    #[must_use]
    pub fn log_location(&self) -> Option<&Path> {
        self.config.log_location.as_deref()
    }

    /// Collects what happened in the background since the last call: device events, capture and
    /// operation progress. Reports which parts of the state changed.
    pub fn poll(&mut self, _now: Instant) -> Changes {
        Changes::NONE
    }

    /// Executes one command. A panic inside it is contained: the state stays as it was and the
    /// user gets a `host.internal` notice.
    pub fn apply(&mut self, command: Command) -> Outcome {
        let name = command.name();
        tracing::info!(target: "la_host", command = name, "command");
        let mut working = self.state.clone();
        let result = catch_unwind(AssertUnwindSafe(|| self.execute(&mut working, command)));
        match result {
            Ok(Ok(changes)) => {
                if !changes.is_empty() {
                    working.version = working.version.saturating_add(1);
                    working.versions.bump(changes);
                }
                self.state = working;
                Outcome::Done(changes)
            }
            Ok(Err(Reject::Ignored(code))) => {
                tracing::debug!(target: "la_host", command = name, code, "command ignored");
                Outcome::Ignored(code)
            }
            Ok(Err(Reject::Failed(error))) => Outcome::Failed(self.report(&error)),
            Err(_) => {
                tracing::error!(target: "la_host", command = name, "a command panicked");
                let error = error::internal(format!("the command {name} panicked"));
                Outcome::Failed(self.report(&error))
            }
        }
    }

    /// Adds a notice for `error` and logs it with its code and technical detail.
    fn report(&mut self, error: &Error) -> NoticeId {
        self.report_with(Severity::Error, error)
    }

    fn report_with(&mut self, severity: Severity, error: &Error) -> NoticeId {
        match severity {
            Severity::Error => {
                tracing::error!(target: "la_host", code = error.code(), detail = error.detail(), "failure");
            }
            _ => {
                tracing::warn!(target: "la_host", code = error.code(), detail = error.detail(), "problem");
            }
        }
        let id =
            self.state
                .notices
                .add(severity, NoticeContent::error(error.code()), now_seconds());
        self.state.versions.bump(Changes::NOTICES);
        id
    }

    /// Runs `command` against the working copy `state`.
    fn execute(&mut self, state: &mut State, command: Command) -> Result<Changes, Reject> {
        match command {
            Command::DismissNotice(id) => {
                state.notices.dismiss(id);
                Ok(Changes::NOTICES)
            }
            Command::DismissAllNotices => {
                state.notices.dismiss_all();
                Ok(Changes::NOTICES)
            }
            #[cfg(feature = "test-support")]
            #[allow(clippy::panic, reason = "provokes a contained panic for the tests")]
            Command::TestPanic => panic!("provoked panic"),
            #[cfg(feature = "test-support")]
            Command::TestError(code) => Err(Reject::Failed(Error::new(
                la_core::ErrorKind::Internal,
                code,
                "provoked failure",
            ))),
            _ => {
                let _ = &self.settings_fs;
                Err(Reject::Ignored(error::INVALID_COMMAND))
            }
        }
    }
}
