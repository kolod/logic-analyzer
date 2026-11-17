//! Saving and opening session files, in the background.
//!
//! A save or load runs on a worker of `la-session`; the shell watches it from `poll`. Nothing here
//! can lose data: a failed or cancelled save leaves the old file as it was and the capture
//! unsaved, a failed load leaves the capture on screen untouched, and a command that waited for
//! the user's decision about unsaved data runs only after the save it asked for succeeded.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use la_core::Error;
use la_session::{
    LoadLimits, LoadOperation, Progress, SaveOperation, Section, SectionTag, Session, SessionData,
};

use crate::capture::{CaptureOrigin, CaptureView};
use crate::command::{Changes, Guarded};
use crate::error;
use crate::markers::Markers;
use crate::notices::{NoticeContent, Severity};
use crate::settings::MAX_RECENT_FILES;
use crate::shell::{
    Executed, Reject, Shell, capture_limits, clear_capture, default_presentation,
    device_channel_mask, report_into, revalidate, window_limits,
};
use crate::state::{Activity, OperationKind, OperationView, State, Unsaved};
use crate::view_section::{self, Bounds, Decoded, ViewState};
use crate::window::ViewWindow;

/// One entry of the list of recent files.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecentFile {
    /// The path as it was remembered.
    pub path: PathBuf,
    /// Whether the file is there now.
    pub exists: bool,
}

/// The save or load the shell is watching.
pub(crate) enum Operation {
    /// Writing `path`; `then` is the command that waits for this save.
    Save {
        op: SaveOperation,
        path: PathBuf,
        then: Option<Guarded>,
    },
    /// Reading `path`.
    Load { op: LoadOperation, path: PathBuf },
}

/// How an operation ended.
enum Done {
    Saved,
    Loaded(Arc<Session>),
    Failed(Arc<Error>),
}

impl Operation {
    pub(crate) fn cancel(&self) {
        match self {
            Operation::Save { op, .. } => op.cancel(),
            Operation::Load { op, .. } => op.cancel(),
        }
    }

    fn kind(&self) -> OperationKind {
        match self {
            Operation::Save { .. } => OperationKind::Save,
            Operation::Load { .. } => OperationKind::Load,
        }
    }

    fn progress(&self) -> Progress {
        match self {
            Operation::Save { op, .. } => op.progress(),
            Operation::Load { op, .. } => op.progress(),
        }
    }

    fn outcome(&self) -> Option<Done> {
        match self {
            Operation::Save { op, .. } => op.outcome().map(|r| match r {
                Ok(_) => Done::Saved,
                Err(e) => Done::Failed(e),
            }),
            Operation::Load { op, .. } => op.outcome().map(|r| match r {
                Ok(session) => Done::Loaded(session),
                Err(e) => Done::Failed(e),
            }),
        }
    }
}

fn layout_tag() -> Option<SectionTag> {
    SectionTag::new(crate::layout_section::TAG).ok()
}

fn view_tag() -> Option<SectionTag> {
    SectionTag::new(view_section::TAG).ok()
}

/// The `VIEW` state of the screen, or `None` when everything is at its defaults (then the file
/// has no `VIEW` section at all).
fn view_state(state: &State) -> Option<ViewState> {
    let capture = state.capture.as_ref()?;
    let defaults = default_presentation(capture);
    let presentation = (state.presentation != defaults).then(|| state.presentation.clone());
    let window = (!state.auto_fit).then_some((state.window.start, state.window.length));
    if presentation.is_none() && state.markers.list.is_empty() && window.is_none() {
        return None;
    }
    Some(ViewState {
        presentation,
        markers: state.markers.clone(),
        window,
        follow_latest: false,
    })
}

/// What to save for the capture on screen.
fn session_data(shell: &Shell, state: &State) -> Result<SessionData, Error> {
    let capture = state
        .capture
        .as_ref()
        .ok_or_else(|| error::internal("there is no capture to save"))?;
    let mut data = SessionData::new(capture.description.clone(), capture.snapshot.clone());
    for section in &state.session_extras {
        data = data.with_section(section.clone())?;
    }
    // The decoders: a section of a newer version that was kept is not replaced by ours.
    for section in shell.decoder_sections(state)? {
        if !state.session_extras.iter().any(|s| s.tag == section.tag) {
            data = data.with_section(section)?;
        }
    }
    // The heights of the signals: a newer LAYT section of the opened file is kept as it is.
    if let (Some(payload), Some(tag)) = (
        crate::layout_section::encode(&state.presentation),
        layout_tag(),
    ) {
        if !state.session_extras.iter().any(|s| s.tag == tag) {
            data = data.with_section(Section {
                tag,
                critical: false,
                payload,
            })?;
        }
    }
    if let (Some(view), Some(tag)) = (view_state(state), view_tag()) {
        // A newer VIEW section of the opened file is kept as it is; ours would replace it.
        if !state.session_extras.iter().any(|s| s.tag == tag) {
            data = data.with_section(Section {
                tag,
                critical: false,
                payload: view_section::encode(&view),
            })?;
        }
    }
    Ok(data)
}

impl Shell {
    /// Starts saving the capture on screen to `path`.
    pub(crate) fn begin_save(
        &mut self,
        state: &mut State,
        path: PathBuf,
    ) -> Result<Executed, Reject> {
        if self.operation.is_some()
            || self.running.is_some()
            || state.activity != Activity::Idle
            || state.capture.is_none()
        {
            return Err(Reject::Ignored(error::INVALID_COMMAND));
        }
        let data = session_data(self, state).map_err(Reject::Failed)?;
        let then = state.pending_decision.clone();
        #[cfg(feature = "test-support")]
        let op =
            la_session::harness::start_save_with(Arc::clone(&self.session_fs), path.clone(), data);
        #[cfg(not(feature = "test-support"))]
        let op = la_session::start_save(path.clone(), data);
        self.operation = Some(Operation::Save { op, path, then });
        state.operation = Some(OperationView {
            kind: OperationKind::Save,
            done_bytes: 0,
            total_bytes: 0,
        });
        Ok(Executed::Changed(Changes::ACTIVITY))
    }

    /// Starts opening the session file `path`.
    pub(crate) fn begin_load(
        &mut self,
        state: &mut State,
        path: PathBuf,
    ) -> Result<Executed, Reject> {
        if self.operation.is_some() || self.running.is_some() || state.activity != Activity::Idle {
            return Err(Reject::Ignored(error::INVALID_COMMAND));
        }
        let limits = LoadLimits {
            storage: capture_limits(&state.settings),
            ..LoadLimits::default()
        };
        #[cfg(feature = "test-support")]
        let op = la_session::harness::start_load_with(
            Arc::clone(&self.session_fs),
            path.clone(),
            limits,
        );
        #[cfg(not(feature = "test-support"))]
        let op = la_session::start_load(path.clone(), limits);
        self.operation = Some(Operation::Load { op, path });
        state.operation = Some(OperationView {
            kind: OperationKind::Load,
            done_bytes: 0,
            total_bytes: 0,
        });
        Ok(Executed::Changed(Changes::ACTIVITY))
    }

    /// Carries on with the command that waited for a decision about unsaved data.
    pub(crate) fn continue_guarded(
        &mut self,
        state: &mut State,
        guarded: Guarded,
    ) -> Result<Changes, Reject> {
        let mut changes = Changes::NONE;
        match guarded {
            Guarded::NewCapture => {
                clear_capture(state);
                changes.insert(Changes::CAPTURE | Changes::PROGRESS | Changes::WINDOW);
            }
            Guarded::StartCapture => {
                // The decoders stay: they decode the new capture too.
                let kept = state.decoders.instances.clone();
                clear_capture(state);
                state.decoders.instances = kept;
                let Executed::Changed(started) = self.begin_capture(state)? else {
                    return Err(Reject::Ignored(error::INVALID_COMMAND));
                };
                changes.insert(started);
            }
            Guarded::RemoveDecoder(id) => {
                if let Executed::Changed(removed) = self.remove_with_dependants(state, id)? {
                    changes.insert(removed);
                }
            }
            Guarded::Close | Guarded::Quit => changes.insert(Changes::QUIT),
            Guarded::OpenSession(path) => {
                if let Executed::Changed(started) = self.begin_load(state, path)? {
                    changes.insert(started);
                }
            }
        }
        Ok(changes)
    }

    /// Watches the running save or load; applies its end.
    pub(crate) fn poll_operation(&mut self, state: &mut State) -> Changes {
        let Some(operation) = &self.operation else {
            return Changes::NONE;
        };
        let mut changes = Changes::NONE;
        let progress = operation.progress();
        let view = OperationView {
            kind: operation.kind(),
            done_bytes: progress.done_bytes,
            total_bytes: progress.total_bytes,
        };
        if state.operation != Some(view) {
            state.operation = Some(view);
            changes.insert(Changes::ACTIVITY);
        }
        let Some(done) = operation.outcome() else {
            return changes;
        };
        let Some(finished) = self.operation.take() else {
            return changes;
        };
        state.operation = None;
        changes.insert(Changes::ACTIVITY);
        match (finished, done) {
            (Operation::Save { path, then, .. }, Done::Saved) => {
                state.unsaved = Unsaved::Clean;
                state.session_path = Some(path.clone());
                changes.insert(self.add_recent(state, &path));
                // The command that waited goes on only if the user still wants it.
                let waiting = then.filter(|g| state.pending_decision.as_ref() == Some(g));
                if let Some(guarded) = waiting {
                    state.pending_decision = None;
                    changes.insert(Changes::DECISION);
                    // The data is saved. If the command that waited then fails, the capture on
                    // screen stays as it was (like a refused command): only the message is new.
                    let saved_state = state.clone();
                    match self.continue_guarded(state, guarded) {
                        Ok(more) => changes.insert(more),
                        Err(rejected) => {
                            *state = saved_state;
                            if let Reject::Failed(e) = rejected {
                                report_into(state, Severity::Error, &e);
                                changes.insert(Changes::NOTICES);
                            }
                        }
                    }
                }
            }
            (Operation::Load { path, .. }, Done::Loaded(session)) => {
                changes.insert(self.apply_loaded(state, &session, &path));
            }
            (_, Done::Failed(e)) => {
                // A cancelled operation is the user's own doing: no message.
                if e.code() != la_session::error::CANCELLED {
                    report_into(state, Severity::Error, &e);
                    changes.insert(Changes::NOTICES);
                }
            }
            _ => {}
        }
        changes
    }

    /// Puts an opened session on screen.
    fn apply_loaded(&mut self, state: &mut State, session: &Session, path: &Path) -> Changes {
        let snapshot = session.snapshot();
        let bounds = Bounds {
            channels: snapshot.channels(),
            length: snapshot.length(),
        };
        let view = CaptureView::new(
            snapshot,
            session.description().clone(),
            CaptureOrigin::File(path.to_path_buf()),
            None,
        );
        let mut presentation = default_presentation(&view);
        let mut markers = Markers::new();
        let mut window = None;
        let mut extras: Vec<Section> = Vec::new();
        let mut notices = false;
        let deco_tag = SectionTag::new(crate::decoder_sessions::DECO_TAG).ok();
        let dann_tag = SectionTag::new(crate::decoder_sessions::DANN_TAG).ok();
        let mut deco_section: Option<&Section> = None;
        let mut dann_section: Option<&Section> = None;
        let layout = layout_tag();
        let mut heights: Vec<(u8, u16)> = Vec::new();
        for section in session.extras() {
            if Some(&section.tag) == layout.as_ref() {
                match crate::layout_section::decode(&section.payload, bounds.channels) {
                    Ok(crate::layout_section::Decoded::Heights(list)) => heights = list,
                    Ok(crate::layout_section::Decoded::Newer) => {
                        // Kept untouched; the capture shows with automatic heights.
                        extras.push(section.clone());
                        state.notices.add(
                            Severity::Info,
                            NoticeContent::Info {
                                key: "notice-layout-newer",
                                text: None,
                            },
                            crate::shell::now_seconds(),
                        );
                        notices = true;
                    }
                    Err(e) => {
                        // The invalid section is dropped: it is not written back.
                        report_into(state, Severity::Warning, &e);
                        notices = true;
                    }
                }
                continue;
            }
            if Some(&section.tag) == deco_tag.as_ref() {
                deco_section = Some(section);
                continue;
            }
            if Some(&section.tag) == dann_tag.as_ref() {
                dann_section = Some(section);
                continue;
            }
            if Some(&section.tag) != view_tag().as_ref() {
                extras.push(section.clone());
                continue;
            }
            match view_section::decode(&section.payload, bounds) {
                Ok(Decoded::View(v)) => {
                    if let Some(p) = v.presentation {
                        presentation = p;
                    }
                    markers = v.markers;
                    window = v.window;
                }
                Ok(Decoded::Newer) => {
                    // Kept untouched; the capture shows with defaults.
                    extras.push(section.clone());
                    state.notices.add(
                        Severity::Info,
                        NoticeContent::Info {
                            key: "notice-view-newer",
                            text: None,
                        },
                        crate::shell::now_seconds(),
                    );
                    notices = true;
                }
                Err(e) => {
                    // The invalid section is dropped: it is not written back.
                    report_into(state, Severity::Warning, &e);
                    notices = true;
                }
            }
        }
        for (channel, height) in heights {
            presentation.set_height(channel, height);
        }
        let limits = window_limits(bounds.length, state.view_size.pixels);
        state.decoders.reset();
        state.capture = Some(view);
        state.presentation = presentation;
        // The one flag of a channel is also what the next capture records.
        // Even "none": then Start says that no channel is chosen, and the flags show it.
        state.config.enabled_channels =
            state.presentation.visible_mask() & device_channel_mask(state);
        revalidate(state);
        state.markers = markers;
        let (decoder_extras, decoder_notices) =
            self.apply_decoder_sections(state, deco_section, dann_section, bounds);
        extras.extend(decoder_extras);
        notices |= decoder_notices;
        state.session_extras = extras;
        match window {
            Some((start, length)) => {
                state.window = ViewWindow { start, length }.clamp(&limits);
                state.auto_fit = false;
            }
            None => {
                state.window = ViewWindow::fit_all(&limits);
                state.auto_fit = true;
            }
        }
        state.progress = None;
        state.activity = Activity::Idle;
        state.unsaved = Unsaved::Clean;
        state.pending_decision = None;
        state.session_path = Some(path.to_path_buf());
        let mut changes = Changes::CAPTURE
            | Changes::DECODERS
            | Changes::PROGRESS
            | Changes::WINDOW
            | Changes::PRESENTATION
            | Changes::MARKERS
            | Changes::DECISION;
        if notices {
            changes.insert(Changes::NOTICES);
        }
        changes.insert(self.add_recent(state, path));
        changes
    }

    /// Takes a file out of the list of recent files (the file itself stays); `None` when it is
    /// not listed.
    pub(crate) fn remove_recent(&mut self, state: &mut State, path: &Path) -> Option<Changes> {
        let text = path.to_string_lossy();
        let recent = &mut state.settings.recent_files;
        let before = recent.len();
        recent.retain(|r| r.as_str() != text);
        (recent.len() != before).then(|| Changes::SETTINGS | self.persist_settings(state))
    }

    /// Empties the list of recent files.
    pub(crate) fn clear_recent(&mut self, state: &mut State) -> Changes {
        state.settings.recent_files.clear();
        Changes::SETTINGS | self.persist_settings(state)
    }

    /// Remembers a file in the list of recent files and saves the settings.
    fn add_recent(&mut self, state: &mut State, path: &Path) -> Changes {
        let text = path.to_string_lossy().into_owned();
        let recent = &mut state.settings.recent_files;
        recent.retain(|r| *r != text);
        recent.insert(0, text);
        recent.truncate(MAX_RECENT_FILES);
        Changes::SETTINGS | self.persist_settings(state)
    }

    /// Writes the settings file; a failure becomes a warning (the choice stays in effect).
    /// Returns what changed besides the settings themselves.
    pub(crate) fn persist_settings(&self, state: &mut State) -> Changes {
        match state
            .settings
            .save(self.settings_fs.as_ref(), &self.config.settings_path)
        {
            Ok(()) => Changes::NONE,
            Err(e) => {
                report_into(state, Severity::Warning, &e);
                Changes::NOTICES
            }
        }
    }

    /// The recent files, newest first, each marked with whether it still exists.
    #[must_use]
    pub fn recent_files(&self) -> Vec<RecentFile> {
        self.state
            .settings
            .recent_files
            .iter()
            .map(|text| {
                let path = PathBuf::from(text);
                let exists = path.is_file();
                RecentFile { path, exists }
            })
            .collect()
    }

    /// Replaces the file system of saves and loads (tests: fault injection).
    #[cfg(feature = "test-support")]
    pub fn set_session_fs(&mut self, fs: Arc<dyn la_session::harness::FileSystem>) {
        self.session_fs = fs;
    }

    /// Whether a save or load is running.
    #[must_use]
    pub fn operation_running(&self) -> bool {
        self.operation.is_some()
    }
}
