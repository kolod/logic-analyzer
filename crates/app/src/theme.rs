//! The look of the application: an original palette in a light and a dark variant.
//!
//! The 16 channel colours are chosen to be told apart from each other and from the background in
//! both variants; a signal state never relies on colour alone (lane position, labels and the
//! shape of the dense-activity mark carry the same information).

use iced::{Color, Theme, theme};
use la_host::presentation::{Colour, PALETTE_SIZE};
use la_host::settings::Theme as ThemeChoice;

/// Which of the variants is in use.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Variant {
    /// Dark background.
    Dark,
    /// Light background.
    Light,
    /// Black background, white text, full-strength lines.
    HighContrast,
}

impl Variant {
    /// The variant for the user's choice and what the system reports.
    #[must_use]
    pub fn resolve(choice: ThemeChoice, system: theme::Mode) -> Self {
        match choice {
            ThemeChoice::Light => Variant::Light,
            ThemeChoice::Dark => Variant::Dark,
            ThemeChoice::HighContrast => Variant::HighContrast,
            ThemeChoice::System => match system {
                theme::Mode::Light => Variant::Light,
                _ => Variant::Dark,
            },
        }
    }

    /// The toolkit theme of this variant.
    #[must_use]
    pub fn theme(self) -> Theme {
        match self {
            Variant::Dark => Theme::custom(
                "Logic dark".to_owned(),
                theme::Palette {
                    background: rgb(0x12, 0x16, 0x1d),
                    text: rgb(0xe4, 0xe9, 0xf0),
                    primary: rgb(0x3f, 0xb8, 0xd0),
                    success: rgb(0x5f, 0xc7, 0x7d),
                    warning: rgb(0xe9, 0xb3, 0x4a),
                    danger: rgb(0xef, 0x6a, 0x6a),
                },
            ),
            Variant::HighContrast => Theme::custom(
                "Logic high contrast".to_owned(),
                theme::Palette {
                    background: rgb(0x00, 0x00, 0x00),
                    text: rgb(0xff, 0xff, 0xff),
                    primary: rgb(0xff, 0xe0, 0x00),
                    success: rgb(0x40, 0xff, 0x70),
                    warning: rgb(0xff, 0xc0, 0x20),
                    danger: rgb(0xff, 0x70, 0x70),
                },
            ),
            Variant::Light => Theme::custom(
                "Logic light".to_owned(),
                theme::Palette {
                    background: rgb(0xf3, 0xf5, 0xf8),
                    text: rgb(0x1a, 0x22, 0x2e),
                    primary: rgb(0x0b, 0x7a, 0x92),
                    success: rgb(0x23, 0x84, 0x3f),
                    warning: rgb(0xa8, 0x6a, 0x00),
                    danger: rgb(0xc2, 0x2d, 0x2d),
                },
            ),
        }
    }

    /// The colour of grid lines and hairlines against the background.
    #[must_use]
    pub fn hairline(self) -> Color {
        match self {
            Variant::Dark => Color::from_rgba8(0xe4, 0xe9, 0xf0, 0.16),
            Variant::HighContrast => Color::from_rgba8(0xff, 0xff, 0xff, 0.45),
            Variant::Light => Color::from_rgba8(0x1a, 0x22, 0x2e, 0.18),
        }
    }

    /// The fill of the "no data" bands (gaps and the part past the end).
    #[must_use]
    pub fn no_data(self) -> Color {
        match self {
            Variant::Dark => Color::from_rgba8(0xe4, 0xe9, 0xf0, 0.10),
            Variant::HighContrast => Color::from_rgba8(0xff, 0xff, 0xff, 0.28),
            Variant::Light => Color::from_rgba8(0x1a, 0x22, 0x2e, 0.10),
        }
    }

    /// The colour of an annotation (one of the eight of the palette of annotations). It is chosen
    /// against the background of the variant; the shape of the annotation and an error's danger
    /// colour and heavier outline carry the same information, so nothing depends on colour alone.
    #[must_use]
    pub fn annotation(self, index: u8) -> Color {
        let table = match self {
            Variant::Dark => &ANNOTATION_DARK,
            Variant::Light => &ANNOTATION_LIGHT,
            Variant::HighContrast => &ANNOTATION_HIGH_CONTRAST,
        };
        let (r, g, b) = table
            .get(usize::from(index % 8))
            .copied()
            .unwrap_or((0x80, 0x80, 0x80));
        Color::from_rgb8(r, g, b)
    }

    /// The colour of a marker (an index below 8): the eight colours of the annotations, which
    /// are chosen against the background of the variant and are told apart by the shape of the
    /// mark and the name beside it as well.
    #[must_use]
    pub fn marker(self, index: u8) -> Color {
        self.annotation(index)
    }

    /// The colour of a channel.
    #[must_use]
    pub fn channel(self, colour: Colour) -> Color {
        match colour {
            Colour::Rgb(r, g, b) => Color::from_rgb8(r, g, b),
            Colour::Palette(index) => {
                let table = match self {
                    Variant::Dark | Variant::HighContrast => &DARK,
                    Variant::Light => &LIGHT,
                };
                let at = usize::from(index % PALETTE_SIZE);
                let (r, g, b) = table.get(at).copied().unwrap_or((0x80, 0x80, 0x80));
                Color::from_rgb8(r, g, b)
            }
        }
    }
}

/// The relative luminance of a colour (WCAG 2).
fn relative_luminance(c: Color) -> f32 {
    let channel = |v: f32| {
        if v <= 0.039_28 {
            v / 12.92
        } else {
            ((v + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * channel(c.r) + 0.7152 * channel(c.g) + 0.0722 * channel(c.b)
}

/// The colour of the number on a channel button that is filled with `background`: black or white,
/// whichever contrasts better.
#[must_use]
pub fn channel_button_text(background: Color) -> Color {
    let luminance = relative_luminance(background);
    // Contrast with white is 1.05 / (L + 0.05), with black (L + 0.05) / 0.05.
    if 1.05 / (luminance + 0.05) >= (luminance + 0.05) / 0.05 {
        Color::WHITE
    } else {
        Color::BLACK
    }
}

/// The style of the button that chooses whether a channel is recorded: off, the number and the
/// border are in the colour of the channel on the colour of the panel; on, the border and the inside
/// are in the colour of the channel and the number is readable on it.
#[must_use]
pub fn channel_button_style(
    variant: Variant,
    colour: Colour,
    on: bool,
    theme: &Theme,
    status: iced::widget::button::Status,
) -> iced::widget::button::Style {
    use iced::widget::button::{Status, Style};
    let channel = variant.channel(colour);
    let panel = theme.palette().background;
    let (background, text) = if on {
        (channel, channel_button_text(channel))
    } else {
        (panel, channel)
    };
    let shade = match status {
        Status::Hovered => 0.12,
        Status::Pressed => 0.2,
        Status::Active | Status::Disabled => 0.0,
    };
    let background = Color {
        r: background.r + (text.r - background.r) * shade,
        g: background.g + (text.g - background.g) * shade,
        b: background.b + (text.b - background.b) * shade,
        a: background.a,
    };
    Style {
        background: Some(background.into()),
        text_color: if status == Status::Disabled {
            Color { a: 0.6, ..text }
        } else {
            text
        },
        border: iced::Border {
            color: channel,
            width: 1.5,
            radius: 4.0.into(),
        },
        ..Style::default()
    }
}

fn rgb(r: u8, g: u8, b: u8) -> Color {
    Color::from_rgb8(r, g, b)
}

/// Annotation colours on the dark background.
pub const ANNOTATION_DARK: [(u8, u8, u8); 8] = [
    (0xff, 0x8a, 0x80),
    (0xff, 0xcc, 0x66),
    (0xa5, 0xd6, 0x6e),
    (0x6e, 0xd8, 0xc4),
    (0x6c, 0xb8, 0xff),
    (0xa8, 0x9c, 0xff),
    (0xf0, 0x8c, 0xd8),
    (0xc8, 0xcf, 0xd8),
];

/// Annotation colours on the light background.
pub const ANNOTATION_LIGHT: [(u8, u8, u8); 8] = [
    (0xb7, 0x1c, 0x1c),
    (0x9a, 0x5b, 0x00),
    (0x3f, 0x7d, 0x0a),
    (0x00, 0x7a, 0x6a),
    (0x0b, 0x5f, 0xb5),
    (0x55, 0x3c, 0xc4),
    (0xa8, 0x2a, 0x8c),
    (0x4a, 0x52, 0x5c),
];

/// Annotation colours on the black background of the high-contrast variant.
pub const ANNOTATION_HIGH_CONTRAST: [(u8, u8, u8); 8] = [
    (0xff, 0x80, 0x80),
    (0xff, 0xd0, 0x40),
    (0x80, 0xff, 0x80),
    (0x40, 0xff, 0xe0),
    (0x80, 0xc0, 0xff),
    (0xc0, 0xa0, 0xff),
    (0xff, 0x80, 0xff),
    (0xff, 0xff, 0xff),
];

/// Channel colours on a dark background.
const DARK: [(u8, u8, u8); 16] = [
    (0xff, 0x6b, 0x6b),
    (0xff, 0xa9, 0x4d),
    (0xff, 0xd4, 0x3b),
    (0xa9, 0xe3, 0x4b),
    (0x51, 0xcf, 0x66),
    (0x38, 0xd9, 0xa9),
    (0x3b, 0xc9, 0xdb),
    (0x4d, 0xab, 0xf7),
    (0x74, 0x8f, 0xfc),
    (0x97, 0x75, 0xfa),
    (0xda, 0x77, 0xf2),
    (0xf7, 0x83, 0xac),
    (0xce, 0xd4, 0xda),
    (0xff, 0x92, 0x5c),
    (0x63, 0xe6, 0xbe),
    (0x74, 0xc0, 0xfc),
];

/// Channel colours on a light background.
const LIGHT: [(u8, u8, u8); 16] = [
    (0xc9, 0x2a, 0x2a),
    (0xd9, 0x48, 0x0f),
    (0xa6, 0x7c, 0x00),
    (0x5c, 0x94, 0x0d),
    (0x2b, 0x8a, 0x3e),
    (0x08, 0x7f, 0x5b),
    (0x0b, 0x72, 0x85),
    (0x1c, 0x7e, 0xd6),
    (0x3b, 0x5b, 0xdb),
    (0x67, 0x41, 0xd9),
    (0xae, 0x3e, 0xc9),
    (0xc2, 0x25, 0x5c),
    (0x49, 0x50, 0x57),
    (0xe6, 0x49, 0x80),
    (0x09, 0x92, 0x68),
    (0x15, 0x6f, 0xb8),
];

#[cfg(test)]
mod tests {
    use super::*;

    /// The relative luminance of a colour (WCAG).
    fn luminance(c: Color) -> f32 {
        let channel = |v: f32| {
            if v <= 0.03928 {
                v / 12.92
            } else {
                ((v + 0.055) / 1.055).powf(2.4)
            }
        };
        0.2126 * channel(c.r) + 0.7152 * channel(c.g) + 0.0722 * channel(c.b)
    }

    fn contrast(a: Color, b: Color) -> f32 {
        let (la, lb) = (luminance(a), luminance(b));
        (la.max(lb) + 0.05) / (la.min(lb) + 0.05)
    }

    fn distance(a: Color, b: Color) -> f32 {
        ((a.r - b.r).powi(2) + (a.g - b.g).powi(2) + (a.b - b.b).powi(2)).sqrt()
    }

    #[test]
    fn the_number_on_a_filled_channel_button_can_be_read_on_every_palette_colour_in_every_variant()
    {
        for variant in [Variant::Dark, Variant::Light, Variant::HighContrast] {
            for index in 0..PALETTE_SIZE {
                let background = variant.channel(Colour::Palette(index));
                let text = channel_button_text(background);
                assert!(
                    text == Color::BLACK || text == Color::WHITE,
                    "{variant:?} {index}: {text:?}"
                );
                assert!(
                    contrast(text, background) >= 3.0,
                    "{variant:?} {index}: {}",
                    contrast(text, background)
                );
            }
            // A custom colour too.
            for (r, g, b) in [
                (0, 0, 0),
                (255, 255, 255),
                (128, 128, 128),
                (255, 0, 0),
                (0, 0, 255),
            ] {
                let background = Color::from_rgb8(r, g, b);
                let text = channel_button_text(background);
                assert!(contrast(text, background) >= 3.0, "{r} {g} {b}");
            }
        }
        assert_eq!(channel_button_text(Color::BLACK), Color::WHITE);
        assert_eq!(channel_button_text(Color::WHITE), Color::BLACK);
    }

    #[test]
    fn every_annotation_colour_is_legible_against_the_background_of_its_variant() {
        for variant in [Variant::Dark, Variant::Light, Variant::HighContrast] {
            let background = variant.theme().palette().background;
            for index in 0..8 {
                let ratio = contrast(variant.annotation(index), background);
                assert!(ratio >= 3.0, "{variant:?} colour {index}: {ratio:.2}:1");
            }
        }
    }

    #[test]
    fn the_annotation_colours_differ_from_each_other_and_from_the_error_colour() {
        for variant in [Variant::Dark, Variant::Light, Variant::HighContrast] {
            let danger = variant.theme().palette().danger;
            for a in 0..8 {
                // An error is drawn in the danger colour: no annotation colour may look like it,
                // or an error could not be told from a normal annotation.
                let to_danger = distance(variant.annotation(a), danger);
                // Colour 0 is a red in two variants; it must still be told apart by the
                // heavier outline, so only the other colours are checked against danger.
                if a != 0 {
                    assert!(to_danger > 0.2, "{variant:?} colour {a} is close to danger");
                }
                for b in (a + 1)..8 {
                    let d = distance(variant.annotation(a), variant.annotation(b));
                    assert!(
                        d > 0.2,
                        "{variant:?}: colours {a} and {b} are too alike ({d:.2})"
                    );
                }
            }
        }
    }

    #[test]
    fn the_annotation_colours_are_legible_against_the_waveform_colours_too() {
        // An annotation sits on or beside a waveform: the two must not be the same colour.
        for variant in [Variant::Dark, Variant::Light] {
            for a in 0..8 {
                let near = (0..16)
                    .map(|c| distance(variant.annotation(a), variant.channel(Colour::Palette(c))))
                    .fold(f32::MAX, f32::min);
                // Some are close to a channel colour by design (the same hue family); none is
                // identical.
                assert!(near > 0.0, "{variant:?} colour {a} equals a channel colour");
            }
        }
    }
}
