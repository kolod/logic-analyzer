//! The compact coding of samples, in pieces: the same bytes as `la_proto::encode_compact` over the whole,
//! produced as the capture is read from memory in chunks and sent to the port in chunks.

/// The most bytes one sample can take in the compact stream: a distance of five bytes and the mask.
const fn worst_case(per: usize) -> usize {
    5_usize.saturating_add(per)
}

/// What a call to [`CompactEncoder::push`] did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Pushed {
    /// The bytes of `raw` that were taken (a whole number of samples).
    pub consumed: usize,
    /// The bytes written to the output.
    pub written: usize,
}

/// The compact coder of one capture.
#[derive(Debug, Clone)]
pub struct CompactEncoder {
    per: usize,
    previous: [u8; 4],
    started: bool,
    distance: u32,
}

impl CompactEncoder {
    /// A coder for samples of `per` bytes (1 to 4).
    #[must_use]
    pub fn new(per: usize) -> Option<Self> {
        (1..=4).contains(&per).then_some(Self {
            per,
            previous: [0; 4],
            started: false,
            distance: 0,
        })
    }

    /// Codes samples from `raw` (a whole number of samples) into `out`, as many as fit. Call again with the
    /// rest of `raw` (and an empty `out` after it was sent) while `consumed` is less than `raw.len()`.
    #[must_use]
    pub fn push(&mut self, raw: &[u8], out: &mut [u8]) -> Pushed {
        let (mut consumed, mut written) = (0_usize, 0_usize);
        for sample in raw.chunks_exact(self.per) {
            let room = out.len().saturating_sub(written);
            let needed = if self.started {
                worst_case(self.per)
            } else {
                self.per
            };
            if room < needed {
                break;
            }
            if self.started {
                self.distance = self.distance.saturating_add(1);
                if sample != self.previous.get(..self.per).unwrap_or(&[]) {
                    written = self.write_change(sample, out, written);
                }
            } else {
                if let Some(slot) = out.get_mut(written..written.saturating_add(self.per)) {
                    slot.copy_from_slice(sample);
                }
                written = written.saturating_add(self.per);
                self.started = true;
                self.distance = 0;
                self.remember(sample);
            }
            consumed = consumed.saturating_add(self.per);
        }
        Pushed { consumed, written }
    }

    /// Writes the distance and the mask of a change; returns the new length of the output.
    fn write_change(&mut self, sample: &[u8], out: &mut [u8], mut at: usize) -> usize {
        let mut value = self.distance;
        loop {
            let low = (value & 0x7F) as u8;
            value >>= 7;
            let byte = if value == 0 { low } else { low | 0x80 };
            if let Some(slot) = out.get_mut(at) {
                *slot = byte;
            }
            at = at.saturating_add(1);
            if value == 0 {
                break;
            }
        }
        for (old, new) in self.previous.iter().zip(sample) {
            if let Some(slot) = out.get_mut(at) {
                *slot = old ^ new;
            }
            at = at.saturating_add(1);
        }
        self.remember(sample);
        self.distance = 0;
        at
    }

    fn remember(&mut self, sample: &[u8]) {
        for (slot, byte) in self.previous.iter_mut().zip(sample) {
            *slot = *byte;
        }
    }
}
