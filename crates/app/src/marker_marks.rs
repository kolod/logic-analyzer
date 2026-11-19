//! The marks and lines of the markers in the signal view: the pentagon-arrow of a single marker,
//! the squares of a pair, the solid line of a fixed marker and the dotted line of one that is
//! being placed or dragged (`specs/015-markers-rework/contracts/ui-interaction.md`).
//!
//! Only geometry and drawing: no state. The shapes are plain functions of the point they hang on,
//! so the rules they must keep (the tip of the pentagon on the line; the start's square left of
//! its line and the end's square right of its line) are tested without a screen.

use iced::widget::canvas::{Frame, LineDash, Path, Stroke};
use iced::{Color, Point, Rectangle, Size};
use la_host::markers::End;

/// The width of the pentagon-arrow.
pub const PENTAGON_WIDTH: f32 = 14.0;
/// The height of its rectangular body.
pub const PENTAGON_BODY: f32 = 9.0;
/// The height of its pointed end.
pub const PENTAGON_TIP: f32 = 5.0;
/// The side of the square of a pair.
pub const SQUARE: f32 = 12.0;
/// The gap kept between a mark and whatever is beside it.
pub const GAP: f32 = 4.0;
/// The width of a solid marker line.
pub const LINE_WIDTH: f32 = 1.5;
/// The width of the line of the selected marker.
pub const SELECTED_LINE_WIDTH: f32 = 2.5;
/// The dash and the gap of a line that is being placed.
pub const DASH: [f32; 2] = [4.0, 3.0];
/// How far from a line a press still takes it.
pub const LINE_HIT: f32 = 4.0;

/// The corners of the pentagon whose lowest point is `(x, bottom)`: top left, top right, right at
/// the end of the body, the tip, left at the end of the body.
#[must_use]
pub fn pentagon_points(x: f32, bottom: f32) -> [Point; 5] {
    let half = PENTAGON_WIDTH / 2.0;
    let body_bottom = bottom - PENTAGON_TIP;
    let top = body_bottom - PENTAGON_BODY;
    [
        Point::new(x - half, top),
        Point::new(x + half, top),
        Point::new(x + half, body_bottom),
        Point::new(x, bottom),
        Point::new(x - half, body_bottom),
    ]
}

/// The square of a pair next to its line at `x`: the start's touches it from the left, the
/// end's from the right. Its bottom is at `bottom`.
#[must_use]
pub fn square_bounds(x: f32, bottom: f32, end: End) -> Rectangle {
    let left = match end {
        End::Start => x - SQUARE,
        End::End | End::Point => x,
    };
    Rectangle::new(Point::new(left, bottom - SQUARE), Size::new(SQUARE, SQUARE))
}

/// The area of the mark of an end of a marker, for a press to find it.
#[must_use]
pub fn mark_bounds(x: f32, bottom: f32, end: End) -> Rectangle {
    match end {
        End::Point => {
            let [top_left, ..] = pentagon_points(x, bottom);
            Rectangle::new(
                Point::new(x - PENTAGON_WIDTH / 2.0, top_left.y),
                Size::new(PENTAGON_WIDTH, bottom - top_left.y),
            )
        }
        End::Start | End::End => square_bounds(x, bottom, end),
    }
}

fn pentagon_path(x: f32, bottom: f32) -> Path {
    let [first, rest @ ..] = pentagon_points(x, bottom);
    Path::new(|b| {
        b.move_to(first);
        for point in rest {
            b.line_to(point);
        }
        b.close();
    })
}

/// A vertical line of a marker from `top` to `bottom`: solid when fixed, dotted while it is being
/// placed or dragged.
pub fn draw_line(
    frame: &mut Frame,
    x: f32,
    top: f32,
    bottom: f32,
    colour: Color,
    selected: bool,
    dotted: bool,
) {
    let width = if selected {
        SELECTED_LINE_WIDTH
    } else {
        LINE_WIDTH
    };
    let mut stroke = Stroke::default().with_color(colour).with_width(width);
    if dotted {
        stroke.line_dash = LineDash {
            segments: &DASH,
            offset: 0,
        };
    }
    frame.stroke(
        &Path::line(Point::new(x, top), Point::new(x, bottom)),
        stroke,
    );
}

/// The mark of one end: the pentagon of a single marker or the square of a start or an end, filled
/// with `colour` and outlined with `outline` (so it stays visible on any background).
pub fn draw_mark(
    frame: &mut Frame,
    x: f32,
    bottom: f32,
    end: End,
    colour: Color,
    outline: Color,
    selected: bool,
) {
    let path = match end {
        End::Point => pentagon_path(x, bottom),
        End::Start | End::End => Path::rectangle(
            square_bounds(x, bottom, end).position(),
            Size::new(SQUARE, SQUARE),
        ),
    };
    frame.fill(&path, colour);
    let width = if selected { 2.0 } else { 1.0 };
    frame.stroke(
        &path,
        Stroke::default().with_color(outline).with_width(width),
    );
}

#[cfg(test)]
#[allow(
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::float_cmp,
    reason = "geometry checks on hand-written numbers"
)]
mod tests {
    use super::*;

    #[test]
    fn the_tip_of_the_pentagon_is_on_the_line_and_it_is_symmetric_about_it() {
        let points = pentagon_points(100.0, 30.0);
        assert_eq!(points[3], Point::new(100.0, 30.0), "the lowest point");
        let lowest = points.iter().map(|p| p.y).fold(f32::MIN, f32::max);
        assert!((lowest - 30.0).abs() < f32::EPSILON);
        assert!((points[0].x + points[1].x - 200.0).abs() < f32::EPSILON);
        assert!((points[2].x + points[4].x - 200.0).abs() < f32::EPSILON);
        // 14 wide, 9 + 5 high.
        assert!((points[1].x - points[0].x - 14.0).abs() < f32::EPSILON);
        assert!((30.0 - points[0].y - 14.0).abs() < f32::EPSILON);
    }

    #[test]
    fn the_square_of_a_start_is_left_of_its_line_and_that_of_an_end_right_of_it() {
        let start = square_bounds(100.0, 30.0, End::Start);
        assert!(
            (start.x + start.width - 100.0).abs() < f32::EPSILON,
            "touches from the left"
        );
        let end = square_bounds(100.0, 30.0, End::End);
        assert!(
            (end.x - 100.0).abs() < f32::EPSILON,
            "touches from the right"
        );
        for square in [start, end] {
            assert!((square.width - 12.0).abs() < f32::EPSILON);
            assert!((square.height - 12.0).abs() < f32::EPSILON);
            assert!(
                (square.y + square.height - 30.0).abs() < f32::EPSILON,
                "sits in the ruler"
            );
        }
    }

    #[test]
    fn a_press_finds_the_mark_of_each_end() {
        let single = mark_bounds(100.0, 30.0, End::Point);
        assert!(single.contains(Point::new(100.0, 20.0)));
        assert!(!single.contains(Point::new(120.0, 20.0)));
        let start = mark_bounds(100.0, 30.0, End::Start);
        assert!(start.contains(Point::new(95.0, 25.0)));
        assert!(!start.contains(Point::new(105.0, 25.0)));
        let end = mark_bounds(100.0, 30.0, End::End);
        assert!(end.contains(Point::new(105.0, 25.0)));
    }

    #[test]
    fn the_fit_margin_of_the_host_is_a_square_and_a_gap() {
        assert!((la_host::markers::MARK_MARGIN_PX as f32 - (SQUARE + GAP)).abs() < f32::EPSILON);
    }
}
