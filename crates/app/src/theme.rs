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

fn rgb(r: u8, g: u8, b: u8) -> Color {
    Color::from_rgb8(r, g, b)
}

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
