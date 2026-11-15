//! The signal view: draws a [`FramePlan`] on a canvas.
//!
//! The expensive part (the waveforms, the axis and the labels) is drawn into a cached layer that
//! is rebuilt only when the plan, the theme or the language change; the hover readout is drawn
//! into a cheap layer on top that never invalidates the cache.

use iced::alignment::Vertical;
use iced::mouse;
use iced::widget::canvas::{self, Cache, Frame, Geometry, LineDash, Path, Stroke, Text};
use iced::{Color, Pixels, Point, Rectangle, Renderer, Size, Theme};
use la_host::decoders::AnnotationRef;
use la_host::plan::{FramePlan, FrameState, Segment};
use la_host::{Command, ZoomAnchor, ZoomFactor};
use la_i18n::{Args, Key, Localizer};

use crate::app::Message;
use crate::decoder_view::{self, Look};
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
    /// The selected annotation, if any.
    pub selected: Option<(u32, AnnotationRef)>,
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
    last_click: Option<(std::time::Instant, u32, AnnotationRef)>,
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

    /// The sample under a pointer position (clamped to the window) and the channel of the lane
    /// it is in (the first lane when the pointer is outside the lanes).
    fn sample_and_channel(&self, position: Point, height: f32) -> Option<(u64, u8)> {
        let plan = self.plan;
        Some({
            let columns = plan.pixels.max(1) as f64;
            let x = f64::from((position.x - GUTTER).max(0.0)).min(columns);
            let sample = plan
                .window
                .start
                .saturating_add((x / columns * plan.window.length as f64) as u64);
            let lane = lane_height(height, plan.channels.len().saturating_add(plan.lanes.len()));
            let index = (((position.y - AXIS_HEIGHT) / lane).max(0.0) as usize)
                .min(plan.channels.len().saturating_sub(1));
            let channel = plan.channels.get(index).map_or(0, |c| c.channel);
            (sample, channel)
        })
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
        let lane = lane_height(
            size.height,
            plan.channels.len().saturating_add(plan.lanes.len()),
        );
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
        let lane_top = |index: usize| AXIS_HEIGHT + index as f32 * lane;
        let look = Look {
            variant: self.variant,
            localizer: self.localizer,
            selected: self.selected,
            theme,
        };
        // The lanes of the decoders come after the channels.
        for index in 0..plan.lanes.len() {
            let top = lane_top(plan.channels.len().saturating_add(index));
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
            lane,
            scale,
            &look,
        );
        decoder_view::draw_overlays(frame, plan, &lane_top, scale, &look);
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
                let (sample, channel) = self.sample_and_channel(position, bounds.height)?;
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
                if position.x >= GUTTER {
                    let scale = (bounds.width - GUTTER).max(1.0) / self.plan.pixels.max(1) as f32;
                    // A click on an annotation selects it.
                    let lane = lane_height(
                        bounds.height,
                        self.plan
                            .channels
                            .len()
                            .saturating_add(self.plan.lanes.len()),
                    );
                    let lane_top = |index: usize| AXIS_HEIGHT + index as f32 * lane;
                    if let Some(decoder_view::Hit {
                        instance,
                        annotation,
                        ..
                    }) = decoder_view::hit_test(
                        self.plan,
                        position,
                        self.plan.channels.len(),
                        &lane_top,
                        lane,
                        scale,
                    ) {
                        let now = std::time::Instant::now();
                        let double = state.last_click.is_some_and(|(at, i, a)| {
                            i == instance
                                && a == annotation
                                && now.saturating_duration_since(at)
                                    < std::time::Duration::from_millis(400)
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
                    return Some(canvas::Action::capture());
                }
                None
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
                    let (sample, channel) = self.sample_and_channel(position, bounds.height)?;
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
                None
            }
            _ => None,
        }
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
        let waveforms = self.cache.draw(renderer, size, |frame| {
            let started = std::time::Instant::now();
            self.draw_plan(frame, size, theme);
            crate::frame_log::record(crate::frame_log::Stage::Draw, started.elapsed());
        });
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
        self.draw_annotation_readout(frame, size, position, theme);
    }

    /// The full text of the annotation under the pointer and its length in time, in a box next to
    /// the pointer (a span's text may be cut in the lane).
    fn draw_annotation_readout(
        &self,
        frame: &mut Frame,
        size: Size,
        position: Point,
        theme: &Theme,
    ) {
        let plan = self.plan;
        let lane = lane_height(
            size.height,
            plan.channels.len().saturating_add(plan.lanes.len()),
        );
        let lane_top = |index: usize| AXIS_HEIGHT + index as f32 * lane;
        let scale = (size.width - GUTTER).max(1.0) / plan.pixels.max(1) as f32;
        let Some(hit) =
            decoder_view::hit_test(plan, position, plan.channels.len(), &lane_top, lane, scale)
        else {
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
}
