//! The text of the status line: what the application is doing and how the capture ended.
//!
//! Every text comes from `ui.ftl` through the localizer, so counts use the plural forms and
//! separators of the language and sizes and frequencies follow its conventions.

use la_i18n::{Args, Key, Localizer};

use crate::capture::CaptureEnd;
use crate::state::{Activity, State, Unsaved};

/// The parts of the status line; a part is `None` when it does not apply.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatusLine {
    /// What the application is doing ("Idle", "Capturing" ...).
    pub activity: String,
    /// Time since the capture started.
    pub elapsed: Option<String>,
    /// How many samples the capture holds ("21 samples").
    pub samples: Option<String>,
    /// Memory in use over the limit ("30.5 MiB / 1 GiB").
    pub memory: Option<String>,
    /// How the last capture ended.
    pub end_reason: Option<String>,
    /// The sample rate of the capture ("1 kHz").
    pub rate: Option<String>,
    /// The data on screen is simulated.
    pub simulated: bool,
    /// Samples were lost or the capture ended abnormally.
    pub incomplete: bool,
    /// There are changes that are not saved.
    pub unsaved: bool,
}

/// "1 sample", "21 samples": the count with the plural form of the language.
#[must_use]
pub fn samples_text(localizer: &Localizer, count: u64) -> String {
    localizer.message(
        &Key::from_static("count-samples"),
        &Args::new().uint("count", count),
    )
}

/// "30.5 MiB / 1 GiB": memory in use over the limit.
#[must_use]
pub fn memory_text(localizer: &Localizer, used: u64, limit: u64) -> String {
    localizer.message(
        &Key::from_static("status-memory"),
        &Args::new()
            .trusted("used", &localizer.format_size(used))
            .trusted("limit", &localizer.format_size(limit)),
    )
}

fn activity_key(activity: Activity) -> Key {
    Key::from_static(match activity {
        Activity::Idle => "status-idle",
        Activity::Capturing => "status-capturing",
        Activity::Saving => "status-saving",
        Activity::Loading => "status-loading",
    })
}

/// The key of the message that names how a capture ended.
#[must_use]
pub fn end_reason_key(end: CaptureEnd) -> Key {
    Key::from_static(match end {
        CaptureEnd::Completed => "end-reason-completed",
        CaptureEnd::Stopped => "end-reason-stopped",
        CaptureEnd::MemoryLimit => "end-reason-memory-limit",
        CaptureEnd::Disconnected => "end-reason-disconnected",
        CaptureEnd::DeviceError(_) => "end-reason-device-error",
        CaptureEnd::SamplesLost => "end-reason-samples-lost",
        CaptureEnd::Loaded => "end-reason-loaded",
    })
}

/// The status line for `state` in the language of `localizer`.
#[must_use]
pub fn status_line(state: &State, localizer: &Localizer) -> StatusLine {
    let text = |key: &Key| localizer.message(key, &Args::new());
    let capture = state.capture.as_ref();
    StatusLine {
        activity: text(&activity_key(state.activity)),
        elapsed: state
            .progress
            .as_ref()
            .map(|p| localizer.format_duration(p.elapsed_ms as f64 / 1000.0)),
        samples: capture.map(|c| samples_text(localizer, c.snapshot.length())),
        memory: state
            .progress
            .as_ref()
            .map(|p| memory_text(localizer, p.memory_used, p.memory_limit)),
        end_reason: capture
            .and_then(|c| c.end)
            .map(|end| text(&end_reason_key(end))),
        rate: capture.map(|c| localizer.format_frequency(c.snapshot.sample_rate_hz() as f64)),
        simulated: capture.is_some_and(|c| c.simulated),
        incomplete: capture.is_some_and(|c| c.incomplete),
        unsaved: state.unsaved != Unsaved::Clean,
    }
}
