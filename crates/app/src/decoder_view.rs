//! Drawing the lanes and overlays of decoder instances, and finding the annotation under the
//! pointer.
//!
//! Everything here follows the frame plan: a lane has up to three rows of spans and marks and a
//! thin strip of dense cells; an overlay sits on the lane of its channel. The shapes and colours come
//! from the annotation style (a palette index and a shape); an error is drawn in the danger colour
//! with a heavier outline, so it never looks like a normal annotation.

use iced::alignment::Vertical;
use iced::widget::canvas::{Frame, Path, Stroke, Text};
use iced::{Color, Pixels, Point, Size, Theme};
use la_decoders::lanes::{Cell, DenseCell, MarkCell, Plan, SpanCell};
use la_decoders::model::{Shape, Style, Target};
use la_host::decoders::{AnnotationRef, annotation_text};
use la_host::plan::FramePlan;
use la_host::presentation::Colour;
use la_i18n::Localizer;

use crate::signal_view::GUTTER;
use crate::theme::Variant;

const TEXT_SIZE: f32 = 12.0;
/// How near (in pixels) the pointer must be to a mark to pick it.
const PICK: f32 = 5.0;
/// Height of the strip of dense cells at the bottom of a lane.
const STRIP: f32 = 5.0;
/// Height of the area where an overlay draws, at the top of a channel lane.
pub const OVERLAY_BAND: f32 = 14.0;

/// What the drawing needs besides the plan.
pub struct Look<'a> {
    /// The colour variant.
    pub variant: Variant,
    /// The language of the annotation texts.
    pub localizer: &'a Localizer,
    /// The selected annotation, if any.
    pub selected: Option<(u32, AnnotationRef)>,
    /// The theme (for the danger colour).
    pub theme: &'a Theme,
}

fn colour_of(style: Style, error: bool, look: &Look<'_>) -> Color {
    if error {
        look.theme.palette().danger
    } else {
        look.variant.channel(Colour::Palette(style.colour))
    }
}

/// A small shape centred on `(x, y)`.
fn draw_shape(frame: &mut Frame, shape: Shape, edge: Option<bool>, x: f32, y: f32, colour: Color) {
    let r = 4.0;
    let stroke = Stroke::default().with_color(colour).with_width(1.6);
    if let Some(rising) = edge {
        let (tip, base) = if rising {
            (y - r, y + r)
        } else {
            (y + r, y - r)
        };
        let path = Path::new(|b| {
            b.move_to(Point::new(x, tip));
            b.line_to(Point::new(x - r, base));
            b.line_to(Point::new(x + r, base));
            b.close();
        });
        frame.fill(&path, colour);
        return;
    }
    match shape {
        Shape::Dot => frame.fill(&Path::circle(Point::new(x, y), r - 0.5), colour),
        Shape::Arrow => {
            let path = Path::new(|b| {
                b.move_to(Point::new(x, y + r));
                b.line_to(Point::new(x - r, y - r));
                b.line_to(Point::new(x + r, y - r));
                b.close();
            });
            frame.fill(&path, colour);
        }
        Shape::Bar => frame.stroke(
            &Path::line(Point::new(x, y - r), Point::new(x, y + r)),
            Stroke::default().with_color(colour).with_width(2.5),
        ),
        Shape::Flag => {
            frame.stroke(
                &Path::line(Point::new(x, y - r), Point::new(x, y + r)),
                stroke,
            );
            frame.fill_rectangle(Point::new(x, y - r), Size::new(r, r), colour);
        }
        Shape::Cross => {
            frame.stroke(
                &Path::line(Point::new(x - r, y - r), Point::new(x + r, y + r)),
                stroke,
            );
            frame.stroke(
                &Path::line(Point::new(x - r, y + r), Point::new(x + r, y - r)),
                stroke,
            );
        }
    }
}

fn fit(text: &str, max_chars: usize) -> String {
    if text.chars().count() <= max_chars {
        return text.to_owned();
    }
    let kept: String = text.chars().take(max_chars.saturating_sub(1)).collect();
    format!("{kept}\u{2026}")
}

fn is_selected(look: &Look<'_>, instance: u32, target: Target, id: u64, start: u64) -> bool {
    look.selected
        .is_some_and(|(i, r)| i == instance && r.target == target && r.id == id && r.start == start)
}

fn draw_span(
    frame: &mut Frame,
    cell: &SpanCell,
    top: f32,
    height: f32,
    scale: f32,
    selected: bool,
    look: &Look<'_>,
) {
    let x0 = GUTTER + cell.x0 as f32 * scale;
    let width = ((cell.x1 - cell.x0) as f32 * scale).max(1.0);
    let colour = colour_of(cell.style, cell.error, look);
    frame.fill_rectangle(
        Point::new(x0, top),
        Size::new(width, height),
        Color { a: 0.3, ..colour },
    );
    let outline = Path::rectangle(Point::new(x0, top), Size::new(width, height));
    frame.stroke(
        &outline,
        Stroke::default()
            .with_color(colour)
            .with_width(if cell.error { 2.0 } else { 1.0 }),
    );
    if selected {
        frame.stroke(
            &outline,
            Stroke::default()
                .with_color(look.theme.palette().text)
                .with_width(2.0),
        );
    }
    let room = ((width - 6.0) / 6.5).floor();
    if room >= 2.0 {
        let content = fit(&annotation_text(look.localizer, &cell.text), room as usize);
        frame.fill_text(Text {
            content,
            position: Point::new(x0 + 3.0, top + height / 2.0),
            color: look.theme.palette().text,
            size: Pixels(TEXT_SIZE),
            align_y: Vertical::Center,
            ..Text::default()
        });
    }
}

fn draw_mark(
    frame: &mut Frame,
    cell: &MarkCell,
    y: f32,
    scale: f32,
    selected: bool,
    look: &Look<'_>,
) {
    let x = GUTTER + cell.x as f32 * scale;
    let colour = colour_of(cell.style, cell.error, look);
    if selected {
        frame.stroke(
            &Path::circle(Point::new(x, y), 7.0),
            Stroke::default()
                .with_color(look.theme.palette().text)
                .with_width(1.5),
        );
    }
    draw_shape(frame, cell.style.shape, cell.edge, x, y, colour);
}

fn draw_dense(frame: &mut Frame, cell: &DenseCell, y: f32, scale: f32, look: &Look<'_>) {
    let x = GUTTER + cell.column as f32 * scale;
    let width = scale.max(2.0);
    let colour = if cell.errors > 0 {
        look.theme.palette().danger
    } else {
        look.theme.palette().text
    };
    frame.fill_rectangle(Point::new(x, y), Size::new(width, STRIP), colour);
}

/// Draws the lane plans, one lane after another starting at `first_lane`.
pub fn draw_lanes(
    frame: &mut Frame,
    plan: &FramePlan,
    first_lane: usize,
    lane_top: &dyn Fn(usize) -> f32,
    lane: f32,
    scale: f32,
    look: &Look<'_>,
) {
    for (index, decoder) in plan.lanes.iter().enumerate() {
        let top = lane_top(first_lane.saturating_add(index));
        let label_colour = look.theme.palette().text;
        frame.fill_text(Text {
            content: fit(&decoder.name, 14),
            position: Point::new(8.0, top + lane / 2.0),
            color: label_colour,
            size: Pixels(TEXT_SIZE + 1.0),
            align_y: Vertical::Center,
            ..Text::default()
        });
        let rows = decoder.plan.rows.len().max(1);
        let usable = lane - STRIP - 6.0;
        let row_height = (usable / rows as f32).min(24.0);
        for (r, row) in decoder.plan.rows.iter().enumerate() {
            let row_top = top + 3.0 + r as f32 * row_height;
            for cell in row {
                match cell {
                    Cell::Span(span) => draw_span(
                        frame,
                        span,
                        row_top + 1.0,
                        row_height - 2.0,
                        scale,
                        is_selected(look, decoder.instance, Target::Lane, span.id, span.start),
                        look,
                    ),
                    Cell::Mark(mark) => draw_mark(
                        frame,
                        mark,
                        row_top + row_height / 2.0,
                        scale,
                        is_selected(look, decoder.instance, Target::Lane, mark.id, mark.start),
                        look,
                    ),
                }
            }
        }
        for cell in &decoder.plan.dense {
            draw_dense(frame, cell, top + lane - STRIP - 2.0, scale, look);
        }
        if decoder.incomplete {
            // Results that are not whole are marked along the bottom of the lane.
            let dashes = [4.0_f32, 3.0];
            frame.stroke(
                &Path::line(
                    Point::new(GUTTER, top + lane - 1.0),
                    Point::new(GUTTER + plan.pixels as f32 * scale, top + lane - 1.0),
                ),
                Stroke {
                    line_dash: iced::widget::canvas::LineDash {
                        segments: &dashes,
                        offset: 0,
                    },
                    ..Stroke::default()
                        .with_color(look.theme.palette().danger)
                        .with_width(1.5)
                },
            );
        }
    }
}

/// Draws the overlays on the lanes of their channels.
pub fn draw_overlays(
    frame: &mut Frame,
    plan: &FramePlan,
    lane_top: &dyn Fn(usize) -> f32,
    scale: f32,
    look: &Look<'_>,
) {
    for overlay in &plan.overlays {
        let Some(index) = plan
            .channels
            .iter()
            .position(|c| c.channel == overlay.channel)
        else {
            continue;
        };
        let top = lane_top(index);
        for cell in overlay.plan.rows.iter().flatten() {
            let target = Target::Channel(overlay.channel);
            match cell {
                Cell::Mark(mark) => draw_mark(
                    frame,
                    mark,
                    top + OVERLAY_BAND / 2.0 + 1.0,
                    scale,
                    is_selected(look, overlay.instance, target, mark.id, mark.start),
                    look,
                ),
                Cell::Span(span) => draw_span(
                    frame,
                    span,
                    top + 1.0,
                    OVERLAY_BAND - 2.0,
                    scale,
                    is_selected(look, overlay.instance, target, span.id, span.start),
                    look,
                ),
            }
        }
        for cell in &overlay.plan.dense {
            draw_dense(frame, cell, top + 1.0, scale, look);
        }
    }
}

/// What a plan shows at a point: a mark within [`PICK`] pixels, or the span the point is in.
fn pick_in(plan: &Plan, x: f32, scale: f32, rows: Option<(f32, f32)>) -> Option<(u64, u64)> {
    let column = (x - GUTTER) / scale;
    for (r, row) in plan.rows.iter().enumerate() {
        if let Some((y, row_height)) = rows {
            let low = r as f32 * row_height;
            if y < low || y >= low + row_height {
                continue;
            }
        }
        for cell in row {
            match cell {
                Cell::Mark(m) if (m.x as f32 - column).abs() * scale <= PICK => {
                    return Some((m.id, m.start));
                }
                Cell::Span(s) if column >= s.x0 as f32 && column <= s.x1 as f32 => {
                    return Some((s.id, s.start));
                }
                _ => {}
            }
        }
    }
    None
}

/// The annotation under `position`, with the instance it belongs to.
#[must_use]
pub fn hit_test(
    plan: &FramePlan,
    position: Point,
    first_lane: usize,
    lane_top: &dyn Fn(usize) -> f32,
    lane: f32,
    scale: f32,
) -> Option<(u32, AnnotationRef)> {
    if position.x < GUTTER {
        return None;
    }
    for (index, decoder) in plan.lanes.iter().enumerate() {
        let top = lane_top(first_lane.saturating_add(index));
        if position.y >= top && position.y < top + lane {
            let rows = decoder.plan.rows.len().max(1);
            let row_height = ((lane - STRIP - 6.0) / rows as f32).min(24.0);
            let y = position.y - top - 3.0;
            if let Some((id, start)) =
                pick_in(&decoder.plan, position.x, scale, Some((y, row_height)))
            {
                return Some((
                    decoder.instance,
                    AnnotationRef {
                        target: Target::Lane,
                        id,
                        start,
                    },
                ));
            }
        }
    }
    for overlay in &plan.overlays {
        let Some(index) = plan
            .channels
            .iter()
            .position(|c| c.channel == overlay.channel)
        else {
            continue;
        };
        let top = lane_top(index);
        if position.y < top || position.y >= top + OVERLAY_BAND + 4.0 {
            continue;
        }
        if let Some((id, start)) = pick_in(&overlay.plan, position.x, scale, None) {
            return Some((
                overlay.instance,
                AnnotationRef {
                    target: Target::Channel(overlay.channel),
                    id,
                    start,
                },
            ));
        }
    }
    None
}
