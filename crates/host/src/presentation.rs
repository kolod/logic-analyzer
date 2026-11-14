//! How channels are shown: names, colours, order and visibility.
//!
//! The presentation is the user's choice, kept apart from the signal data: the capture is never
//! changed by it. Names are user text: they are sanitized for display and never translated.

use crate::sanitize::{MAX_NAME_CHARS, sanitize_text};

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

/// How one channel is shown.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChannelStyle {
    /// The channel number in the capture.
    pub index: u8,
    /// The user's name for it (sanitized, possibly empty: the interface then shows its default
    /// label).
    pub name: String,
    /// The colour of its waveform.
    pub colour: Colour,
    /// Whether it is shown.
    pub visible: bool,
}

/// The presentation of every channel of a capture, in display order.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Presentation {
    /// One entry per channel of the capture, in display order.
    pub channels: Vec<ChannelStyle>,
}

impl Presentation {
    /// The defaults for a capture of `channel_count` channels of which those in `enabled_mask`
    /// were enabled: index order, palette colours cycled, only the enabled channels shown.
    #[must_use]
    pub fn defaults(channel_count: u8, enabled_mask: u32) -> Self {
        let channels = (0..channel_count)
            .map(|index| ChannelStyle {
                index,
                name: String::new(),
                colour: Colour::Palette(index % PALETTE_SIZE),
                visible: u32::from(index) < 32 && enabled_mask >> u32::from(index) & 1 == 1,
            })
            .collect();
        Self { channels }
    }

    fn style_mut(&mut self, channel: u8) -> Option<&mut ChannelStyle> {
        self.channels.iter_mut().find(|s| s.index == channel)
    }

    /// Renames a channel (the name is sanitized). `false` if the channel does not exist.
    pub fn rename(&mut self, channel: u8, name: &str) -> bool {
        self.style_mut(channel)
            .map(|s| {
                s.name = sanitize_text(name, MAX_NAME_CHARS);
            })
            .is_some()
    }

    /// Changes the colour of a channel. `false` if the channel does not exist.
    pub fn set_colour(&mut self, channel: u8, colour: Colour) -> bool {
        let colour = match colour {
            Colour::Palette(index) => Colour::Palette(index % PALETTE_SIZE),
            rgb @ Colour::Rgb(..) => rgb,
        };
        self.style_mut(channel).map(|s| s.colour = colour).is_some()
    }

    /// Shows or hides a channel. `false` if the channel does not exist.
    pub fn set_visible(&mut self, channel: u8, visible: bool) -> bool {
        self.style_mut(channel)
            .map(|s| s.visible = visible)
            .is_some()
    }

    /// Moves the entry at position `from` of the order to position `to`. `false` if either is
    /// outside the order.
    pub fn move_channel(&mut self, from: usize, to: usize) -> bool {
        if from >= self.channels.len() || to >= self.channels.len() {
            return false;
        }
        let style = self.channels.remove(from);
        self.channels.insert(to, style);
        true
    }
}
