//! Spike (task T007): can layout be measured headlessly on both CI platforms?
//!
//! Deleted once the decision is written into `research.md` (R10 addendum).

use iced::widget::{button, column, container, text};
use iced::{Element, Length, Size};
use iced_test::Simulator;
use iced_test::selector::{Bounded, Candidate};

const GERMAN: &str = "Geschwindigkeitsbegrenzungsüberschreitungsmeldung wurde ausgelöst";

fn roomy<'a>() -> Element<'a, ()> {
    container(column![text(GERMAN), button("Speichern")])
        .width(Length::Fixed(200.0))
        .into()
}

fn clipped<'a>() -> Element<'a, ()> {
    container(text(GERMAN))
        .width(Length::Fixed(200.0))
        .height(Length::Fixed(20.0))
        .clip(true)
        .into()
}

/// Layout and visible bounds of every text in the view.
fn texts(view: Element<'_, ()>) -> Vec<(iced::Rectangle, Option<iced::Rectangle>)> {
    let mut ui = Simulator::with_size(iced::Settings::default(), Size::new(800.0, 560.0), view);
    let mut found = Vec::new();
    let _ = ui.find(|candidate: Candidate<'_>| -> Option<()> {
        if let Candidate::Text {
            bounds,
            visible_bounds,
            ..
        } = candidate
        {
            found.push((bounds, visible_bounds));
        }
        None
    });
    found
}

#[test]
fn text_bounds_and_clipping_are_available_headlessly() {
    let roomy = texts(roomy());
    println!("roomy: {roomy:?}");
    assert!(!roomy.is_empty());
    for (bounds, visible) in &roomy {
        let v = visible.expect("visible");
        assert!((v.height - bounds.height).abs() < 0.05 && (v.width - bounds.width).abs() < 0.05);
    }

    let clipped = texts(clipped());
    println!("clipped: {clipped:?}");
    assert!(!clipped.is_empty());
    assert!(
        clipped
            .iter()
            .any(|(b, v)| v.is_none_or(|v| v.height < b.height - 0.05)),
        "clipping is detected through visible bounds"
    );
    let _ = <iced_test::selector::Text as Bounded>::bounds;
}
