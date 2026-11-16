//! Finding the bus clock period from the signal (research R1).
//!
//! The intervals between consecutive rising edges of SCL go into a ring of the last
//! [`WINDOW`] intervals and into a histogram of quarter-octave bins (four bins to every doubling).
//! The period is known after the first interval for which three neighbouring bins hold at least
//! [`NEEDED`] of the intervals of the ring: it is then their mean. Inside a byte the rising edges of
//! SCL are one period apart, so any real traffic fills one bin quickly; noise is spread over many
//! bins and a long noisy start-up cannot keep the clock from being recognised once it comes, because
//! only the last intervals count. The check happens interval by interval, so the position at which
//! the period becomes known depends on the signal alone, never on how it was cut into chunks.
//!
//! The state is a few kilobytes and every operation is constant time. All arithmetic is integer.

/// How many of the latest intervals are looked at.
pub const WINDOW: usize = 32;
/// How many of them three neighbouring bins must hold.
pub const NEEDED: u32 = 16;
/// Quarter-octave bins: 4 per doubling of a 64-bit interval.
const BINS: usize = 256;

#[derive(Debug, Clone, Copy, Default)]
struct Bin {
    count: u32,
    sum: u128,
}

/// The estimator of the bus clock period.
#[derive(Debug, Clone)]
pub struct Period {
    bins: [Bin; BINS],
    ring: [u64; WINDOW],
    filled: usize,
    head: usize,
    previous: Option<u64>,
    found: Option<u64>,
}

impl Default for Period {
    fn default() -> Self {
        Self::new()
    }
}

/// The bin of an interval of at least 1: four to every doubling.
fn bin_of(interval: u64) -> usize {
    let octave = interval.ilog2();
    let sub = if octave >= 2 {
        (interval >> octave.saturating_sub(2)) & 3
    } else {
        (interval << 2_u32.saturating_sub(octave)) & 3
    };
    let index = usize::try_from(octave)
        .unwrap_or(0)
        .saturating_mul(4)
        .saturating_add(usize::try_from(sub).unwrap_or(0));
    index.min(BINS.saturating_sub(1))
}

impl Period {
    /// An estimator that has seen nothing.
    #[must_use]
    pub fn new() -> Self {
        Self {
            bins: [Bin::default(); BINS],
            ring: [0; WINDOW],
            filled: 0,
            head: 0,
            previous: None,
            found: None,
        }
    }

    /// The period, once it is known; it never changes after that.
    #[must_use]
    pub fn found(&self) -> Option<u64> {
        self.found
    }

    /// The position of the last rising edge given, if no break came after it.
    #[must_use]
    pub fn previous(&self) -> Option<u64> {
        self.previous
    }

    /// The next rising edge of SCL pairs with no earlier one (a gap in the capture came between).
    pub fn break_at_gap(&mut self) {
        self.previous = None;
    }

    /// Takes the position of a rising edge of SCL, in increasing order. Returns the period as soon
    /// as it is known (and on every call after that).
    pub fn edge(&mut self, position: u64) -> Option<u64> {
        if self.found.is_some() {
            return self.found;
        }
        let before = self.previous.replace(position)?;
        let interval = position.checked_sub(before).filter(|d| *d > 0)?;
        self.push(interval);
        self.check(bin_of(interval));
        self.found
    }

    fn bin_mut(&mut self, index: usize) -> Option<&mut Bin> {
        self.bins.get_mut(index)
    }

    fn push(&mut self, interval: u64) {
        if self.filled == WINDOW {
            let old = self.ring.get(self.head).copied().unwrap_or(0);
            if old > 0 {
                if let Some(bin) = self.bin_mut(bin_of(old)) {
                    bin.count = bin.count.saturating_sub(1);
                    bin.sum = bin.sum.saturating_sub(u128::from(old));
                }
            }
        } else {
            self.filled = self.filled.saturating_add(1);
        }
        if let Some(slot) = self.ring.get_mut(self.head) {
            *slot = interval;
        }
        self.head = self.head.saturating_add(1) % WINDOW;
        if let Some(bin) = self.bin_mut(bin_of(interval)) {
            bin.count = bin.count.saturating_add(1);
            bin.sum = bin.sum.saturating_add(u128::from(interval));
        }
    }

    /// Looks at the three windows of three bins that contain `bin` (the only ones that grew).
    fn check(&mut self, bin: usize) {
        let mut best: Option<(u32, u128)> = None;
        for centre in bin.saturating_sub(1)..=bin.saturating_add(1) {
            if centre >= BINS {
                continue;
            }
            let (mut count, mut sum) = (0_u32, 0_u128);
            for index in centre.saturating_sub(1)..=centre.saturating_add(1) {
                if let Some(b) = self.bins.get(index) {
                    count = count.saturating_add(b.count);
                    sum = sum.saturating_add(b.sum);
                }
            }
            if count >= NEEDED && best.is_none_or(|(c, _)| count > c) {
                best = Some((count, sum));
            }
        }
        if let Some((count, sum)) = best {
            let mean = sum.checked_div(u128::from(count)).unwrap_or(0);
            self.found = Some(u64::try_from(mean).unwrap_or(u64::MAX).max(1));
        }
    }
}
