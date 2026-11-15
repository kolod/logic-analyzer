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

    /// Makes the list exactly `channel_count` long. What the user set for the channels that stay
    /// (name, colour, order) is kept; a new channel is shown when `enabled_mask` records it.
    pub fn resize(&mut self, channel_count: u8, enabled_mask: u32) {
        self.channels.retain(|s| s.index < channel_count);
        for index in 0..channel_count {
            if !self.channels.iter().any(|s| s.index == index) {
                self.channels.push(ChannelStyle {
                    index,
                    name: String::new(),
                    colour: Colour::Palette(index % PALETTE_SIZE),
                    visible: u32::from(index) < 32 && enabled_mask >> u32::from(index) & 1 == 1,
                });
            }
        }
    }

    /// Sets the flag of every channel below `limit` from the recording mask: the one flag of a
    /// channel means both "shown" and "recorded by the next capture". Channels at or above the
    /// limit (a capture opened from a file made with a bigger device) keep their flag.
    pub fn visible_from_mask(&mut self, enabled_mask: u32, limit: u8) {
        for style in self.channels.iter_mut().filter(|s| s.index < limit) {
            style.visible =
                u32::from(style.index) < 32 && enabled_mask >> u32::from(style.index) & 1 == 1;
        }
    }

    /// The channels whose flag is on, as a mask.
    #[must_use]
    pub fn visible_mask(&self) -> u32 {
        self.channels
            .iter()
            .filter(|s| s.visible && s.index < 32)
            .fold(0, |mask, s| {
                mask | 1_u32.checked_shl(u32::from(s.index)).unwrap_or(0)
            })
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
