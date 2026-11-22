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
/// The margin above and below the rows of a decoder lane.
const LANE_MARGIN: f32 = 3.0;
/// The height of a row of a decoder lane that is as high as its rows need.
const ROW_HEIGHT: f32 = 24.0;
/// The height a row may take when the user made the lane higher.
const ROW_HEIGHT_MAX: f32 = 64.0;

/// The height of a decoder lane of `rows` rows when the user gave it none: just what its rows need,
/// with a small margin above and below.
#[must_use]
pub fn fitting_height(rows: usize) -> u16 {
    let height = 2.0 * LANE_MARGIN + rows.max(1) as f32 * ROW_HEIGHT;
    // The height is a few dozen pixels: far inside the range of `u16`.
    height as u16
}

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

/// The colour of an annotation: the danger colour for an error; the decoder's colour when it names
/// one; else the colour of the signal the mark is on (a mark in the lane has no signal and takes
/// the text colour).
fn colour_of(style: Style, error: bool, signal: Option<Color>, look: &Look<'_>) -> Color {
    if error {
        look.theme.palette().danger
    } else if let Some(index) = style.colour {
        look.variant.annotation(index)
    } else {
        signal.unwrap_or(look.theme.palette().text)
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
        // A block narrower than a column is a thin filled bar of its colour.
        Shape::Part | Shape::Whole => {
            frame.fill_rectangle(Point::new(x - 1.0, y - r), Size::new(2.0, 2.0 * r), colour);
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

/// The least room, in logical pixels, each mark needs on average to be told from its neighbours.
pub const MIN_MARK_SPACING: f32 = 8.0;

/// Whether the marks of a plan can be told apart at this zoom: no two of them share a column (no
/// dense cell) and they are not closer together, on average, than [`MIN_MARK_SPACING`]. When they
/// cannot, none of them is drawn (nor the dense cells that stand for them): a field of marks that
/// merge says nothing. Ranges wide enough to be spans are not marks and are always drawn; the plan
/// itself still holds every annotation, so zooming in shows them again.
#[must_use]
pub fn marks_legible(plan: &Plan, width: f32) -> bool {
    if !plan.dense.is_empty() {
        return false;
    }
    let marks = plan
        .rows
        .iter()
        .flatten()
        .filter(|cell| matches!(cell, Cell::Mark(_)))
        .count();
    marks as f32 * MIN_MARK_SPACING <= width
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
/// The room a block keeps free at each end of its text, in logical pixels.
const BLOCK_PADDING: f32 = 4.0;

/// The text of a block `width` pixels wide: the whole text if all of it fits inside with a margin,
/// else nothing (a block never shows a cut or an overflowing text).
#[must_use]
pub fn block_label(text: &str, width: f32) -> Option<String> {
    let needed = text.chars().count() as f32 * CHAR_WIDTH + 2.0 * BLOCK_PADDING;
    (!text.is_empty() && needed <= width).then(|| text.to_owned())
}

/// The relative luminance of a colour (WCAG 2).
fn luminance(colour: Color) -> f32 {
    let channel = |c: f32| {
        if c <= 0.039_28 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * channel(colour.r) + 0.7152 * channel(colour.g) + 0.0722 * channel(colour.b)
}

/// The contrast ratio of two colours (WCAG 2), from 1 to 21.
#[must_use]
pub fn contrast(a: Color, b: Color) -> f32 {
    let (la, lb) = (luminance(a), luminance(b));
    (la.max(lb) + 0.05) / (la.min(lb) + 0.05)
}

/// The colour of a text on a `fill`: black or white, whichever reads better.
#[must_use]
pub fn contrast_text(fill: Color) -> Color {
    let (black, white) = (Color::BLACK, Color::WHITE);
    if contrast(fill, black) >= contrast(fill, white) {
        black
    } else {
        white
    }
}

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
    /// The colour of the signal it is on, for a span that names no colour.
    signal: Option<Color>,
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
        signal,
        selected,
        right_limit,
    } = placement;
    let x0 = GUTTER + cell.x0 as f32 * scale;
    // A span never reaches into the cell after it in its row: the later cell wins, this one is
    // shortened.
    let x1 = GUTTER + cell.x1 as f32 * scale;
    let x1 = right_limit.map_or(x1, |limit| x1.min(limit.max(x0)));
    let width = (x1 - x0).max(1.0);
    let colour = colour_of(cell.style, cell.error, signal, look);
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

/// A block of a band (`Part`, `Whole`): a solid rectangle in the colour of its style, whatever the error
/// flag says (an error has a heavier outline), with its text if all of it fits.
fn draw_block(
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
        ..
    } = placement;
    let x0 = GUTTER + cell.x0 as f32 * scale;
    let x1 = GUTTER + cell.x1 as f32 * scale;
    let x1 = right_limit.map_or(x1, |limit| x1.min(limit.max(x0)));
    let width = (x1 - x0).max(1.0);
    let text_colour = look.theme.palette().text;
    let fill = cell
        .style
        .colour
        .map_or(text_colour, |index| look.variant.annotation(index));
    frame.fill_rectangle(Point::new(x0, top), Size::new(width, height), fill);
    let outline = Path::rectangle(Point::new(x0, top), Size::new(width, height));
    frame.stroke(
        &outline,
        Stroke::default()
            .with_color(Color {
                a: if cell.error { 1.0 } else { 0.35 },
                ..text_colour
            })
            .with_width(if cell.error { 2.0 } else { 1.0 }),
    );
    if selected {
        frame.stroke(
            &outline,
            Stroke::default().with_color(text_colour).with_width(2.0),
        );
    }
    if let Some(content) = block_label(&annotation_text(look.localizer, &cell.text), width) {
        frame.fill_text(Text {
            content,
            position: Point::new(x0 + width / 2.0, top + height / 2.0),
            color: contrast_text(fill),
            size: Pixels(TEXT_SIZE),
            align_x: iced::alignment::Horizontal::Center.into(),
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
    signal: Option<Color>,
    look: &Look<'_>,
) {
    let x = GUTTER + cell.x as f32 * scale;
    // The arrow of an edge is always in the colour of its signal (unless it reports an error).
    let colour = if cell.edge.is_some() && !cell.error {
        signal.unwrap_or_else(|| colour_of(cell.style, false, None, look))
    } else {
        colour_of(cell.style, cell.error, signal, look)
    };
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
    lane_height: &dyn Fn(usize) -> f32,
    scale: f32,
    look: &Look<'_>,
) {
    for (index, decoder) in plan.lanes.iter().enumerate() {
        let top = lane_top(first_lane.saturating_add(index));
        let lane = lane_height(first_lane.saturating_add(index));
        let label_colour = look.theme.palette().text;
        frame.fill_text(Text {
            content: fit(&decoder.name, 14),
            position: Point::new(8.0, top + lane / 2.0),
            color: label_colour,
            size: Pixels(TEXT_SIZE + 1.0),
            align_y: Vertical::Center,
            ..Text::default()
        });
        let legible = marks_legible(&decoder.plan, plan.pixels as f32 * scale);
        let rows = decoder.plan.rows.len().max(1);
        // The strip of dense cells takes room only where there are some to draw.
        let strip = if legible && !decoder.plan.dense.is_empty() {
            STRIP + 2.0
        } else {
            0.0
        };
        let usable = lane - 2.0 * LANE_MARGIN - strip;
        let row_height = (usable / rows as f32).min(ROW_HEIGHT_MAX);
        for (r, row) in decoder.plan.rows.iter().enumerate() {
            let row_top = top + LANE_MARGIN + r as f32 * row_height;
            for (at, cell) in row.iter().enumerate() {
                if !legible && matches!(cell, Cell::Mark(_)) {
                    continue;
                }
                let right_limit = row
                    .get(at.saturating_add(1))
                    .map(|next| left_of(next, scale));
                match cell {
                    Cell::Span(span) if span.style.shape.band().is_some() => draw_block(
                        frame,
                        span,
                        row_top + 1.0,
                        row_height - 2.0,
                        scale,
                        Placement {
                            signal: None,
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
                    Cell::Span(span) => draw_span(
                        frame,
                        span,
                        row_top + 1.0,
                        row_height - 2.0,
                        scale,
                        Placement {
                            signal: None,
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
                        None,
                        look,
                    ),
                }
            }
        }
        if legible {
            for cell in &decoder.plan.dense {
                draw_dense(frame, cell, top + lane - STRIP - 2.0, scale, look);
            }
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
    lane_height: &dyn Fn(usize) -> f32,
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
        // Everything an overlay draws is centred in the lane of its channel, on the signal's
        // line of travel, and takes the colour of that signal unless it names its own.
        let centre = lane_top(index) + lane_height(index) / 2.0;
        let signal = plan
            .channels
            .get(index)
            .map(|c| look.variant.channel(c.colour));
        let legible = marks_legible(&overlay.plan, plan.pixels as f32 * scale);
        for cell in overlay.plan.rows.iter().flatten() {
            if !legible && matches!(cell, Cell::Mark(_)) {
                continue;
            }
            let target = Target::Channel(overlay.channel);
            match cell {
                Cell::Mark(mark) => draw_mark(
                    frame,
                    mark,
                    centre,
                    scale,
                    is_selected(look, overlay.instance, target, mark.id, mark.start),
                    signal,
                    look,
                ),
                Cell::Span(span) => draw_span(
                    frame,
                    span,
                    centre - OVERLAY_BAND / 2.0 + 1.0,
                    OVERLAY_BAND - 2.0,
                    scale,
                    Placement {
                        signal,
                        selected: is_selected(look, overlay.instance, target, span.id, span.start),
                        right_limit: None,
                    },
                    look,
                ),
            }
        }
        if legible {
            for cell in &overlay.plan.dense {
                draw_dense(frame, cell, centre - STRIP / 2.0, scale, look);
            }
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
    marks: bool,
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
                Cell::Mark(m) if marks && (m.x as f32 - column).abs() * scale <= PICK => {
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
    lane_height: &dyn Fn(usize) -> f32,
    scale: f32,
) -> Option<Hit> {
    if position.x < GUTTER {
        return None;
    }
    for (index, decoder) in plan.lanes.iter().enumerate() {
        let top = lane_top(first_lane.saturating_add(index));
        let lane = lane_height(first_lane.saturating_add(index));
        if position.y >= top && position.y < top + lane {
            let rows = decoder.plan.rows.len().max(1);
            let row_height = ((lane - STRIP - 6.0) / rows as f32).min(24.0);
            let y = position.y - top - 3.0;
            if let Some((id, start, end, text, error)) = pick_in(
                &decoder.plan,
                position.x,
                scale,
                Some((y, row_height)),
                marks_legible(&decoder.plan, plan.pixels as f32 * scale),
            ) {
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
        let lane = lane_height(index);
        // The overlays are centred in the lane of their channel.
        if (position.y - (top + lane / 2.0)).abs() > OVERLAY_BAND / 2.0 + 3.0 {
            continue;
        }
        if let Some((id, start, end, text, error)) = pick_in(
            &overlay.plan,
            position.x,
            scale,
            None,
            marks_legible(&overlay.plan, plan.pixels as f32 * scale),
        ) {
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

    fn mark(x: f64) -> Cell {
        Cell::Mark(la_decoders::lanes::MarkCell {
            id: 0,
            start: 0,
            end: 0,
            text: la_decoders::model::Text::Literal(String::new()),
            x,
            style: Style {
                colour: None,
                shape: Shape::Dot,
            },
            edge: None,
            error: false,
        })
    }

    #[test]
    fn a_few_marks_with_room_between_them_are_legible() {
        let plan = Plan {
            rows: vec![(0..10).map(|i| mark(f64::from(i) * 50.0)).collect()],
            ..Plan::default()
        };
        assert!(marks_legible(&plan, 1_000.0));
    }

    #[test]
    fn too_many_marks_for_the_width_are_not_drawn_at_all() {
        let plan = Plan {
            rows: vec![(0..200).map(|i| mark(f64::from(i) * 5.0)).collect()],
            ..Plan::default()
        };
        // 200 marks need 1,600 pixels at 8 pixels each.
        assert!(!marks_legible(&plan, 1_000.0));
        assert!(marks_legible(&plan, 1_600.0));
    }

    #[test]
    fn a_dense_cell_means_the_marks_cannot_be_told_apart() {
        let plan = Plan {
            rows: vec![vec![mark(10.0)]],
            dense: vec![la_decoders::lanes::DenseCell {
                column: 30,
                count: 3,
                errors: 0,
            }],
            ..Plan::default()
        };
        assert!(!marks_legible(&plan, 1_000.0));
        assert!(marks_legible(&Plan::default(), 1_000.0));
    }

    #[test]
    fn a_block_shows_its_whole_text_or_none_never_a_cut_text() {
        // 4 pixels of margin at each end and 6.5 per character: "Stop" needs 34 pixels.
        assert_eq!(block_label("Stop", 34.0).as_deref(), Some("Stop"));
        assert_eq!(block_label("Stop", 33.9), None);
        assert_eq!(block_label("Repeated start", 60.0), None);
        assert_eq!(
            block_label("Repeated start", 100.0).as_deref(),
            Some("Repeated start")
        );
        assert_eq!(block_label("", 500.0), None);
        for width in 0..300 {
            if let Some(text) = block_label("Address 0x50 (write)", width as f32) {
                assert_eq!(text, "Address 0x50 (write)");
            }
        }
    }

    /// The colours the I2C blocks use (research R3): red 0, yellow 1, green 2, blue 4, grey 7.
    const ROLES: [u8; 5] = [0, 1, 2, 4, 7];

    #[test]
    fn the_text_on_a_block_has_contrast_on_every_variant() {
        for variant in [Variant::Dark, Variant::Light, Variant::HighContrast] {
            for index in ROLES {
                let fill = variant.annotation(index);
                let text = contrast_text(fill);
                let ratio = contrast(fill, text);
                assert!(ratio >= 4.5, "{variant:?} colour {index}: {ratio}");
            }
        }
    }

    #[test]
    fn the_colours_of_the_blocks_tell_apart_on_every_variant() {
        for variant in [Variant::Dark, Variant::Light, Variant::HighContrast] {
            for (n, a) in ROLES.iter().enumerate() {
                for b in ROLES.iter().skip(n + 1) {
                    let (ca, cb) = (variant.annotation(*a), variant.annotation(*b));
                    let distance =
                        ((ca.r - cb.r).powi(2) + (ca.g - cb.g).powi(2) + (ca.b - cb.b).powi(2))
                            .sqrt();
                    assert!(
                        distance > 0.25,
                        "{variant:?}: colours {a} and {b}: {distance}"
                    );
                }
            }
        }
    }
}
