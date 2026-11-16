//! The vector icons of the interface, drawn with the canvas, so that no icon font or image is
//! needed and the icons follow the theme.

use iced::widget::canvas::{self, Frame, Geometry, Path, Stroke};
use iced::{Element, Point, Rectangle, Renderer, Size, Theme, mouse};

use crate::app::Message;

/// The side of the square an icon is drawn in.
pub const SIZE: f32 = 20.0;

/// The icons.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Icon {
    /// A ring with a dot: the settings of the capture.
    Capture,
    /// Three stacked boxes: the decoders.
    Decoders,
    /// A flag: the markers.
    Markers,
}

/// What an icon draws: the strokes and fills of one icon, in the unit square `0..SIZE`.
#[derive(Debug, Clone, Copy)]
struct IconView {
    icon: Icon,
    active: bool,
}

/// The icon as a widget, in a square of [`SIZE`] pixels; `active` draws it in the accent colour.
pub fn icon<'a>(icon: Icon, active: bool) -> Element<'a, Message> {
    canvas::Canvas::new(IconView { icon, active })
        .width(SIZE)
        .height(SIZE)
        .into()
}

/// The paths of an icon, in the square `0..SIZE`: `(path, filled)`. Exposed for the tests, which
/// check that every icon stays inside its square.
fn paths(icon: Icon) -> Vec<(Path, bool)> {
    match icon {
        Icon::Capture => vec![
            (Path::circle(Point::new(10.0, 10.0), 7.5), false),
            (Path::circle(Point::new(10.0, 10.0), 3.2), true),
        ],
        Icon::Decoders => (0..3)
            .map(|i: u8| {
                let y = 2.0 + f32::from(i) * 6.0;
                (
                    Path::rounded_rectangle(
                        Point::new(2.5, y),
                        Size::new(15.0, 4.5),
                        iced::border::Radius::from(1.5),
                    ),
                    i == 1,
                )
            })
            .collect(),
        Icon::Markers => {
            let pole = Path::line(Point::new(5.0, 2.0), Point::new(5.0, 18.0));
            let flag = Path::new(|b| {
                b.move_to(Point::new(5.0, 3.0));
                b.line_to(Point::new(16.0, 6.5));
                b.line_to(Point::new(5.0, 10.0));
                b.close();
            });
            vec![(pole, false), (flag, true)]
        }
    }
}

impl canvas::Program<Message> for IconView {
    type State = ();

    fn draw(
        &self,
        _state: &(),
        renderer: &Renderer,
        theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<Geometry> {
        let palette = theme.palette();
        let colour = if self.active {
            palette.primary
        } else {
            palette.text
        };
        let mut frame = Frame::new(renderer, bounds.size());
        // The icon is centred in the space it is given.
        let dx = ((bounds.width - SIZE) / 2.0).max(0.0);
        let dy = ((bounds.height - SIZE) / 2.0).max(0.0);
        frame.translate(iced::Vector::new(dx, dy));
        for (path, filled) in paths(self.icon) {
            if filled {
                frame.fill(&path, colour);
            } else {
                frame.stroke(&path, Stroke::default().with_color(colour).with_width(1.6));
            }
        }
        vec![frame.into_geometry()]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_icon_draws_inside_its_square_and_draws_something() {
        for icon in [Icon::Capture, Icon::Decoders, Icon::Markers] {
            let list = paths(icon);
            assert!(!list.is_empty(), "{icon:?}");
            for (path, _) in list {
                // The control points of the path stay within 0..SIZE.
                let b = path_bounds(&path);
                assert!(
                    b.0 >= 0.0 && b.1 >= 0.0 && b.2 <= SIZE && b.3 <= SIZE,
                    "{icon:?}: {b:?}"
                );
            }
        }
    }

    /// `(min x, min y, max x, max y)` of the points of a path.
    fn path_bounds(path: &Path) -> (f32, f32, f32, f32) {
        use iced::widget::canvas::path::lyon_path::PathEvent;
        let mut b = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
        let mut take = |p: iced::widget::canvas::path::lyon_path::math::Point| {
            b = (b.0.min(p.x), b.1.min(p.y), b.2.max(p.x), b.3.max(p.y));
        };
        for event in path.raw().iter() {
            match event {
                PathEvent::Begin { at } => take(at),
                PathEvent::Line { to, .. } => take(to),
                PathEvent::Quadratic { ctrl, to, .. } => {
                    take(ctrl);
                    take(to);
                }
                PathEvent::Cubic {
                    ctrl1, ctrl2, to, ..
                } => {
                    take(ctrl1);
                    take(ctrl2);
                    take(to);
                }
                PathEvent::End { .. } => {}
            }
        }
        b
    }
}
