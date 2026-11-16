//! The frame plan: the signal view reduced to a few segments per channel.
//!
//! The plan is a pure function of the capture snapshot, the visible window, the width in pixels
//! and the channels to show. It is computed with the storage's indexed viewport query, so its
//! cost does not depend on the length of the capture (constitution Principle VIII), and it holds
//! no toolkit types: a user interface only draws it. Information is never dropped silently: a
//! column with several transitions is a [`Segment::Dense`] column, lost samples are a
//! [`Segment::Gap`], and columns past the end of the capture are [`Segment::Beyond`].

use std::panic::{AssertUnwindSafe, catch_unwind};

use la_storage::{Activity, LevelState, PixelSummary, Snapshot};

use crate::axis::{AxisPlan, build_ticks};
use crate::decoders::InstanceState;
use crate::error;
use crate::presentation::Colour;
use crate::window::ViewWindow;

/// One channel to show, in display order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChannelView {
    /// The channel number in the capture.
    pub channel: u8,
    /// The (sanitized) name to show.
    pub name: String,
    /// The colour of the waveform.
    pub colour: Colour,
    /// The height of its signal in logical pixels (0: automatic).
    pub height: u16,
}

/// What the view can show.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameState {
    /// There is data to draw.
    Ok,
    /// There is no capture.
    NoCapture,
    /// The capture has no samples yet.
    Empty,
    /// The plan could not be built; the code is a stable error code.
    CannotDraw(&'static str),
}

/// A run of pixel columns of one channel. Columns are counted from the left of the signal area.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Segment {
    /// A constant level over the columns `from..to`.
    Level {
        /// First column.
        from: u32,
        /// One past the last column.
        to: u32,
        /// `true` for the high level.
        high: bool,
    },
    /// Exactly one transition inside `column`, at the sub-pixel position `x`.
    Edge {
        /// The column holding the transition.
        column: u32,
        /// Position of the transition in pixels from the left of the signal area.
        x: f64,
        /// `true` for a change from low to high.
        rising: bool,
    },
    /// More than one transition inside `column` (a dense-activity mark).
    Dense {
        /// The column.
        column: u32,
        /// Number of transitions in the column (saturating).
        count: u32,
    },
    /// Lost samples over the columns `from..to`: drawn as a gap, never as a level.
    Gap {
        /// First column.
        from: u32,
        /// One past the last column.
        to: u32,
    },
    /// Columns at or past the end of the capture.
    Beyond {
        /// First column.
        from: u32,
        /// One past the last column.
        to: u32,
    },
}

impl Segment {
    /// The columns the segment covers, `(from, to)` with `to` exclusive.
    #[must_use]
    pub fn range(&self) -> (u32, u32) {
        match *self {
            Segment::Level { from, to, .. }
            | Segment::Gap { from, to }
            | Segment::Beyond { from, to } => (from, to),
            Segment::Edge { column, .. } | Segment::Dense { column, .. } => {
                (column, column.saturating_add(1))
            }
        }
    }
}

/// The plan of one channel.
#[derive(Debug, Clone, PartialEq)]
pub struct ChannelPlan {
    /// The channel number in the capture.
    pub channel: u8,
    /// The name to show.
    pub name: String,
    /// The colour of the waveform.
    pub colour: Colour,
    /// The height of its signal in logical pixels (0: automatic).
    pub height: u16,
    /// Contiguous, sorted segments that cover every column.
    pub segments: Vec<Segment>,
}

/// A marker inside the visible window.
#[derive(Debug, Clone, PartialEq)]
pub struct MarkerPlan {
    /// The marker's identity.
    pub id: u32,
    /// Its sample position.
    pub position: u64,
    /// Its horizontal position in pixels from the left of the signal area.
    pub x: f64,
    /// Its (sanitized) name.
    pub name: String,
}

/// The lane of one decoder instance: what to draw in it.
#[derive(Debug, Clone, PartialEq)]
pub struct LanePlan {
    /// The instance.
    pub instance: u32,
    /// Its name (the localized name of the decoder when the user gave none).
    pub name: String,
    /// Where the instance is in its life.
    pub state: InstanceState,
    /// The results are not whole.
    pub incomplete: bool,
    /// The results no longer match the setup or the source.
    pub outdated: bool,
    /// The rows of spans and marks and the dense cells.
    pub plan: la_decoders::lanes::Plan,
}

/// What a decoder instance draws on top of one channel.
#[derive(Debug, Clone, PartialEq)]
pub struct OverlayPlan {
    /// The channel.
    pub channel: u8,
    /// The instance.
    pub instance: u32,
    /// The marks, edges and dense cells.
    pub plan: la_decoders::lanes::Plan,
}

/// Cost counters of one plan, for the benchmarks and the frame log.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PlanStats {
    /// Index probes of all viewport queries.
    pub probes: u64,
    /// Number of segments of all channels.
    pub segments: usize,
}

/// Everything a user interface needs to draw the signal view once.
#[derive(Debug, Clone, PartialEq)]
pub struct FramePlan {
    /// Whether and why there is something to draw.
    pub state: FrameState,
    /// The visible window the plan was made for.
    pub window: ViewWindow,
    /// The width in pixel columns.
    pub pixels: u32,
    /// The sample rate of the capture in hertz (0 without a capture).
    pub sample_rate_hz: u64,
    /// The number of positions of the capture at the time of the snapshot.
    pub capture_length: u64,
    /// One plan per shown channel, in display order.
    pub channels: Vec<ChannelPlan>,
    /// The time axis.
    pub axis: AxisPlan,
    /// The markers inside the window (filled in by the shell; see `Shell::frame_plan`).
    pub markers: Vec<MarkerPlan>,
    /// One lane per visible decoder instance (filled in by the shell).
    pub lanes: Vec<LanePlan>,
    /// The overlays of decoder instances on the shown channels (filled in by the shell).
    pub overlays: Vec<OverlayPlan>,
    /// Cost counters.
    pub stats: PlanStats,
}

impl FramePlan {
    fn blank(state: FrameState, window: &ViewWindow, pixels: u32) -> Self {
        Self {
            state,
            window: *window,
            pixels,
            sample_rate_hz: 0,
            capture_length: 0,
            channels: Vec::new(),
            axis: AxisPlan::empty(),
            markers: Vec::new(),
            lanes: Vec::new(),
            overlays: Vec::new(),
            stats: PlanStats::default(),
        }
    }
}

/// Builds the plan. Never panics and never fails: a problem gives
/// [`FrameState::CannotDraw`].
#[must_use]
pub fn build_plan(
    snapshot: Option<&Snapshot>,
    window: &ViewWindow,
    pixels: u32,
    channels: &[ChannelView],
) -> FramePlan {
    let built = catch_unwind(AssertUnwindSafe(|| {
        build_inner(snapshot, window, pixels, channels)
    }));
    built.unwrap_or_else(|_| {
        tracing::error!(target: "la_host", "the frame plan panicked");
        FramePlan::blank(FrameState::CannotDraw(error::RENDER_FAILED), window, pixels)
    })
}

fn cannot_draw(window: &ViewWindow, pixels: u32, detail: &str) -> FramePlan {
    tracing::warn!(target: "la_host", code = error::RENDER_FAILED, detail, "cannot draw the view");
    FramePlan::blank(FrameState::CannotDraw(error::RENDER_FAILED), window, pixels)
}

fn build_inner(
    snapshot: Option<&Snapshot>,
    window: &ViewWindow,
    pixels: u32,
    channels: &[ChannelView],
) -> FramePlan {
    let Some(snapshot) = snapshot else {
        return FramePlan::blank(FrameState::NoCapture, window, pixels);
    };
    if pixels == 0 || window.length == 0 {
        return cannot_draw(window, pixels, "the window or its width is empty");
    }
    let Some(end) = window.start.checked_add(window.length) else {
        return cannot_draw(window, pixels, "the window overflows");
    };
    let length = snapshot.length();
    let sample_rate_hz = snapshot.sample_rate_hz();
    let axis = build_ticks(window, pixels, sample_rate_hz);
    let mut plan = FramePlan {
        state: FrameState::Ok,
        window: *window,
        pixels,
        sample_rate_hz,
        capture_length: length,
        channels: Vec::with_capacity(channels.len()),
        axis,
        markers: Vec::new(),
        lanes: Vec::new(),
        overlays: Vec::new(),
        stats: PlanStats::default(),
    };
    if length == 0 {
        plan.state = FrameState::Empty;
        for view in channels {
            plan.channels.push(ChannelPlan {
                channel: view.channel,
                name: view.name.clone(),
                colour: view.colour,
                height: view.height,
                segments: vec![Segment::Beyond {
                    from: 0,
                    to: pixels,
                }],
            });
        }
        plan.stats.segments = plan.channels.len();
        return plan;
    }
    for view in channels {
        let result = match snapshot.viewport(view.channel, window.start, end, pixels) {
            Ok(result) => result,
            Err(e) => return cannot_draw(window, pixels, e.detail()),
        };
        plan.stats.probes = plan.stats.probes.saturating_add(result.stats.probes);
        let segments = fold(&result.pixels, window, pixels);
        plan.stats.segments = plan.stats.segments.saturating_add(segments.len());
        plan.channels.push(ChannelPlan {
            channel: view.channel,
            name: view.name.clone(),
            colour: view.colour,
            height: view.height,
            segments,
        });
    }
    plan
}

/// Turns the pixel summaries of one channel into merged segments.
fn fold(summaries: &[PixelSummary], window: &ViewWindow, pixels: u32) -> Vec<Segment> {
    let scale = f64::from(pixels) / window.length as f64;
    let mut segments: Vec<Segment> = Vec::new();
    for (index, summary) in summaries.iter().enumerate() {
        let column = u32::try_from(index).unwrap_or(u32::MAX);
        let next = column.saturating_add(1);
        let segment = if summary.beyond_end {
            Segment::Beyond {
                from: column,
                to: next,
            }
        } else if summary.missing_data {
            Segment::Gap {
                from: column,
                to: next,
            }
        } else {
            match summary.activity {
                Activity::None => match summary.start_level {
                    LevelState::Missing => Segment::Gap {
                        from: column,
                        to: next,
                    },
                    level => Segment::Level {
                        from: column,
                        to: next,
                        high: level == LevelState::High,
                    },
                },
                Activity::One(edge) => Segment::Edge {
                    column,
                    x: edge.position.saturating_sub(window.start) as f64 * scale,
                    rising: edge.rising,
                },
                Activity::Many { count } => Segment::Dense { column, count },
            }
        };
        push_merged(&mut segments, segment);
    }
    segments
}

/// Appends `segment`, extending the previous one when both are runs of the same kind.
fn push_merged(segments: &mut Vec<Segment>, segment: Segment) {
    if let Some(last) = segments.last_mut() {
        match (&mut *last, &segment) {
            (
                Segment::Level {
                    to, high: previous, ..
                },
                Segment::Level {
                    to: new_to,
                    high: new,
                    ..
                },
            ) if previous == new => {
                *to = *new_to;
                return;
            }
            (Segment::Gap { to, .. }, Segment::Gap { to: new_to, .. })
            | (Segment::Beyond { to, .. }, Segment::Beyond { to: new_to, .. }) => {
                *to = *new_to;
                return;
            }
            _ => {}
        }
    }
    segments.push(segment);
}
