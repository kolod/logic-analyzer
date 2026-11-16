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
use la_host::plan::{FramePlan, FrameState, Segment};
use la_host::presentation::{Colour, PALETTE_SIZE};
use la_host::{Command, EdgeDirection, ZoomAnchor, ZoomFactor};
use la_i18n::{Args, Key, Localizer};

use crate::app::Message;
use crate::decoder_view::{self, Look};
use crate::lane_layout::{
    self, AUTO_MAX, BAR_WIDTH, Hit as LegendHit, LaneKind, LaneSpec, Layout, Zone,
};
pub use crate::lane_layout::{AXIS_HEIGHT, GUTTER};
use crate::theme::Variant;

const TEXT_SIZE: f32 = 13.0;
/// How far the pointer must move after a press on a name before it is a drag.
const DRAG_THRESHOLD: f32 = 4.0;
/// Two presses on a border this close in time are a double click.
const DOUBLE_CLICK: Duration = Duration::from_millis(400);
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

/// What the canvas remembers between events.
#[derive(Debug, Default)]
pub struct ViewState {
    /// The last width reported to the application.
    reported_width: f32,
    /// The modifier keys currently held.
    modifiers: iced::keyboard::Modifiers,
    /// The x position where a drag last was, while the left button is down.
    drag_from: Option<f32>,
    /// The marker being dragged and the channel under the pointer.
    drag_marker: Option<u32>,
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

    /// The sample under a pointer position (clamped to the window) and the channel of the lane
    /// it is in (the first lane when the pointer is outside the lanes).
    fn sample_and_channel(&self, position: Point, layout: &Layout) -> Option<(u64, u8)> {
        let plan = self.plan;
        let columns = plan.pixels.max(1) as f64;
        let x = f64::from((position.x - GUTTER).max(0.0)).min(columns);
        let sample = plan
            .window
            .start
            .saturating_add((x / columns * plan.window.length as f64) as u64);
        let index = (0..plan.channels.len())
            .find(|i| {
                layout
                    .lane(*i)
                    .is_some_and(|l| position.y >= l.top && position.y < l.bottom())
            })
            .unwrap_or(0);
        let channel = plan.channels.get(index).map_or(0, |c| c.channel);
        Some((sample, channel))
    }

    fn draw_plan(&self, frame: &mut Frame, size: Size, theme: &Theme, scroll: f32) {
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
        let lane_top = |index: usize| layout.lane(index).map_or(AXIS_HEIGHT, |l| l.top);
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
            frame.fill_text(Text {
                content: fit(&self.channel_label(index), 13),
                position: Point::new(BAR_WIDTH + 6.0, top + lane / 2.0),
                color: colour,
                size: Pixels(TEXT_SIZE),
                align_y: Vertical::Center,
                ..Text::default()
            });
            frame.stroke(
                &Path::line(
                    Point::new(0.0, top + lane),
                    Point::new(size.width, top + lane),
                ),
                Stroke::default().with_color(hairline).with_width(1.0),
            );
            self.draw_bands(frame, &channel.segments, top, lane, scale, colour);
            frame.stroke(
                &waveform(&channel.segments, scale, top + 7.0, top + lane - 7.0),
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
            frame.fill_text(Text {
                content: self.localizer.format_duration(tick.seconds),
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
        for marker in &plan.markers {
            let x = GUTTER + marker.x as f32 * scale;
            let accent = palette.primary;
            frame.stroke(
                &Path::line(Point::new(x, AXIS_HEIGHT - 6.0), Point::new(x, size.height)),
                Stroke::default().with_color(accent).with_width(1.5),
            );
            let label = if marker.name.is_empty() {
                format!("{}", marker.id)
            } else {
                marker.name.clone()
            };
            frame.fill_text(Text {
                content: label,
                position: Point::new(x + 4.0, AXIS_HEIGHT - 8.0),
                color: accent,
                size: Pixels(TEXT_SIZE),
                align_y: Vertical::Bottom,
                ..Text::default()
            });
        }
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
            let (y, height) = (top + 3.0, lane - 6.0);
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
            canvas::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Right)) => {
                let position = cursor.position_in(bounds)?;
                let layout = self.layout(bounds.height, state.scroll);
                let (sample, channel) = self.sample_and_channel(position, &layout)?;
                Some(
                    canvas::Action::publish(Message::Run(Command::PlaceMarker {
                        position: sample,
                        channel,
                    }))
                    .and_capture(),
                )
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
                state.drag_marker = self
                    .plan
                    .markers
                    .iter()
                    .find(|m| (GUTTER + m.x as f32 * scale - position.x).abs() <= 4.0)
                    .map(|m| m.id);
                state.drag_from = Some(position.x);
                Some(canvas::Action::capture())
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
                Some(canvas::Action::request_redraw())
            }
            canvas::Event::Mouse(mouse::Event::CursorLeft) => {
                Some(canvas::Action::request_redraw())
            }
            canvas::Event::Mouse(mouse::Event::CursorMoved { .. }) => {
                let from = state.drag_from?;
                let position = cursor.position_in(bounds)?;
                if let Some(id) = state.drag_marker {
                    let layout = self.layout(bounds.height, state.scroll);
                    let (sample, channel) = self.sample_and_channel(position, &layout)?;
                    return Some(
                        canvas::Action::publish(Message::Run(Command::MoveMarker {
                            id,
                            position: sample,
                            channel,
                        }))
                        .and_capture(),
                    );
                }
                let moved = (from - position.x) as i64;
                if moved == 0 {
                    return None;
                }
                state.drag_from = Some(position.x);
                Some(
                    canvas::Action::publish(Message::Run(Command::Pan { pixels: moved }))
                        .and_capture(),
                )
            }
            canvas::Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) => {
                state.drag_from = None;
                state.drag_marker = None;
                let press = state.press.take();
                let drag = state.drag.take();
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
                                let top = layout.lane(press.lane).map_or(AXIS_HEIGHT, |l| l.top);
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
        let size = bounds.size();
        let waveforms = self.cache.draw(renderer, size, |frame| {
            let started = Instant::now();
            self.draw_plan(frame, size, theme, state.scroll);
            crate::frame_log::record(crate::frame_log::Stage::Draw, started.elapsed());
        });
        let mut overlay = Frame::new(renderer, size);
        if let Some(position) = cursor.position_in(bounds) {
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
        vec![waveforms, overlay.into_geometry()]
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

    /// The lane that is being moved, lifted under the pointer, and the line where it will land.
    fn draw_lane_drag(&self, frame: &mut Frame, size: Size, theme: &Theme, state: &ViewState) {
        let Some(Drag::Lane { from, y }) = state.drag else {
            return;
        };
        let layout = self.layout(size.height, state.scroll);
        let palette = theme.palette();
        let line = layout.landing_line(layout.landing(y));
        frame.stroke(
            &Path::line(Point::new(0.0, line), Point::new(GUTTER, line)),
            Stroke::default()
                .with_color(palette.primary)
                .with_width(2.0),
        );
        let Some(lane) = layout.lane(from) else {
            return;
        };
        let top = y - lane.height / 2.0;
        frame.fill_rectangle(
            Point::new(0.0, top),
            Size::new(GUTTER, lane.height),
            Color {
                a: 0.55,
                ..palette.background
            },
        );
        let colour = self
            .plan
            .channels
            .get(from)
            .map_or(palette.text, |c| self.variant.channel(c.colour));
        frame.fill_rectangle(
            Point::new(0.0, top),
            Size::new(BAR_WIDTH, lane.height),
            colour,
        );
        frame.fill_text(Text {
            content: fit(&self.channel_label(from), 13),
            position: Point::new(BAR_WIDTH + 6.0, top + lane.height / 2.0),
            color: colour,
            size: Pixels(TEXT_SIZE),
            align_y: Vertical::Center,
            ..Text::default()
        });
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
        frame.fill_text(Text {
            content: self.localizer.format_duration(seconds),
            position: Point::new(size.width - 8.0, 4.0),
            color: colour,
            size: Pixels(TEXT_SIZE),
            align_x: iced::alignment::Horizontal::Right.into(),
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
        let y = (position.y + 14.0).min((size.height - 24.0).max(0.0));
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
        let dir = std::env::temp_dir().join(format!("la-legend-{}", std::process::id()));
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
        assert!((top - 100.0).abs() < f32::EPSILON);
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
}
