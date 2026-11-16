//! The strip of icons at the right edge and the frame of the panel next to it
//! (`specs/014-gui-improvements/contracts/ui-interaction.md`).
//!
//! The panel shows one view at a time, or none. The strip is always there.

use iced::widget::{Space, button, column, container, row, tooltip};
use iced::{Element, Length, Theme};
use la_host::SidePanel;

use crate::app::Message;
use crate::icons::{self, Icon};

/// Width of the strip of icons.
pub const STRIP_WIDTH: f32 = 36.0;
/// Side of an icon button.
pub const BUTTON: f32 = 28.0;
/// Gap between the buttons and from the edges.
const GAP: f32 = 4.0;
/// Width of the panel.
pub const PANEL_WIDTH: f32 = 280.0;
/// The views in the order of the strip, top to bottom.
pub const VIEWS: [SidePanel; 3] = [SidePanel::Capture, SidePanel::Decoders, SidePanel::Markers];

/// What a click on the icon of `clicked` does: the icon of the view that is shown hides the panel,
/// any other shows its view.
#[must_use]
pub fn next_state(current: SidePanel, clicked: SidePanel) -> SidePanel {
    if clicked == current || clicked == SidePanel::Hidden {
        SidePanel::Hidden
    } else {
        clicked
    }
}

/// The icon of a view.
fn icon_of(view: SidePanel) -> Icon {
    match view {
        SidePanel::Decoders => Icon::Decoders,
        SidePanel::Markers => Icon::Markers,
        SidePanel::Capture | SidePanel::Hidden => Icon::Capture,
    }
}

/// The strip: one button for each view with a bar at its left edge for the view that is shown.
/// `tips` are the tooltips in the language in use, in the order of [`VIEWS`].
pub fn strip<'a>(current: SidePanel, tips: [String; 3]) -> Element<'a, Message> {
    let mut buttons = column![].spacing(GAP).padding(GAP);
    for (view, tip) in VIEWS.into_iter().zip(tips) {
        let active = current == view;
        let bar = container(Space::new().width(3).height(BUTTON)).style(move |theme: &Theme| {
            container::Style {
                background: active.then(|| theme.palette().primary.into()),
                ..container::Style::default()
            }
        });
        let press = button(icons::icon(icon_of(view), active))
            .width(BUTTON)
            .height(BUTTON)
            .padding(0)
            .style(button::text)
            .on_press(Message::SidePanel(next_state(current, view)));
        buttons = buttons.push(row![
            bar,
            tooltip(
                press,
                container(iced::widget::text(tip).size(12))
                    .padding(4)
                    .style(container::bordered_box),
                tooltip::Position::Left
            )
        ]);
    }
    container(buttons)
        .width(STRIP_WIDTH + 3.0)
        .height(Length::Fill)
        .style(container::bordered_box)
        .into()
}

/// The frame of the panel around the content of the shown view.
pub fn frame<'a>(content: Element<'a, Message>) -> Element<'a, Message> {
    container(content)
        .width(Length::Fixed(PANEL_WIDTH))
        .max_width(PANEL_WIDTH)
        .height(Length::Fill)
        .into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_click_shows_a_view_and_a_second_click_on_it_hides_the_panel() {
        use SidePanel::{Capture, Decoders, Hidden, Markers};
        assert_eq!(next_state(Capture, Decoders), Decoders);
        assert_eq!(next_state(Decoders, Decoders), Hidden);
        assert_eq!(next_state(Hidden, Markers), Markers);
        assert_eq!(next_state(Markers, Capture), Capture);
        assert_eq!(next_state(Capture, Capture), Hidden);
        // Hidden is not a button; asking for it hides.
        assert_eq!(next_state(Markers, Hidden), Hidden);
        assert_eq!(next_state(Hidden, Hidden), Hidden);
    }

    #[test]
    fn the_strip_is_made_of_28_px_buttons_with_a_gap_of_4_px() {
        // 4 + 28 + 4 + 28 + 4 + 28 + 4: three buttons fit a strip 36 wide with room to spare.
        assert!((STRIP_WIDTH - BUTTON) / 2.0 >= GAP);
        assert!((BUTTON - 28.0).abs() < f32::EPSILON && (GAP - 4.0).abs() < f32::EPSILON);
        assert!(PANEL_WIDTH >= 220.0);
    }

    #[test]
    fn the_views_are_the_three_the_roadmap_names_in_order() {
        assert_eq!(
            VIEWS,
            [SidePanel::Capture, SidePanel::Decoders, SidePanel::Markers]
        );
    }
}
