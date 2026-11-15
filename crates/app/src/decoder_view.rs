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
        look.variant.annotation(style.colour)
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

/// The fewest characters of a text worth drawing in a span: under this, the span is a plain bar.
pub const MIN_LABEL_CHARS: usize = 3;
/// The width of a character of the label, in logical pixels, and the room kept at the left.
const CHAR_WIDTH: f32 = 6.5;
const LABEL_PADDING: f32 = 6.0;

/// The text drawn in a span `width` pixels wide: the whole text if it fits, else cut with an
/// ellipsis, and nothing when fewer than [`MIN_LABEL_CHARS`] characters would show.
#[must_use]
pub fn span_label(text: &str, width: f32) -> Option<String> {
    let room = ((width - LABEL_PADDING) / CHAR_WIDTH).floor();
    if room < MIN_LABEL_CHARS as f32 {
        return None;
    }
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let room = room as usize;
    Some(fit(text, room))
}

/// The left edge of a cell in pixels.
fn left_of(cell: &Cell, scale: f32) -> f32 {
    match cell {
        Cell::Span(span) => GUTTER + span.x0 as f32 * scale,
        Cell::Mark(mark) => GUTTER + mark.x as f32 * scale,
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

/// How a span is placed among its neighbours.
#[derive(Clone, Copy)]
struct Placement {
    /// Whether it is the selected annotation.
    selected: bool,
    /// The left edge of the cell after it, which it must not reach into.
    right_limit: Option<f32>,
}

fn draw_span(
    frame: &mut Frame,
    cell: &SpanCell,
    top: f32,
    height: f32,
    scale: f32,
    placement: Placement,
    look: &Look<'_>,
) {
    let Placement {
        selected,
        right_limit,
    } = placement;
    let x0 = GUTTER + cell.x0 as f32 * scale;
    // A span never reaches into the cell after it in its row: the later cell wins, this one is
    // shortened.
    let x1 = GUTTER + cell.x1 as f32 * scale;
    let x1 = right_limit.map_or(x1, |limit| x1.min(limit.max(x0)));
    let width = (x1 - x0).max(1.0);
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
    if let Some(content) = span_label(&annotation_text(look.localizer, &cell.text), width) {
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
            for (at, cell) in row.iter().enumerate() {
                let right_limit = row
                    .get(at.saturating_add(1))
                    .map(|next| left_of(next, scale));
                match cell {
                    Cell::Span(span) => draw_span(
                        frame,
                        span,
                        row_top + 1.0,
                        row_height - 2.0,
                        scale,
                        Placement {
                            selected: is_selected(
                                look,
                                decoder.instance,
                                Target::Lane,
                                span.id,
                                span.start,
                            ),
                            right_limit,
                        },
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
                    Placement {
                        selected: is_selected(look, overlay.instance, target, span.id, span.start),
                        right_limit: None,
                    },
                    look,
                ),
            }
        }
        for cell in &overlay.plan.dense {
            draw_dense(frame, cell, top + 1.0, scale, look);
        }
    }
}

/// An annotation found under the pointer.
#[derive(Debug, Clone, PartialEq)]
pub struct Hit {
    /// The instance it belongs to.
    pub instance: u32,
    /// How to find it again.
    pub annotation: AnnotationRef,
    /// Its text.
    pub text: la_decoders::model::Text,
    /// Its first position.
    pub start: u64,
    /// Its end position.
    pub end: u64,
    /// Whether it reports a problem in the signal.
    pub error: bool,
}

/// What a plan shows at a point: a mark within [`PICK`] pixels, or the span the point is in.
fn pick_in(
    plan: &Plan,
    x: f32,
    scale: f32,
    rows: Option<(f32, f32)>,
) -> Option<(u64, u64, u64, la_decoders::model::Text, bool)> {
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
                    return Some((m.id, m.start, m.end, m.text.clone(), m.error));
                }
                Cell::Span(s) if column >= s.x0 as f32 && column <= s.x1 as f32 => {
                    return Some((s.id, s.start, s.end, s.text.clone(), s.error));
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
) -> Option<Hit> {
    if position.x < GUTTER {
        return None;
    }
    for (index, decoder) in plan.lanes.iter().enumerate() {
        let top = lane_top(first_lane.saturating_add(index));
        if position.y >= top && position.y < top + lane {
            let rows = decoder.plan.rows.len().max(1);
            let row_height = ((lane - STRIP - 6.0) / rows as f32).min(24.0);
            let y = position.y - top - 3.0;
            if let Some((id, start, end, text, error)) =
                pick_in(&decoder.plan, position.x, scale, Some((y, row_height)))
            {
                return Some(Hit {
                    instance: decoder.instance,
                    annotation: AnnotationRef {
                        target: Target::Lane,
                        id,
                        start,
                    },
                    text,
                    start,
                    end,
                    error,
                });
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
        if let Some((id, start, end, text, error)) = pick_in(&overlay.plan, position.x, scale, None)
        {
            return Some(Hit {
                instance: overlay.instance,
                annotation: AnnotationRef {
                    target: Target::Channel(overlay.channel),
                    id,
                    start,
                },
                text,
                start,
                end,
                error,
            });
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_text_that_fits_is_drawn_whole() {
        // 6 pixels of padding and 6.5 per character: 11 characters need 77.5 pixels.
        assert_eq!(span_label("Byte 0x5A", 100.0).as_deref(), Some("Byte 0x5A"));
        assert_eq!(span_label("abc", 26.0).as_deref(), Some("abc"));
    }

    #[test]
    fn a_text_that_does_not_fit_is_cut_with_an_ellipsis() {
        let label = span_label("Incomplete byte: 4 bits", 60.0).unwrap();
        assert!(label.ends_with('\u{2026}'), "{label}");
        assert!(label.chars().count() <= 8, "{label}");
        assert!("Incomplete byte: 4 bits".starts_with(label.trim_end_matches('\u{2026}')));
    }

    #[test]
    fn under_three_characters_nothing_is_drawn_and_the_span_is_a_bar() {
        assert_eq!(span_label("Byte", 20.0), None);
        assert_eq!(span_label("Byte", 6.0 + 6.5 * 2.9), None);
        assert!(span_label("Byte", 6.0 + 6.5 * 3.0).is_some());
        assert_eq!(span_label("Byte", 0.0), None);
    }

    #[test]
    fn a_cut_label_never_has_more_characters_than_there_is_room_for() {
        for width in [26.0_f32, 40.0, 61.0, 130.0, 400.0] {
            let room = ((width - 6.0) / 6.5).floor() as usize;
            if let Some(label) = span_label("A text that is quite a bit longer than most", width) {
                assert!(label.chars().count() <= room, "{width}: {label}");
            }
        }
    }

    #[test]
    fn a_wide_script_is_cut_by_characters_not_bytes() {
        let label = span_label(
            "\u{30d0}\u{30a4}\u{30c8}\u{30c7}\u{30fc}\u{30bf}\u{3067}\u{3059}",
            40.0,
        )
        .unwrap();
        assert!(label.chars().count() <= 5);
        assert!(label.ends_with('\u{2026}'));
    }
}
