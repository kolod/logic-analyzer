//! The bodies of the marker commands that are more than a change of a field: placement, moving
//! an end, going to a marker, and (in later stories) the actions of single and pair markers and
//! the background crop operation (see `specs/015-markers-rework`).

use crate::command::Changes;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, PoisonError};

use la_core::Error;
use la_storage::{CropControl, Snapshot};

use crate::command::{Command, CropPlan, CropSide, Guarded};
use crate::error;
use crate::markers::{
    End, Kind, MARK_MARGIN_PX, MAX_MARKERS, Placement, Pointer, Resolved, SavedView, Shape,
    Snapped, ZoomTarget,
};
use crate::notices::{NoticeContent, Severity};
use crate::shell::{
    Executed, Reject, Shell, capture_limits, capture_limits_of, current_window, mark_dirty,
    plan_pixels,
};
use crate::snap;
use crate::state::{Activity, State};
use crate::window::ViewWindow;

/// How far from the pointer a transition still attracts the line, in pixels.
const SNAP_PIXELS: u64 = 12;
/// How far apart, in pixels, the edges near the pointer are after the automatic zoom: two snapping
/// distances, so that each has a zone of its own.
const APART_PIXELS: u64 = 2 * SNAP_PIXELS;

/// What a marker command did, or why it was not allowed.
pub(crate) type Done = Result<Changes, Reject>;

fn not_allowed() -> Reject {
    Reject::Ignored(error::INVALID_COMMAND)
}

/// The sample position under a pointer at `x` pixels from the left of the signal area: the sample
/// of the visible window that the pixel column shows, held inside the capture (position 0 for an
/// empty one). Total: a negative, infinite or NaN `x` gives a position at an end.
pub(crate) fn sample_under(state: &State, x: f32) -> u64 {
    let Some(capture) = state.capture.as_ref() else {
        return 0;
    };
    let length = capture.snapshot.length();
    if length == 0 {
        return 0;
    }
    let Some(limits) = capture_limits_of(state) else {
        return 0;
    };
    let window = current_window(state, &limits);
    let pixels = f64::from(plan_pixels(state));
    let x = if x.is_nan() {
        0.0
    } else {
        f64::from(x).clamp(0.0, pixels)
    };
    let offset = (x * window.length as f64 / pixels).floor();
    // `offset` is finite and not negative; the cast saturates.
    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "a finite, non-negative offset inside the window length"
    )]
    let offset = offset as u64;
    window
        .start
        .saturating_add(offset)
        .min(length.saturating_sub(1))
}

/// The snapping distance in samples for a window of `window_length` samples on `pixels` pixels
/// (at least one sample).
fn snap_radius(window_length: u64, pixels: u64) -> u64 {
    let samples = u128::from(window_length)
        .saturating_mul(u128::from(SNAP_PIXELS))
        .checked_div(u128::from(pixels.max(1)))
        .unwrap_or(1);
    u64::try_from(samples).unwrap_or(u64::MAX).max(1)
}

/// What a pointer means now: the sample under it, or the transition it snaps to (only the
/// transitions of the lane it is over attract it), and whether the zone is crowded.
pub(crate) fn resolve(state: &State, pointer: Pointer) -> Resolved {
    let position = sample_under(state, pointer.x);
    let plain = Resolved {
        position,
        snapped: None,
        crowded: false,
    };
    let (true, Some(lane), Some(capture), Some(limits)) = (
        state.snap,
        pointer.lane,
        state.capture.as_ref(),
        capture_limits_of(state),
    ) else {
        return plain;
    };
    if lane >= capture.snapshot.channels() {
        return plain;
    }
    let window = current_window(state, &limits);
    let radius = snap_radius(window.length, u64::from(plan_pixels(state)));
    let zone = snap::zone(&capture.snapshot, lane, position, radius);
    match zone.nearest {
        Some(edge) => Resolved {
            position: edge.min(capture.snapshot.length().saturating_sub(1)),
            snapped: Some(Snapped {
                channel: lane,
                position: edge,
            }),
            crowded: zone.crowded,
        },
        None => Resolved {
            crowded: zone.crowded,
            ..plain
        },
    }
}

/// The window an automatic zoom of a crowded zone goes to (research R4): the edges nearest to
/// the pointer are `APART_PIXELS` apart, the point under the pointer stays where it is as far as
/// the pointer's own column still fits in the view. `None` when the zone is not crowded or the
/// view is already that close.
pub(crate) fn crowded_zoom_target(state: &State, pointer: Pointer) -> Option<ZoomTarget> {
    if !resolve(state, pointer).crowded {
        return None;
    }
    let lane = pointer.lane?;
    let capture = state.capture.as_ref()?;
    let limits = capture_limits_of(state)?;
    let window = current_window(state, &limits);
    let pixels = u64::from(plan_pixels(state));
    let sample = sample_under(state, pointer.x);
    let edges = snap::nearest_four(&capture.snapshot, lane, sample);
    let gap = edges
        .iter()
        .zip(edges.iter().skip(1))
        .map(|(a, b)| b.saturating_sub(*a))
        .min()?
        .max(1);
    let wanted = u128::from(gap)
        .saturating_mul(u128::from(pixels))
        .div_ceil(u128::from(APART_PIXELS));
    let length = u64::try_from(wanted).unwrap_or(u64::MAX).max(1);
    // The pointer's own column is `columns` samples wide: half of it must fit on each side.
    let columns = window.length.div_ceil(pixels.max(1));
    let half = columns / 2;
    let x = if pointer.x.is_nan() {
        0.0
    } else {
        f64::from(pointer.x).clamp(0.0, pixels as f64)
    };
    // Finite, not negative and not above the length.
    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "a finite offset between 0 and the length of the window"
    )]
    let offset = (x * length as f64 / pixels.max(1) as f64).floor() as u64;
    let wanted_start = sample.saturating_sub(offset);
    let lowest = sample
        .saturating_add(half)
        .saturating_add(1)
        .saturating_sub(length);
    let highest = sample.saturating_sub(half).saturating_sub(1);
    let start = if lowest <= highest {
        wanted_start.clamp(lowest, highest)
    } else {
        sample.saturating_sub(length / 2)
    };
    let target = ViewWindow { start, length }.clamp(&limits);
    (target.length < window.length).then_some(ZoomTarget {
        start: target.start,
        length: target.length,
    })
}

/// Puts back the view an automatic zoom replaced, if one did.
pub(crate) fn restore_view(state: &mut State) {
    let Some(saved) = state.snap_zoom.take() else {
        return;
    };
    let Some(limits) = capture_limits_of(state) else {
        return;
    };
    state.window = if saved.auto_fit {
        ViewWindow::fit_all(&limits)
    } else {
        ViewWindow {
            start: saved.start,
            length: saved.length,
        }
        .clamp(&limits)
    };
    state.auto_fit = saved.auto_fit;
    state.follow_latest = saved.follow_latest;
}

/// Whether a command moves or zooms the view by the user's hand (an automatic zoom is then no
/// longer the view to put back: the view is the user's).
pub(crate) fn changes_window_by_hand(command: &Command) -> bool {
    matches!(
        command,
        Command::Zoom { .. }
            | Command::ZoomStep(_)
            | Command::Pan { .. }
            | Command::PanStep(_)
            | Command::FitAll
            | Command::GoToStart
            | Command::GoToEnd
            | Command::ZoomToRange { .. }
            | Command::ScrollToEdge { .. }
            | Command::ScrollToMarker(_)
            | Command::FitToPair(_)
    )
}

/// `PlacementZoomTo { start, length }`: one frame of the automatic zoom.
pub(crate) fn placement_zoom_to(state: &mut State, start: u64, length: u64) -> Done {
    if state.placement.is_none() && state.drag.is_none() {
        return Err(not_allowed());
    }
    let limits = capture_limits_of(state).ok_or_else(not_allowed)?;
    if state.snap_zoom.is_none() {
        let window = current_window(state, &limits);
        state.snap_zoom = Some(SavedView {
            start: window.start,
            length: window.length,
            auto_fit: state.auto_fit,
            follow_latest: state.follow_latest,
        });
    }
    state.window = ViewWindow { start, length }.clamp(&limits);
    state.auto_fit = false;
    state.follow_latest = false;
    Ok(Changes::WINDOW | Changes::MARKERS)
}

/// `BeginPlacement(kind)`: the same kind again cancels, the other kind switches.
pub(crate) fn begin_placement(state: &mut State, kind: Kind) -> Done {
    if state.capture.is_none()
        || state.markers.list.len() >= MAX_MARKERS
        || state.operation.is_some()
        || matches!(
            state.activity,
            Activity::Saving | Activity::Loading | Activity::Cropping
        )
    {
        return Err(not_allowed());
    }
    restore_view(state);
    state.placement = match state.placement {
        Some(placement) if placement.kind == kind => None,
        _ => Some(Placement {
            kind,
            first: None,
            colour: state.markers.next_colour(),
            number: state.markers.next_number(kind),
        }),
    };
    Ok(Changes::MARKERS)
}

/// `CancelPlacement`.
pub(crate) fn cancel_placement(state: &mut State) -> Done {
    if state.placement.take().is_none() {
        return Err(not_allowed());
    }
    let had_zoom = state.snap_zoom.is_some();
    restore_view(state);
    Ok(if had_zoom {
        Changes::MARKERS | Changes::WINDOW
    } else {
        Changes::MARKERS
    })
}

/// `FixPlacement { pointer }`: a click while placing.
pub(crate) fn fix_placement(state: &mut State, pointer: Pointer) -> Done {
    let Some(placement) = state.placement else {
        return Err(not_allowed());
    };
    // What was drawn is what is fixed: the same resolution, on the view as it is now.
    let position = resolve(state, pointer).position;
    let had_zoom = state.snap_zoom.is_some();
    restore_view(state);
    let windowed = if had_zoom {
        Changes::MARKERS | Changes::WINDOW
    } else {
        Changes::MARKERS
    };
    let shape = match (placement.kind, placement.first) {
        (Kind::Single, _) => Shape::Point(position),
        (Kind::Pair, None) => {
            state.placement = Some(Placement {
                first: Some(position),
                ..placement
            });
            return Ok(windowed);
        }
        (Kind::Pair, Some(first)) => Shape::interval(first, position),
    };
    state.placement = None;
    let id = state.markers.add(shape).ok_or_else(not_allowed)?;
    state.markers.select(Some(id));
    mark_dirty(state);
    Ok(windowed)
}

/// `MoveMarkerEnd { id, end, pointer }`: a drag. The end that is dragged is remembered, because
/// a pair whose ends cross swaps their roles.
pub(crate) fn move_marker_end(state: &mut State, id: u32, end: End, pointer: Pointer) -> Done {
    if state.markers.get(id).is_none() {
        return Err(not_allowed());
    }
    let position = resolve(state, pointer).position;
    let role = state
        .markers
        .move_end(id, end, position)
        .ok_or_else(not_allowed)?;
    state.drag = Some((id, role));
    mark_dirty(state);
    Ok(Changes::MARKERS)
}

/// `EndMarkerDrag`.
pub(crate) fn end_marker_drag(state: &mut State) -> Done {
    if state.drag.take().is_none() {
        return Err(not_allowed());
    }
    let had_zoom = state.snap_zoom.is_some();
    restore_view(state);
    Ok(if had_zoom {
        Changes::MARKERS | Changes::WINDOW
    } else {
        Changes::MARKERS
    })
}

/// `ScrollToMarker(id)`: the window of the same length centred on the marker (a pair: on its
/// start), as near as the capture allows.
pub(crate) fn scroll_to_marker(state: &mut State, id: u32) -> Done {
    let position = state
        .markers
        .get(id)
        .map(|m| m.shape.start())
        .ok_or_else(not_allowed)?;
    let limits = capture_limits_of(state).ok_or_else(not_allowed)?;
    state.window = current_window(state, &limits).centre_on(&limits, position);
    state.auto_fit = false;
    state.follow_latest = false;
    Ok(Changes::WINDOW)
}

/// `SetMarkerColour { id, colour }`.
pub(crate) fn set_marker_colour(state: &mut State, id: u32, colour: u8) -> Done {
    if !state.markers.set_colour(id, colour) {
        return Err(not_allowed());
    }
    mark_dirty(state);
    Ok(Changes::MARKERS)
}

/// `SetMarkerComment { id, comment }`.
pub(crate) fn set_marker_comment(state: &mut State, id: u32, comment: &str) -> Done {
    if !state.markers.set_comment(id, comment) {
        return Err(not_allowed());
    }
    mark_dirty(state);
    Ok(Changes::MARKERS)
}

/// `SetTimeOrigin(id)`: time counts from a single marker.
pub(crate) fn set_time_origin(state: &mut State, id: u32) -> Done {
    if !state.markers.set_origin(id) {
        return Err(not_allowed());
    }
    mark_dirty(state);
    Ok(Changes::MARKERS)
}

/// `ResetTimeOrigin`: time counts from the start of the capture again.
pub(crate) fn reset_time_origin(state: &mut State) -> Done {
    if state.markers.origin == 0 {
        return Err(not_allowed());
    }
    state.markers.origin = 0;
    mark_dirty(state);
    Ok(Changes::MARKERS)
}

/// `FitToPair(id)`: zoom and position so that the pair fills the view with room for its marks on
/// both sides (as much as the capture has at its ends).
pub(crate) fn fit_to_pair(state: &mut State, id: u32) -> Done {
    let (start, end) = match state.markers.get(id).map(|m| m.shape) {
        Some(Shape::Interval { start, end }) => (start, end),
        _ => return Err(not_allowed()),
    };
    let limits = capture_limits_of(state).ok_or_else(not_allowed)?;
    state.window = ViewWindow::fit_range(&limits, start, end, MARK_MARGIN_PX);
    state.auto_fit = false;
    state.follow_latest = false;
    Ok(Changes::WINDOW)
}

/// `SplitPair(id)`: a pair becomes two single markers.
pub(crate) fn split_pair(state: &mut State, id: u32) -> Done {
    state.markers.split(id).ok_or_else(not_allowed)?;
    mark_dirty(state);
    Ok(Changes::MARKERS)
}

/// `MakePair { first, second }`: two single markers become one pair.
pub(crate) fn make_pair(state: &mut State, first: u32, second: u32) -> Done {
    state.markers.join(first, second).ok_or_else(not_allowed)?;
    mark_dirty(state);
    Ok(Changes::MARKERS)
}

// ---- cutting and trimming the capture ---------------------------------------------------------

/// What the thread that builds the cropped copy shares with the shell.
struct CropShared {
    cancel: AtomicBool,
    done: AtomicU64,
    total: AtomicU64,
    outcome: Mutex<Option<Result<Snapshot, Arc<Error>>>>,
}

impl CropControl for CropShared {
    fn cancelled(&self) -> bool {
        self.cancel.load(Ordering::Relaxed)
    }

    fn progress(&self, done: u64, total: u64) {
        self.done.store(done, Ordering::Relaxed);
        self.total.store(total, Ordering::Relaxed);
    }
}

/// A cropped copy that is being built on a thread of its own.
pub(crate) struct CropOperation {
    shared: Arc<CropShared>,
}

impl CropOperation {
    /// Starts building the copy of `start..end` of `snapshot`.
    fn start(
        snapshot: Snapshot,
        start: u64,
        end: u64,
        limits: la_storage::Limits,
        hold: Arc<AtomicBool>,
    ) -> Self {
        let shared = Arc::new(CropShared {
            cancel: AtomicBool::new(false),
            done: AtomicU64::new(0),
            total: AtomicU64::new(0),
            outcome: Mutex::new(None),
        });
        let worker = Arc::clone(&shared);
        let spawned = std::thread::Builder::new()
            .name("la-crop".to_owned())
            .spawn(move || {
                // Tests hold a cut here to look at the state while it is under way.
                while hold.load(Ordering::Relaxed) && !worker.cancelled() {
                    std::thread::sleep(std::time::Duration::from_millis(1));
                }
                let result = catch_unwind(AssertUnwindSafe(|| {
                    la_storage::crop(&snapshot, start, end, limits, worker.as_ref())
                        .map(|capture| capture.reader().snapshot())
                }));
                let outcome = match result {
                    Ok(Ok(copy)) => Ok(copy),
                    Ok(Err(error)) => Err(Arc::new(error)),
                    Err(_) => Err(Arc::new(Error::new(
                        la_core::ErrorKind::Internal,
                        crate::error::INTERNAL,
                        "the cut panicked",
                    ))),
                };
                *worker
                    .outcome
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner) = Some(outcome);
            });
        if let Err(error) = spawned {
            *shared
                .outcome
                .lock()
                .unwrap_or_else(PoisonError::into_inner) = Some(Err(Arc::new(Error::new(
                la_core::ErrorKind::Internal,
                crate::error::INTERNAL,
                format!("the cut could not start: {error}"),
            ))));
        }
        Self { shared }
    }

    /// Asks the copy to stop; the capture on screen stays as it is.
    pub(crate) fn cancel(&self) {
        self.shared.cancel.store(true, Ordering::Relaxed);
    }

    /// How far it has come, as steps `(done, total)`.
    pub(crate) fn progress(&self) -> (u64, u64) {
        (
            self.shared.done.load(Ordering::Relaxed),
            self.shared.total.load(Ordering::Relaxed),
        )
    }

    /// The end of the copy, once it is there.
    pub(crate) fn outcome(&self) -> Option<Result<Snapshot, Arc<Error>>> {
        self.shared
            .outcome
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }
}

/// The question a cut or a trim asks: what would be kept, if anything would be deleted and the
/// capture is not busy.
pub(crate) fn crop_question(
    shell: &Shell,
    state: &State,
    command: &Command,
) -> Result<Executed, Reject> {
    let busy =
        state.activity != Activity::Idle || shell.running.is_some() || shell.operation.is_some();
    let capture = state
        .capture
        .as_ref()
        .filter(|_| !busy)
        .ok_or_else(not_allowed)?;
    let total = capture.snapshot.length();
    let (side, start, end) = match *command {
        Command::CutLeft(id) => match state.markers.get(id).map(|m| m.shape) {
            Some(Shape::Point(position)) if position > 0 => (CropSide::Left, position, total),
            _ => return Err(not_allowed()),
        },
        Command::CutRight(id) => match state.markers.get(id).map(|m| m.shape) {
            Some(Shape::Point(position)) if position.saturating_add(1) < total => {
                (CropSide::Right, 0, position.saturating_add(1))
            }
            _ => return Err(not_allowed()),
        },
        Command::TrimToPair(id) => match state.markers.get(id).map(|m| m.shape) {
            Some(Shape::Interval { start, end }) if start > 0 || end.saturating_add(1) < total => {
                (CropSide::Pair, start, end.saturating_add(1))
            }
            _ => return Err(not_allowed()),
        },
        _ => return Err(not_allowed()),
    };
    Ok(Executed::Needs(Guarded::Crop(CropPlan {
        side,
        start,
        end,
        total,
    })))
}

impl Shell {
    /// Starts building the cropped copy the user confirmed.
    pub(crate) fn begin_crop(
        &mut self,
        state: &mut State,
        plan: CropPlan,
    ) -> Result<Executed, Reject> {
        if state.activity != Activity::Idle || self.running.is_some() || self.operation.is_some() {
            return Err(not_allowed());
        }
        let snapshot = state
            .capture
            .as_ref()
            .map(|c| c.snapshot.clone())
            .ok_or_else(not_allowed)?;
        if plan.end > snapshot.length() || plan.start >= plan.end {
            return Err(not_allowed());
        }
        let op = CropOperation::start(
            snapshot,
            plan.start,
            plan.end,
            capture_limits(&state.settings),
            Arc::clone(&self.crop_hold),
        );
        self.operation = Some(crate::sessions::Operation::Crop { op, plan });
        state.operation = Some(crate::state::OperationView {
            kind: crate::state::OperationKind::Crop,
            done_bytes: 0,
            total_bytes: 0,
        });
        state.activity = Activity::Cropping;
        state.placement = None;
        state.snap_zoom = None;
        state.drag = None;
        Ok(Executed::Changed(
            Changes::ACTIVITY | Changes::MARKERS | Changes::PROGRESS,
        ))
    }

    /// Puts the cropped copy on screen in one change: the capture, the markers, the window, the
    /// decoders.
    pub(crate) fn apply_crop(
        &mut self,
        state: &mut State,
        plan: CropPlan,
        snapshot: Snapshot,
    ) -> Changes {
        let Some(old) = state.capture.take() else {
            state.activity = Activity::Idle;
            return Changes::ACTIVITY;
        };
        let (old_start, old_length, was_fit) =
            (state.window.start, state.window.length, state.auto_fit);
        state.capture = Some(crate::capture::CaptureView::new(
            snapshot.clone(),
            old.description,
            old.origin,
            old.end,
        ));
        let report = state.markers.remap(plan.start, plan.end);
        state.placement = None;
        state.snap_zoom = None;
        state.drag = None;
        // The same span of time as far as it still exists.
        let limits = crate::shell::window_limits(snapshot.length(), plan_pixels(state));
        state.window = if was_fit {
            ViewWindow::fit_all(&limits)
        } else {
            ViewWindow {
                start: old_start.saturating_sub(plan.start),
                length: old_length,
            }
            .clamp(&limits)
        };
        state.auto_fit = was_fit;
        state.activity = Activity::Idle;
        state.unsaved = crate::state::Unsaved::Dirty;
        // The results of the decoders belong to the data that is gone: they decode the copy.
        let kept = state.decoders.instances.clone();
        state.decoders.reset();
        state.decoders.instances = kept;
        self.clear_decoder_runtime();
        self.restart_decoders(state);
        let mut changes = Changes::CAPTURE
            | Changes::MARKERS
            | Changes::WINDOW
            | Changes::DECODERS
            | Changes::ACTIVITY
            | Changes::PROGRESS;
        if report.split_pairs > 0 {
            state.notices.add(
                Severity::Info,
                NoticeContent::info(
                    "marker-cut-pair-removed",
                    Some(report.split_pairs.to_string()),
                ),
                crate::shell::now_seconds(),
            );
            changes.insert(Changes::NOTICES);
        }
        changes
    }
}
