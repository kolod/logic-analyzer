//! The shell: executes commands on a working copy of the state and commits it on success.

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use la_core::Error;
use la_decoders::setup::Setup;
use la_devices::{AcquisitionOptions, DeviceDescriptor, DeviceId, DeviceRegistry, RegistryConfig};
use la_i18n::{Args, Key, Language, Localizer};
use la_storage::Limits;

use crate::capture::{
    CaptureEnd, CaptureOrigin, CaptureView, ProgressView, Running, is_incomplete,
};
use crate::command::{
    Changes, Command, Decision, Guarded, Outcome, PanStep, Tick, ZoomAnchor, ZoomDirection,
};
use crate::decoders::{DecoderRuntime, DecoderState, catalogue};
use crate::devices::{ConfigDraft, fresh_seed, problems};
use crate::error;
use crate::marker_ops;
use crate::markers::{
    MarkerText, Markers, Pointer, Resolved, Shape, ZoomTarget, measurement, measurement_text,
};
use crate::notices::{Notice, NoticeContent, NoticeId, NoticeList, Severity, notice_text};
use crate::plan::{
    ChannelView, FramePlan, MarkerPlan, PlanShape, build_plan, real_edge_after, real_edge_before,
};
use crate::presentation::Presentation;
use crate::sessions::Operation;
use crate::settings::{RealSettingsFs, Settings, SettingsFs};
use crate::state::{Activity, EdgeDirection, State, Unsaved, Versions};
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
    pub(crate) decoders: DecoderRuntime,
    /// While set, a cut waits before it copies anything (tests: a cut that is under way).
    pub(crate) crop_hold: Arc<std::sync::atomic::AtomicBool>,
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
        let decoders = DecoderRuntime::new(loaded.settings.decoder_memory_limit_bytes);
        let mut decoder_state = DecoderState::new(catalogue(&decoders.registry, &localizer));
        decoder_state.limits.memory_bytes = loaded.settings.decoder_memory_limit_bytes;
        decoder_state.save_results = loaded.settings.save_decoder_results;
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
            placement: None,
            snap_zoom: None,
            drag: None,
            session_path: None,
            operation: None,
            session_extras: Vec::new(),
            snap: loaded.settings.snap_markers,
            pending_decision: None,
            follow_latest: loaded.settings.follow_latest,
            notices: NoticeList::new(),
            language: localizer.language(),
            settings: loaded.settings,
            decoders: decoder_state,
            side_panel: crate::state::SidePanel::default(),
        };
        if let Some(text) = state.settings.side_panel.as_deref() {
            state.side_panel = crate::state::SidePanel::from_setting(text);
        }
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
            decoders,
            crop_hold: Arc::new(std::sync::atomic::AtomicBool::new(false)),
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
        if self.running.is_some() || self.operation.is_some() || self.decoders_running() {
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

    /// Registers a device provider (a driver for real hardware, or a scripted one in tests). The
    /// application does this once at start-up; the registry scans the provider from then on and its
    /// devices appear in the device list. The host does not depend on any driver: the application
    /// is the one place that knows which drivers exist.
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
                    height: style.height,
                })
                .collect()
        } else {
            Vec::new()
        };
        let mut plan = build_plan(capture.map(|c| &c.snapshot), &window, pixels, &channels);
        // Time counted from a marker: the ticks and their labels follow the origin.
        if let Some(capture) = capture.filter(|_| self.state.markers.origin != 0) {
            plan.axis = crate::axis::build_ticks(
                &window,
                pixels,
                capture.snapshot.sample_rate_hz(),
                self.state.markers.origin,
            );
        }
        if capture.is_some() && window.length > 0 {
            let shown: Vec<u8> = channels.iter().map(|c| c.channel).collect();
            let (lanes, overlays) = self.decoder_plans(&window, pixels, &shown);
            plan.lanes = lanes;
            plan.overlays = overlays;
        }
        if capture.is_some() && window.length > 0 {
            let scale = f64::from(pixels) / window.length as f64;
            let at = |position: u64| position.saturating_sub(window.start) as f64 * scale;
            let before = |position: u64| -(window.start.saturating_sub(position) as f64 * scale);
            let x = |position: u64| {
                if position >= window.start {
                    at(position)
                } else {
                    before(position)
                }
            };
            plan.markers = self
                .state
                .markers
                .list
                .iter()
                .filter(|m| match m.shape {
                    Shape::Point(p) => p >= window.start && p < window.end(),
                    Shape::Interval { start, end } => start < window.end() && end >= window.start,
                })
                .map(|m| MarkerPlan {
                    id: m.id,
                    name: m.name(),
                    number: m.number,
                    colour: m.colour,
                    comment: m.comment.clone(),
                    selected: self.state.markers.selected == Some(m.id),
                    shape: match m.shape {
                        Shape::Point(p) => PlanShape::Point { x: x(p) },
                        Shape::Interval { start, end } => PlanShape::Interval {
                            start_x: x(start),
                            end_x: x(end),
                        },
                    },
                })
                .collect();
        }
        plan
    }

    /// The window to zoom to when the pointer rests where many edges lie within the snapping
    /// distance, so that the one wanted can be told from its neighbours; `None` when there is no
    /// such crowd or the view is already that close. Cheap and total; it changes nothing.
    #[must_use]
    pub fn crowded_zoom_target(&self, pointer: Pointer) -> Option<ZoomTarget> {
        marker_ops::crowded_zoom_target(&self.state, pointer)
    }

    /// What a pointer over the signal area means to a marker being placed or moved: the sample
    /// under it and, once snapping is in, the transition it snaps to. Cheap and total; it changes
    /// nothing.
    #[must_use]
    pub fn resolve_pointer(&self, pointer: Pointer) -> Resolved {
        marker_ops::resolve(&self.state, pointer)
    }

    /// The texts of every marker in the current language: its times relative to the origin of
    /// time and, for a pair, the duration and the frequency of that duration.
    #[must_use]
    pub fn marker_texts(&self) -> Vec<MarkerText> {
        let Some(capture) = self.state.capture.as_ref() else {
            return Vec::new();
        };
        let rate = capture.snapshot.sample_rate_hz();
        if rate == 0 {
            return Vec::new();
        }
        let origin = self.state.markers.origin as f64;
        let time = |position: u64| {
            self.localizer
                .format_duration((position as f64 - origin) / rate as f64)
        };
        self.state
            .markers
            .list
            .iter()
            .map(|m| {
                let (start, end) = (m.shape.start(), m.shape.end());
                let pair = matches!(m.shape, Shape::Interval { .. });
                let measured = pair
                    .then(|| measurement(rate, start, end))
                    .flatten()
                    .map(|measured| measurement_text(&self.localizer, &measured));
                MarkerText {
                    id: m.id,
                    name: m.name(),
                    time_a: time(start),
                    time_b: pair.then(|| time(end)),
                    duration: measured.as_ref().map(|t| t.distance.clone()),
                    frequency: measured.and_then(|t| t.frequency),
                }
            })
            .collect()
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
        changes.insert(self.poll_decoders(state));
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
        // The outcome first, the snapshot after it: once there is an outcome the capture is closed,
        // so a snapshot taken afterwards holds every sample. The other way round, a capture that
        // ends in between would be shown (and saved) as finished with the last samples missing.
        let outcome = acquisition.outcome();
        let snapshot = acquisition.capture().snapshot();
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
                | Command::MoveMarkerEnd { .. }
                | Command::PlacementZoomTo { .. }
        ) {
            tracing::debug!(target: "la_host", command = name, "command");
        } else {
            tracing::info!(target: "la_host", command = name, "command");
        }
        let mut working = self.state.clone();
        let result = catch_unwind(AssertUnwindSafe(|| self.execute(&mut working, command)));
        match result {
            Ok(Ok(Executed::Changed(changes))) => {
                // A marker that is gone cannot be dragged any more.
                let changes = changes | marker_ops::drop_stale_drag(&mut working);
                if !changes.is_empty() {
                    working.version = working.version.saturating_add(1);
                    working.versions.bump(changes);
                }
                self.state = working;
                self.sync_decoders();
                Outcome::Done(changes)
            }
            Ok(Ok(Executed::Needs(guarded))) => {
                // The answer to a question about unsaved data may be remembered: it is applied at
                // once, as if it had been given (a question about removing a decoder is not about
                // unsaved data and is always asked).
                let remembered = working
                    .settings
                    .unsaved_choice
                    .as_deref()
                    .and_then(Decision::from_setting)
                    .filter(|_| !matches!(guarded, Guarded::RemoveDecoder(_) | Guarded::Crop(_)))
                    // Only Save and Discard can be remembered: a remembered Cancel would silently
                    // stop quitting, closing and opening. A stale word in the file asks instead.
                    .filter(|decision| *decision != Decision::Cancel);
                working.pending_decision = Some(guarded.clone());
                working.version = working.version.saturating_add(1);
                working.versions.bump(Changes::DECISION);
                self.state = working;
                match remembered {
                    Some(decision) => {
                        tracing::info!(target: "la_host", answer = decision.as_setting(), "remembered answer");
                        self.apply(Command::Decide(decision))
                    }
                    None => Outcome::NeedsDecision(guarded),
                }
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
        // A zoom or a move of the view by the user's hand: the view is theirs, not the one an
        // automatic zoom of a crowded zone would put back.
        if marker_ops::changes_window_by_hand(&command) {
            state.snap_zoom = None;
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
                state.decoders.catalogue = catalogue(&self.decoders.registry, &self.localizer);
                state.settings.language = Some(language.tag().to_owned());
                let saved = self.persist_settings(state);
                changed(Changes::LANGUAGE | Changes::SETTINGS | Changes::DECODERS | saved)
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
            Command::AddDecoder { decoder } => self.add_decoder(state, &decoder),
            Command::ConfigureDecoder { id, setup } => self.configure_decoder(state, id, setup),
            Command::SetDecoderChannel { id, role, channel } => {
                self.edit_decoder(state, id, |setup| match channel {
                    Some(channel) => {
                        setup.channels.insert(role, channel);
                    }
                    None => {
                        setup.channels.remove(&role);
                    }
                })
            }
            Command::SetDecoderOption { id, option, value } => {
                self.edit_decoder(state, id, |setup| {
                    setup.options.insert(option, value);
                })
            }
            Command::RenameDecoder { id, name } => {
                self.edit_decoder(state, id, |setup| setup.name = name)
            }
            Command::ShowDecoderLane { id, visible } => {
                self.edit_decoder(state, id, |setup| setup.lane_visible = visible)
            }
            Command::RemoveDecoder(id) => {
                let setups: Vec<Setup> = state
                    .decoders
                    .instances
                    .iter()
                    .map(|i| i.setup.clone())
                    .collect();
                if state.decoders.instance(id).is_some()
                    && !la_decoders::stack::dependants(&setups, id).is_empty()
                {
                    // Others read from it: ask first. The capture is not touched.
                    return Ok(Executed::Needs(Guarded::RemoveDecoder(id)));
                }
                self.remove_decoder(state, id)
            }
            Command::StackDecoder { id, source } => {
                self.edit_decoder(state, id, |setup| setup.source = source)
            }
            Command::RunDecoder(id) => self.run_decoder(state, id),
            Command::CancelDecoder(id) => self.cancel_decoder(state, id),
            Command::SetSaveDecoderResults(save) => {
                state.settings.save_decoder_results = save;
                state.decoders.save_results = save;
                let saved = self.persist_settings(state);
                changed(Changes::SETTINGS | Changes::DECODERS | saved)
            }
            Command::SetDecoderMemoryLimit(bytes) => {
                if !crate::settings::decoder_memory_limit_is_valid(bytes) {
                    return Err(Reject::Ignored(error::INVALID_COMMAND));
                }
                state.settings.decoder_memory_limit_bytes = bytes;
                state.decoders.limits.memory_bytes = bytes;
                // A pass that is running keeps the limit it started with.
                self.decoders.pending_limit = Some(bytes);
                self.apply_pending_decoder_limit();
                let saved = self.persist_settings(state);
                changed(Changes::SETTINGS | Changes::DECODERS | saved)
            }
            Command::SelectAnnotation {
                instance,
                annotation,
            } => self.select_annotation(state, instance, annotation),
            Command::GoToAnnotation(which) => self.go_to_annotation(state, which),
            Command::ClearAnnotationSelection => {
                state.decoders.selection = None;
                changed(Changes::DECODERS)
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
            // Placing, moving and the actions of the markers: their stories fill these in; until then
            // they are not allowed (`specs/015-markers-rework/tasks.md`).
            Command::BeginPlacement(kind) => {
                marker_ops::begin_placement(state, kind).map(Executed::Changed)
            }
            Command::CancelPlacement => marker_ops::cancel_placement(state).map(Executed::Changed),
            Command::FixPlacement { pointer } => {
                marker_ops::fix_placement(state, pointer).map(Executed::Changed)
            }
            Command::MoveMarkerEnd { id, end, pointer } => {
                marker_ops::move_marker_end(state, id, end, pointer).map(Executed::Changed)
            }
            Command::EndMarkerDrag => marker_ops::end_marker_drag(state).map(Executed::Changed),
            Command::ScrollToMarker(id) => {
                marker_ops::scroll_to_marker(state, id).map(Executed::Changed)
            }
            Command::SetMarkerColour { id, colour } => {
                marker_ops::set_marker_colour(state, id, colour).map(Executed::Changed)
            }
            Command::SetMarkerComment { id, comment } => {
                marker_ops::set_marker_comment(state, id, &comment).map(Executed::Changed)
            }
            Command::SetTimeOrigin(id) => {
                marker_ops::set_time_origin(state, id).map(Executed::Changed)
            }
            Command::ResetTimeOrigin => marker_ops::reset_time_origin(state).map(Executed::Changed),
            Command::SplitPair(id) => marker_ops::split_pair(state, id).map(Executed::Changed),
            Command::MakePair { first, second } => {
                marker_ops::make_pair(state, first, second).map(Executed::Changed)
            }
            // The automatic zoom, the data cuts and the fit come with their stories.
            Command::PlacementZoomTo { start, length } => {
                marker_ops::placement_zoom_to(state, start, length).map(Executed::Changed)
            }
            Command::CutLeft(_) | Command::CutRight(_) | Command::TrimToPair(_) => {
                marker_ops::crop_question(self, state, &command)
            }
            Command::FitToPair(id) => marker_ops::fit_to_pair(state, id).map(Executed::Changed),
            Command::SelectMarker(id) => {
                if !state.markers.select(id) {
                    return Err(Reject::Ignored(error::INVALID_COMMAND));
                }
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
            Command::SetChannelHeight { channel, height } => {
                edit_presentation(state, |p| p.set_height(channel, height))
            }
            Command::SetDecoderLaneHeight { instance, height } => {
                if state.decoders.instance(instance).is_none() {
                    return Err(Reject::Ignored(error::INVALID_COMMAND));
                }
                edit_presentation(state, |p| {
                    p.set_decoder_height(instance, height);
                    true
                })
            }
            Command::MoveLane { from, to } => self.move_lane(state, from, to),
            Command::SetSidePanel(view) => {
                state.side_panel = view;
                state.settings.side_panel = Some(view.as_setting().to_owned());
                let saved = self.persist_settings(state);
                changed(Changes::SETTINGS | saved)
            }
            Command::SetPanelWidth(width) => {
                let (low, high) = crate::settings::PANEL_WIDTH_RANGE;
                state.settings.panel_width = Some(width.clamp(low, high));
                let saved = self.persist_settings(state);
                changed(Changes::SETTINGS | saved)
            }
            Command::SetUnsavedChoice(choice) => {
                if choice == Some(Decision::Cancel) {
                    return Err(Reject::Ignored(error::INVALID_COMMAND));
                }
                state.settings.unsaved_choice = choice.map(|c| c.as_setting().to_owned());
                let saved = self.persist_settings(state);
                changed(Changes::SETTINGS | saved)
            }
            Command::ScrollToEdge { channel, direction } => {
                let limits =
                    capture_limits_of(state).ok_or(Reject::Ignored(error::INVALID_COMMAND))?;
                let capture = state
                    .capture
                    .as_ref()
                    .ok_or(Reject::Ignored(error::INVALID_COMMAND))?;
                let shown = capture.description.enabled_mask() >> u32::from(channel).min(31) & 1
                    == 1
                    && state
                        .presentation
                        .channels
                        .iter()
                        .any(|style| style.index == channel && style.visible);
                if !shown {
                    return Err(Reject::Ignored(error::INVALID_COMMAND));
                }
                let current = current_window(state, &limits);
                let window = match direction {
                    EdgeDirection::Next => {
                        let edge = real_edge_after(
                            &capture.snapshot,
                            channel,
                            current.end().saturating_sub(1),
                        )
                        .ok_or(Reject::Ignored(error::INVALID_COMMAND))?;
                        current.with_edge_at(&limits, edge, current.length / 4)
                    }
                    EdgeDirection::Previous => {
                        let edge = real_edge_before(&capture.snapshot, channel, current.start)
                            .ok_or(Reject::Ignored(error::INVALID_COMMAND))?;
                        current.with_edge_at(&limits, edge, current.length.saturating_mul(3) / 4)
                    }
                };
                state.window = window;
                state.auto_fit = false;
                state.follow_latest = false;
                changed(Changes::WINDOW)
            }
            Command::RemoveRecentFile(path) => self
                .remove_recent(state, &path)
                .map(Executed::Changed)
                .ok_or(Reject::Ignored(error::INVALID_COMMAND)),
            Command::ClearRecentFiles => Ok(Executed::Changed(self.clear_recent(state))),
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
        if let Guarded::Crop(plan) = guarded {
            // The confirmation of a cut: Delete is the answer `Discard`.
            state.pending_decision = None;
            return match decision {
                Decision::Cancel => Ok(Executed::Changed(Changes::DECISION)),
                Decision::Save => {
                    state.pending_decision = Some(Guarded::Crop(plan));
                    Err(Reject::Ignored(error::INVALID_COMMAND))
                }
                Decision::Discard => {
                    let Executed::Changed(started) = self.begin_crop(state, plan)? else {
                        return Err(Reject::Ignored(error::INVALID_COMMAND));
                    };
                    Ok(Executed::Changed(Changes::DECISION | started))
                }
            };
        }
        if let Guarded::RemoveDecoder(id) = guarded {
            // A question about decoders only: the capture and what is running stay as they are.
            return match decision {
                Decision::Cancel => {
                    state.pending_decision = None;
                    Ok(Executed::Changed(Changes::DECISION))
                }
                Decision::Save => Err(Reject::Ignored(error::INVALID_COMMAND)),
                Decision::Discard => {
                    state.pending_decision = None;
                    let Executed::Changed(removed) = self.remove_with_dependants(state, id)? else {
                        return Err(Reject::Ignored(error::INVALID_COMMAND));
                    };
                    Ok(Executed::Changed(Changes::DECISION | removed))
                }
            };
        }
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
        // Instances whose decoder has a generator make the signals of their channels when the
        // capture is simulated; the other channels stay random (constitution Principle XI).
        let source = (device.id.provider() == la_devices::VIRTUAL_PROVIDER_NAME)
            .then(|| self.generator_source(state))
            .flatten();
        let config = state
            .config
            .to_device_config(&device, seed, &failures, source);
        let limits = capture_limits(&state.settings);
        let chunk_samples =
            chunk_samples_for(limits.memory_budget_bytes, device.capabilities.channels);
        let options = AcquisitionOptions {
            limits,
            chunk_samples,
            channel_capacity: channel_capacity_for(config.sample_rate_hz, chunk_samples),
            ..AcquisitionOptions::default()
        };
        let acquisition = self
            .registry
            .start(&device.id, config, options)
            .map_err(Reject::Failed)?;
        let snapshot = acquisition.capture().snapshot();
        // The device may already have delivered its first samples: the window is made for what
        // the snapshot holds, not for an empty capture.
        let first_length = snapshot.length();
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
        state.placement = None;
        state.snap_zoom = None;
        state.drag = None;
        state.activity = Activity::Capturing;
        state.unsaved = Unsaved::Running;
        state.auto_fit = true;
        state.pending_decision = None;
        state.window = ViewWindow::fit_all(&window_limits(first_length, state.view_size.pixels));
        self.running = Some(Running {
            acquisition,
            started: Instant::now(),
            memory_limit: limits.memory_budget_bytes,
            planned: Some(state.config.length),
        });
        self.restart_decoders(state);
        Ok(Executed::Changed(
            Changes::DECODERS
                | Changes::ACTIVITY
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

/// How many chunks may wait for the store: a device that cannot wait (a real board) loses the
/// samples that arrive while the queue is full, so the queue holds about a fifth of a second of
/// the device's rate, never fewer than the default of 8 nor more than 512 chunks. Measured on a
/// WeAct analyzer at 24 MS/s: 8 chunks (5 ms) lost 27 % to 38 % of a capture, 32 chunks lost none.
fn channel_capacity_for(rate_hz: u64, chunk_samples: usize) -> usize {
    let chunk = u64::try_from(chunk_samples).unwrap_or(u64::MAX).max(1);
    let wanted = (rate_hz / 5).div_ceil(chunk);
    usize::try_from(wanted.clamp(8, 512)).unwrap_or(512)
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
pub(crate) fn plan_pixels(state: &State) -> u32 {
    state.view_size.pixels.clamp(1, MAX_PLAN_PIXELS)
}

/// The limits of the view of the capture on screen; `None` without a capture.
pub(crate) fn capture_limits_of(state: &State) -> Option<WindowLimits> {
    state
        .capture
        .as_ref()
        .map(|c| window_limits(c.snapshot.length(), plan_pixels(state)))
}

/// The window now shown (the whole capture while it follows "fit all").
pub(crate) fn current_window(state: &State, limits: &WindowLimits) -> ViewWindow {
    if state.auto_fit {
        ViewWindow::fit_all(limits)
    } else {
        state.window.clamp(limits)
    }
}

/// An unsaved capture stays unsaved; a saved one becomes unsaved when its presentation changes.
pub(crate) fn mark_dirty(state: &mut State) {
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
    state.placement = None;
    state.snap_zoom = None;
    state.drag = None;
    state.session_path = None;
    state.session_extras.clear();
    state.capture = None;
    state.decoders.reset();
    state.progress = None;
    state.activity = Activity::Idle;
    state.unsaved = Unsaved::Clean;
    state.auto_fit = true;
    state.pending_decision = None;
    state.window = ViewWindow::fit_all(&window_limits(0, state.view_size.pixels));
    // What the user set for the channels (names, colours, order) stays for the next capture.
    sync_presentation(state);
}

#[cfg(test)]
mod capacity_tests {
    use super::channel_capacity_for;

    #[test]
    fn the_queue_holds_about_a_fifth_of_a_second_between_eight_and_five_hundred_twelve_chunks() {
        // 24 MS/s in chunks of 16,384: 4.8 million samples in a fifth of a second is 293 chunks.
        assert_eq!(channel_capacity_for(24_000_000, 16_384), 293);
        // Slow rates keep the default of 8.
        assert_eq!(channel_capacity_for(1_000_000, 16_384), 13);
        assert_eq!(channel_capacity_for(20_000, 16_384), 8);
        assert_eq!(channel_capacity_for(0, 16_384), 8);
        // Never more than 512, also for tiny chunks.
        assert_eq!(channel_capacity_for(24_000_000, 64), 512);
        // A chunk of zero samples does not divide by zero.
        assert_eq!(channel_capacity_for(24_000_000, 0), 512);
    }
}
