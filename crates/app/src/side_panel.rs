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
/// Width of the splitter between the signals and the panel.
pub const SPLITTER_WIDTH: f32 = 5.0;
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
    let mut buttons = column![].spacing(GAP).padding(GAP).height(Length::Fill);
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
    // The settings are at the bottom of the strip: a cog, which is not a view of the panel.
    let cog = button(icons::icon(Icon::Settings, false))
        .width(BUTTON)
        .height(BUTTON)
        .padding(0)
        .style(button::text)
        .on_press(Message::OpenSettings);
    buttons = buttons
        .push(Space::new().height(Length::Fill))
        .push(row![Space::new().width(3), cog]);
    container(buttons)
        .width(STRIP_WIDTH + 3.0)
        .height(Length::Fill)
        .style(container::bordered_box)
        .into()
}

/// The frame of the panel around the content of the shown view, `width` pixels wide, with the
/// splitter at its left edge: a press on it starts changing the width.
pub fn frame<'a>(content: Element<'a, Message>, width: f32) -> Element<'a, Message> {
    let splitter = iced::widget::mouse_area(
        container(Space::new().width(SPLITTER_WIDTH).height(Length::Fill)).style(
            |theme: &Theme| container::Style {
                background: Some(theme.extended_palette().background.weak.color.into()),
                ..container::Style::default()
            },
        ),
    )
    .interaction(iced::mouse::Interaction::ResizingHorizontally)
    .on_press(Message::SplitterPressed);
    row![
        splitter,
        container(content)
            .width(Length::Fixed(width))
            .height(Length::Fill)
    ]
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
        const { assert!((STRIP_WIDTH - BUTTON) / 2.0 >= GAP) };
        const { assert!(BUTTON == 28.0 && GAP == 4.0) };
        const { assert!(SPLITTER_WIDTH >= 3.0 && SPLITTER_WIDTH <= STRIP_WIDTH) };
    }

    #[test]
    fn the_views_are_the_three_the_roadmap_names_in_order() {
        assert_eq!(
            VIEWS,
            [SidePanel::Capture, SidePanel::Decoders, SidePanel::Markers]
        );
    }
}
