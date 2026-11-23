//! Framing of host-to-device messages: `55 AA <body> AA 55`, with `AA` sent as `F0 5A` and `F0` as `F0 00`
//! inside the body.

/// The longest frame after unescaping, with its four marker bytes.
pub const MAX_FRAME_LEN: usize = 256;
/// The longest body (the command and its payload) of a frame.
pub const MAX_BODY_LEN: usize = MAX_FRAME_LEN - 4;

const START: [u8; 2] = [0x55, 0xAA];
const END: [u8; 2] = [0xAA, 0x55];
const ESCAPE: u8 = 0xF0;

/// Why a frame could not be built.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameError {
    /// The body is longer than [`MAX_BODY_LEN`].
    BodyTooLong,
    /// The output buffer cannot hold the frame.
    OutputTooSmall,
}

impl core::fmt::Display for FrameError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(match self {
            Self::BodyTooLong => "the frame body is longer than 252 bytes",
            Self::OutputTooSmall => "the output buffer is too small for the frame",
        })
    }
}

#[cfg(feature = "std")]
impl std::error::Error for FrameError {}

/// Writes the frame of `body` (the command byte and its payload) into `out` and returns its length.
///
/// # Errors
///
/// [`FrameError::BodyTooLong`] for a body over [`MAX_BODY_LEN`], [`FrameError::OutputTooSmall`] if `out` is
/// too short (at most `4 + 2 * body.len()` bytes are needed).
pub fn encode_frame(body: &[u8], out: &mut [u8]) -> Result<usize, FrameError> {
    if body.len() > MAX_BODY_LEN {
        return Err(FrameError::BodyTooLong);
    }
    let mut position = 0_usize;
    let mut put = |bytes: &[u8]| -> Result<(), FrameError> {
        let end = position
            .checked_add(bytes.len())
            .ok_or(FrameError::OutputTooSmall)?;
        out.get_mut(position..end)
            .ok_or(FrameError::OutputTooSmall)?
            .copy_from_slice(bytes);
        position = end;
        Ok(())
    };
    put(&START)?;
    for &byte in body {
        match byte {
            0xAA => put(&[ESCAPE, 0x5A])?,
            ESCAPE => put(&[ESCAPE, 0x00])?,
            other => put(&[other])?,
        }
    }
    put(&END)?;
    Ok(position)
}

/// What the parser found.
#[derive(Debug, PartialEq, Eq)]
pub enum Event<'a> {
    /// A whole frame: its unescaped body (the command byte first).
    Frame(&'a [u8]),
    /// A frame longer than [`MAX_BODY_LEN`] (`ERR_MSG_OVERFLOW`); it was dropped.
    Overflow,
    /// A damaged frame (a bad escape, or an end that did not follow); it was dropped.
    Damaged,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum State {
    /// Outside a frame.
    Idle,
    /// Saw `55`.
    Start1,
    /// Inside the body.
    Body,
    /// Saw `F0` in the body.
    Escaped,
    /// Saw a raw `AA` in the body; `55` ends the frame. `ambiguous`: the byte before it was `55`, so it
    /// may also have been the start of a new frame.
    End1 { ambiguous: bool },
}

/// A byte-at-a-time parser of frames. Fed the same bytes in any chunking it gives the same events; it never
/// panics and never holds more than one body ([`MAX_BODY_LEN`] bytes).
pub struct FrameParser {
    body: [u8; MAX_BODY_LEN],
    len: usize,
    state: State,
    overflowed: bool,
    bad: bool,
}

impl Default for FrameParser {
    fn default() -> Self {
        Self::new()
    }
}

impl FrameParser {
    /// A parser outside any frame.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            body: [0; MAX_BODY_LEN],
            len: 0,
            state: State::Idle,
            overflowed: false,
            bad: false,
        }
    }

    /// Forgets a frame in progress.
    pub fn reset(&mut self) {
        self.state = State::Idle;
        self.len = 0;
        self.overflowed = false;
        self.bad = false;
    }

    fn begin(&mut self) {
        self.len = 0;
        self.overflowed = false;
        self.bad = false;
        self.state = State::Body;
    }

    fn store(&mut self, byte: u8) {
        match self.body.get_mut(self.len) {
            Some(slot) => {
                *slot = byte;
                self.len = self.len.saturating_add(1);
            }
            None => self.overflowed = true,
        }
    }

    /// Feeds one byte; returns what it completed, if anything.
    pub fn push(&mut self, byte: u8) -> Option<Event<'_>> {
        match self.state {
            State::Idle => {
                if byte == START[0] {
                    self.state = State::Start1;
                }
                None
            }
            State::Start1 => {
                if byte == START[1] {
                    self.begin();
                } else if byte != START[0] {
                    self.state = State::Idle;
                }
                None
            }
            State::Body => {
                if byte == ESCAPE {
                    self.state = State::Escaped;
                } else if byte == END[0] {
                    let ambiguous = self.len > 0
                        && !self.overflowed
                        && self.body.get(self.len.saturating_sub(1)) == Some(&START[0]);
                    self.state = State::End1 { ambiguous };
                } else {
                    self.store(byte);
                }
                None
            }
            State::Escaped => {
                match byte {
                    0x5A => self.store(0xAA),
                    0x00 => self.store(ESCAPE),
                    _ => self.bad = true,
                }
                self.state = State::Body;
                None
            }
            State::End1 { ambiguous } => {
                if byte == END[1] {
                    self.state = State::Idle;
                    return Some(self.finish());
                }
                // The `AA` did not end the frame: this frame is damaged. If a `55` came just before it,
                // those two bytes were the start of the next frame, whose first body byte is this one.
                if ambiguous {
                    self.begin();
                    let _ = self.push(byte);
                } else {
                    self.state = if byte == START[0] {
                        State::Start1
                    } else {
                        State::Idle
                    };
                }
                Some(Event::Damaged)
            }
        }
    }

    fn finish(&mut self) -> Event<'_> {
        if self.bad {
            Event::Damaged
        } else if self.overflowed {
            Event::Overflow
        } else {
            Event::Frame(self.body.get(..self.len).unwrap_or(&[]))
        }
    }

    /// Feeds `bytes` and calls `on_event` for each completed frame, overflow or damaged frame.
    pub fn feed(&mut self, bytes: &[u8], mut on_event: impl FnMut(Event<'_>)) {
        for &byte in bytes {
            if let Some(event) = self.push(byte) {
                on_event(event);
            }
        }
    }
}
