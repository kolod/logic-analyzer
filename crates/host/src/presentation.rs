//! How channels are shown: names, colours, order and visibility.
//!
//! Only the colour type exists so far; the presentation itself arrives with the signal-view work.

/// The number of colours of the built-in palette.
pub const PALETTE_SIZE: u8 = 16;

/// The colour of a channel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Colour {
    /// An entry of the built-in palette (`0..PALETTE_SIZE`), which the theme maps to a colour
    /// that is legible on light, dark and high-contrast backgrounds.
    Palette(u8),
    /// A colour the user picked.
    Rgb(u8, u8, u8),
}
