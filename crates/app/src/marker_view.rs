//! The Markers view of the side panel: the two buttons that start a placement, one row per marker
//! with its colour, name, comment and times, and the action list of the marker that was clicked (see
//! `specs/015-markers-rework/contracts/ui-interaction.md`).
//!
//! The view only draws what the host says and turns presses into messages; every rule is the
//! host's. It is built from a [`Panel`] that holds plain data, so it is tested without the
//! application around it.

use std::collections::BTreeMap;

use iced::widget::{Space, button, column, container, row, text, text_input, tooltip};
use iced::{Border, Color, Element, Fill, Theme};
use la_host::markers::{
    Kind, MARKER_COLOURS, MAX_COMMENT_CHARS, MAX_MARKERS, Marker, MarkerText, Shape,
};
use la_host::{Command, State};

use crate::app::Message;
use crate::icons::{self, Icon};
use crate::theme::Variant;

/// The texts of the view, in the language in use (fetched by the application).
#[derive(Debug, Clone)]
pub struct Strings {
    /// The title of the view.
    pub title: String,
    /// The label of the button that places a single marker.
    pub single: String,
    /// The label of the button that places a pair marker.
    pub pair: String,
    /// The text of an empty list.
    pub empty: String,
    /// The text shown at the limit of 64 markers.
    pub limit: String,
    /// The label of the delete button in the action list.
    pub delete: String,
    /// The placeholder of the comment field.
    pub comment: String,
    /// The label of the action that makes a pair of two single markers.
    pub make_pair: String,
    /// What to do after it: click the second single marker.
    pub join_hint: String,
    /// The names of the eight colours.
    pub colours: Vec<String>,
    /// Count time from a single marker.
    pub origin: String,
    /// Count time from the start of the capture again.
    pub origin_reset: String,
    /// Delete the data to the left of a single marker.
    pub cut_left: String,
    /// Delete the data to the right of a single marker.
    pub cut_right: String,
    /// Scroll the view to a single marker.
    pub scroll: String,
    /// Keep only the data between the ends of a pair.
    pub trim: String,
    /// Fit the view to a pair.
    pub fit: String,
    /// Replace a pair by two single markers.
    pub split: String,
    /// Why a cut is not possible: a capture is running.
    pub disabled_running: String,
    /// Why a cut is not possible: there is nothing to delete.
    pub disabled_nothing: String,
}

/// What the view shows.
pub struct Panel<'a> {
    /// The host state.
    pub state: &'a State,
    /// The texts of the markers, in the order of `state.markers.list`.
    pub texts: &'a [MarkerText],
    /// The colour variant of the theme.
    pub variant: Variant,
    /// The marker whose actions are open.
    pub open: Option<u32>,
    /// The comments being edited.
    pub comments: &'a BTreeMap<u32, String>,
    /// The single marker a pair is being made from: the next single marker that is clicked is
    /// the other half.
    pub joining: Option<u32>,
    /// The texts.
    pub strings: &'a Strings,
}

/// A button that shows a colour: a swatch, with a ring when it is the one chosen.
fn swatch_style(colour: Color, chosen: bool) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |theme, status| {
        let ring = if chosen {
            theme.palette().text
        } else {
            Color::TRANSPARENT
        };
        let lift = match status {
            button::Status::Hovered | button::Status::Pressed => 0.15,
            button::Status::Active | button::Status::Disabled => 0.0,
        };
        let shade = |v: f32| v + (1.0 - v) * lift;
        button::Style {
            background: Some(
                Color {
                    r: shade(colour.r),
                    g: shade(colour.g),
                    b: shade(colour.b),
                    a: colour.a,
                }
                .into(),
            ),
            border: Border {
                color: ring,
                width: 2.0,
                radius: 3.0.into(),
            },
            ..button::Style::default()
        }
    }
}

fn swatch<'a>(
    colour: Color,
    chosen: bool,
    message: Option<Message>,
) -> button::Button<'a, Message> {
    button(Space::new().width(14).height(14))
        .padding(2)
        .style(swatch_style(colour, chosen))
        .on_press_maybe(message)
}

impl<'a> Panel<'a> {
    fn text_of(&self, id: u32) -> Option<&MarkerText> {
        self.texts.iter().find(|t| t.id == id)
    }

    /// The summary line of a marker: its time, or its two times with the duration and frequency.
    fn summary(&self, id: u32) -> String {
        let Some(entry) = self.text_of(id) else {
            return String::new();
        };
        let mut summary = entry.time_a.clone();
        if let (Some(end), Some(duration)) = (&entry.time_b, &entry.duration) {
            // The times on one line, the delta on the next.
            summary = format!("{summary} \u{2192} {end}\n\u{394} {duration}");
            if let Some(frequency) = &entry.frequency {
                summary = format!("{summary}  ({frequency})");
            }
        }
        summary
    }

    fn add_button(&self, kind: Kind, label: &str, full: bool) -> Element<'static, Message> {
        let placing = self.state.placement.map(|p| p.kind) == Some(kind);
        let icon = icons::icon(
            match kind {
                Kind::Single => Icon::SingleMarker,
                Kind::Pair => Icon::PairMarker,
            },
            placing,
        );
        let allowed = self.state.capture.is_some() && !full;
        button(
            row![icon, text(label.to_owned()).size(12)]
                .spacing(4)
                .align_y(iced::Alignment::Center),
        )
        // The two buttons share the width of the view, in one line.
        .width(Fill)
        .style(if placing {
            button::success
        } else {
            button::primary
        })
        .padding(4)
        .on_press_maybe(allowed.then_some(Message::Run(Command::BeginPlacement(kind))))
        .into()
    }

    fn row_of(&self, marker: &Marker) -> Element<'static, Message> {
        let id = marker.id;
        let colour = self.variant.marker(marker.colour);
        let name = self
            .text_of(id)
            .map_or_else(|| marker.name(), |t| t.name.clone());
        // The name and the comment share a line; the times are under them.
        let mut title = row![text(name).size(13)]
            .spacing(6)
            .align_y(iced::Alignment::Center);
        if !marker.comment.is_empty() {
            title = title.push(text(marker.comment.clone()).size(11));
        }
        let label = column![title, text(self.summary(id)).size(11)].spacing(1);
        // While a pair is being made only a single marker other than the first can be chosen: the
        // others are shaded (they cannot be pressed).
        let choosable = match self.joining {
            None => true,
            Some(first) => first != id && matches!(marker.shape, Shape::Point(_)),
        };
        let press = choosable.then_some(Message::MarkerMenu(id));
        let icon = icons::icon(
            match marker.kind() {
                Kind::Single => Icon::SingleMarker,
                Kind::Pair => Icon::PairMarker,
            },
            false,
        );
        row![
            icon,
            swatch(colour, false, press.clone()),
            // A press on the marker selects it and opens or closes its actions.
            button(label)
                .style(button::text)
                .width(Fill)
                .padding(3)
                .on_press_maybe(press),
        ]
        .spacing(4)
        .align_y(iced::Alignment::Center)
        .into()
    }

    /// One action of the list: a button, or a greyed one that says why it cannot be used.
    fn entry(
        label: &str,
        press: Option<Message>,
        reason: Option<&str>,
    ) -> Element<'static, Message> {
        let b = button(text(label.to_owned()).size(12))
            .style(button::primary)
            .width(Fill)
            .padding(4)
            .on_press_maybe(press);
        match reason {
            Some(reason) => tooltip(b, text(reason.to_owned()), tooltip::Position::Bottom).into(),
            None => b.into(),
        }
    }

    /// The actions of the kind of an open marker.
    fn kind_actions(&self, marker: &Marker) -> Vec<Element<'static, Message>> {
        let id = marker.id;
        let state = self.state;
        let strings = self.strings;
        let length = state.capture.as_ref().map_or(0, |c| c.snapshot.length());
        let busy = state.activity != la_host::Activity::Idle || state.operation.is_some();
        let cut = |label: &str, command: Command, enough: bool| {
            let reason = if busy {
                Some(strings.disabled_running.as_str())
            } else if !enough {
                Some(strings.disabled_nothing.as_str())
            } else {
                None
            };
            Self::entry(
                label,
                reason.is_none().then_some(Message::Run(command)),
                reason,
            )
        };
        match marker.shape {
            Shape::Point(position) => {
                let is_origin = state.markers.origin == position && position != 0;
                vec![
                    if is_origin {
                        Self::entry(
                            &strings.origin_reset,
                            Some(Message::Run(Command::ResetTimeOrigin)),
                            None,
                        )
                    } else {
                        Self::entry(
                            &strings.origin,
                            Some(Message::Run(Command::SetTimeOrigin(id))),
                            None,
                        )
                    },
                    cut(&strings.cut_left, Command::CutLeft(id), position > 0),
                    cut(
                        &strings.cut_right,
                        Command::CutRight(id),
                        position.saturating_add(1) < length,
                    ),
                    Self::entry(
                        &strings.scroll,
                        Some(Message::Run(Command::ScrollToMarker(id))),
                        None,
                    ),
                    // Needs another single marker to pair with.
                    Self::entry(
                        &strings.make_pair,
                        state
                            .markers
                            .list
                            .iter()
                            .any(|m| m.id != id && matches!(m.shape, Shape::Point(_)))
                            .then_some(Message::MarkerJoin(id)),
                        None,
                    ),
                ]
            }
            Shape::Interval { start, end } => vec![
                cut(
                    &strings.trim,
                    Command::TrimToPair(id),
                    start > 0 || end.saturating_add(1) < length,
                ),
                Self::entry(
                    &strings.fit,
                    Some(Message::Run(Command::FitToPair(id))),
                    None,
                ),
                Self::entry(
                    &strings.split,
                    (state.markers.list.len() < MAX_MARKERS)
                        .then_some(Message::Run(Command::SplitPair(id))),
                    None,
                ),
            ],
        }
    }

    /// The actions of an open marker: those of its kind, its colours, its comment and the delete
    /// button.
    fn actions_of(&self, marker: &Marker) -> Element<'static, Message> {
        let id = marker.id;
        let mut colours = row![].spacing(4);
        for colour in 0..MARKER_COLOURS {
            let name = self
                .strings
                .colours
                .get(usize::from(colour))
                .cloned()
                .unwrap_or_default();
            colours = colours.push(tooltip(
                swatch(
                    self.variant.marker(colour),
                    marker.colour == colour,
                    Some(Message::Run(Command::SetMarkerColour { id, colour })),
                ),
                text(name),
                tooltip::Position::Bottom,
            ));
        }
        let comment = self.comments.get(&id).map_or("", String::as_str);
        let mut list = column![].spacing(6);
        for action in self.kind_actions(marker) {
            list = list.push(action);
        }
        let list = list.push(colours).push(
            text_input(&self.strings.comment, comment)
                .size(12)
                .width(Fill)
                .on_input(move |t| Message::MarkerComment(id, t)),
        );
        let list = list.push(
            button(text(self.strings.delete.clone()).size(12))
                .style(button::danger)
                .padding(4)
                .on_press(Message::Run(Command::RemoveMarker(id))),
        );
        container(list)
            .padding(6)
            .width(Fill)
            .style(container::rounded_box)
            .into()
    }

    /// The whole view.
    #[must_use]
    pub fn view(&self) -> Element<'static, Message> {
        let state = self.state;
        let full = state.markers.list.len() >= MAX_MARKERS;
        let mut list = column![text(self.strings.title.clone()).size(14)]
            .spacing(4)
            .padding(10);
        list = list.push(
            row![
                self.add_button(Kind::Single, &self.strings.single, full),
                self.add_button(Kind::Pair, &self.strings.pair, full),
            ]
            .spacing(6)
            .width(Fill),
        );
        // A gap between the buttons and the first marker.
        list = list.push(Space::new().height(6));
        if self.joining.is_some() {
            list = list.push(text(self.strings.join_hint.clone()).size(12));
        }
        if full {
            list = list.push(text(self.strings.limit.clone()).size(12));
        }
        if state.markers.list.is_empty() {
            list = list.push(text(self.strings.empty.clone()).size(12));
        }
        for marker in state.markers.ordered() {
            // Each marker has its own outline; the view itself has none, like the Capture view.
            let mut block = column![self.row_of(marker)].spacing(4);
            if self.open == Some(marker.id) {
                block = block.push(self.actions_of(marker));
            }
            // The selected marker is told by a stronger outline, not by a background.
            let selected = state.markers.selected == Some(marker.id);
            list = list.push(container(block).padding(4).width(Fill).style(
                move |theme: &Theme| {
                    let mut style = container::bordered_box(theme);
                    if selected {
                        style.border.color = theme.palette().primary;
                        style.border.width = 2.0;
                    }
                    style
                },
            ));
        }
        list.into()
    }
}

/// The comment of every marker, as the text fields hold it.
#[must_use]
pub fn comments_of(state: &State) -> BTreeMap<u32, String> {
    state
        .markers
        .list
        .iter()
        .map(|m| (m.id, m.comment.clone()))
        .collect()
}

/// The longest comment the field takes, so a long paste is cut as the host will cut it.
#[must_use]
pub fn limit_comment(text: &str) -> String {
    text.chars().take(MAX_COMMENT_CHARS).collect()
}
