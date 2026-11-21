//! The visible window of the signal view and its zoom and pan arithmetic.
//!
//! A window is a span of samples `(start, length)`. Every operation is total: it never fails and
//! always returns a window that satisfies the invariants (see [`ViewWindow::is_valid`]), whatever
//! its arguments. Arithmetic is widened to 128 bits so nothing overflows at the top of the `u64`
//! range.

/// The widest zoom: at most this many pixels show one sample.
pub const MAX_PIXELS_PER_SAMPLE: u32 = 64;

/// The width of the signal area, as the host needs it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ViewSize {
    /// Width of the signal area in physical pixels.
    pub pixels: u32,
    /// The display scale factor (1.0 is 96 dpi).
    pub scale: f32,
}

impl Default for ViewSize {
    fn default() -> Self {
        Self {
            pixels: 800,
            scale: 1.0,
        }
    }
}

/// What bounds a window: the capture length and the number of pixels it is shown on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WindowLimits {
    /// Number of positions of the capture (0 for an empty or absent capture).
    pub capture_length: u64,
    /// Width of the signal area in pixels.
    pub pixels: u32,
}

impl WindowLimits {
    /// The shortest window: at most [`MAX_PIXELS_PER_SAMPLE`] pixels per sample, never longer
    /// than a non-empty capture, never shorter than one sample.
    #[must_use]
    pub fn min_length(&self) -> u64 {
        let by_pixels = u64::from(self.pixels)
            .div_ceil(u64::from(MAX_PIXELS_PER_SAMPLE))
            .max(1);
        if self.capture_length == 0 {
            by_pixels
        } else {
            by_pixels.min(self.capture_length)
        }
    }

    /// The longest window: the whole capture (the nominal minimum for an empty capture).
    #[must_use]
    pub fn max_length(&self) -> u64 {
        if self.capture_length == 0 {
            self.min_length()
        } else {
            self.capture_length
        }
    }

    /// The largest start for a window of `length` samples.
    fn max_start(&self, length: u64) -> u64 {
        self.capture_length.saturating_sub(length)
    }
}

/// The visible span of samples.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ViewWindow {
    /// First visible sample.
    pub start: u64,
    /// Number of visible samples.
    pub length: u64,
}

/// `value * numerator / denominator` in 128 bits, saturated to `u64`. A zero denominator gives
/// the value unchanged.
fn scale(value: u64, numerator: u64, denominator: u64) -> u64 {
    if denominator == 0 {
        return value;
    }
    let scaled = u128::from(value)
        .saturating_mul(u128::from(numerator))
        .checked_div(u128::from(denominator))
        .unwrap_or(0);
    u64::try_from(scaled).unwrap_or(u64::MAX)
}

impl ViewWindow {
    /// The window that shows the whole capture.
    #[must_use]
    pub fn fit_all(limits: &WindowLimits) -> Self {
        Self {
            start: 0,
            length: limits.max_length(),
        }
    }

    /// Whether the window satisfies the invariants: its length is within
    /// `[min_length, max_length]` and it lies inside the capture.
    #[must_use]
    pub fn is_valid(&self, limits: &WindowLimits) -> bool {
        if self.length < limits.min_length() || self.length > limits.max_length() {
            return false;
        }
        if limits.capture_length == 0 {
            return self.start == 0;
        }
        self.start
            .checked_add(self.length)
            .is_some_and(|end| end <= limits.capture_length)
    }

    /// The nearest valid window.
    #[must_use]
    pub fn clamp(self, limits: &WindowLimits) -> Self {
        let length = self.length.clamp(limits.min_length(), limits.max_length());
        Self {
            start: self.start.min(limits.max_start(length)),
            length,
        }
    }

    /// One past the last visible sample.
    #[must_use]
    pub fn end(&self) -> u64 {
        self.start.saturating_add(self.length)
    }

    /// Changes the length by the factor `numerator / denominator` (below 1 zooms in) and keeps
    /// the sample at `anchor = (p, q)` — the fraction `p / q` of the window — at the same
    /// fraction of the new window.
    #[must_use]
    pub fn zoom(
        self,
        limits: &WindowLimits,
        numerator: u64,
        denominator: u64,
        anchor: (u64, u64),
    ) -> Self {
        if numerator == 0 || denominator == 0 {
            return self.clamp(limits);
        }
        let (p, q) = if anchor.1 == 0 {
            (1, 2)
        } else {
            (anchor.0.min(anchor.1), anchor.1)
        };
        let length = scale(self.length, numerator, denominator)
            .clamp(limits.min_length(), limits.max_length());
        let anchor_sample = self.start.saturating_add(scale(self.length, p, q));
        let start = anchor_sample.saturating_sub(scale(length, p, q));
        Self { start, length }.clamp(limits)
    }

    /// Moves the window by `delta` samples, staying inside the capture.
    #[must_use]
    pub fn pan_samples(self, limits: &WindowLimits, delta: i64) -> Self {
        let moved = i128::from(self.start).saturating_add(i128::from(delta));
        let max_start = i128::from(limits.max_start(self.length));
        let start = u64::try_from(moved.clamp(0, max_start)).unwrap_or(0);
        Self {
            start,
            length: self.length,
        }
        .clamp(limits)
    }

    /// Moves the window by `delta_px` pixels of the current zoom.
    #[must_use]
    pub fn pan_pixels(self, limits: &WindowLimits, delta_px: i64) -> Self {
        if limits.pixels == 0 {
            return self.clamp(limits);
        }
        let samples = i128::from(delta_px)
            .saturating_mul(i128::from(self.length))
            .checked_div(i128::from(limits.pixels))
            .unwrap_or(0);
        self.pan_samples(
            limits,
            i64::try_from(samples).unwrap_or(if samples < 0 { i64::MIN } else { i64::MAX }),
        )
    }

    /// The same length, placed so that `edge` is `offset` samples from the left side (clamped
    /// to the capture; a window longer than the capture starts at 0).
    #[must_use]
    pub fn with_edge_at(self, limits: &WindowLimits, edge: u64, offset: u64) -> Self {
        Self {
            start: edge.saturating_sub(offset),
            length: self.length,
        }
        .clamp(limits)
    }

    /// The same length at the start of the capture.
    #[must_use]
    pub fn go_to_start(self, limits: &WindowLimits) -> Self {
        Self {
            start: 0,
            length: self.length,
        }
        .clamp(limits)
    }

    /// The same length at the end of the capture.
    #[must_use]
    pub fn go_to_end(self, limits: &WindowLimits) -> Self {
        Self {
            start: u64::MAX,
            length: self.length,
        }
        .clamp(limits)
    }

    /// The window that shows the samples from `a` to `b` (in either order); a range shorter
    /// than the minimum is widened around its centre.
    #[must_use]
    pub fn zoom_to(limits: &WindowLimits, a: u64, b: u64) -> Self {
        let (mut low, mut high) = (a.min(b), a.max(b));
        if limits.capture_length > 0 {
            low = low.min(limits.capture_length);
            high = high.min(limits.capture_length);
        }
        let length = high.saturating_sub(low);
        if length >= limits.min_length() {
            return Self { start: low, length }.clamp(limits);
        }
        let minimum = limits.min_length();
        let centre = low.saturating_add(length / 2);
        Self {
            start: centre.saturating_sub(minimum / 2),
            length: minimum,
        }
        .clamp(limits)
    }

    /// The window of the same length whose centre is `sample`, as near as the capture allows (a
    /// window never extends beyond the capture).
    #[must_use]
    pub fn centre_on(self, limits: &WindowLimits, sample: u64) -> Self {
        Self {
            start: sample.saturating_sub(self.length / 2),
            length: self.length,
        }
        .clamp(limits)
    }

    /// The window that shows the samples from `a` to `b` (in either order) with a margin of
    /// `margin_px` pixels on each side, so that something `margin_px` wide drawn beside each end
    /// fits. Where the capture ends before the margin does, that side gets what the capture has.
    /// Never narrower than the zoom limit allows.
    ///
    /// The margin is in pixels, so it depends on the zoom it produces: with `n = b - a` samples and
    /// `W` pixels, the window of `L` samples has `margin_px · L / W` samples of margin on each
    /// side and `L = n + 2 · margin_px · L / W`, that is `L = n · W / (W - 2 · margin_px)`.
    #[must_use]
    pub fn fit_range(limits: &WindowLimits, a: u64, b: u64, margin_px: u32) -> Self {
        let (low, high) = (a.min(b), a.max(b));
        let span = high.saturating_sub(low);
        let pixels = u64::from(limits.pixels);
        let room = pixels.saturating_sub(u64::from(margin_px).saturating_mul(2));
        if room == 0 || span == 0 {
            return Self::zoom_to(limits, low, high);
        }
        // Rounded up, so the margin is never a hair under `margin_px`.
        let wanted = u128::from(span)
            .saturating_mul(u128::from(pixels))
            .div_ceil(u128::from(room));
        let length = u64::try_from(wanted)
            .unwrap_or(u64::MAX)
            .max(limits.min_length());
        let margin = length.saturating_sub(span) / 2;
        Self {
            start: low.saturating_sub(margin),
            length,
        }
        .clamp(limits)
    }

    /// Moves the window, if needed, so that `sample` is visible (centred when it was outside).
    #[must_use]
    pub fn reveal(self, limits: &WindowLimits, sample: u64) -> Self {
        if sample >= self.start && sample < self.end() {
            return self;
        }
        Self {
            start: sample.saturating_sub(self.length / 2),
            length: self.length,
        }
        .clamp(limits)
    }
}
