//! The shell: executes commands on a working copy of the state and commits it on success.

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use la_core::Error;
use la_devices::{AcquisitionOptions, DeviceDescriptor, DeviceId, DeviceRegistry, RegistryConfig};
use la_i18n::{Args, Key, Language, Localizer};
use la_storage::Limits;

use crate::capture::{
    CaptureEnd, CaptureOrigin, CaptureView, ProgressView, Running, is_incomplete,
};
use crate::command::{
    Changes, Command, Decision, Guarded, Outcome, PanStep, Tick, ZoomAnchor, ZoomDirection,
};
use crate::devices::{ConfigDraft, fresh_seed, problems};
use crate::error;
use crate::markers::{Markers, MeasurementText, measurement, measurement_text, nearest_transition};
use crate::notices::{Notice, NoticeContent, NoticeId, NoticeList, Severity, notice_text};
use crate::plan::{ChannelView, FramePlan, MarkerPlan, build_plan};
use crate::presentation::Presentation;
use crate::sanitize::{MAX_NAME_CHARS, sanitize_text};
use crate::sessions::Operation;
use crate::settings::{RealSettingsFs, Settings, SettingsFs};
use crate::state::{Activity, State, Unsaved, Versions};
use crate::status::{StatusLine, status_line};
use crate::window::{ViewSize, ViewWindow, WindowLimits};

/// The widest signal area the shell plans for (the storage's default pixel limit).
const MAX_PLAN_PIXELS: u32 = 16_384;

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
            log_location: Some(la_diagnostics::LogConfig::default().directory),
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

/// Why a command did not complete.
pub(crate) enum Reject {
    /// Not valid now; nothing to report to the user.
    Ignored(&'static str),
    /// Failed; the user is told.
    Failed(Error),
}

/// What a command did.
pub(crate) enum Executed {
    /// It ran; these parts changed.
    Changed(Changes),
    /// Unsaved data would be lost: the command waits for a decision.
    Needs(Guarded),
}

/// The host: state, commands and text.
pub struct Shell {
    pub(crate) state: State,
    pub(crate) localizer: Localizer,
    pub(crate) config: ShellConfig,
    pub(crate) settings_fs: Arc<dyn SettingsFs>,
    registry: DeviceRegistry,
    pub(crate) running: Option<Running>,
    pub(crate) operation: Option<Operation>,
    #[cfg(feature = "test-support")]
    pub(crate) session_fs: Arc<dyn la_session::harness::FileSystem>,
    #[cfg(feature = "test-support")]
    test_failures: Vec<la_devices::ScriptedFailure>,
}

pub(crate) fn now_seconds() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(i64::MAX))
}

/// Adds a notice for `error` to `state` and logs it with its code and technical detail.
pub(crate) fn report_into(state: &mut State, severity: Severity, error: &Error) -> NoticeId {
    match severity {
        Severity::Error => {
            tracing::error!(target: "la_host", code = error.code(), detail = error.detail(), "failure");
        }
        _ => {
            tracing::warn!(target: "la_host", code = error.code(), detail = error.detail(), "problem");
        }
    }
    let id = state
        .notices
        .add(severity, NoticeContent::error(error.code()), now_seconds());
    state.versions.bump(Changes::NOTICES);
    id
}

/// The limits a capture is created with, from the user's memory limit.
pub(crate) fn capture_limits(settings: &Settings) -> Limits {
    Limits {
        memory_budget_bytes: settings.memory_limit_bytes,
        ..Limits::default()
    }
}

/// The limits of the window for a signal area `pixels` wide. The width is kept within what a plan
/// is built for, so every window the shell stores is valid for the same limits.
pub(crate) fn window_limits(capture_length: u64, pixels: u32) -> WindowLimits {
    WindowLimits {
        capture_length,
        pixels: pixels.clamp(1, MAX_PLAN_PIXELS),
    }
}

impl Shell {
    /// Builds the shell: loads the settings (a problem becomes a notice, never a failure), chooses
    /// the language, lists the devices and starts without a capture.
    #[must_use]
    pub fn new(config: ShellConfig) -> Self {
        let settings_fs: Arc<dyn SettingsFs> = Arc::new(RealSettingsFs);
        let loaded = Settings::load(settings_fs.as_ref(), &config.settings_path);
        let system: Vec<&str> = config.system_languages.iter().map(String::as_str).collect();
        let localizer = Localizer::from_settings(loaded.settings.language.as_deref(), &system);
        let registry = DeviceRegistry::new(RegistryConfig::default());
        let view_size = ViewSize::default();
        let devices = registry.list();
        let selected = default_device(&devices);
        let draft = selected
            .and_then(|id| devices.iter().find(|d| d.id == *id))
            .map_or_else(
                || ConfigDraft::for_device(&la_devices::virtual_descriptor().capabilities),
                |d| ConfigDraft::for_device(&d.capabilities),
            );
        let mut state = State {
            version: 0,
            versions: Versions::default(),
            activity: Activity::Idle,
            unsaved: Unsaved::Clean,
            view_size,
            window: ViewWindow::fit_all(&window_limits(0, view_size.pixels)),
            auto_fit: true,
            selected_device: selected.cloned(),
            devices,
            config: draft,
            config_problems: Vec::new(),
            capture: None,
            progress: None,
            presentation: Presentation::default(),
            markers: Markers::new(),
            session_path: None,
            operation: None,
            session_extras: Vec::new(),
            snap: loaded.settings.snap_markers,
            pending_decision: None,
            follow_latest: loaded.settings.follow_latest,
            notices: NoticeList::new(),
            language: localizer.language(),
            settings: loaded.settings,
        };
        revalidate(&mut state);
        sync_presentation(&mut state);
        if let Some(problem) = loaded.problem {
            report_into(&mut state, Severity::Warning, &problem);
        }
        Shell {
            state,
            localizer,
            config,
            settings_fs,
            registry,
            running: None,
            operation: None,
            #[cfg(feature = "test-support")]
            session_fs: Arc::new(la_session::harness::RealFileSystem),
            #[cfg(feature = "test-support")]
            test_failures: Vec::new(),
        }
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

    /// The status line in the current language.
    #[must_use]
    pub fn status_line(&self) -> StatusLine {
        status_line(&self.state, &self.localizer)
    }

    /// The table of actions with their menu paths and shortcuts.
    #[must_use]
    pub fn shortcut_table(&self) -> Vec<crate::shortcuts::ShortcutEntry> {
        crate::shortcuts::table()
    }

    /// How often the shell wants to be polled.
    #[must_use]
    pub fn wants_ticks(&self) -> Tick {
        if self.running.is_some() || self.operation.is_some() {
            Tick::Fast
        } else {
            Tick::Slow
        }
    }

    /// Writes the settings file now. A failure becomes a notice. The application calls this
    /// before it exits, so that what is only kept in memory while it runs (the window geometry)
    /// is remembered.
    pub fn save_settings(&mut self) {
        let mut working = self.state.clone();
        let changes = self.persist_settings(&mut working);
        if !changes.is_empty() {
            working.version = working.version.saturating_add(1);
            working.versions.bump(changes);
            self.state = working;
        }
    }

    /// Replaces the file system of the settings (tests: a disk that cannot be written).
    #[cfg(feature = "test-support")]
    pub fn set_settings_fs(&mut self, fs: Arc<dyn SettingsFs>) {
        self.settings_fs = fs;
    }

    /// Where the log files are, if known.
    #[must_use]
    pub fn log_location(&self) -> Option<&Path> {
        self.config.log_location.as_deref()
    }

    /// Scripts failures into the next captures of the virtual device (tests and failure checks).
    #[cfg(feature = "test-support")]
    pub fn script_failures(&mut self, failures: Vec<la_devices::ScriptedFailure>) {
        self.test_failures = failures;
    }

    /// Adds a device provider (tests and failure checks: scripted devices).
    #[cfg(feature = "test-support")]
    pub fn add_provider(&self, provider: Arc<dyn la_devices::DeviceProvider>) {
        self.registry.add_provider(provider);
    }

    /// The plan of the signal view for a signal area of `size`: a pure function of the state and
    /// the size, with the cost bound of constitution Principle VIII.
    #[must_use]
    pub fn frame_plan(&self, size: ViewSize) -> FramePlan {
        let pixels = size.pixels.clamp(1, MAX_PLAN_PIXELS);
        let capture = self.state.capture.as_ref();
        let length = capture.map_or(0, |c| c.snapshot.length());
        let limits = window_limits(length, pixels);
        let window = if self.state.auto_fit {
            ViewWindow::fit_all(&limits)
        } else {
            self.state.window.clamp(&limits)
        };
        let channels: Vec<ChannelView> = if let Some(capture) = capture {
            let recorded = capture.description.enabled_mask();
            self.state
                .presentation
                .channels
                .iter()
                .filter(|style| {
                    style.visible
                        && u32::from(style.index) < 32
                        && recorded >> u32::from(style.index) & 1 == 1
                })
                .map(|style| ChannelView {
                    channel: style.index,
                    name: style.name.clone(),
                    colour: style.colour,
                })
                .collect()
        } else {
            Vec::new()
        };
        let mut plan = build_plan(capture.map(|c| &c.snapshot), &window, pixels, &channels);
        if capture.is_some() && window.length > 0 {
            let scale = f64::from(pixels) / window.length as f64;
            plan.markers = self
                .state
                .markers
                .list
                .iter()
                .filter(|m| m.position >= window.start && m.position < window.end())
                .map(|m| MarkerPlan {
                    id: m.id,
                    position: m.position,
                    x: m.position.saturating_sub(window.start) as f64 * scale,
                    name: m.name.clone(),
                })
                .collect();
        }
        plan
    }

    /// The distance and frequency between the two selected markers, in the current language.
    #[must_use]
    pub fn measurement_text(&self) -> Option<MeasurementText> {
        let capture = self.state.capture.as_ref()?;
        let (a, b) = self.state.markers.measured_positions()?;
        let m = measurement(capture.snapshot.sample_rate_hz(), a, b)?;
        Some(measurement_text(&self.localizer, &m))
    }

    /// Collects what happened in the background since the last call: device changes, capture
    /// progress and the end of a capture. Reports which parts of the state changed. A panic inside
    /// is contained like one inside a command.
    pub fn poll(&mut self, now: Instant) -> Changes {
        let mut working = self.state.clone();
        let result = catch_unwind(AssertUnwindSafe(|| self.poll_inner(&mut working, now)));
        match result {
            Ok(changes) => {
                if !changes.is_empty() {
                    working.version = working.version.saturating_add(1);
                    working.versions.bump(changes);
                    self.state = working;
                }
                changes
            }
            Err(_) => {
                tracing::error!(target: "la_host", "polling panicked");
                let error = error::internal("polling the background work panicked");
                report_into(&mut self.state, Severity::Error, &error);
                Changes::NOTICES
            }
        }
    }

    fn poll_inner(&mut self, state: &mut State, now: Instant) -> Changes {
        let mut changes = self.poll_devices(state);
        changes.insert(self.poll_operation(state));
        changes.insert(self.poll_capture(state, now));
        changes
    }

    /// Brings the device list up to date; a vanished selection falls back to the default device.
    fn poll_devices(&mut self, state: &mut State) -> Changes {
        let list = self.registry.list();
        if list == state.devices {
            return Changes::NONE;
        }
        state.devices = list;
        let mut changes = Changes::DEVICES;
        let still_there = state
            .selected_device
            .as_ref()
            .is_some_and(|id| state.devices.iter().any(|d| d.id == *id));
        if !still_there && state.activity == Activity::Idle {
            let fallback = default_device(&state.devices).cloned();
            let name = state
                .selected_device
                .as_ref()
                .map_or_else(String::new, ToString::to_string);
            if let Some(id) = fallback {
                select_device(state, &id);
            } else {
                state.selected_device = None;
                state.config_problems.clear();
            }
            state.notices.add(
                Severity::Info,
                NoticeContent::info("notice-device-removed", Some(name)),
                now_seconds(),
            );
            changes.insert(Changes::CONFIG | Changes::NOTICES);
        }
        changes
    }

    /// Follows the running capture and finishes it when the acquisition has an outcome.
    fn poll_capture(&mut self, state: &mut State, now: Instant) -> Changes {
        let Some(run) = &self.running else {
            return Changes::NONE;
        };
        let mut changes = Changes::NONE;
        let acquisition = &run.acquisition;
        let _ = acquisition.drain_events();
        let progress = acquisition.progress();
        let snapshot = acquisition.capture().snapshot();
        let outcome = acquisition.outcome();
        let elapsed_ms = u64::try_from(now.saturating_duration_since(run.started).as_millis())
            .unwrap_or(u64::MAX);
        let view = ProgressView {
            elapsed_ms,
            samples: progress.samples_captured,
            positions: progress.positions,
            memory_used: snapshot.memory_used(),
            memory_limit: run.memory_limit,
            planned: run.planned,
        };
        if state.progress.as_ref() != Some(&view) {
            state.progress = Some(view);
            changes.insert(Changes::PROGRESS);
        }
        let description = acquisition.description();
        let end = outcome.as_ref().map(CaptureEnd::from_outcome);
        let outcome_is_none = outcome.is_none();
        let grown = state
            .capture
            .as_ref()
            .is_none_or(|c| c.snapshot.length() != snapshot.length() || c.end != end);
        if grown || outcome.is_some() {
            state.capture = Some(CaptureView {
                incomplete: is_incomplete(&snapshot) || end.is_some_and(CaptureEnd::is_abnormal),
                simulated: description.simulated(),
                snapshot: snapshot.clone(),
                description,
                origin: CaptureOrigin::Acquired,
                end,
            });
            changes.insert(Changes::CAPTURE);
            let limits = window_limits(snapshot.length(), plan_pixels(state));
            if state.auto_fit {
                state.window = ViewWindow::fit_all(&limits);
                changes.insert(Changes::WINDOW);
            } else {
                // A window made for a shorter (or empty) capture may no longer fit it: keep the
                // stored window valid, and follow the newest data when asked to.
                let mut next = state.window.clamp(&limits);
                if state.follow_latest && outcome_is_none {
                    next = next.go_to_end(&limits);
                }
                if next != state.window {
                    state.window = next;
                    changes.insert(Changes::WINDOW);
                }
            }
        }
        if let Some(outcome) = outcome {
            if let Some(error) = &outcome.error {
                report_into(state, Severity::Error, error);
                changes.insert(Changes::NOTICES);
            }
            state.activity = Activity::Idle;
            state.unsaved = Unsaved::Dirty;
            changes.insert(Changes::ACTIVITY);
            self.running = None;
        }
        changes
    }

    /// Executes one command. A panic inside it is contained: the state stays as it was and the
    /// user gets a `host.internal` notice.
    pub fn apply(&mut self, command: Command) -> Outcome {
        let name = command.name();
        // Commands that arrive many times a second (a drag, a resize, the wheel) are logged at
        // debug level, so they do not fill the log.
        if matches!(
            command,
            Command::SetViewSize(_)
                | Command::SetWindowGeometry(_)
                | Command::Pan { .. }
                | Command::Zoom { .. }
                | Command::MoveMarker { .. }
        ) {
            tracing::debug!(target: "la_host", command = name, "command");
        } else {
            tracing::info!(target: "la_host", command = name, "command");
        }
        let mut working = self.state.clone();
        let result = catch_unwind(AssertUnwindSafe(|| self.execute(&mut working, command)));
        match result {
            Ok(Ok(Executed::Changed(changes))) => {
                if !changes.is_empty() {
                    working.version = working.version.saturating_add(1);
                    working.versions.bump(changes);
                }
                self.state = working;
                Outcome::Done(changes)
            }
            Ok(Ok(Executed::Needs(guarded))) => {
                working.pending_decision = Some(guarded.clone());
                working.version = working.version.saturating_add(1);
                working.versions.bump(Changes::DECISION);
                self.state = working;
                Outcome::NeedsDecision(guarded)
            }
            Ok(Err(Reject::Ignored(code))) => {
                tracing::debug!(target: "la_host", command = name, code, "command ignored");
                Outcome::Ignored(code)
            }
            Ok(Err(Reject::Failed(error))) => {
                Outcome::Failed(report_into(&mut self.state, Severity::Error, &error))
            }
            Err(_) => {
                tracing::error!(target: "la_host", command = name, "a command panicked");
                let error = error::internal(format!("the command {name} panicked"));
                Outcome::Failed(report_into(&mut self.state, Severity::Error, &error))
            }
        }
    }

    /// Runs `command` against the working copy `state`.
    fn execute(&mut self, state: &mut State, command: Command) -> Result<Executed, Reject> {
        if self.operation.is_some() && command.waits_for_operation() {
            return Err(Reject::Ignored(error::INVALID_COMMAND));
        }
        let changed = |changes: Changes| Ok(Executed::Changed(changes));
        match command {
            Command::DismissNotice(id) => {
                state.notices.dismiss(id);
                changed(Changes::NOTICES)
            }
            Command::DismissAllNotices => {
                state.notices.dismiss_all();
                changed(Changes::NOTICES)
            }
            Command::RefreshDevices => {
                let list = self.registry.list();
                if list == state.devices {
                    return changed(Changes::NONE);
                }
                state.devices = list;
                changed(Changes::DEVICES)
            }
            Command::SelectDevice(id) => {
                if state.activity != Activity::Idle || !state.devices.iter().any(|d| d.id == id) {
                    return Err(Reject::Ignored(error::INVALID_COMMAND));
                }
                select_device(state, &id);
                changed(Changes::DEVICES | Changes::CONFIG)
            }
            Command::EditConfig(edit) => {
                if state.activity != Activity::Idle {
                    return Err(Reject::Ignored(error::INVALID_COMMAND));
                }
                state.config.apply(edit);
                revalidate(state);
                // The one flag of a channel follows what the next capture records.
                sync_visibility(state);
                changed(Changes::CONFIG | Changes::PRESENTATION)
            }
            Command::StartCapture => self.start_capture(state),
            Command::StopCapture => {
                let Some(run) = &self.running else {
                    return Err(Reject::Ignored(error::INVALID_COMMAND));
                };
                run.acquisition.stop();
                changed(Changes::NONE)
            }
            Command::NewCapture => {
                if state.capture.is_none() && state.activity == Activity::Idle {
                    return changed(Changes::NONE);
                }
                if state.unsaved != Unsaved::Clean || self.running.is_some() {
                    return Ok(self.guard(Guarded::NewCapture));
                }
                clear_capture(state);
                changed(Changes::CAPTURE | Changes::PROGRESS | Changes::WINDOW)
            }
            Command::RequestClose => self.quit_or_guard(state, Guarded::Close),
            Command::Quit => self.quit_or_guard(state, Guarded::Quit),
            Command::Decide(decision) => self.decide(state, decision),
            Command::SetLanguage(language) => {
                self.localizer.set_language(language);
                state.language = language;
                state.settings.language = Some(language.tag().to_owned());
                let saved = self.persist_settings(state);
                changed(Changes::LANGUAGE | Changes::SETTINGS | saved)
            }
            Command::SetTheme(theme) => {
                state.settings.theme = theme;
                let saved = self.persist_settings(state);
                changed(Changes::THEME | Changes::SETTINGS | saved)
            }
            Command::SetUiScale(percent) => {
                let (low, high) = crate::settings::UI_SCALE_RANGE;
                state.settings.ui_scale_percent = percent.clamp(low, high);
                let saved = self.persist_settings(state);
                changed(Changes::THEME | Changes::SETTINGS | saved)
            }
            Command::SetMemoryLimit(bytes) => {
                if !crate::settings::memory_limit_is_valid(bytes) {
                    return Err(Reject::Ignored(error::INVALID_COMMAND));
                }
                // A running capture keeps the limit it started with.
                state.settings.memory_limit_bytes = bytes;
                let saved = self.persist_settings(state);
                changed(Changes::SETTINGS | saved)
            }
            Command::SetWindowGeometry(geometry) => {
                if !geometry.is_usable() {
                    return Err(Reject::Ignored(error::INVALID_COMMAND));
                }
                state.settings.window = Some(geometry);
                changed(Changes::SETTINGS)
            }
            Command::SaveSession => {
                let path = state
                    .session_path
                    .clone()
                    .ok_or(Reject::Ignored(crate::command::NEEDS_PATH))?;
                self.begin_save(state, path)
            }
            Command::SaveSessionAs(path) => self.begin_save(state, path),
            Command::OpenSession(path) => {
                if state.unsaved != Unsaved::Clean || self.running.is_some() {
                    return Ok(self.guard(Guarded::OpenSession(path)));
                }
                if state.activity != Activity::Idle {
                    return Err(Reject::Ignored(error::INVALID_COMMAND));
                }
                self.begin_load(state, path)
            }
            Command::CancelOperation => match &self.operation {
                Some(operation) => {
                    operation.cancel();
                    changed(Changes::NONE)
                }
                None => Err(Reject::Ignored(error::INVALID_COMMAND)),
            },
            Command::SetViewSize(size) => {
                if size.pixels == 0 || size == state.view_size {
                    return changed(Changes::NONE);
                }
                state.view_size = size;
                let length = state.capture.as_ref().map_or(0, |c| c.snapshot.length());
                let limits = window_limits(length, size.pixels.min(MAX_PLAN_PIXELS));
                state.window = if state.auto_fit {
                    ViewWindow::fit_all(&limits)
                } else {
                    state.window.clamp(&limits)
                };
                changed(Changes::WINDOW)
            }
            Command::Zoom { factor, anchor } => {
                let limits =
                    capture_limits_of(state).ok_or(Reject::Ignored(error::INVALID_COMMAND))?;
                let pixels = u64::from(limits.pixels.max(1));
                let anchor = match anchor {
                    ZoomAnchor::Column(column) => (u64::from(column).min(pixels), pixels),
                    ZoomAnchor::Centre => (1, 2),
                };
                let current = current_window(state, &limits);
                state.window = current.zoom(&limits, factor.numerator, factor.denominator, anchor);
                state.auto_fit = false;
                changed(Changes::WINDOW)
            }
            Command::ZoomStep(direction) => {
                let (numerator, denominator) = match direction {
                    ZoomDirection::In => (1, 2),
                    ZoomDirection::Out => (2, 1),
                };
                self.execute(
                    state,
                    Command::Zoom {
                        factor: crate::command::ZoomFactor {
                            numerator,
                            denominator,
                        },
                        anchor: ZoomAnchor::Centre,
                    },
                )
            }
            Command::Pan { pixels } => {
                let limits =
                    capture_limits_of(state).ok_or(Reject::Ignored(error::INVALID_COMMAND))?;
                state.window = current_window(state, &limits).pan_pixels(&limits, pixels);
                state.auto_fit = false;
                state.follow_latest = false;
                changed(Changes::WINDOW)
            }
            Command::PanStep(step) => {
                let limits =
                    capture_limits_of(state).ok_or(Reject::Ignored(error::INVALID_COMMAND))?;
                let current = current_window(state, &limits);
                let tenth = i64::try_from(current.length / 10)
                    .unwrap_or(i64::MAX)
                    .max(1);
                let page = i64::try_from(current.length).unwrap_or(i64::MAX);
                let delta = match step {
                    PanStep::Left => tenth.saturating_neg(),
                    PanStep::Right => tenth,
                    PanStep::PageLeft => page.saturating_neg(),
                    PanStep::PageRight => page,
                };
                state.window = current.pan_samples(&limits, delta);
                state.auto_fit = false;
                state.follow_latest = false;
                changed(Changes::WINDOW)
            }
            Command::FitAll => {
                let limits =
                    capture_limits_of(state).ok_or(Reject::Ignored(error::INVALID_COMMAND))?;
                state.window = ViewWindow::fit_all(&limits);
                state.auto_fit = true;
                changed(Changes::WINDOW)
            }
            Command::GoToStart => {
                let limits =
                    capture_limits_of(state).ok_or(Reject::Ignored(error::INVALID_COMMAND))?;
                state.window = current_window(state, &limits).go_to_start(&limits);
                state.auto_fit = false;
                state.follow_latest = false;
                changed(Changes::WINDOW)
            }
            Command::GoToEnd => {
                let limits =
                    capture_limits_of(state).ok_or(Reject::Ignored(error::INVALID_COMMAND))?;
                state.window = current_window(state, &limits).go_to_end(&limits);
                state.auto_fit = false;
                changed(Changes::WINDOW)
            }
            Command::ZoomToRange { start, end } => {
                let limits =
                    capture_limits_of(state).ok_or(Reject::Ignored(error::INVALID_COMMAND))?;
                state.window = ViewWindow::zoom_to(&limits, start, end);
                state.auto_fit = false;
                state.follow_latest = false;
                changed(Changes::WINDOW)
            }
            Command::SetFollowLatest(follow) => {
                state.follow_latest = follow;
                changed(Changes::WINDOW)
            }
            Command::PlaceMarker { position, channel } => {
                let (snapshot_length, position) = marker_target(state, position, channel)?;
                let _ = snapshot_length;
                if state.markers.place(position).is_none() {
                    return Err(Reject::Ignored(error::INVALID_COMMAND));
                }
                mark_dirty(state);
                changed(Changes::MARKERS)
            }
            Command::MoveMarker {
                id,
                position,
                channel,
            } => {
                if state.markers.get(id).is_none() {
                    return Err(Reject::Ignored(error::INVALID_COMMAND));
                }
                let (_, position) = marker_target(state, position, channel)?;
                if let Some(marker) = state.markers.get_mut(id) {
                    marker.position = position;
                }
                mark_dirty(state);
                changed(Changes::MARKERS)
            }
            Command::RenameMarker { id, name } => {
                let Some(marker) = state.markers.get_mut(id) else {
                    return Err(Reject::Ignored(error::INVALID_COMMAND));
                };
                marker.name = sanitize_text(&name, MAX_NAME_CHARS);
                mark_dirty(state);
                changed(Changes::MARKERS)
            }
            Command::RemoveMarker(id) => {
                if !state.markers.remove(id) {
                    return Err(Reject::Ignored(error::INVALID_COMMAND));
                }
                mark_dirty(state);
                changed(Changes::MARKERS)
            }
            Command::RemoveAllMarkers => {
                state.markers.clear();
                mark_dirty(state);
                changed(Changes::MARKERS)
            }
            Command::SelectMarkers { first, second } => {
                if !state.markers.select(first, second) {
                    return Err(Reject::Ignored(error::INVALID_COMMAND));
                }
                changed(Changes::MARKERS)
            }
            Command::GoToMarker(id) => {
                let position = state
                    .markers
                    .get(id)
                    .map(|m| m.position)
                    .ok_or(Reject::Ignored(error::INVALID_COMMAND))?;
                let limits =
                    capture_limits_of(state).ok_or(Reject::Ignored(error::INVALID_COMMAND))?;
                state.window = current_window(state, &limits).reveal(&limits, position);
                state.auto_fit = false;
                state.follow_latest = false;
                changed(Changes::WINDOW)
            }
            Command::SetSnap(snap) => {
                state.snap = snap;
                changed(Changes::MARKERS)
            }
            Command::RenameChannel { channel, name } => {
                edit_presentation(state, |p| p.rename(channel, &name))
            }
            Command::SetChannelColour { channel, colour } => {
                edit_presentation(state, |p| p.set_colour(channel, colour))
            }
            Command::SetChannelVisible { channel, visible } => {
                // One flag: the channel is shown, and the next capture records it. While a
                // capture runs, what it records cannot change, so neither can the flag.
                if state.activity != Activity::Idle
                    || !state.presentation.set_visible(channel, visible)
                {
                    return Err(Reject::Ignored(error::INVALID_COMMAND));
                }
                let bit =
                    1_u32.checked_shl(u32::from(channel)).unwrap_or(0) & device_channel_mask(state);
                if visible {
                    state.config.enabled_channels |= bit;
                } else {
                    state.config.enabled_channels &= !bit;
                }
                revalidate(state);
                mark_dirty(state);
                changed(Changes::PRESENTATION | Changes::CONFIG)
            }
            Command::MoveChannel { from, to } => {
                edit_presentation(state, |p| p.move_channel(from, to))
            }
            Command::ResetPresentation => {
                state.presentation = Presentation::defaults(
                    presentation_channels(state),
                    state.config.enabled_channels,
                );
                sync_visibility(state);
                mark_dirty(state);
                changed(Changes::PRESENTATION)
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
        }
    }

    /// Stops a running capture (its data is kept) and asks the user what to do with unsaved data.
    fn guard(&mut self, guarded: Guarded) -> Executed {
        if let Some(run) = &self.running {
            run.acquisition.stop();
        }
        Executed::Needs(guarded)
    }

    fn quit_or_guard(&mut self, state: &State, guarded: Guarded) -> Result<Executed, Reject> {
        if state.unsaved != Unsaved::Clean || self.running.is_some() {
            return Ok(self.guard(guarded));
        }
        Ok(Executed::Changed(Changes::QUIT))
    }

    fn decide(&mut self, state: &mut State, decision: Decision) -> Result<Executed, Reject> {
        let Some(guarded) = state.pending_decision.clone() else {
            return Err(Reject::Ignored(error::INVALID_COMMAND));
        };
        match decision {
            Decision::Cancel => {
                state.pending_decision = None;
                Ok(Executed::Changed(Changes::DECISION))
            }
            Decision::Save => {
                // A capture that is still stopping cannot be saved yet: the interface offers the
                // choice only when the activity is idle.
                if state.capture.is_none()
                    || self.running.is_some()
                    || state.activity != Activity::Idle
                {
                    return Err(Reject::Ignored(error::INVALID_COMMAND));
                }
                let path = state
                    .session_path
                    .clone()
                    .ok_or(Reject::Ignored(crate::command::NEEDS_PATH))?;
                self.begin_save(state, path)
            }
            Decision::Discard => {
                state.pending_decision = None;
                // The data is thrown away: let the acquisition end unobserved. The shell is idle
                // from now on, with the data so far still on screen (and unsaved) until the
                // command that waited replaces it.
                let stopped = self.running.take();
                if let Some(run) = &stopped {
                    run.acquisition.stop();
                    state.activity = Activity::Idle;
                    state.unsaved = Unsaved::Dirty;
                }
                let mut changes = Changes::DECISION;
                match self.continue_guarded(state, guarded) {
                    Ok(more) => changes.insert(more),
                    Err(rejected) => {
                        // The command did not run and the state is rolled back: the capture
                        // that was stopped is watched again (it ends by itself).
                        self.running = stopped;
                        return Err(rejected);
                    }
                }
                Ok(Executed::Changed(changes))
            }
        }
    }

    fn start_capture(&mut self, state: &mut State) -> Result<Executed, Reject> {
        if state.activity != Activity::Idle
            || state.selected_device.is_none()
            || !state.config_problems.is_empty()
        {
            return Err(Reject::Ignored(error::INVALID_COMMAND));
        }
        if state.capture.is_some() && state.unsaved != Unsaved::Clean {
            return Ok(self.guard(Guarded::StartCapture));
        }
        self.begin_capture(state)
    }

    /// Starts the acquisition for the selected device with the validated draft.
    pub(crate) fn begin_capture(&mut self, state: &mut State) -> Result<Executed, Reject> {
        let device: DeviceDescriptor = state
            .selected_device
            .as_ref()
            .and_then(|id| state.devices.iter().find(|d| d.id == *id))
            .cloned()
            .ok_or(Reject::Ignored(error::INVALID_COMMAND))?;
        let seed = state.config.seed.unwrap_or_else(fresh_seed);
        #[cfg(feature = "test-support")]
        let failures = self.test_failures.clone();
        #[cfg(not(feature = "test-support"))]
        let failures = Vec::new();
        let config = state.config.to_device_config(&device, seed, &failures);
        let limits = capture_limits(&state.settings);
        let options = AcquisitionOptions {
            limits,
            chunk_samples: chunk_samples_for(
                limits.memory_budget_bytes,
                device.capabilities.channels,
            ),
            ..AcquisitionOptions::default()
        };
        let acquisition = self
            .registry
            .start(&device.id, config, options)
            .map_err(Reject::Failed)?;
        let snapshot = acquisition.capture().snapshot();
        let description = acquisition.description();
        state.capture = Some(CaptureView::new(
            snapshot,
            description,
            CaptureOrigin::Acquired,
            None,
        ));
        state.progress = Some(ProgressView {
            elapsed_ms: 0,
            samples: 0,
            positions: 0,
            memory_used: 0,
            memory_limit: limits.memory_budget_bytes,
            planned: Some(state.config.length),
        });
        sync_presentation(state);
        state.markers = Markers::new();
        state.activity = Activity::Capturing;
        state.unsaved = Unsaved::Running;
        state.auto_fit = true;
        state.pending_decision = None;
        state.window = ViewWindow::fit_all(&window_limits(0, state.view_size.pixels));
        self.running = Some(Running {
            acquisition,
            started: Instant::now(),
            memory_limit: limits.memory_budget_bytes,
            planned: Some(state.config.length),
        });
        Ok(Executed::Changed(
            Changes::ACTIVITY
                | Changes::CAPTURE
                | Changes::PROGRESS
                | Changes::WINDOW
                | Changes::DECISION,
        ))
    }
}

/// The chunk size of an acquisition: storing a chunk needs up to `4 × samples × channels` bytes
/// of temporary memory, which must stay well inside the memory budget (an eighth of it).
fn chunk_samples_for(budget_bytes: u64, channels: u8) -> usize {
    let per_sample = u64::from(channels.max(1)).saturating_mul(32);
    let samples = budget_bytes.checked_div(per_sample).unwrap_or(0);
    usize::try_from(samples.clamp(64, 16_384)).unwrap_or(16_384)
}

/// The device selected when nothing else is: the first simulated one, else the first.
fn default_device(devices: &[DeviceDescriptor]) -> Option<&DeviceId> {
    devices
        .iter()
        .find(|d| d.simulated)
        .or_else(|| devices.first())
        .map(|d| &d.id)
}

/// Selects `id` and resets the draft to the defaults of that device.
fn select_device(state: &mut State, id: &DeviceId) {
    state.selected_device = Some(id.clone());
    if let Some(device) = state.devices.iter().find(|d| d.id == *id) {
        state.config = ConfigDraft::for_device(&device.capabilities);
    }
    if state.capture.is_none() {
        sync_presentation(state);
    }
    sync_visibility(state);
    revalidate(state);
}

/// Checks the draft against the selected device and the memory limit.
pub(crate) fn revalidate(state: &mut State) {
    let limits = capture_limits(&state.settings);
    state.config_problems = state
        .selected_device
        .as_ref()
        .and_then(|id| state.devices.iter().find(|d| d.id == *id))
        .map(|device| problems(device, &state.config, &limits))
        .unwrap_or_default();
}

/// Where a marker dropped at `position` ends up: clamped to the capture and, when snapping is
/// on, moved to the nearest transition of `channel` within about twelve pixels. Returns the
/// capture length too.
fn marker_target(state: &State, position: u64, channel: u8) -> Result<(u64, u64), Reject> {
    let capture = state
        .capture
        .as_ref()
        .ok_or(Reject::Ignored(error::INVALID_COMMAND))?;
    let length = capture.snapshot.length();
    if length == 0 {
        return Err(Reject::Ignored(error::INVALID_COMMAND));
    }
    let mut position = position.min(length.saturating_sub(1));
    if state.snap {
        let limits = window_limits(length, plan_pixels(state));
        let window = current_window(state, &limits);
        let radius = window
            .length
            .saturating_mul(12)
            .checked_div(u64::from(limits.pixels.max(1)))
            .unwrap_or(1)
            .max(1);
        if let Some(near) = nearest_transition(&capture.snapshot, channel, position, radius) {
            position = near.min(length.saturating_sub(1));
        }
    }
    Ok((length, position))
}

/// The default presentation of a capture: every channel in index order, the recorded ones shown.
pub(crate) fn default_presentation(capture: &CaptureView) -> Presentation {
    Presentation::defaults(
        capture.snapshot.channels(),
        capture.description.enabled_mask(),
    )
}

/// How many channels the selected device has.
pub(crate) fn device_channels(state: &State) -> u8 {
    state
        .selected_device
        .as_ref()
        .and_then(|id| state.devices.iter().find(|d| d.id == *id))
        .map_or(8, |d| d.capabilities.channels)
}

/// Makes the flag of every channel of the selected device the one the recording mask says.
pub(crate) fn sync_visibility(state: &mut State) {
    let mask = state.config.enabled_channels;
    let limit = device_channels(state);
    state.presentation.visible_from_mask(mask, limit);
}

/// The mask that has every channel of the selected device.
pub(crate) fn device_channel_mask(state: &State) -> u32 {
    1_u32
        .checked_shl(u32::from(device_channels(state)))
        .map_or(u32::MAX, |bit| bit.wrapping_sub(1))
}

/// How many channels the presentation describes: those of the capture on screen, or else those of
/// the selected device.
pub(crate) fn presentation_channels(state: &State) -> u8 {
    state.capture.as_ref().map_or_else(
        || {
            state
                .selected_device
                .as_ref()
                .and_then(|id| state.devices.iter().find(|d| d.id == *id))
                .map_or(8, |d| d.capabilities.channels)
        },
        |capture| capture.snapshot.channels(),
    )
}

/// Keeps the presentation as long as the channels it describes; what the user set is kept.
pub(crate) fn sync_presentation(state: &mut State) {
    let count = presentation_channels(state);
    let mask = state.config.enabled_channels;
    state.presentation.resize(count, mask);
}

/// The pixel width the window limits use.
fn plan_pixels(state: &State) -> u32 {
    state.view_size.pixels.clamp(1, MAX_PLAN_PIXELS)
}

/// The limits of the view of the capture on screen; `None` without a capture.
fn capture_limits_of(state: &State) -> Option<WindowLimits> {
    state
        .capture
        .as_ref()
        .map(|c| window_limits(c.snapshot.length(), plan_pixels(state)))
}

/// The window now shown (the whole capture while it follows "fit all").
fn current_window(state: &State, limits: &WindowLimits) -> ViewWindow {
    if state.auto_fit {
        ViewWindow::fit_all(limits)
    } else {
        state.window.clamp(limits)
    }
}

/// An unsaved capture stays unsaved; a saved one becomes unsaved when its presentation changes.
fn mark_dirty(state: &mut State) {
    if state.capture.is_some() && state.unsaved == Unsaved::Clean {
        state.unsaved = Unsaved::Dirty;
    }
}

/// Applies a presentation edit; the edit returns `false` for an unknown channel or position.
fn edit_presentation(
    state: &mut State,
    edit: impl FnOnce(&mut Presentation) -> bool,
) -> Result<Executed, Reject> {
    if !edit(&mut state.presentation) {
        return Err(Reject::Ignored(error::INVALID_COMMAND));
    }
    mark_dirty(state);
    Ok(Executed::Changed(Changes::PRESENTATION))
}

/// Removes the capture and everything about it.
pub(crate) fn clear_capture(state: &mut State) {
    state.markers = Markers::new();
    state.session_path = None;
    state.session_extras.clear();
    state.capture = None;
    state.progress = None;
    state.activity = Activity::Idle;
    state.unsaved = Unsaved::Clean;
    state.auto_fit = true;
    state.pending_decision = None;
    state.window = ViewWindow::fit_all(&window_limits(0, state.view_size.pixels));
    // What the user set for the channels (names, colours, order) stays for the next capture.
    sync_presentation(state);
}
