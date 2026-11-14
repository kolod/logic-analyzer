//! The signal view: draws a [`FramePlan`] on a canvas.
//!
//! The expensive part (the waveforms, the axis and the labels) is drawn into a cached layer that
//! is rebuilt only when the plan, the theme or the language change; the hover readout is drawn
//! into a cheap layer on top that never invalidates the cache.

use iced::alignment::Vertical;
use iced::mouse;
use iced::widget::canvas::{self, Cache, Frame, Geometry, LineDash, Path, Stroke, Text};
use iced::{Color, Pixels, Point, Rectangle, Renderer, Size, Theme};
use la_host::plan::{FramePlan, FrameState, Segment};
use la_i18n::{Args, Key, Localizer};

use crate::app::Message;
use crate::theme::Variant;

/// Width of the column with the channel names, in logical pixels.
pub const GUTTER: f32 = 112.0;
/// Height of the time axis, in logical pixels.
pub const AXIS_HEIGHT: f32 = 28.0;
const LANE_MIN: f32 = 30.0;
const LANE_MAX: f32 = 72.0;
const TEXT_SIZE: f32 = 13.0;

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
}

/// What the canvas remembers between events.
#[derive(Debug, Default)]
pub struct ViewState {
    /// The last width reported to the application.
    reported_width: f32,
}

fn lane_height(total: f32, lanes: usize) -> f32 {
    if lanes == 0 {
        return LANE_MAX;
    }
    ((total - AXIS_HEIGHT) / lanes as f32).clamp(LANE_MIN, LANE_MAX)
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

    fn draw_plan(&self, frame: &mut Frame, size: Size, theme: &Theme) {
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
        let lane = lane_height(size.height, plan.channels.len());
        let hairline = self.variant.hairline();

        // Time axis.
        for tick in &plan.axis.ticks {
            let x = GUTTER + tick.x as f32 * (area_width / plan.pixels.max(1) as f32);
            let depth = if tick.major { 10.0 } else { 6.0 };
            frame.stroke(
                &Path::line(
                    Point::new(x, AXIS_HEIGHT - depth),
                    Point::new(x, AXIS_HEIGHT),
                ),
                Stroke::default().with_color(text_colour).with_width(1.0),
            );
            frame.stroke(
                &Path::line(Point::new(x, AXIS_HEIGHT), Point::new(x, size.height)),
                Stroke::default().with_color(hairline).with_width(1.0),
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

        // Lanes.
        for (index, channel) in plan.channels.iter().enumerate() {
            let top = AXIS_HEIGHT + index as f32 * lane;
            let colour = self.variant.channel(channel.colour);
            frame.fill_text(Text {
                content: fit(&self.channel_label(index), 14),
                position: Point::new(8.0, top + lane / 2.0),
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
        if plan.state == FrameState::Empty {
            frame.fill_text(Text {
                content: message(self.localizer, "view-empty"),
                position: Point::new(GUTTER + area_width / 2.0, AXIS_HEIGHT + lane / 2.0),
                color: text_colour,
                size: Pixels(TEXT_SIZE + 1.0),
                align_x: iced::alignment::Horizontal::Center.into(),
                align_y: Vertical::Center,
                ..Text::default()
            });
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

    fn update(
        &self,
        state: &mut ViewState,
        _event: &canvas::Event,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Option<canvas::Action<Message>> {
        let width = (bounds.width - GUTTER).max(1.0);
        if (width - state.reported_width).abs() >= 1.0 {
            state.reported_width = width;
            return Some(canvas::Action::publish(Message::AreaResized(width)));
        }
        None
    }

    fn draw(
        &self,
        _state: &ViewState,
        renderer: &Renderer,
        theme: &Theme,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Vec<Geometry> {
        let size = bounds.size();
        let waveforms = self
            .cache
            .draw(renderer, size, |frame| self.draw_plan(frame, size, theme));
        let mut overlay = Frame::new(renderer, size);
        if let Some(position) = cursor.position_in(bounds) {
            self.draw_hover(&mut overlay, size, position, theme);
        }
        vec![waveforms, overlay.into_geometry()]
    }

    fn mouse_interaction(
        &self,
        _state: &ViewState,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> mouse::Interaction {
        if cursor.is_over(bounds) {
            mouse::Interaction::Crosshair
        } else {
            mouse::Interaction::default()
        }
    }
}

impl SignalView<'_> {
    /// The time under the pointer, with a vertical hairline.
    fn draw_hover(&self, frame: &mut Frame, size: Size, position: Point, theme: &Theme) {
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
    }
}
