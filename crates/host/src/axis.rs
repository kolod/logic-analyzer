//! The time axis: tick step selection and tick positions (research R11).
//!
//! Ticks are placed at multiples of a step taken from {1, 2, 5} × 10^k seconds, at least
//! [`MIN_TICK_SPACING_PX`] pixels apart. The time of a tick is the exact decimal number (parsed
//! from text such as `3e-1`), so `0.3` is `0.3` and never `0.30000000000000004`.

use crate::window::ViewWindow;

/// The closest two ticks may be, in pixels.
pub const MIN_TICK_SPACING_PX: f64 = 80.0;

/// The most ticks one axis ever has (a safety bound; the spacing allows far fewer).
const MAX_TICKS: usize = 200;

/// A tick step: `mantissa × 10^exponent` seconds, with the mantissa 1, 2 or 5.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TickStep {
    /// 1, 2 or 5.
    pub mantissa: u8,
    /// The power of ten, in seconds.
    pub exponent: i32,
}

impl TickStep {
    /// The step in seconds.
    #[must_use]
    pub fn seconds(&self) -> f64 {
        decimal(i128::from(self.mantissa), self.exponent)
    }
}

/// One tick of the axis.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Tick {
    /// Pixel position within the signal area.
    pub x: f64,
    /// Time of the tick in seconds from the start of the capture (an exact decimal number).
    pub seconds: f64,
    /// A tick at a multiple of the next decade (longer mark, always labelled).
    pub major: bool,
}

/// The axis of one frame.
#[derive(Debug, Clone, PartialEq)]
pub struct AxisPlan {
    /// The chosen step.
    pub step: TickStep,
    /// The ticks, left to right, from the first at or after the window start to the last at or
    /// before its end.
    pub ticks: Vec<Tick>,
}

impl AxisPlan {
    /// An axis without ticks.
    #[must_use]
    pub fn empty() -> Self {
        Self {
            step: TickStep {
                mantissa: 1,
                exponent: 0,
            },
            ticks: Vec::new(),
        }
    }
}

/// The decimal number `mantissa × 10^exponent` as the nearest `f64`.
fn decimal(mantissa: i128, exponent: i32) -> f64 {
    format!("{mantissa}e{exponent}").parse().unwrap_or(0.0)
}

/// Chooses the smallest step whose spacing is at least [`MIN_TICK_SPACING_PX`].
fn choose_step(seconds_per_pixel: f64) -> TickStep {
    let minimum = MIN_TICK_SPACING_PX * seconds_per_pixel * (1.0 - 1e-9);
    // Start two decades below the minimum and walk up.
    let first_exponent = minimum.log10().floor();
    let first_exponent = if first_exponent.is_finite() {
        first_exponent.clamp(-60.0, 60.0) as i32
    } else {
        0
    };
    for exponent in first_exponent.saturating_sub(1)..=first_exponent.saturating_add(1) {
        for mantissa in [1_u8, 2, 5] {
            let step = TickStep { mantissa, exponent };
            if step.seconds() >= minimum {
                return step;
            }
        }
    }
    TickStep {
        mantissa: 1,
        exponent: first_exponent.saturating_add(2),
    }
}

/// The ticks of `window` shown on `pixels` pixels at `sample_rate_hz`.
#[must_use]
pub fn build_ticks(window: &ViewWindow, pixels: u32, sample_rate_hz: u64) -> AxisPlan {
    if pixels == 0 || sample_rate_hz == 0 || window.length == 0 {
        return AxisPlan::empty();
    }
    let rate = sample_rate_hz as f64;
    let width = f64::from(pixels);
    let length = window.length as f64;
    let start_time = window.start as f64 / rate;
    let end_time = window.end() as f64 / rate;
    let seconds_per_pixel = length / rate / width;
    let step = choose_step(seconds_per_pixel);
    let step_seconds = step.seconds();
    if step_seconds <= 0.0 {
        return AxisPlan::empty();
    }
    let first = (start_time / step_seconds - 1e-9).ceil();
    let last = (end_time / step_seconds + 1e-9).floor();
    // Beyond 2^53 steps from zero the index is no longer exact; draw no ticks rather than wrong ones.
    if !(first.is_finite() && last.is_finite()) || first.abs() > 9.0e15 || last.abs() > 9.0e15 {
        return AxisPlan {
            step,
            ticks: Vec::new(),
        };
    }
    let (first, last) = (first as i64, last as i64);
    let mut ticks = Vec::new();
    let mut index = first;
    while index <= last && ticks.len() < MAX_TICKS {
        let multiple = i128::from(index).saturating_mul(i128::from(step.mantissa));
        let seconds = decimal(multiple, step.exponent);
        let sample = seconds * rate;
        let x = (sample - window.start as f64) * (width / length);
        ticks.push(Tick {
            x,
            seconds,
            major: multiple.rem_euclid(10) == 0,
        });
        index = index.saturating_add(1);
    }
    AxisPlan { step, ticks }
}
