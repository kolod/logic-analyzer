//! The `VIEW` section of a session file: how the capture was shown (channel names, colours,
//! order and visibility, markers, the window).
//!
//! The format is `specs/006-ui-shell-signal-view/contracts/view-section.md`. The reader treats
//! every byte as untrusted and accepts only the canonical form, so an accepted payload always
//! re-encodes to the same bytes. Rejection never fails the opening of a file: the caller shows
//! the capture with defaults and a warning.

use la_core::Result;

use crate::error;
use crate::markers::{MAX_MARKERS, Marker, Markers};
use crate::presentation::{ChannelStyle, Colour, PALETTE_SIZE, Presentation};
use crate::sanitize::{MAX_NAME_CHARS, sanitize_text};

/// The section tag.
pub const TAG: &str = "VIEW";
/// The version of the payload this code reads and writes.
pub const VERSION: u8 = 1;
/// The longest name in bytes.
pub const MAX_NAME_BYTES: u64 = 256;
/// The most channels a payload can describe.
pub const MAX_CHANNELS: u64 = 32;

const FLAG_VISIBLE: u8 = 0b01;
const FLAG_CUSTOM_COLOUR: u8 = 0b10;

/// What the payload is checked against: the capture it belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Bounds {
    /// The number of channels of the capture.
    pub channels: u8,
    /// The length of the capture in samples.
    pub length: u64,
}

/// The state stored in a `VIEW` section.
#[derive(Debug, Clone, PartialEq)]
pub struct ViewState {
    /// Names, colours, order and visibility; `None` when none are stored (the defaults apply).
    pub presentation: Option<Presentation>,
    /// The markers and the measured pair.
    pub markers: Markers,
    /// The remembered window as `(start, length)`; `None` when there is none.
    pub window: Option<(u64, u64)>,
    /// Whether the view followed the newest data.
    pub follow_latest: bool,
}

/// The outcome of reading a payload that is not invalid.
#[derive(Debug, Clone, PartialEq)]
pub enum Decoded {
    /// A state this version understands.
    View(ViewState),
    /// A newer version of the section: kept as it is, not interpreted.
    Newer,
}

// ---- writer ---------------------------------------------------------------------------------

fn put_varint(mut value: u64, out: &mut Vec<u8>) {
    loop {
        let byte = u8::try_from(value & 0x7f).unwrap_or(0);
        value >>= 7;
        if value == 0 {
            out.push(byte);
            return;
        }
        out.push(byte | 0x80);
    }
}

fn put_name(name: &str, out: &mut Vec<u8>) {
    // The host keeps names within the limit; this guard makes a longer one harmless.
    let clean = sanitize_text(name, MAX_NAME_CHARS);
    put_varint(clean.len() as u64, out);
    out.extend_from_slice(clean.as_bytes());
}

/// The payload of `state`. Deterministic: the same state gives the same bytes.
#[must_use]
pub fn encode(state: &ViewState) -> Vec<u8> {
    let mut out = vec![VERSION];
    match &state.presentation {
        Some(presentation) => {
            put_varint(presentation.channels.len() as u64, &mut out);
            for style in &presentation.channels {
                put_varint(u64::from(style.index), &mut out);
                match style.colour {
                    Colour::Palette(index) => {
                        out.push(if style.visible { FLAG_VISIBLE } else { 0 });
                        out.push(index);
                    }
                    Colour::Rgb(r, g, b) => {
                        out.push(FLAG_CUSTOM_COLOUR | if style.visible { FLAG_VISIBLE } else { 0 });
                        out.extend_from_slice(&[r, g, b]);
                    }
                }
                put_name(&style.name, &mut out);
            }
        }
        None => put_varint(0, &mut out),
    }
    put_varint(state.markers.list.len() as u64, &mut out);
    for marker in &state.markers.list {
        put_varint(u64::from(marker.id), &mut out);
        put_varint(marker.position, &mut out);
        put_name(&marker.name, &mut out);
    }
    for slot in state.markers.selected {
        put_varint(
            slot.map_or(0, |id| u64::from(id).saturating_add(1)),
            &mut out,
        );
    }
    let (start, length) = state.window.unwrap_or((0, 0));
    put_varint(start, &mut out);
    put_varint(length, &mut out);
    out.push(u8::from(state.follow_latest));
    out
}

// ---- reader ---------------------------------------------------------------------------------

struct Reader<'a> {
    data: &'a [u8],
    at: usize,
}

fn invalid<T>(detail: &str) -> Result<T> {
    Err(error::view_invalid(detail))
}

impl<'a> Reader<'a> {
    fn byte(&mut self) -> Result<u8> {
        let Some(&b) = self.data.get(self.at) else {
            return invalid("the section ends too early");
        };
        self.at = self.at.saturating_add(1);
        Ok(b)
    }

    fn bytes(&mut self, count: u64) -> Result<&'a [u8]> {
        let count = usize::try_from(count).or_else(|_| invalid("a length does not fit"))?;
        let end = self.at.saturating_add(count);
        let Some(slice) = self.data.get(self.at..end) else {
            return invalid("the section ends too early");
        };
        self.at = end;
        Ok(slice)
    }

    /// A canonical unsigned LEB128 varint: at most 10 bytes, no redundant final zero group.
    fn varint(&mut self) -> Result<u64> {
        let mut value = 0_u64;
        for count in 0..10_u32 {
            let byte = self.byte()?;
            if count == 9 && byte > 1 {
                return invalid("a varint does not fit 64 bits");
            }
            value |= u64::from(byte & 0x7f)
                .checked_shl(count.saturating_mul(7))
                .unwrap_or(0);
            if byte & 0x80 == 0 {
                if byte == 0 && count > 0 {
                    return invalid("a varint is not in canonical form");
                }
                return Ok(value);
            }
        }
        invalid("a varint is longer than 10 bytes")
    }

    fn name(&mut self) -> Result<String> {
        let len = self.varint()?;
        if len > MAX_NAME_BYTES {
            return invalid("a name is too long");
        }
        let raw = self.bytes(len)?;
        let Ok(text) = std::str::from_utf8(raw) else {
            return invalid("a name is not valid UTF-8");
        };
        // Only text that is already in the form the writer produces is accepted, so that an
        // accepted payload re-encodes to identical bytes.
        if sanitize_text(text, MAX_NAME_CHARS) != text {
            return invalid("a name is not in its sanitized form");
        }
        Ok(text.to_owned())
    }
}

fn read_presentation(r: &mut Reader<'_>, bounds: Bounds) -> Result<Option<Presentation>> {
    let count = r.varint()?;
    if count == 0 {
        return Ok(None);
    }
    if count > MAX_CHANNELS || count != u64::from(bounds.channels) {
        return invalid("the channel count does not match the capture");
    }
    let mut seen = 0_u32;
    let mut channels = Vec::new();
    for _ in 0..count {
        let index = r.varint()?;
        if index >= u64::from(bounds.channels) {
            return invalid("a channel index is beyond the capture");
        }
        let index = u8::try_from(index).or_else(|_| invalid("a channel index does not fit"))?;
        let bit = 1_u32.checked_shl(u32::from(index)).unwrap_or(0);
        if seen & bit != 0 {
            return invalid("a channel index repeats");
        }
        seen |= bit;
        let flags = r.byte()?;
        if flags & !(FLAG_VISIBLE | FLAG_CUSTOM_COLOUR) != 0 {
            return invalid("a reserved flag bit is set");
        }
        let colour = if flags & FLAG_CUSTOM_COLOUR != 0 {
            Colour::Rgb(r.byte()?, r.byte()?, r.byte()?)
        } else {
            let palette = r.byte()?;
            if palette >= PALETTE_SIZE {
                return invalid("a palette index is out of range");
            }
            Colour::Palette(palette)
        };
        channels.push(ChannelStyle {
            index,
            name: r.name()?,
            colour,
            visible: flags & FLAG_VISIBLE != 0,
        });
    }
    Ok(Some(Presentation { channels }))
}

fn read_markers(r: &mut Reader<'_>, bounds: Bounds) -> Result<Markers> {
    let count = r.varint()?;
    if count > MAX_MARKERS as u64 {
        return invalid("there are too many markers");
    }
    let limit = bounds.length.max(1);
    let mut list: Vec<Marker> = Vec::new();
    for _ in 0..count {
        let id = r.varint()?;
        let id = u32::try_from(id).or_else(|_| invalid("a marker id does not fit"))?;
        if list.iter().any(|m| m.id == id) {
            return invalid("a marker id repeats");
        }
        let position = r.varint()?;
        if position >= limit {
            return invalid("a marker is beyond the capture");
        }
        list.push(Marker {
            id,
            position,
            name: r.name()?,
        });
    }
    let mut selected = [None, None];
    for slot in &mut selected {
        let value = r.varint()?;
        if value == 0 {
            continue;
        }
        let id = u32::try_from(value.saturating_sub(1))
            .or_else(|_| invalid("a selected marker id does not fit"))?;
        if !list.iter().any(|m| m.id == id) {
            return invalid("a selected marker does not exist");
        }
        *slot = Some(id);
    }
    Ok(Markers::restore(list, selected))
}

/// Reads a payload belonging to a capture with the given `bounds`.
///
/// # Errors
///
/// `host.view_invalid` for anything that breaks a rule of the contract.
pub fn decode(payload: &[u8], bounds: Bounds) -> Result<Decoded> {
    let mut r = Reader {
        data: payload,
        at: 0,
    };
    let version = r.byte()?;
    if version > VERSION {
        return Ok(Decoded::Newer);
    }
    if version != VERSION {
        return invalid("the section version is not 1");
    }
    let presentation = read_presentation(&mut r, bounds)?;
    let markers = read_markers(&mut r, bounds)?;
    let start = r.varint()?;
    let length = r.varint()?;
    let window = match (start, length) {
        (0, 0) => None,
        (_, 0) => return invalid("the window has no length"),
        (start, length) => {
            if start
                .checked_add(length)
                .is_none_or(|end| end > bounds.length)
            {
                return invalid("the window is beyond the capture");
            }
            Some((start, length))
        }
    };
    let follow_latest = match r.byte()? {
        0 => false,
        1 => true,
        _ => return invalid("the follow flag is not 0 or 1"),
    };
    if r.at != payload.len() {
        return invalid("bytes remain after the section");
    }
    Ok(Decoded::View(ViewState {
        presentation,
        markers,
        window,
        follow_latest,
    }))
}
