//! The signal view: draws a [`FramePlan`] on a canvas.
//!
//! The expensive part (the waveforms, the axis and the labels) is drawn into a cached layer that
//! is rebuilt only when the plan, the theme, the language or the layout change; the hover readout
//! and the marks of a drag are drawn into a cheap layer on top that never invalidates the cache.
//!
//! The legend strip (the first [`GUTTER`] pixels) is where channels are edited: a click on the
//! colour bar steps the colour, a click on the name edits it, a drag on the name moves the lane, a
//! drag on the border below a lane changes its height (`specs/014-gui-improvements`). The geometry
//! comes from [`crate::lane_layout`], so drawing and hit tests agree.

use std::time::{Duration, Instant};

use iced::alignment::Vertical;
use iced::mouse;
use iced::widget::canvas::{self, Cache, Frame, Geometry, LineDash, Path, Stroke, Text};
use iced::{Color, Pixels, Point, Rectangle, Renderer, Size, Theme};
use la_host::decoders::AnnotationRef;
use la_host::markers::{End, Kind, Placement, Pointer, Resolved};
use la_host::plan::{FramePlan, FrameState, PlanShape, Segment};
use la_host::presentation::{Colour, PALETTE_SIZE};
use la_host::{Command, EdgeDirection, ZoomAnchor, ZoomFactor};
use la_i18n::{Args, Key, Localizer};

use crate::app::Message;
use crate::decoder_view::{self, Look};
use crate::lane_layout::{
    self, AUTO_MAX, BAR_WIDTH, Hit as LegendHit, LaneKind, LaneSpec, Layout, Zone,
};
pub use crate::lane_layout::{AXIS_HEIGHT, GUTTER};
use crate::marker_marks;
use crate::theme::Variant;

const TEXT_SIZE: f32 = 13.0;
/// The size of the number of the channel under its name.
const NUMBER_SIZE: f32 = 10.0;
/// Half the height of the text field that edits a name.
const EDITOR_HALF: f32 = 11.0;
/// A lane at least this high shows its name and its number on two lines.
const TWO_LINES_FROM: f32 = 34.0;
/// How far the pointer must move after a press on a name before it is a drag.
const DRAG_THRESHOLD: f32 = 4.0;
/// Two presses on a border this close in time are a double click.
const DOUBLE_CLICK: Duration = Duration::from_millis(400);
/// The distance of the high and low levels of a waveform from the top and the bottom of its lane.
/// A dense band has the same extent, so that a lone dense column is a vertical stroke of the signal
/// and never a mark above or below its levels.
const WAVE_MARGIN: f32 = 7.0;
/// Far enough above the view that a lane moved there is not drawn.
const OFF_SCREEN: f32 = 100_000.0;
/// How far one notch of the wheel scrolls the legend.
const SCROLL_STEP: f32 = 30.0;
/// The side of a button that scrolls to the next edge.
const EDGE_BUTTON: f32 = 24.0;
/// The distance of that button from the end of the waveform area.
const EDGE_MARGIN: f32 = 4.0;
/// How near to an end the pointer must be for the button to show.
const EDGE_ZONE: f32 = EDGE_BUTTON + EDGE_MARGIN;

/// What the canvas draws.
pub struct SignalView<'a> {
    /// The plan to draw.
    pub plan: &'a FramePlan,
    /// The cache of the expensive layer.
    pub cache: &'a Cache,
    /// The colour variant.
    pub variant: Variant,
    /// The language of every text.
    pub localizer: &'a Localizer,
    /// The selected annotation, if any.
    pub selected: Option<(u32, AnnotationRef)>,
    /// A channel name is being edited: a press anywhere in the view accepts it.
    pub editing: bool,
    /// The marker being placed with the mouse, if any.
    pub placement: Option<Placement>,
    /// What the pointer means to that placement; `None` while it is not over the signal area.
    pub preview: Option<Resolved>,
    /// The marker end being dragged, as the host knows it (a pair whose ends crossed swaps roles).
    pub drag_end: Option<(u32, End)>,
}

/// A press in the legend strip that has not been released yet.
#[derive(Debug, Clone, Copy)]
struct Press {
    lane: usize,
    zone: Zone,
    at: Point,
}

/// What a drag in the legend strip is doing.
#[derive(Debug, Clone, Copy)]
enum Drag {
    /// A lane is being moved; `y` is where the pointer is.
    Lane { from: usize, y: f32 },
    /// The border below `lane` is being dragged.
    Resize {
        lane: usize,
        start_height: f32,
        start_y: f32,
    },
}

/// The room around the text of the time under the pointer.
const HOVER_PAD_X: f32 = 5.0;
/// The height of the box of the time under the pointer.
const HOVER_LABEL_HEIGHT: f32 = 20.0;
/// How far the box is from the crosshair, both ways.
const HOVER_OFFSET: f32 = 10.0;

/// The size of the box for `content` (the glyphs of a proportional font are estimated).
fn hover_label_size(content: &str) -> Size {
    let glyphs = content.chars().count() as f32;
    Size::new(
        glyphs * TEXT_SIZE * 0.62 + 2.0 * HOVER_PAD_X,
        HOVER_LABEL_HEIGHT,
    )
}

/// Where the box with the time goes: below and to the right of the crosshair; below and to the left
/// when there is no room on the right; above when there is no room below. It never covers the
/// time axis or the legend.
fn hover_label_rect(pointer: Point, size: Size, label: Size) -> Rectangle {
    let right_fits = pointer.x + HOVER_OFFSET + label.width <= size.width - 2.0;
    let x = if right_fits {
        pointer.x + HOVER_OFFSET
    } else {
        (pointer.x - HOVER_OFFSET - label.width).max(GUTTER)
    };
    let below_fits = pointer.y + HOVER_OFFSET + label.height <= size.height - 2.0;
    let y = if below_fits {
        pointer.y + HOVER_OFFSET
    } else {
        pointer.y - HOVER_OFFSET - label.height
    };
    Rectangle::new(Point::new(x, y.max(AXIS_HEIGHT + 2.0)), label)
}

/// What the canvas remembers between events.
#[derive(Debug, Default)]
pub struct ViewState {
    /// The last width reported to the application.
    reported_width: f32,
    /// The modifier keys currently held.
    modifiers: iced::keyboard::Modifiers,
    /// The x position where a drag last was, while the left button is down.
    drag_from: Option<f32>,
    /// The view has been moved sideways since the button went down: the pointer is a hand.
    panning: bool,
    /// The marker end being dragged.
    drag_marker: Option<(u32, End)>,
    /// The last click on an annotation (a second one on it soon after is a double click).
    last_click: Option<(Instant, u32, AnnotationRef)>,
    /// How far the lanes are scrolled.
    scroll: f32,
    /// A press in the legend strip.
    press: Option<Press>,
    /// A drag that began in the legend strip.
    drag: Option<Drag>,
    /// The last press on a border, for the double click.
    last_border: Option<(Instant, usize)>,
    /// How far each channel lane is moved aside, right now, while a lane is carried: the lanes
    /// between the old slot and the new one make room (see [`Layout::shifts`]).
    shifts: Vec<f32>,
    /// When the shifts were last advanced.
    anim_at: Option<Instant>,
}

fn message(localizer: &Localizer, key: &'static str) -> String {
    localizer.message(&Key::from_static(key), &Args::new())
}

/// Cuts `text` to fit about `max_chars` characters, with an ellipsis.
fn fit(text: &str, max_chars: usize) -> String {
    if text.chars().count() <= max_chars {
        return text.to_owned();
    }
    let kept: String = text.chars().take(max_chars.saturating_sub(1)).collect();
    format!("{kept}\u{2026}")
}

/// The next colour of the palette for a click on the colour bar (a custom colour goes to the first).
fn next_colour(colour: Colour) -> Colour {
    match colour {
        Colour::Palette(index) => Colour::Palette(index.saturating_add(1) % PALETTE_SIZE),
        Colour::Rgb(..) => Colour::Palette(0),
    }
}

impl SignalView<'_> {
    fn channel_label(&self, index: usize) -> String {
        let Some(channel) = self.plan.channels.get(index) else {
            return String::new();
        };
        if channel.name.is_empty() {
            self.localizer.message(
                &Key::from_static("channel-default"),
                &Args::new().uint("number", u64::from(channel.channel)),
            )
        } else {
            channel.name.clone()
        }
    }

    /// The lanes: the channels with their heights, then the decoders (automatic).
    fn specs(&self) -> Vec<LaneSpec> {
        self.plan
            .channels
            .iter()
            .map(|c| LaneSpec {
                kind: LaneKind::Channel,
                height: c.height,
            })
            .chain(self.plan.lanes.iter().map(|_| LaneSpec {
                kind: LaneKind::Decoder,
                height: 0,
            }))
            .collect()
    }

    fn layout(&self, height: f32, scroll: f32) -> Layout {
        Layout::new(&self.specs(), height, scroll)
    }

    /// The button that scrolls to an edge under the pointer: the lane of the channel, the side and
    /// where it is drawn. It exists only inside the waveform area of a channel lane, near the end
    /// where the plan says an edge lies beyond the window.
    fn edge_button(
        &self,
        position: Point,
        width: f32,
        layout: &Layout,
    ) -> Option<(usize, EdgeDirection, Rectangle)> {
        if self.plan.state != FrameState::Ok || position.x < GUTTER || position.y < AXIS_HEIGHT {
            return None;
        }
        let index = (0..self.plan.channels.len()).find(|i| {
            layout
                .lane(*i)
                .is_some_and(|l| position.y >= l.top && position.y < l.bottom())
        })?;
        let lane = layout.lane(index)?;
        let channel = self.plan.channels.get(index)?;
        let top = lane.top + (lane.height - EDGE_BUTTON) / 2.0;
        let (direction, left) = if channel.edge_after && position.x >= width - EDGE_ZONE {
            (EdgeDirection::Next, width - EDGE_ZONE)
        } else if channel.edge_before && position.x <= GUTTER + EDGE_ZONE {
            (EdgeDirection::Previous, GUTTER + EDGE_MARGIN)
        } else {
            return None;
        };
        Some((
            index,
            direction,
            Rectangle::new(Point::new(left, top), Size::new(EDGE_BUTTON, EDGE_BUTTON)),
        ))
    }

    fn centred_text(&self, frame: &mut Frame, size: Size, content: String, color: Color) {
        frame.fill_text(Text {
            content,
            position: Point::new(size.width / 2.0, size.height / 2.0),
            color,
            size: Pixels(TEXT_SIZE + 2.0),
            align_x: iced::alignment::Horizontal::Center.into(),
            align_y: Vertical::Center,
            ..Text::default()
        });
    }

    /// The channel whose lane the pointer is in; `None` over the ruler, the legend, between lanes,
    /// over a decoder lane or outside the lanes.
    fn lane_under(&self, position: Point, layout: &Layout) -> Option<u8> {
        if position.x < GUTTER || position.y < AXIS_HEIGHT {
            return None;
        }
        let index = (0..self.plan.channels.len()).find(|i| {
            layout
                .lane(*i)
                .is_some_and(|l| position.y >= l.top && position.y < l.bottom())
        })?;
        self.plan.channels.get(index).map(|c| c.channel)
    }

    /// The pointer as the host takes it: pixels of the plan from the left of the signal area, and
    /// the lane it is over.
    fn host_pointer(&self, position: Point, width: f32, layout: &Layout) -> Pointer {
        let scale = (width - GUTTER).max(1.0) / self.plan.pixels.max(1) as f32;
        Pointer {
            x: (position.x - GUTTER).max(0.0) / scale,
            lane: self.lane_under(position, layout),
        }
    }

    /// The marker end under a press: its mark or its line.
    fn marker_hit(&self, position: Point, scale: f32) -> Option<(u32, End)> {
        let bottom = AXIS_HEIGHT;
        let near = |x: f32, end: End| {
            ((x - position.x).abs() <= marker_marks::LINE_HIT && position.y >= bottom)
                || marker_marks::mark_bounds(x, bottom, end).contains(position)
        };
        for marker in self.plan.markers.iter().rev() {
            match marker.shape {
                PlanShape::Point { x } => {
                    if near(GUTTER + x as f32 * scale, End::Point) {
                        return Some((marker.id, End::Point));
                    }
                }
                PlanShape::Interval { start_x, end_x } => {
                    if near(GUTTER + end_x as f32 * scale, End::End) {
                        return Some((marker.id, End::End));
                    }
                    if near(GUTTER + start_x as f32 * scale, End::Start) {
                        return Some((marker.id, End::Start));
                    }
                }
            }
        }
        None
    }

    /// Where the marks of the fixed markers are in the ruler.
    fn mark_rectangles(&self, scale: f32) -> Vec<Rectangle> {
        let mut rectangles = Vec::new();
        for marker in &self.plan.markers {
            match marker.shape {
                PlanShape::Point { x } => rectangles.push(marker_marks::mark_bounds(
                    GUTTER + x as f32 * scale,
                    AXIS_HEIGHT,
                    End::Point,
                )),
                PlanShape::Interval { start_x, end_x } => {
                    rectangles.push(marker_marks::mark_bounds(
                        GUTTER + start_x as f32 * scale,
                        AXIS_HEIGHT,
                        End::Start,
                    ));
                    rectangles.push(marker_marks::mark_bounds(
                        GUTTER + end_x as f32 * scale,
                        AXIS_HEIGHT,
                        End::End,
                    ));
                }
            }
        }
        rectangles
    }

    /// The fixed markers: a solid line and a mark for each end, with the number in the mark.
    fn draw_markers(&self, frame: &mut Frame, size: Size) {
        let scale = (size.width - GUTTER).max(1.0) / self.plan.pixels.max(1) as f32;
        let inside = |x: f32| x > GUTTER - 40.0 && x < size.width + 40.0;
        for marker in &self.plan.markers {
            let colour = self.variant.marker(marker.colour);
            let ends: Vec<(f32, End)> = match marker.shape {
                PlanShape::Point { x } => vec![(GUTTER + x as f32 * scale, End::Point)],
                PlanShape::Interval { start_x, end_x } => vec![
                    (GUTTER + start_x as f32 * scale, End::Start),
                    (GUTTER + end_x as f32 * scale, End::End),
                ],
            };
            for (x, end) in ends {
                if !inside(x) {
                    continue;
                }
                marker_marks::draw_line(
                    frame,
                    x,
                    marker_marks::line_top(AXIS_HEIGHT, end),
                    size.height,
                    colour,
                    marker.selected,
                    false,
                );
                marker_marks::draw_mark(frame, x, AXIS_HEIGHT, end, colour, marker.number);
            }
        }
    }

    fn draw_plan(&self, frame: &mut Frame, size: Size, theme: &Theme, scroll: f32, shifts: &[f32]) {
        let palette = theme.palette();
        let text_colour = palette.text;
        let plan = self.plan;
        match plan.state {
            FrameState::NoCapture => {
                self.centred_text(
                    frame,
                    size,
                    message(self.localizer, "view-no-capture"),
                    text_colour,
                );
                return;
            }
            FrameState::CannotDraw(_) => {
                self.centred_text(
                    frame,
                    size,
                    message(self.localizer, "view-cannot-draw"),
                    palette.danger,
                );
                return;
            }
            FrameState::Ok | FrameState::Empty => {}
        }
        let area_width = (size.width - GUTTER).max(1.0);
        let scale = area_width / plan.pixels.max(1) as f32;
        let layout = self.layout(size.height, scroll);
        let lane_top = |index: usize| {
            layout.lane(index).map_or(AXIS_HEIGHT, |l| l.top)
                + shifts.get(index).copied().unwrap_or(0.0)
        };
        let lane_height = |index: usize| layout.lane(index).map_or(AUTO_MAX, |l| l.height);
        let hairline = self.variant.hairline();

        // The vertical grid of the time axis.
        for tick in &plan.axis.ticks {
            let x = GUTTER + tick.x as f32 * scale;
            frame.stroke(
                &Path::line(Point::new(x, AXIS_HEIGHT), Point::new(x, size.height)),
                Stroke::default().with_color(hairline).with_width(1.0),
            );
        }

        // Lanes of the channels.
        for (index, channel) in plan.channels.iter().enumerate() {
            let top = lane_top(index);
            let lane = lane_height(index);
            if top + lane < AXIS_HEIGHT || top > size.height {
                continue;
            }
            let colour = self.variant.channel(channel.colour);
            // The colour bar, which a click steps through the palette.
            frame.fill_rectangle(Point::new(0.0, top), Size::new(BAR_WIDTH, lane), colour);
            self.draw_legend_text(frame, top, lane, index, colour);
            frame.stroke(
                &Path::line(
                    Point::new(0.0, top + lane),
                    Point::new(size.width, top + lane),
                ),
                Stroke::default().with_color(hairline).with_width(1.0),
            );
            self.draw_bands(frame, &channel.segments, top, lane, scale, colour);
            frame.stroke(
                &waveform(
                    &channel.segments,
                    scale,
                    top + WAVE_MARGIN,
                    top + lane - WAVE_MARGIN,
                ),
                Stroke::default().with_color(colour).with_width(1.6),
            );
        }
        let look = Look {
            variant: self.variant,
            localizer: self.localizer,
            selected: self.selected,
            theme,
        };
        // The lanes of the decoders come after the channels.
        for index in 0..plan.lanes.len() {
            let at = plan.channels.len().saturating_add(index);
            let (top, lane) = (lane_top(at), lane_height(at));
            frame.stroke(
                &Path::line(
                    Point::new(0.0, top + lane),
                    Point::new(size.width, top + lane),
                ),
                Stroke::default().with_color(hairline).with_width(1.0),
            );
        }
        decoder_view::draw_lanes(
            frame,
            plan,
            plan.channels.len(),
            &lane_top,
            &lane_height,
            scale,
            &look,
        );
        decoder_view::draw_overlays(frame, plan, &lane_top, &lane_height, scale, &look);
        if plan.state == FrameState::Empty {
            frame.fill_text(Text {
                content: message(self.localizer, "view-empty"),
                position: Point::new(GUTTER + area_width / 2.0, AXIS_HEIGHT + AUTO_MAX / 2.0),
                color: text_colour,
                size: Pixels(TEXT_SIZE + 1.0),
                align_x: iced::alignment::Horizontal::Center.into(),
                align_y: Vertical::Center,
                ..Text::default()
            });
        }

        // The time axis, drawn last so that scrolled lanes pass under it.
        frame.fill_rectangle(
            Point::new(0.0, 0.0),
            Size::new(size.width, AXIS_HEIGHT),
            palette.background,
        );
        let marks = self.mark_rectangles(scale);
        for tick in &plan.axis.ticks {
            let x = GUTTER + tick.x as f32 * scale;
            let depth = if tick.major { 10.0 } else { 6.0 };
            frame.stroke(
                &Path::line(
                    Point::new(x, AXIS_HEIGHT - depth),
                    Point::new(x, AXIS_HEIGHT),
                ),
                Stroke::default().with_color(text_colour).with_width(1.0),
            );
            // A label that a mark of a marker covers is left out: the mark is over the axis.
            let content = self.localizer.format_duration(tick.seconds);
            let label = Rectangle::new(
                Point::new(x + 3.0, 4.0),
                Size::new(
                    content.chars().count() as f32 * (TEXT_SIZE - 1.0) * 0.62,
                    TEXT_SIZE,
                ),
            );
            if marks.iter().any(|mark| mark.intersects(&label)) {
                continue;
            }
            frame.fill_text(Text {
                content,
                position: Point::new(x + 3.0, 4.0),
                color: text_colour,
                size: Pixels(TEXT_SIZE - 1.0),
                ..Text::default()
            });
        }
        frame.stroke(
            &Path::line(
                Point::new(0.0, AXIS_HEIGHT),
                Point::new(size.width, AXIS_HEIGHT),
            ),
            Stroke::default().with_color(hairline).with_width(1.0),
        );
        // A thin bar at the right edge of the legend when the lanes are taller than the area.
        if layout.max_offset() > 0.0 {
            let visible = (size.height - AXIS_HEIGHT).max(1.0);
            let thumb = (visible * visible / layout.total_height()).clamp(16.0, visible);
            let travel = (visible - thumb).max(0.0);
            let y = AXIS_HEIGHT + travel * (layout.offset() / layout.max_offset());
            frame.fill_rectangle(
                Point::new(GUTTER - 4.0, AXIS_HEIGHT),
                Size::new(3.0, visible),
                Color {
                    a: 0.15,
                    ..text_colour
                },
            );
            frame.fill_rectangle(
                Point::new(GUTTER - 4.0, y),
                Size::new(3.0, thumb),
                Color {
                    a: 0.55,
                    ..text_colour
                },
            );
        }
    }

    /// Dense activity, gaps and the part past the end.
    fn draw_bands(
        &self,
        frame: &mut Frame,
        segments: &[Segment],
        top: f32,
        lane: f32,
        scale: f32,
        colour: Color,
    ) {
        let mut run: Option<(u8, u32, u32)> = None;
        let flush = |frame: &mut Frame, run: &mut Option<(u8, u32, u32)>| {
            let Some((kind, from, to)) = run.take() else {
                return;
            };
            let x = GUTTER + from as f32 * scale;
            let width = (to.saturating_sub(from) as f32 * scale).max(1.0);
            let (y, height) = band_extent(kind, top, lane);
            match kind {
                // Dense activity: a band with a solid border in the channel colour.
                0 => {
                    frame.fill_rectangle(
                        Point::new(x, y),
                        Size::new(width, height),
                        Color { a: 0.38, ..colour },
                    );
                    for edge_y in [y, y + height] {
                        frame.stroke(
                            &Path::line(Point::new(x, edge_y), Point::new(x + width, edge_y)),
                            Stroke::default().with_color(colour).with_width(1.4),
                        );
                    }
                }
                // Lost samples: a muted band with a dashed border.
                1 => {
                    frame.fill_rectangle(
                        Point::new(x, y),
                        Size::new(width, height),
                        self.variant.no_data(),
                    );
                    let dashes = [4.0_f32, 3.0];
                    for edge_y in [y, y + height] {
                        frame.stroke(
                            &Path::line(Point::new(x, edge_y), Point::new(x + width, edge_y)),
                            Stroke {
                                line_dash: LineDash {
                                    segments: &dashes,
                                    offset: 0,
                                },
                                ..Stroke::default().with_color(colour).with_width(1.0)
                            },
                        );
                    }
                }
                // Past the end of the capture.
                _ => {
                    frame.fill_rectangle(
                        Point::new(x, y),
                        Size::new(width, height),
                        self.variant.no_data(),
                    );
                }
            }
        };
        for segment in segments {
            let (kind, from, to) = match *segment {
                Segment::Dense { column, .. } => (0, column, column.saturating_add(1)),
                Segment::Gap { from, to } => (1, from, to),
                Segment::Beyond { from, to } => (2, from, to),
                Segment::Level { .. } | Segment::Edge { .. } => {
                    flush(frame, &mut run);
                    continue;
                }
            };
            match &mut run {
                Some((k, _, end)) if *k == kind && *end == from => *end = to,
                _ => {
                    flush(frame, &mut run);
                    run = Some((kind, from, to));
                }
            }
        }
        flush(frame, &mut run);
    }
}

/// The top and the height of a band of kind `kind` (0 dense, 1 gap, 2 past the end) in a lane. A
/// dense band spans exactly the levels of the waveform.
fn band_extent(kind: u8, top: f32, lane: f32) -> (f32, f32) {
    let margin = if kind == 0 { WAVE_MARGIN } else { 3.0 };
    (top + margin, lane - 2.0 * margin)
}

/// The y of the middle of the name in a lane at `top` of `height`: the name is the upper line when
/// the number is shown under it.
fn name_centre(top: f32, height: f32) -> f32 {
    if height >= TWO_LINES_FROM {
        top + height / 2.0 - 6.0
    } else {
        top + height / 2.0
    }
}

/// The label of the channel of the analyzer a lane shows: `D` and the number, as the digital
/// inputs are marked on the devices. It is a mark, not a word, so it is not translated.
fn physical_label(channel: u8) -> String {
    format!("D{channel}")
}

/// The path of a waveform: levels as horizontal lines, single transitions as vertical lines at
/// their exact position. The pen is lifted over dense columns, gaps and the part past the end.
fn waveform(segments: &[Segment], scale: f32, y_high: f32, y_low: f32) -> Path {
    let y = |high: bool| if high { y_high } else { y_low };
    Path::new(|builder| {
        let mut level: Option<bool> = None;
        for segment in segments {
            match *segment {
                Segment::Level { from, to, high } => {
                    let start = Point::new(GUTTER + from as f32 * scale, y(high));
                    if level.is_none() {
                        builder.move_to(start);
                    } else if level != Some(high) {
                        builder.line_to(start);
                    }
                    builder.line_to(Point::new(GUTTER + to as f32 * scale, y(high)));
                    level = Some(high);
                }
                Segment::Edge { column, x, rising } => {
                    let before = !rising;
                    let column_start = Point::new(GUTTER + column as f32 * scale, y(before));
                    if level.is_none() {
                        builder.move_to(column_start);
                    } else if level != Some(before) {
                        builder.line_to(column_start);
                    }
                    let edge_x = GUTTER + x as f32 * scale;
                    builder.line_to(Point::new(edge_x, y(before)));
                    builder.line_to(Point::new(edge_x, y(rising)));
                    builder.line_to(Point::new(
                        GUTTER + (column as f32 + 1.0) * scale,
                        y(rising),
                    ));
                    level = Some(rising);
                }
                Segment::Dense { .. } | Segment::Gap { .. } | Segment::Beyond { .. } => {
                    level = None;
                }
            }
        }
    })
}

impl canvas::Program<Message> for SignalView<'_> {
    type State = ViewState;

    #[allow(
        clippy::too_many_lines,
        reason = "one match over the events of the view"
    )]
    fn update(
        &self,
        state: &mut ViewState,
        event: &canvas::Event,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Option<canvas::Action<Message>> {
        let width = (bounds.width - GUTTER).max(1.0);
        if (width - state.reported_width).abs() >= 1.0 {
            state.reported_width = width;
            return Some(canvas::Action::publish(Message::AreaResized(width)));
        }
        match event {
            // A carried lane: the lanes between its old slot and its new one slide aside.
            canvas::Event::Window(iced::window::Event::RedrawRequested(now)) => {
                let Some(Drag::Lane { from, y }) = state.drag else {
                    return None;
                };
                let layout = self.layout(bounds.height, state.scroll);
                let count = self.plan.channels.len();
                let targets = layout.shifts(from, layout.move_target(from, y), count);
                let dt = state
                    .anim_at
                    .map_or(0.0, |t| now.saturating_duration_since(t).as_secs_f32())
                    .min(0.1);
                state.shifts.resize(count, 0.0);
                let mut moving = false;
                for (current, target) in state.shifts.iter_mut().zip(&targets) {
                    *current = lane_layout::ease(*current, *target, dt);
                    if (*current - *target).abs() > 0.3 {
                        moving = true;
                    } else {
                        *current = *target;
                    }
                }
                state.anim_at = moving.then_some(*now);
                moving.then(canvas::Action::request_redraw)
            }
            canvas::Event::Keyboard(iced::keyboard::Event::ModifiersChanged(modifiers)) => {
                state.modifiers = *modifiers;
                None
            }
            canvas::Event::Mouse(mouse::Event::WheelScrolled { delta }) => {
                let position = cursor.position_in(bounds)?;
                let lines = match delta {
                    mouse::ScrollDelta::Lines { y, .. } => *y,
                    mouse::ScrollDelta::Pixels { y, .. } => *y / 40.0,
                };
                if lines == 0.0 {
                    return None;
                }
                if position.x < GUTTER {
                    // Over the legend the wheel scrolls the lanes.
                    let layout = self.layout(bounds.height, state.scroll);
                    let wanted =
                        (state.scroll - lines * SCROLL_STEP).clamp(0.0, layout.max_offset());
                    if (wanted - state.scroll).abs() > f32::EPSILON {
                        state.scroll = wanted;
                        self.cache.clear();
                    }
                    return Some(canvas::Action::request_redraw().and_capture());
                }
                let command = if state.modifiers.shift() {
                    Command::Pan {
                        pixels: (-lines * 60.0) as i64,
                    }
                } else {
                    // One notch changes the zoom by a factor of 5/4; a few notches at once
                    // are combined.
                    let notches = lines.abs().ceil().clamp(1.0, 5.0) as u32;
                    let (small, large) = (4_u64.pow(notches), 5_u64.pow(notches));
                    let (numerator, denominator) = if lines > 0.0 {
                        (small, large)
                    } else {
                        (large, small)
                    };
                    Command::Zoom {
                        factor: ZoomFactor {
                            numerator,
                            denominator,
                        },
                        anchor: ZoomAnchor::Column((position.x - GUTTER).max(0.0) as u32),
                    }
                };
                Some(canvas::Action::publish(Message::Run(command)).and_capture())
            }
            // The right button cancels a placement; on a marker it opens its actions.
            canvas::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Right)) => {
                let position = cursor.position_in(bounds)?;
                if self.placement.is_some() {
                    return Some(
                        canvas::Action::publish(Message::Run(Command::CancelPlacement))
                            .and_capture(),
                    );
                }
                let scale = (bounds.width - GUTTER).max(1.0) / self.plan.pixels.max(1) as f32;
                let (id, _) = self.marker_hit(position, scale)?;
                Some(canvas::Action::publish(Message::MarkerContext(id)).and_capture())
            }
            canvas::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                let position = cursor.position_in(bounds)?;
                if self.editing {
                    // A click anywhere accepts the name being edited and does nothing else.
                    return Some(canvas::Action::publish(Message::ChannelNameAccept).and_capture());
                }
                let layout = self.layout(bounds.height, state.scroll);
                if position.x < GUTTER {
                    let LegendHit { lane, zone } = layout.zone_at(position.x, position.y)?;
                    let channel = self.plan.channels.get(lane)?;
                    if zone == Zone::Border {
                        let now = Instant::now();
                        let double = state.last_border.is_some_and(|(at, l)| {
                            l == lane && now.saturating_duration_since(at) < DOUBLE_CLICK
                        });
                        state.last_border = Some((now, lane));
                        if double {
                            state.last_border = None;
                            return Some(
                                canvas::Action::publish(Message::Run(Command::SetChannelHeight {
                                    channel: channel.channel,
                                    height: 0,
                                }))
                                .and_capture(),
                            );
                        }
                        state.drag = Some(Drag::Resize {
                            lane,
                            start_height: layout.lane(lane).map_or(AUTO_MAX, |l| l.height),
                            start_y: position.y,
                        });
                    }
                    state.press = Some(Press {
                        lane,
                        zone,
                        at: position,
                    });
                    return Some(canvas::Action::capture());
                }
                // A click on the edge button scrolls to the edge and does nothing else.
                if let Some((index, direction, rectangle)) =
                    self.edge_button(position, bounds.width, &layout)
                    && rectangle.contains(position)
                {
                    let channel = self.plan.channels.get(index)?;
                    return Some(
                        canvas::Action::publish(Message::Run(Command::ScrollToEdge {
                            channel: channel.channel,
                            direction,
                        }))
                        .and_capture(),
                    );
                }
                let scale = (bounds.width - GUTTER).max(1.0) / self.plan.pixels.max(1) as f32;
                // While a marker is being placed a click fixes it (and does nothing else).
                if self.placement.is_some() {
                    let pointer = self.host_pointer(position, bounds.width, &layout);
                    return Some(
                        canvas::Action::publish(Message::Run(Command::FixPlacement { pointer }))
                            .and_capture(),
                    );
                }
                // A click on an annotation selects it.
                let lane_top = |index: usize| layout.lane(index).map_or(AXIS_HEIGHT, |l| l.top);
                let lane_height = |index: usize| layout.lane(index).map_or(AUTO_MAX, |l| l.height);
                if let Some(decoder_view::Hit {
                    instance,
                    annotation,
                    ..
                }) = decoder_view::hit_test(
                    self.plan,
                    position,
                    self.plan.channels.len(),
                    &lane_top,
                    &lane_height,
                    scale,
                ) {
                    let now = Instant::now();
                    let double = state.last_click.is_some_and(|(at, i, a)| {
                        i == instance
                            && a == annotation
                            && now.saturating_duration_since(at) < Duration::from_millis(400)
                    });
                    state.last_click = Some((now, instance, annotation));
                    let command = if double {
                        Command::GoToAnnotation(la_host::AnnotationTarget::Selected)
                    } else {
                        Command::SelectAnnotation {
                            instance,
                            annotation,
                        }
                    };
                    return Some(canvas::Action::publish(Message::Run(command)).and_capture());
                }
                state.drag_marker = self.marker_hit(position, scale);
                state.drag_from = Some(position.x);
                match state.drag_marker {
                    // A press on a marker selects it and starts a drag of that end.
                    Some((id, _)) => Some(
                        canvas::Action::publish(Message::Run(Command::SelectMarker(Some(id))))
                            .and_capture(),
                    ),
                    None => Some(canvas::Action::capture()),
                }
            }
            // A drag that began in the legend strip: a border is dragged (the height follows the
            // pointer at once), or a lane is lifted and moved.
            canvas::Event::Mouse(mouse::Event::CursorMoved { .. })
                if state.press.is_some() || state.drag.is_some() =>
            {
                let position = cursor.position_in(bounds)?;
                match state.drag {
                    Some(Drag::Resize {
                        lane,
                        start_height,
                        start_y,
                    }) => {
                        let channel = self.plan.channels.get(lane)?;
                        let height = lane_layout::resized_height(start_height, start_y, position.y);
                        if height != channel.height {
                            return Some(
                                canvas::Action::publish(Message::Run(Command::SetChannelHeight {
                                    channel: channel.channel,
                                    height,
                                }))
                                .and_capture(),
                            );
                        }
                        None
                    }
                    Some(Drag::Lane { from, .. }) => {
                        state.drag = Some(Drag::Lane {
                            from,
                            y: position.y,
                        });
                        Some(canvas::Action::request_redraw().and_capture())
                    }
                    None => {
                        let press = state.press?;
                        let far = (position.x - press.at.x).hypot(position.y - press.at.y)
                            > DRAG_THRESHOLD;
                        if press.zone == Zone::Name && far {
                            state.shifts = vec![0.0; self.plan.channels.len()];
                            state.anim_at = None;
                            state.drag = Some(Drag::Lane {
                                from: press.lane,
                                y: position.y,
                            });
                            return Some(canvas::Action::request_redraw().and_capture());
                        }
                        None
                    }
                }
            }
            // The hover line follows the pointer: it is drawn from the pointer position, so a
            // plain move over the view (no button, nothing to publish) must still ask for a
            // redraw, or the line waits for the next timer tick. It also has to go away when the
            // pointer leaves.
            canvas::Event::Mouse(mouse::Event::CursorMoved { .. }) if state.drag_from.is_none() => {
                // While a marker is being placed the application resolves the pointer (the snapping
                // and the position are the host's) and hands the answer back in `preview`.
                if self.placement.is_some() {
                    let position = cursor.position_in(bounds).filter(|p| p.x >= GUTTER);
                    let layout = self.layout(bounds.height, state.scroll);
                    let pointer = position.map(|p| self.host_pointer(p, bounds.width, &layout));
                    return Some(canvas::Action::publish(Message::PlacementPointer(
                        pointer,
                        Instant::now(),
                    )));
                }
                Some(canvas::Action::request_redraw())
            }
            canvas::Event::Mouse(mouse::Event::CursorLeft) => {
                if self.placement.is_some() {
                    return Some(canvas::Action::publish(Message::PlacementPointer(
                        None,
                        Instant::now(),
                    )));
                }
                Some(canvas::Action::request_redraw())
            }
            canvas::Event::Mouse(mouse::Event::CursorMoved { .. }) => {
                let from = state.drag_from?;
                let position = cursor.position_in(bounds)?;
                if let Some((id, end)) = state.drag_marker {
                    let layout = self.layout(bounds.height, state.scroll);
                    // A pair whose ends crossed has exchanged their roles: the host says which.
                    let end = self
                        .drag_end
                        .filter(|(dragged, _)| *dragged == id)
                        .map_or(end, |(_, role)| role);
                    let pointer = self.host_pointer(position, bounds.width, &layout);
                    return Some(
                        canvas::Action::publish(Message::Run(Command::MoveMarkerEnd {
                            id,
                            end,
                            pointer,
                        }))
                        .and_capture(),
                    );
                }
                let moved = (from - position.x) as i64;
                if moved == 0 {
                    return None;
                }
                state.drag_from = Some(position.x);
                state.panning = true;
                Some(
                    canvas::Action::publish(Message::Run(Command::Pan { pixels: moved }))
                        .and_capture(),
                )
            }
            canvas::Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) => {
                state.drag_from = None;
                state.panning = false;
                if state.drag_marker.take().is_some() {
                    return Some(
                        canvas::Action::publish(Message::Run(Command::EndMarkerDrag)).and_capture(),
                    );
                }
                let press = state.press.take();
                let drag = state.drag.take();
                state.shifts.clear();
                state.anim_at = None;
                let position = cursor.position_in(bounds);
                let layout = self.layout(bounds.height, state.scroll);
                match (drag, press) {
                    (Some(Drag::Lane { from, y }), _) => {
                        // Released outside the strip: cancelled.
                        let inside = position.is_some_and(|p| p.x < GUTTER);
                        let to = layout.move_target(from, y);
                        if inside && to != from {
                            return Some(
                                canvas::Action::publish(Message::MoveLane { from, to })
                                    .and_capture(),
                            );
                        }
                        Some(canvas::Action::request_redraw().and_capture())
                    }
                    (Some(Drag::Resize { .. }), _) => Some(canvas::Action::capture()),
                    (None, Some(press)) => {
                        let channel = self.plan.channels.get(press.lane)?;
                        match press.zone {
                            Zone::Bar => Some(
                                canvas::Action::publish(Message::Run(Command::SetChannelColour {
                                    channel: channel.channel,
                                    colour: next_colour(channel.colour),
                                }))
                                .and_capture(),
                            ),
                            Zone::Name => {
                                // The editor opens over the name: its top, for the app to place.
                                let top = layout.lane(press.lane).map_or(AXIS_HEIGHT, |l| {
                                    name_centre(l.top, l.height) - EDITOR_HALF
                                });
                                Some(
                                    canvas::Action::publish(Message::EditChannelName(
                                        channel.channel,
                                        top,
                                    ))
                                    .and_capture(),
                                )
                            }
                            Zone::Border => None,
                        }
                    }
                    (None, None) => None,
                }
            }
            _ => None,
        }
    }

    fn draw(
        &self,
        state: &ViewState,
        renderer: &Renderer,
        theme: &Theme,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Vec<Geometry> {
        crate::frame_log::frame_started();
        let whole = Instant::now();
        let size = bounds.size();
        // While a lane is carried the others make room, so the layer is drawn afresh at their
        // moved places (the cache keeps the lanes at rest); the carried lane is left out of it and
        // drawn under the pointer in the overlay.
        let waveforms = if let Some(Drag::Lane { from, .. }) = state.drag {
            let mut shifts = state.shifts.clone();
            shifts.resize(self.plan.channels.len().max(from.saturating_add(1)), 0.0);
            if let Some(carried) = shifts.get_mut(from) {
                *carried = -OFF_SCREEN;
            }
            let mut frame = Frame::new(renderer, size);
            self.draw_plan(&mut frame, size, theme, state.scroll, &shifts);
            frame.into_geometry()
        } else {
            self.cache.draw(renderer, size, |frame| {
                let started = Instant::now();
                self.draw_plan(frame, size, theme, state.scroll, &[]);
                crate::frame_log::record(crate::frame_log::Stage::Draw, started.elapsed());
            })
        };
        let mut overlay = Frame::new(renderer, size);
        // The marks are drawn in the layer above the waveforms and the axis, because the text of
        // a layer is drawn over its shapes: the labels of the axis must not be over the marks.
        self.draw_markers(&mut overlay, size);
        if self.placement.is_some() {
            self.draw_placement(&mut overlay, size, state.scroll);
        } else if let Some(position) = cursor.position_in(bounds) {
            self.draw_hover(&mut overlay, size, position, theme, state.scroll);
        }
        self.draw_lane_drag(&mut overlay, size, theme, state);
        if state.drag.is_none()
            && state.drag_from.is_none()
            && let Some(position) = cursor.position_in(bounds)
        {
            let layout = self.layout(size.height, state.scroll);
            if let Some((_, direction, rectangle)) = self.edge_button(position, size.width, &layout)
            {
                self.draw_edge_button(&mut overlay, theme, direction, rectangle);
            }
        }
        let geometry = vec![waveforms, overlay.into_geometry()];
        crate::frame_log::record(crate::frame_log::Stage::Overlay, whole.elapsed());
        geometry
    }

    fn mouse_interaction(
        &self,
        state: &ViewState,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> mouse::Interaction {
        if matches!(state.drag, Some(Drag::Lane { .. })) {
            return mouse::Interaction::Grabbing;
        }
        if matches!(state.drag, Some(Drag::Resize { .. })) {
            return mouse::Interaction::ResizingVertically;
        }
        // The view is being moved sideways with the mouse: a hand, until the button is released.
        if state.panning {
            return mouse::Interaction::Grabbing;
        }
        let Some(position) = cursor.position_in(bounds) else {
            return mouse::Interaction::default();
        };
        let layout = self.layout(bounds.height, state.scroll);
        if position.x < GUTTER {
            return match layout.zone_at(position.x, position.y).map(|h| h.zone) {
                Some(Zone::Border) => mouse::Interaction::ResizingVertically,
                Some(Zone::Bar | Zone::Name) => mouse::Interaction::Pointer,
                None => mouse::Interaction::default(),
            };
        }
        if self
            .edge_button(position, bounds.width, &layout)
            .is_some_and(|(_, _, rectangle)| rectangle.contains(position))
        {
            return mouse::Interaction::Pointer;
        }
        mouse::Interaction::Crosshair
    }
}

impl SignalView<'_> {
    /// The rounded square with an arrow that scrolls to the next edge on its side.
    fn draw_edge_button(
        &self,
        frame: &mut Frame,
        theme: &Theme,
        direction: EdgeDirection,
        rectangle: Rectangle,
    ) {
        let palette = theme.palette();
        let square = Path::rounded_rectangle(
            rectangle.position(),
            rectangle.size(),
            iced::border::Radius::from(5.0),
        );
        frame.fill(
            &square,
            Color {
                a: 0.9,
                ..palette.background
            },
        );
        frame.stroke(
            &square,
            Stroke::default()
                .with_color(palette.primary)
                .with_width(1.5),
        );
        // A chevron that points to the side the edge lies on.
        let centre = rectangle.center();
        let (tip, tail) = match direction {
            EdgeDirection::Next => (centre.x + 4.0, centre.x - 3.0),
            EdgeDirection::Previous => (centre.x - 4.0, centre.x + 3.0),
        };
        let chevron = Path::new(|b| {
            b.move_to(Point::new(tail, centre.y - 6.0));
            b.line_to(Point::new(tip, centre.y));
            b.line_to(Point::new(tail, centre.y + 6.0));
        });
        frame.stroke(
            &chevron,
            Stroke::default()
                .with_color(palette.primary)
                .with_width(2.0),
        );
    }

    /// The text of a lane in the legend: the name the user gave, as the main text, and under it the
    /// number of the channel of the analyzer, so that the wire is never lost when the lanes are
    /// moved. A lane too low for two lines shows the number small at the right edge instead.
    fn draw_legend_text(
        &self,
        frame: &mut Frame,
        top: f32,
        height: f32,
        index: usize,
        colour: Color,
    ) {
        let number = self
            .plan
            .channels
            .get(index)
            .map(|c| physical_label(c.channel))
            .unwrap_or_default();
        let left = BAR_WIDTH + 6.0;
        let two_lines = height >= TWO_LINES_FROM;
        let name_y = name_centre(top, height);
        frame.fill_text(Text {
            content: fit(&self.channel_label(index), if two_lines { 13 } else { 9 }),
            position: Point::new(left, name_y),
            color: colour,
            size: Pixels(TEXT_SIZE),
            align_y: Vertical::Center,
            ..Text::default()
        });
        let (x, y, align) = if two_lines {
            (
                left,
                top + height / 2.0 + 9.0,
                iced::alignment::Horizontal::Left,
            )
        } else {
            (
                GUTTER - 6.0,
                top + height / 2.0,
                iced::alignment::Horizontal::Right,
            )
        };
        frame.fill_text(Text {
            content: number,
            position: Point::new(x, y),
            color: Color { a: 0.75, ..colour },
            size: Pixels(NUMBER_SIZE),
            align_x: align.into(),
            align_y: Vertical::Center,
            ..Text::default()
        });
    }

    /// The lane that is being moved, lifted under the pointer as a whole (its background, its
    /// bands and its signal, the full width of the view), and the line where it will land (the
    /// other lanes make room there, see `shifts`).
    /// The marker being placed: its line, dotted, at the sample the pointer means, and its mark;
    /// for a pair the half that is already fixed is drawn solid, and each half has the mark of the
    /// role it would have (the start's square left of its line, the end's right of its line).
    fn draw_placement(&self, frame: &mut Frame, size: Size, scroll: f32) {
        let (Some(placement), Some(preview)) = (self.placement, self.preview) else {
            return;
        };
        let plan = self.plan;
        let colour = self.variant.marker(placement.colour);
        let scale = (size.width - GUTTER).max(1.0) / plan.pixels.max(1) as f32;
        let x_of = |sample: u64| {
            let offset = sample as f64 - plan.window.start as f64;
            GUTTER
                + (offset * plan.pixels.max(1) as f64 / plan.window.length.max(1) as f64) as f32
                    * scale
        };
        let pointer_x = x_of(preview.position);
        let draw_half = |frame: &mut Frame, x: f32, end: End, dotted: bool| {
            marker_marks::draw_line(
                frame,
                x,
                marker_marks::line_top(AXIS_HEIGHT, end),
                size.height,
                colour,
                false,
                dotted,
            );
            marker_marks::draw_mark(frame, x, AXIS_HEIGHT, end, colour, placement.number);
        };
        // The transition the line has snapped to: a ring on its lane and a tick over the lane.
        if let Some(snapped) = preview.snapped {
            let layout = self.layout(size.height, scroll);
            let lane = self
                .plan
                .channels
                .iter()
                .position(|c| c.channel == snapped.channel)
                .and_then(|index| layout.lane(index));
            if let Some(lane) = lane {
                let x = x_of(snapped.position);
                let stroke = Stroke::default().with_color(colour).with_width(2.0);
                frame.stroke(
                    &Path::line(Point::new(x, lane.top), Point::new(x, lane.bottom())),
                    stroke,
                );
                frame.stroke(
                    &Path::circle(Point::new(x, lane.top + lane.height / 2.0), 6.0),
                    stroke,
                );
            }
        }
        match (placement.kind, placement.first) {
            (Kind::Single, _) => draw_half(frame, pointer_x, End::Point, true),
            (Kind::Pair, None) => draw_half(frame, pointer_x, End::Start, true),
            (Kind::Pair, Some(first)) => {
                let fixed_x = x_of(first);
                let (fixed_end, moving_end) = if preview.position >= first {
                    (End::Start, End::End)
                } else {
                    (End::End, End::Start)
                };
                draw_half(frame, fixed_x, fixed_end, false);
                draw_half(frame, pointer_x, moving_end, true);
            }
        }
    }

    fn draw_lane_drag(&self, frame: &mut Frame, size: Size, theme: &Theme, state: &ViewState) {
        let Some(Drag::Lane { from, y }) = state.drag else {
            return;
        };
        let layout = self.layout(size.height, state.scroll);
        let palette = theme.palette();
        let line = layout.landing_line(layout.landing(y));
        frame.stroke(
            &Path::line(Point::new(0.0, line), Point::new(size.width, line)),
            Stroke::default()
                .with_color(palette.primary)
                .with_width(2.0),
        );
        let (Some(lane), Some(channel)) = (layout.lane(from), self.plan.channels.get(from)) else {
            return;
        };
        let colour = self.variant.channel(channel.colour);
        // The lifted lane follows the pointer, kept inside the lanes' area.
        let top = (y - lane.height / 2.0)
            .clamp(AXIS_HEIGHT, (size.height - lane.height).max(AXIS_HEIGHT));
        frame.fill_rectangle(
            Point::new(0.0, top),
            Size::new(size.width, lane.height),
            Color {
                a: 0.96,
                ..palette.background
            },
        );
        let scale = (size.width - GUTTER).max(1.0) / self.plan.pixels.max(1) as f32;
        self.draw_bands(frame, &channel.segments, top, lane.height, scale, colour);
        frame.stroke(
            &waveform(
                &channel.segments,
                scale,
                top + WAVE_MARGIN,
                top + lane.height - WAVE_MARGIN,
            ),
            Stroke::default().with_color(colour).with_width(1.6),
        );
        // The legend part on top of the signal, as in the lane at rest.
        frame.fill_rectangle(
            Point::new(0.0, top),
            Size::new(GUTTER, lane.height),
            Color {
                a: 0.96,
                ..palette.background
            },
        );
        frame.fill_rectangle(
            Point::new(0.0, top),
            Size::new(BAR_WIDTH, lane.height),
            colour,
        );
        self.draw_legend_text(frame, top, lane.height, from, colour);
        // The outline of the lifted lane.
        let outline = Path::rectangle(Point::new(0.0, top), Size::new(size.width, lane.height));
        frame.stroke(
            &outline,
            Stroke::default().with_color(colour).with_width(1.5),
        );
    }

    /// The time under the pointer, with a vertical hairline.
    fn draw_hover(
        &self,
        frame: &mut Frame,
        size: Size,
        position: Point,
        theme: &Theme,
        scroll: f32,
    ) {
        let plan = self.plan;
        if plan.state != FrameState::Ok || position.x < GUTTER || plan.pixels == 0 {
            return;
        }
        let area_width = (size.width - GUTTER).max(1.0);
        let fraction = f64::from((position.x - GUTTER) / area_width).clamp(0.0, 1.0);
        let sample = plan.window.start as f64 + fraction * plan.window.length as f64;
        let seconds = sample / plan.sample_rate_hz.max(1) as f64;
        let colour = theme.palette().text;
        frame.stroke(
            &Path::line(
                Point::new(position.x, AXIS_HEIGHT),
                Point::new(position.x, size.height),
            ),
            Stroke::default()
                .with_color(Color { a: 0.5, ..colour })
                .with_width(1.0),
        );
        // The time is attached to the pointer, not to a corner where it would merge with the axis.
        let content = self.localizer.format_duration(seconds);
        let label = hover_label_rect(position, size, hover_label_size(&content));
        let palette = theme.palette();
        frame.fill_rectangle(
            label.position(),
            label.size(),
            Color {
                a: 0.92,
                ..palette.background
            },
        );
        frame.stroke(
            &Path::rectangle(label.position(), label.size()),
            Stroke::default()
                .with_color(Color { a: 0.5, ..colour })
                .with_width(1.0),
        );
        frame.fill_text(Text {
            content,
            position: Point::new(label.x + HOVER_PAD_X, label.y + label.height / 2.0),
            color: colour,
            size: Pixels(TEXT_SIZE),
            align_y: Vertical::Center,
            ..Text::default()
        });
        self.draw_annotation_readout(frame, size, position, theme, scroll);
    }

    /// The full text of the annotation under the pointer and its length in time, in a box next to
    /// the pointer (a span's text may be cut in the lane).
    fn draw_annotation_readout(
        &self,
        frame: &mut Frame,
        size: Size,
        position: Point,
        theme: &Theme,
        scroll: f32,
    ) {
        let plan = self.plan;
        let layout = self.layout(size.height, scroll);
        let lane_top = |index: usize| layout.lane(index).map_or(AXIS_HEIGHT, |l| l.top);
        let lane_height = |index: usize| layout.lane(index).map_or(AUTO_MAX, |l| l.height);
        let scale = (size.width - GUTTER).max(1.0) / plan.pixels.max(1) as f32;
        let Some(hit) = decoder_view::hit_test(
            plan,
            position,
            plan.channels.len(),
            &lane_top,
            &lane_height,
            scale,
        ) else {
            return;
        };
        let seconds = hit.end.saturating_sub(hit.start) as f64 / plan.sample_rate_hz.max(1) as f64;
        let content = format!(
            "{}  \u{b7}  {}",
            la_host::decoders::annotation_text(self.localizer, &hit.text),
            self.localizer.format_duration(seconds)
        );
        let width = content.chars().count() as f32 * 7.0 + 12.0;
        let x = (position.x + 12.0).min((size.width - width - 4.0).max(0.0));
        // Below the time of the pointer, which sits right under the crosshair.
        let y = (position.y + 14.0 + HOVER_LABEL_HEIGHT + 4.0).min((size.height - 24.0).max(0.0));
        let palette = theme.palette();
        frame.fill_rectangle(
            Point::new(x, y),
            Size::new(width, 20.0),
            Color {
                a: 0.92,
                ..palette.background
            },
        );
        frame.stroke(
            &Path::rectangle(Point::new(x, y), Size::new(width, 20.0)),
            Stroke::default()
                .with_color(if hit.error {
                    palette.danger
                } else {
                    palette.text
                })
                .with_width(1.0),
        );
        frame.fill_text(Text {
            content,
            position: Point::new(x + 6.0, y + 10.0),
            color: palette.text,
            size: Pixels(TEXT_SIZE),
            align_y: Vertical::Center,
            ..Text::default()
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use iced::widget::canvas::Program;
    use iced::{Point, Rectangle, mouse};
    use la_host::plan::MarkerPlan;
    use la_host::window::ViewSize;
    use la_host::{Shell, ShellConfig};

    fn event_result(
        state: &mut ViewState,
        event: &canvas::Event,
        cursor: mouse::Cursor,
    ) -> Option<(Option<Message>, iced::window::RedrawRequest)> {
        let shell = Shell::new(ShellConfig::for_tests(&std::env::temp_dir()));
        let plan = shell.frame_plan(ViewSize::default());
        let cache = Cache::new();
        let localizer = la_i18n::Localizer::new(la_i18n::Language::En);
        let view = SignalView {
            plan: &plan,
            cache: &cache,
            variant: Variant::Dark,
            localizer: &localizer,
            selected: None,
            editing: false,
            placement: None,
            preview: None,
            drag_end: None,
        };
        let bounds = Rectangle::new(Point::ORIGIN, iced::Size::new(900.0, 500.0));
        // The width is reported first, as the view does on its first event.
        state.reported_width = bounds.width - GUTTER;
        view.update(state, event, bounds, cursor).map(|action| {
            let (message, redraw, _) = action.into_inner();
            (message, redraw)
        })
    }

    #[test]
    fn a_lane_shows_the_name_above_the_number_of_its_analyzer_channel() {
        // The number is the channel of the analyzer, not the place of the lane: moving a lane
        // never changes it.
        assert_eq!(physical_label(0), "D0");
        assert_eq!(physical_label(13), "D13");
        // Two lines from 34 px: the name is the upper one; a lower lane keeps one line.
        assert!(name_centre(100.0, 72.0) < 100.0 + 36.0);
        assert!((name_centre(100.0, 30.0) - 115.0).abs() < f32::EPSILON);
        const { assert!(NUMBER_SIZE < TEXT_SIZE) };
        // Both lines fit the smallest lane that has two: 13 + 10 pixels of text in 34.
        const { assert!(TWO_LINES_FROM >= TEXT_SIZE + NUMBER_SIZE + 8.0) };
    }

    #[test]
    fn a_dense_band_spans_the_levels_of_the_waveform_and_never_more() {
        for lane in [24.0_f32, 30.0, 72.0, 240.0] {
            let (y, height) = band_extent(0, 100.0, lane);
            assert!((y - (100.0 + WAVE_MARGIN)).abs() < f32::EPSILON, "{lane}");
            assert!(
                (y + height - (100.0 + lane - WAVE_MARGIN)).abs() < f32::EPSILON,
                "{lane}"
            );
            // Gaps and the part past the end keep their wider band.
            let (gap_y, _) = band_extent(1, 100.0, lane);
            assert!(gap_y < y);
        }
    }

    #[test]
    fn a_plain_pointer_move_asks_for_a_redraw_so_the_hover_line_follows_at_once() {
        let mut state = ViewState::default();
        let moved = canvas::Event::Mouse(mouse::Event::CursorMoved {
            position: Point::new(400.0, 200.0),
        });
        let result = event_result(
            &mut state,
            &moved,
            mouse::Cursor::Available(Point::new(400.0, 200.0)),
        )
        .expect("a move over the view is handled");
        assert!(
            result.0.is_none(),
            "nothing is published: no command is needed"
        );
        assert!(
            matches!(result.1, iced::window::RedrawRequest::NextFrame),
            "{:?}",
            result.1
        );
    }

    #[test]
    fn leaving_the_view_asks_for_a_redraw_so_the_hover_line_goes_away() {
        let mut state = ViewState::default();
        let left = canvas::Event::Mouse(mouse::Event::CursorLeft);
        let result = event_result(&mut state, &left, mouse::Cursor::Unavailable)
            .expect("leaving is handled");
        assert!(matches!(result.1, iced::window::RedrawRequest::NextFrame));
    }

    #[test]
    fn the_time_is_below_and_to_the_right_of_the_crosshair_when_there_is_room() {
        let size = Size::new(900.0, 500.0);
        let label = Size::new(80.0, HOVER_LABEL_HEIGHT);
        let place = hover_label_rect(Point::new(300.0, 200.0), size, label);
        assert_eq!(place.x, 300.0 + HOVER_OFFSET);
        assert_eq!(place.y, 200.0 + HOVER_OFFSET);
        assert_eq!(place.size(), label);
    }

    #[test]
    fn the_time_goes_to_the_left_when_the_right_side_is_too_narrow() {
        let size = Size::new(900.0, 500.0);
        let label = Size::new(80.0, HOVER_LABEL_HEIGHT);
        let place = hover_label_rect(Point::new(880.0, 200.0), size, label);
        assert_eq!(place.x, 880.0 - HOVER_OFFSET - 80.0);
        assert!(place.x + place.width <= 880.0, "left of the crosshair");
        assert_eq!(place.y, 200.0 + HOVER_OFFSET, "still below");
    }

    #[test]
    fn the_time_goes_above_near_the_bottom_and_never_over_the_axis_or_outside_the_view() {
        let size = Size::new(900.0, 500.0);
        let label = Size::new(80.0, HOVER_LABEL_HEIGHT);
        let low = hover_label_rect(Point::new(300.0, 495.0), size, label);
        assert!(low.y + low.height <= 495.0, "above the crosshair: {low:?}");
        // At the top the pointer is in the axis: the box stays under it.
        let top = hover_label_rect(Point::new(300.0, 5.0), size, label);
        assert!(top.y >= AXIS_HEIGHT, "{top:?}");
        // At the corners the box stays inside the view and clear of the legend.
        for point in [
            Point::new(GUTTER, 5.0),
            Point::new(size.width, size.height),
            Point::new(size.width, 5.0),
            Point::new(GUTTER + 1.0, size.height),
        ] {
            let r = hover_label_rect(point, size, label);
            assert!(r.x >= GUTTER, "{point:?}: {r:?}");
            assert!(r.x + r.width <= size.width + 0.5, "{point:?}: {r:?}");
            assert!(r.y >= AXIS_HEIGHT, "{point:?}: {r:?}");
        }
    }

    #[test]
    fn the_box_grows_with_its_text() {
        assert!(hover_label_size("1.234567 ms").width > hover_label_size("1 ms").width);
        assert_eq!(hover_label_size("").height, HOVER_LABEL_HEIGHT);
    }

    #[test]
    fn the_pointer_is_a_hand_while_the_view_is_moved_sideways_and_not_before() {
        let shell = Shell::new(ShellConfig::for_tests(&std::env::temp_dir()));
        let plan = shell.frame_plan(ViewSize::default());
        let cache = Cache::new();
        let localizer = la_i18n::Localizer::new(la_i18n::Language::En);
        let view = SignalView {
            plan: &plan,
            cache: &cache,
            variant: Variant::Dark,
            localizer: &localizer,
            selected: None,
            editing: false,
            placement: None,
            preview: None,
            drag_end: None,
        };
        let bounds = Rectangle::new(Point::ORIGIN, iced::Size::new(900.0, 500.0));
        let at = mouse::Cursor::Available(Point::new(400.0, 200.0));
        // Pressed but not moved: still the crosshair.
        let pressed = ViewState {
            drag_from: Some(400.0),
            ..ViewState::default()
        };
        assert_eq!(
            view.mouse_interaction(&pressed, bounds, at),
            mouse::Interaction::Crosshair
        );
        // Moved: a hand.
        let moving = ViewState {
            drag_from: Some(400.0),
            panning: true,
            ..ViewState::default()
        };
        assert_eq!(
            view.mouse_interaction(&moving, bounds, at),
            mouse::Interaction::Grabbing
        );
    }

    #[test]
    fn dragging_sideways_makes_the_pointer_a_hand_until_the_button_is_released() {
        let mut state = ViewState {
            drag_from: Some(500.0),
            ..ViewState::default()
        };
        assert!(!state.panning);
        let moved = canvas::Event::Mouse(mouse::Event::CursorMoved {
            position: Point::new(480.0, 200.0),
        });
        let _ = event_result(
            &mut state,
            &moved,
            mouse::Cursor::Available(Point::new(480.0, 200.0)),
        );
        assert!(state.panning, "the view moved");
        let released = canvas::Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left));
        let _ = event_result(
            &mut state,
            &released,
            mouse::Cursor::Available(Point::new(480.0, 200.0)),
        );
        assert!(!state.panning, "the hand goes with the button");
    }

    #[test]
    fn dragging_still_pans_and_does_not_only_redraw() {
        let mut state = ViewState {
            drag_from: Some(500.0),
            ..ViewState::default()
        };
        let moved = canvas::Event::Mouse(mouse::Event::CursorMoved {
            position: Point::new(480.0, 200.0),
        });
        let result = event_result(
            &mut state,
            &moved,
            mouse::Cursor::Available(Point::new(480.0, 200.0)),
        )
        .expect("a drag is handled");
        assert!(result.0.is_some(), "a drag publishes the pan");
    }

    /// A plan with four channels of a finished capture, for a view of 900 x 500.
    fn plan_with_channels() -> (FramePlan, Shell) {
        use la_host::{Activity, ConfigEdit};
        // A scratch directory of its own: the tests run in parallel.
        static COUNTER: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
        let unique = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("la-legend-{}-{unique}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let mut shell = Shell::new(ShellConfig::for_tests(&dir));
        for edit in [
            ConfigEdit::Realtime(false),
            ConfigEdit::Channels(0b1111),
            ConfigEdit::SampleRate(1_000_000),
            ConfigEdit::Length(50_000),
            ConfigEdit::Seed(Some(3)),
        ] {
            shell.apply(Command::EditConfig(edit));
        }
        shell.apply(Command::StartCapture);
        while shell.state().activity != Activity::Idle {
            shell.poll(Instant::now());
            std::thread::sleep(Duration::from_millis(2));
        }
        let plan = shell.frame_plan(ViewSize {
            pixels: 788,
            scale: 1.0,
        });
        let _ = std::fs::remove_dir_all(&dir);
        (plan, shell)
    }

    /// Delivers events to a view of `plan` one after another and collects what they publish.
    fn drive(
        plan: &FramePlan,
        state: &mut ViewState,
        events: &[(canvas::Event, Point)],
    ) -> Vec<Message> {
        let cache = Cache::new();
        let localizer = la_i18n::Localizer::new(la_i18n::Language::En);
        let view = SignalView {
            plan,
            cache: &cache,
            variant: Variant::Dark,
            localizer: &localizer,
            selected: None,
            editing: false,
            placement: None,
            preview: None,
            drag_end: None,
        };
        let bounds = Rectangle::new(Point::ORIGIN, iced::Size::new(900.0, 500.0));
        state.reported_width = bounds.width - GUTTER;
        let mut published = Vec::new();
        for (event, at) in events {
            if let Some(action) = view.update(state, event, bounds, mouse::Cursor::Available(*at)) {
                let (message, _, _) = action.into_inner();
                published.extend(message);
            }
        }
        published
    }

    fn press(at: Point) -> (canvas::Event, Point) {
        (
            canvas::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)),
            at,
        )
    }

    fn release(at: Point) -> (canvas::Event, Point) {
        (
            canvas::Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)),
            at,
        )
    }

    fn moved(at: Point) -> (canvas::Event, Point) {
        (
            canvas::Event::Mouse(mouse::Event::CursorMoved { position: at }),
            at,
        )
    }

    /// A plan whose first channel has edges on both sides and whose second has none.
    fn plan_with_edges() -> (FramePlan, Shell) {
        let (mut plan, shell) = plan_with_channels();
        for (index, channel) in plan.channels.iter_mut().enumerate() {
            channel.edge_before = index == 0 || index == 2;
            channel.edge_after = index == 0;
        }
        (plan, shell)
    }

    fn view_of<'a>(
        plan: &'a FramePlan,
        cache: &'a Cache,
        localizer: &'a Localizer,
    ) -> SignalView<'a> {
        SignalView {
            plan,
            cache,
            variant: Variant::Dark,
            localizer,
            selected: None,
            editing: false,
            placement: None,
            preview: None,
            drag_end: None,
        }
    }

    #[test]
    fn the_edge_button_shows_only_near_an_end_that_has_an_edge_beyond() {
        let (plan, _shell) = plan_with_edges();
        let (cache, localizer) = (Cache::new(), Localizer::new(la_i18n::Language::En));
        let view = view_of(&plan, &cache, &localizer);
        let layout = view.layout(500.0, 0.0);
        // Lane 0 is 28..100: ten pixels from the right end of the waveform area (900 wide).
        let right = view.edge_button(Point::new(890.0, 60.0), 900.0, &layout);
        let (index, direction, rectangle) = right.expect("the right button");
        assert_eq!((index, direction), (0, EdgeDirection::Next));
        assert!((rectangle.width - EDGE_BUTTON).abs() < f32::EPSILON);
        // Centred in the lane.
        assert!((rectangle.center_y() - 64.0).abs() < 0.01, "{rectangle:?}");
        let left = view.edge_button(Point::new(120.0, 60.0), 900.0, &layout);
        assert_eq!(left.map(|(_, d, _)| d), Some(EdgeDirection::Previous));
        // The middle of the lane, the legend and the axis show nothing.
        for at in [
            Point::new(500.0, 60.0),
            Point::new(60.0, 60.0),
            Point::new(890.0, 10.0),
        ] {
            assert!(view.edge_button(at, 900.0, &layout).is_none(), "{at:?}");
        }
        // Lane 1 has no edge on either side; lane 2 only before.
        assert!(
            view.edge_button(Point::new(890.0, 130.0), 900.0, &layout)
                .is_none()
        );
        assert!(
            view.edge_button(Point::new(120.0, 130.0), 900.0, &layout)
                .is_none()
        );
        assert!(
            view.edge_button(Point::new(890.0, 200.0), 900.0, &layout)
                .is_none()
        );
        assert!(
            view.edge_button(Point::new(120.0, 200.0), 900.0, &layout)
                .is_some()
        );
    }

    #[test]
    fn a_decoder_lane_has_no_edge_button() {
        let (mut plan, _shell) = plan_with_edges();
        // Lanes below the channels are decoders; lane 4 would be the first one.
        plan.lanes.push(la_host::plan::LanePlan {
            instance: 1,
            name: "d".into(),
            state: la_host::decoders::InstanceState::Finished,
            incomplete: false,
            outdated: false,
            plan: la_decoders::lanes::Plan::default(),
        });
        let (cache, localizer) = (Cache::new(), Localizer::new(la_i18n::Language::En));
        let view = view_of(&plan, &cache, &localizer);
        let layout = view.layout(500.0, 0.0);
        let decoder = layout.lane(4).expect("the decoder lane");
        let y = decoder.top + decoder.height / 2.0;
        assert!(
            view.edge_button(Point::new(890.0, y), 900.0, &layout)
                .is_none()
        );
        assert!(
            view.edge_button(Point::new(120.0, y), 900.0, &layout)
                .is_none()
        );
    }

    #[test]
    fn a_press_on_the_button_scrolls_and_a_press_next_to_it_does_not() {
        let (plan, _shell) = plan_with_edges();
        let channel = plan.channels[0].channel;
        // The button is x 872..896 (900 - 28 .. 900 - 4) and y 52..76 for lane 0.
        let on = Point::new(884.0, 64.0);
        let published = drive(&plan, &mut ViewState::default(), &[press(on), release(on)]);
        assert!(
            matches!(
                published.as_slice(),
                [Message::Run(Command::ScrollToEdge { channel: c, direction: EdgeDirection::Next })] if *c == channel
            ),
            "{published:?}"
        );
        let on_left = Point::new(124.0, 64.0);
        let published = drive(
            &plan,
            &mut ViewState::default(),
            &[press(on_left), release(on_left)],
        );
        assert!(
            matches!(
                published.as_slice(),
                [Message::Run(Command::ScrollToEdge {
                    direction: EdgeDirection::Previous,
                    ..
                })]
            ),
            "{published:?}"
        );
        // Inside the zone but outside the button (above it): it is a press on the waveform.
        let beside = Point::new(884.0, 33.0);
        let published = drive(
            &plan,
            &mut ViewState::default(),
            &[press(beside), release(beside)],
        );
        assert!(
            !published
                .iter()
                .any(|m| matches!(m, Message::Run(Command::ScrollToEdge { .. }))),
            "{published:?}"
        );
    }

    #[test]
    fn the_button_is_drawn_in_the_overlay_and_the_cache_is_not_cleared() {
        // The press and the hover never clear the cache: only the plan, the theme and the layout do.
        let (plan, _shell) = plan_with_edges();
        let (cache, localizer) = (Cache::new(), Localizer::new(la_i18n::Language::En));
        let view = view_of(&plan, &cache, &localizer);
        let mut state = ViewState::default();
        let at = Point::new(884.0, 64.0);
        let bounds = Rectangle::new(Point::ORIGIN, iced::Size::new(900.0, 500.0));
        state.reported_width = bounds.width - GUTTER;
        let hover = canvas::Event::Mouse(mouse::Event::CursorMoved { position: at });
        let action = view.update(&mut state, &hover, bounds, mouse::Cursor::Available(at));
        assert!(action.is_some_and(|a| a.into_inner().0.is_none()));
        assert!(matches!(
            view.mouse_interaction(&state, bounds, mouse::Cursor::Available(at)),
            mouse::Interaction::Pointer
        ));
        assert!(matches!(
            view.mouse_interaction(
                &state,
                bounds,
                mouse::Cursor::Available(Point::new(500.0, 64.0))
            ),
            mouse::Interaction::Crosshair
        ));
    }

    #[test]
    fn the_lanes_slide_aside_while_a_lane_is_carried_and_settle_on_their_places() {
        let (plan, _shell) = plan_with_channels();
        let (cache, localizer) = (Cache::new(), Localizer::new(la_i18n::Language::En));
        let view = view_of(&plan, &cache, &localizer);
        let bounds = Rectangle::new(Point::ORIGIN, iced::Size::new(900.0, 500.0));
        // Lane 0 (28..100) is carried down past the middle of lane 2 (172..244): lanes 1 and 2 make
        // room by moving up the height of lane 0.
        let mut state = ViewState {
            reported_width: bounds.width - GUTTER,
            drag: Some(Drag::Lane { from: 0, y: 230.0 }),
            shifts: vec![0.0; plan.channels.len()],
            ..ViewState::default()
        };
        let start = Instant::now();
        let mut frame_at = start;
        let mut asked_again = false;
        for _ in 0..60 {
            frame_at += Duration::from_millis(16);
            let event = canvas::Event::Window(iced::window::Event::RedrawRequested(frame_at));
            let action = view.update(&mut state, &event, bounds, mouse::Cursor::Unavailable);
            asked_again |= action.is_some();
            // Never past the target on the way.
            assert!(
                state.shifts[1] <= 0.0 && state.shifts[1] >= -72.0,
                "{:?}",
                state.shifts
            );
        }
        assert!(
            asked_again,
            "an unfinished movement asks for the next frame"
        );
        assert_eq!(state.shifts[0], 0.0, "the carried lane is not shifted");
        assert!(
            (state.shifts[1] + 72.0).abs() < f32::EPSILON,
            "{:?}",
            state.shifts
        );
        assert!(
            (state.shifts[2] + 72.0).abs() < f32::EPSILON,
            "{:?}",
            state.shifts
        );
        assert_eq!(state.shifts[3], 0.0);
        // Settled: no further frame is asked for.
        frame_at += Duration::from_millis(16);
        let event = canvas::Event::Window(iced::window::Event::RedrawRequested(frame_at));
        assert!(
            view.update(&mut state, &event, bounds, mouse::Cursor::Unavailable)
                .is_none()
        );
        // Without a carried lane the redraw event changes nothing.
        let mut idle = ViewState {
            reported_width: bounds.width - GUTTER,
            ..ViewState::default()
        };
        assert!(
            view.update(&mut idle, &event, bounds, mouse::Cursor::Unavailable)
                .is_none()
        );
        // Releasing the button clears the shifts.
        let release = canvas::Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left));
        let _ = view.update(
            &mut state,
            &release,
            bounds,
            mouse::Cursor::Available(Point::new(60.0, 200.0)),
        );
        assert!(state.shifts.is_empty());
    }

    // Four channels in an area of 500: the automatic height is clamp(472 / 4, 30, 72) = 72, so the
    // lanes are 28..100, 100..172, 172..244 and 244..316.

    #[test]
    fn a_click_on_the_colour_bar_steps_the_colour() {
        let (plan, _shell) = plan_with_channels();
        let first = plan.channels[0].clone();
        let at = Point::new(3.0, 50.0);
        let published = drive(&plan, &mut ViewState::default(), &[press(at), release(at)]);
        assert_eq!(published.len(), 1, "{published:?}");
        let Message::Run(Command::SetChannelColour { channel, colour }) = &published[0] else {
            panic!("{published:?}");
        };
        assert_eq!(*channel, first.channel);
        assert_eq!(*colour, next_colour(first.colour));
        // A custom colour goes back to the first of the palette.
        assert_eq!(next_colour(Colour::Rgb(1, 2, 3)), Colour::Palette(0));
        assert_eq!(next_colour(Colour::Palette(15)), Colour::Palette(0));
        assert_eq!(next_colour(Colour::Palette(4)), Colour::Palette(5));
    }

    #[test]
    fn a_click_on_the_name_opens_the_editor_over_that_lane() {
        let (plan, _shell) = plan_with_channels();
        let at = Point::new(60.0, 120.0); // lane 1 (100..172)
        let published = drive(&plan, &mut ViewState::default(), &[press(at), release(at)]);
        assert_eq!(published.len(), 1, "{published:?}");
        let Message::EditChannelName(channel, top) = &published[0] else {
            panic!("{published:?}");
        };
        assert_eq!(*channel, plan.channels[1].channel);
        // The editor sits over the name: the middle of the upper line of lane 1 (100..172) is 130.
        assert!((top - (130.0 - EDITOR_HALF)).abs() < f32::EPSILON, "{top}");
    }

    #[test]
    fn a_drag_on_the_name_moves_the_lane_and_a_small_movement_does_not() {
        let (plan, _shell) = plan_with_channels();
        let start = Point::new(60.0, 50.0);
        let down = Point::new(60.0, 300.0);
        let published = drive(
            &plan,
            &mut ViewState::default(),
            &[press(start), moved(down), release(down)],
        );
        assert!(
            matches!(published.as_slice(), [Message::MoveLane { from: 0, to: 3 }]),
            "{published:?}"
        );
        // A movement under the threshold is a click on the name.
        let near = Point::new(62.0, 52.0);
        let published = drive(
            &plan,
            &mut ViewState::default(),
            &[press(start), moved(near), release(near)],
        );
        assert!(
            matches!(published.as_slice(), [Message::EditChannelName(..)]),
            "{published:?}"
        );
        // Released outside the strip: cancelled.
        let outside = Point::new(500.0, 300.0);
        let published = drive(
            &plan,
            &mut ViewState::default(),
            &[press(start), moved(outside), release(outside)],
        );
        assert!(published.is_empty(), "{published:?}");
    }

    #[test]
    fn dragging_a_border_sets_the_height_and_a_double_click_makes_it_automatic() {
        let (plan, _shell) = plan_with_channels();
        let border = Point::new(60.0, 100.0); // the border below lane 0 (72 high)
        let to = Point::new(60.0, 130.0);
        let published = drive(
            &plan,
            &mut ViewState::default(),
            &[press(border), moved(to), release(to)],
        );
        assert!(
            matches!(
                published.as_slice(),
                [Message::Run(Command::SetChannelHeight { height: 102, .. })]
            ),
            "{published:?}"
        );
        // The height stops at the limit.
        let far = Point::new(60.0, 480.0);
        let published = drive(
            &plan,
            &mut ViewState::default(),
            &[press(border), moved(far), release(far)],
        );
        assert!(
            matches!(
                published.as_slice(),
                [Message::Run(Command::SetChannelHeight { height: 240, .. })]
            ),
            "{published:?}"
        );
        // Two presses on the border: automatic.
        let published = drive(
            &plan,
            &mut ViewState::default(),
            &[
                press(border),
                release(border),
                press(border),
                release(border),
            ],
        );
        assert!(
            published
                .iter()
                .any(|m| matches!(m, Message::Run(Command::SetChannelHeight { height: 0, .. }))),
            "{published:?}"
        );
    }

    #[test]
    fn the_wheel_over_the_legend_scrolls_and_over_the_waveforms_it_zooms() {
        let (plan, _shell) = plan_with_channels();
        let wheel = |at: Point| {
            (
                canvas::Event::Mouse(mouse::Event::WheelScrolled {
                    delta: mouse::ScrollDelta::Lines { x: 0.0, y: 1.0 },
                }),
                at,
            )
        };
        // Everything fits, so there is nothing to scroll and nothing is published.
        let mut state = ViewState::default();
        let published = drive(&plan, &mut state, &[wheel(Point::new(50.0, 100.0))]);
        assert!(published.is_empty(), "{published:?}");
        assert!(state.scroll.abs() < f32::EPSILON);
        let published = drive(&plan, &mut state, &[wheel(Point::new(400.0, 100.0))]);
        assert!(
            matches!(published.as_slice(), [Message::Run(Command::Zoom { .. })]),
            "{published:?}"
        );
    }

    // ---- markers: placing, dragging (specs/015-markers-rework, user stories 1 and 2) ----------

    fn area() -> Rectangle {
        Rectangle::new(Point::ORIGIN, iced::Size::new(900.0, 500.0))
    }

    /// Sends one event to a view over `area()`, with the pointer at `at`.
    fn send_event(
        view: &SignalView<'_>,
        state: &mut ViewState,
        event: canvas::Event,
        at: Point,
    ) -> Option<(Option<Message>, iced::window::RedrawRequest)> {
        state.reported_width = area().width - GUTTER;
        view.update(state, &event, area(), mouse::Cursor::Available(at))
            .map(|action| {
                let (message, redraw, _) = action.into_inner();
                (message, redraw)
            })
    }

    fn button_down(button: mouse::Button) -> canvas::Event {
        canvas::Event::Mouse(mouse::Event::ButtonPressed(button))
    }

    fn pointer_moved(at: Point) -> canvas::Event {
        canvas::Event::Mouse(mouse::Event::CursorMoved { position: at })
    }

    fn placing(kind: Kind) -> Option<Placement> {
        Some(Placement {
            kind,
            first: None,
            colour: 0,
            number: 1,
        })
    }

    #[test]
    fn a_click_fixes_the_marker_while_one_is_being_placed() {
        let (plan, _shell) = plan_with_channels();
        let (cache, localizer) = (Cache::new(), Localizer::new(la_i18n::Language::En));
        let mut view = view_of(&plan, &cache, &localizer);
        view.placement = placing(Kind::Single);
        let mut state = ViewState::default();
        // The waveform area is 788 wide and the plan 788 columns: one column per pixel.
        let at = Point::new(GUTTER + 188.0, 60.0);
        let (message, _) =
            send_event(&view, &mut state, button_down(mouse::Button::Left), at).unwrap();
        let Some(Message::Run(Command::FixPlacement { pointer })) = message else {
            panic!("{message:?}");
        };
        assert!((pointer.x - 188.0).abs() < 0.01, "{}", pointer.x);
        assert_eq!(
            pointer.lane,
            Some(plan.channels[0].channel),
            "the lane under the pointer"
        );
        // Over the ruler there is no lane, and the click still fixes it.
        let (message, _) = send_event(
            &view,
            &mut state,
            button_down(mouse::Button::Left),
            Point::new(GUTTER + 5.0, 10.0),
        )
        .unwrap();
        let Some(Message::Run(Command::FixPlacement { pointer })) = message else {
            panic!("{message:?}");
        };
        assert_eq!(pointer.lane, None);
        // A click on the legend strip belongs to the legend, not to the placement.
        let message = send_event(
            &view,
            &mut state,
            button_down(mouse::Button::Left),
            Point::new(GUTTER - 50.0, 60.0),
        )
        .and_then(|(message, _)| message);
        assert!(!matches!(
            message,
            Some(Message::Run(Command::FixPlacement { .. }))
        ));
    }

    #[test]
    fn the_right_button_cancels_a_placement_and_means_nothing_otherwise() {
        let (plan, _shell) = plan_with_channels();
        let (cache, localizer) = (Cache::new(), Localizer::new(la_i18n::Language::En));
        let mut view = view_of(&plan, &cache, &localizer);
        let mut state = ViewState::default();
        let at = Point::new(GUTTER + 100.0, 60.0);
        assert!(send_event(&view, &mut state, button_down(mouse::Button::Right), at).is_none());
        view.placement = placing(Kind::Pair);
        let (message, _) =
            send_event(&view, &mut state, button_down(mouse::Button::Right), at).unwrap();
        assert!(matches!(
            message,
            Some(Message::Run(Command::CancelPlacement))
        ));
    }

    #[test]
    fn the_pointer_is_reported_to_the_application_while_placing() {
        let (plan, _shell) = plan_with_channels();
        let (cache, localizer) = (Cache::new(), Localizer::new(la_i18n::Language::En));
        let mut view = view_of(&plan, &cache, &localizer);
        view.placement = placing(Kind::Single);
        let mut state = ViewState::default();
        let at = Point::new(GUTTER + 300.0, 60.0);
        let (message, _) = send_event(&view, &mut state, pointer_moved(at), at).unwrap();
        let Some(Message::PlacementPointer(Some(pointer), _)) = message else {
            panic!("{message:?}");
        };
        assert!((pointer.x - 300.0).abs() < 0.01);
        assert_eq!(pointer.lane, Some(plan.channels[0].channel));
        // Over the legend there is no pointer for the signals; and leaving the view takes it away.
        let legend = Point::new(20.0, 60.0);
        let (message, _) = send_event(&view, &mut state, pointer_moved(legend), legend).unwrap();
        assert!(matches!(message, Some(Message::PlacementPointer(None, _))));
        let (message, _) = send_event(
            &view,
            &mut state,
            canvas::Event::Mouse(mouse::Event::CursorLeft),
            at,
        )
        .unwrap();
        assert!(matches!(message, Some(Message::PlacementPointer(None, _))));
        // Without a placement a move only asks for a redraw (the hover line).
        view.placement = None;
        let (message, redraw) = send_event(&view, &mut state, pointer_moved(at), at).unwrap();
        assert!(message.is_none());
        assert!(matches!(redraw, iced::window::RedrawRequest::NextFrame));
    }

    fn with_markers(plan: &FramePlan) -> FramePlan {
        let mut plan = plan.clone();
        plan.markers = vec![
            MarkerPlan {
                id: 1,
                name: "T1".into(),
                number: 1,
                colour: 0,
                comment: String::new(),
                selected: false,
                shape: PlanShape::Point { x: 200.0 },
            },
            MarkerPlan {
                id: 2,
                name: "P1".into(),
                number: 1,
                colour: 1,
                comment: String::new(),
                selected: false,
                shape: PlanShape::Interval {
                    start_x: 400.0,
                    end_x: 600.0,
                },
            },
        ];
        plan
    }

    #[test]
    fn a_press_on_a_marker_selects_it_and_a_drag_moves_that_end() {
        let (plan, _shell) = plan_with_channels();
        let plan = with_markers(&plan);
        let (cache, localizer) = (Cache::new(), Localizer::new(la_i18n::Language::En));
        let view = view_of(&plan, &cache, &localizer);
        let mut state = ViewState::default();
        // On the line of the single marker, a little below the ruler.
        let on_line = Point::new(GUTTER + 202.0, 60.0);
        let (message, _) =
            send_event(&view, &mut state, button_down(mouse::Button::Left), on_line).unwrap();
        assert!(matches!(
            message,
            Some(Message::Run(Command::SelectMarker(Some(1))))
        ));
        assert_eq!(state.drag_marker, Some((1, End::Point)));
        // Moving drags that end; the host decides where it lands.
        let to = Point::new(GUTTER + 250.0, 60.0);
        let (message, _) = send_event(&view, &mut state, pointer_moved(to), to).unwrap();
        let Some(Message::Run(Command::MoveMarkerEnd { id, end, pointer })) = message else {
            panic!("{message:?}");
        };
        assert_eq!((id, end), (1, End::Point));
        assert!((pointer.x - 250.0).abs() < 0.01);
        // The release ends the drag.
        let (message, _) = send_event(
            &view,
            &mut state,
            canvas::Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)),
            to,
        )
        .unwrap();
        assert!(matches!(
            message,
            Some(Message::Run(Command::EndMarkerDrag))
        ));
        assert_eq!(state.drag_marker, None);
    }

    #[test]
    fn the_squares_of_a_pair_are_the_marks_of_its_start_and_its_end() {
        let (plan, _shell) = plan_with_channels();
        let plan = with_markers(&plan);
        let (cache, localizer) = (Cache::new(), Localizer::new(la_i18n::Language::En));
        let view = view_of(&plan, &cache, &localizer);
        let mut state = ViewState::default();
        // The square of the start is left of its line, in the ruler; that of the end, right of it.
        let start_square = Point::new(GUTTER + 400.0 - 6.0, AXIS_HEIGHT - 6.0);
        send_event(
            &view,
            &mut state,
            button_down(mouse::Button::Left),
            start_square,
        )
        .unwrap();
        assert_eq!(state.drag_marker, Some((2, End::Start)));
        let end_square = Point::new(GUTTER + 600.0 + 6.0, AXIS_HEIGHT - 6.0);
        send_event(
            &view,
            &mut state,
            button_down(mouse::Button::Left),
            end_square,
        )
        .unwrap();
        assert_eq!(state.drag_marker, Some((2, End::End)));
        // The wrong side of a line is not a mark.
        let wrong = Point::new(GUTTER + 400.0 + 8.0, AXIS_HEIGHT - 6.0);
        send_event(&view, &mut state, button_down(mouse::Button::Left), wrong).unwrap();
        assert_eq!(state.drag_marker, None);
    }

    #[test]
    fn after_a_pair_swapped_its_ends_the_drag_goes_on_with_the_role_the_host_gave() {
        let (plan, _shell) = plan_with_channels();
        let plan = with_markers(&plan);
        let (cache, localizer) = (Cache::new(), Localizer::new(la_i18n::Language::En));
        let mut view = view_of(&plan, &cache, &localizer);
        let mut state = ViewState::default();
        let end_line = Point::new(GUTTER + 600.0, 60.0);
        send_event(
            &view,
            &mut state,
            button_down(mouse::Button::Left),
            end_line,
        )
        .unwrap();
        assert_eq!(state.drag_marker, Some((2, End::End)));
        view.drag_end = Some((2, End::Start));
        let to = Point::new(GUTTER + 100.0, 60.0);
        let (message, _) = send_event(&view, &mut state, pointer_moved(to), to).unwrap();
        let Some(Message::Run(Command::MoveMarkerEnd { id, end, .. })) = message else {
            panic!("{message:?}");
        };
        assert_eq!((id, end), (2, End::Start));
    }

    #[test]
    fn a_press_away_from_the_markers_pans_the_view_as_before() {
        let (plan, _shell) = plan_with_channels();
        let plan = with_markers(&plan);
        let (cache, localizer) = (Cache::new(), Localizer::new(la_i18n::Language::En));
        let view = view_of(&plan, &cache, &localizer);
        let mut state = ViewState::default();
        let away = Point::new(GUTTER + 700.0, 60.0);
        let message = send_event(&view, &mut state, button_down(mouse::Button::Left), away)
            .and_then(|(message, _)| message);
        assert!(message.is_none());
        assert_eq!(state.drag_marker, None);
        assert!(state.drag_from.is_some());
    }
}
