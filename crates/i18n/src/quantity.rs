//! Scaling of frequencies, sizes and durations to a unit.
//!
//! Only the arithmetic lives here; the symbol of each unit, and the space between the value and
//! the symbol, come from the resources (`unit-*` messages).

/// A unit a quantity is written in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Unit {
    Hz,
    KHz,
    MHz,
    GHz,
    B,
    KiB,
    MiB,
    GiB,
    TiB,
    Ns,
    Us,
    Ms,
    S,
    Min,
    H,
}

impl Unit {
    /// Every unit.
    pub(crate) const ALL: [Unit; 15] = [
        Unit::Hz,
        Unit::KHz,
        Unit::MHz,
        Unit::GHz,
        Unit::B,
        Unit::KiB,
        Unit::MiB,
        Unit::GiB,
        Unit::TiB,
        Unit::Ns,
        Unit::Us,
        Unit::Ms,
        Unit::S,
        Unit::Min,
        Unit::H,
    ];

    /// The key of the message that writes a value in this unit.
    pub(crate) const fn key(self) -> &'static str {
        match self {
            Unit::Hz => "unit-hz",
            Unit::KHz => "unit-khz",
            Unit::MHz => "unit-mhz",
            Unit::GHz => "unit-ghz",
            Unit::B => "unit-b",
            Unit::KiB => "unit-kib",
            Unit::MiB => "unit-mib",
            Unit::GiB => "unit-gib",
            Unit::TiB => "unit-tib",
            Unit::Ns => "unit-ns",
            Unit::Us => "unit-us",
            Unit::Ms => "unit-ms",
            Unit::S => "unit-s",
            Unit::Min => "unit-min",
            Unit::H => "unit-h",
        }
    }

    /// The symbol used when the resources have no message for the unit.
    pub(crate) const fn english_symbol(self) -> &'static str {
        match self {
            Unit::Hz => "Hz",
            Unit::KHz => "kHz",
            Unit::MHz => "MHz",
            Unit::GHz => "GHz",
            Unit::B => "B",
            Unit::KiB => "KiB",
            Unit::MiB => "MiB",
            Unit::GiB => "GiB",
            Unit::TiB => "TiB",
            Unit::Ns => "ns",
            Unit::Us => "\u{B5}s",
            Unit::Ms => "ms",
            Unit::S => "s",
            Unit::Min => "min",
            Unit::H => "h",
        }
    }
}

/// A quantity ready to be written.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Quantity {
    /// A value with at most `fraction` fraction digits, in `unit`.
    Single {
        value: f64,
        fraction: usize,
        unit: Unit,
    },
    /// Whole hours, minutes or seconds: the largest nonzero part and the next smaller one if it
    /// is nonzero.
    Parts(Vec<(u64, Unit)>),
}

/// `value` rounded to `digits` fraction digits (half up).
fn rounded(value: f64, digits: i32) -> f64 {
    let factor = 10_f64.powi(digits);
    (value * factor).round() / factor
}

/// Scales `value` up through `units` while it reaches `base`; the test uses the rounded value so
/// that 999.9996 is not shown as 1000. The first unit takes `first_fraction` fraction digits and
/// the others `fraction`.
fn scale(
    mut value: f64,
    base: f64,
    units: &[Unit],
    first_fraction: usize,
    fraction: usize,
) -> Quantity {
    let digits = i32::try_from(fraction).unwrap_or(3);
    let mut index = 0_usize;
    while index.saturating_add(1) < units.len() && (value >= base || rounded(value, digits) >= base)
    {
        value /= base;
        index = index.saturating_add(1);
    }
    Quantity::Single {
        value,
        fraction: if index == 0 { first_fraction } else { fraction },
        unit: units.get(index).copied().unwrap_or(Unit::Hz),
    }
}

/// A frequency in hertz scaled to Hz, kHz, MHz or GHz; `None` if it cannot be shown.
pub(crate) fn frequency(hertz: f64) -> Option<Quantity> {
    if !hertz.is_finite() || hertz < 0.0 {
        return None;
    }
    Some(scale(
        hertz,
        1000.0,
        &[Unit::Hz, Unit::KHz, Unit::MHz, Unit::GHz],
        3,
        3,
    ))
}

/// A size in bytes scaled to B, KiB, MiB, GiB or TiB (powers of 1024); `None` if it cannot be shown.
pub(crate) fn size(bytes: f64) -> Option<Quantity> {
    if !bytes.is_finite() || bytes < 0.0 {
        return None;
    }
    Some(scale(
        bytes,
        1024.0,
        &[Unit::B, Unit::KiB, Unit::MiB, Unit::GiB, Unit::TiB],
        0,
        1,
    ))
}

/// A duration in seconds: one scaled value below a minute, from a minute on the largest nonzero
/// part of hours, minutes and seconds and the next one; `None` if it cannot be shown.
pub(crate) fn duration(seconds: f64) -> Option<Quantity> {
    if !seconds.is_finite() || seconds < 0.0 {
        return None;
    }
    if seconds >= 60.0 {
        return Some(duration_parts(seconds));
    }
    if rounded(seconds, 3) >= 60.0 {
        // 59.9996 s is shown as one minute, not as 60 s.
        return Some(duration_parts(60.0));
    }
    if seconds == 0.0 {
        return Some(Quantity::Single {
            value: 0.0,
            fraction: 0,
            unit: Unit::S,
        });
    }
    let steps = [
        (1.0, Unit::S),
        (1.0e-3, Unit::Ms),
        (1.0e-6, Unit::Us),
        (1.0e-9, Unit::Ns),
    ];
    let mut index = steps
        .iter()
        .position(|(factor, _)| seconds >= *factor)
        .unwrap_or(steps.len().saturating_sub(1));
    let mut value = steps
        .get(index)
        .map_or(seconds, |(factor, _)| seconds / factor);
    if index > 0 && rounded(value, 3) >= 1000.0 {
        index = index.saturating_sub(1);
        value /= 1000.0;
    }
    Some(Quantity::Single {
        value,
        fraction: 3,
        unit: steps.get(index).map_or(Unit::S, |(_, unit)| *unit),
    })
}

fn duration_parts(seconds: f64) -> Quantity {
    // Whole seconds, saturating far above any realistic duration.
    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "the value is finite, at least 0 and capped below 2^53"
    )]
    let total = if seconds >= 9.0e15 {
        9_000_000_000_000_000_u64
    } else {
        seconds.floor() as u64
    };
    let hours = total.checked_div(3600).unwrap_or(0);
    let minutes = total
        .checked_rem(3600)
        .unwrap_or(0)
        .checked_div(60)
        .unwrap_or(0);
    let secs = total.checked_rem(60).unwrap_or(0);
    let parts = [(hours, Unit::H), (minutes, Unit::Min), (secs, Unit::S)];
    let Some(first) = parts.iter().position(|(n, _)| *n > 0) else {
        return Quantity::Parts(vec![(0, Unit::S)]);
    };
    let mut out = vec![parts.get(first).copied().unwrap_or((0, Unit::S))];
    if let Some(next) = parts.get(first.saturating_add(1)).filter(|(n, _)| *n > 0) {
        out.push(*next);
    }
    Quantity::Parts(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn single(q: Option<Quantity>) -> (f64, usize, Unit) {
        match q {
            Some(Quantity::Single {
                value,
                fraction,
                unit,
            }) => (value, fraction, unit),
            other => panic!("not a single value: {other:?}"),
        }
    }

    fn parts(q: Option<Quantity>) -> Vec<(u64, Unit)> {
        match q {
            Some(Quantity::Parts(p)) => p,
            other => panic!("not parts: {other:?}"),
        }
    }

    #[test]
    fn frequencies_scale_to_the_largest_unit_with_a_value_of_at_least_one() {
        assert_eq!(single(frequency(1.0)), (1.0, 3, Unit::Hz));
        assert_eq!(single(frequency(999.0)).2, Unit::Hz);
        assert_eq!(single(frequency(1000.0)), (1.0, 3, Unit::KHz));
        assert_eq!(single(frequency(12_500_000.0)), (12.5, 3, Unit::MHz));
        assert_eq!(single(frequency(500_000_000.0)), (500.0, 3, Unit::MHz));
        assert_eq!(single(frequency(1_250_000_000.0)), (1.25, 3, Unit::GHz));
        assert_eq!(single(frequency(0.0)).2, Unit::Hz);
        assert_eq!(single(frequency(0.5)).2, Unit::Hz);
        // Rounding must not show 1000 of a unit.
        assert_eq!(single(frequency(999_999.999_9)).2, Unit::MHz);
        assert_eq!(single(frequency(1.0e18)).2, Unit::GHz);
        for bad in [f64::NAN, f64::INFINITY, -1.0] {
            assert_eq!(frequency(bad), None);
        }
    }

    #[test]
    fn sizes_use_powers_of_1024_and_bytes_have_no_fraction() {
        assert_eq!(single(size(0.0)), (0.0, 0, Unit::B));
        assert_eq!(single(size(1023.0)), (1023.0, 0, Unit::B));
        assert_eq!(single(size(1024.0)), (1.0, 1, Unit::KiB));
        let (value, fraction, unit) = single(size(31_981_568.0));
        assert_eq!((unit, fraction), (Unit::MiB, 1));
        assert!((value - 30.4999).abs() < 0.001);
        assert_eq!(single(size(5_368_709_120.0)), (5.0, 1, Unit::GiB));
        assert_eq!(single(size(1.0e15)).2, Unit::TiB);
        assert_eq!(
            single(size(1_048_575.99)).2,
            Unit::MiB,
            "rounding up to 1024 KiB becomes 1 MiB"
        );
        assert_eq!(size(-1.0), None);
        assert_eq!(size(f64::NAN), None);
    }

    #[test]
    fn short_durations_use_one_unit_and_long_ones_use_parts() {
        assert_eq!(single(duration(0.0)), (0.0, 0, Unit::S));
        let (v, f, u) = single(duration(1.5e-6));
        assert!((v - 1.5).abs() < 1e-9 && f == 3 && u == Unit::Us);
        let (v, _, u) = single(duration(2.3e-3));
        assert!((v - 2.3).abs() < 1e-9 && u == Unit::Ms);
        assert_eq!(single(duration(42.0)), (42.0, 3, Unit::S));
        assert_eq!(single(duration(1.0e-9)).2, Unit::Ns);
        assert_eq!(
            single(duration(0.9996)).2,
            Unit::Ms,
            "999.6 ms stays in milliseconds"
        );
        assert_eq!(
            single(duration(0.999_999_6)).2,
            Unit::S,
            "999.9996 ms rounds to 1000 ms and is shown as 1 s"
        );
        assert_eq!(parts(duration(60.0)), vec![(1, Unit::Min)]);
        assert_eq!(parts(duration(125.0)), vec![(2, Unit::Min), (5, Unit::S)]);
        assert_eq!(parts(duration(3725.0)), vec![(1, Unit::H), (2, Unit::Min)]);
        assert_eq!(parts(duration(3605.0)), vec![(1, Unit::H)]);
        assert_eq!(parts(duration(3600.0)), vec![(1, Unit::H)]);
        assert_eq!(
            parts(duration(90_061.0)),
            vec![(25, Unit::H), (1, Unit::Min)]
        );
        assert_eq!(parts(duration(59.9999)), vec![(1, Unit::Min)]);
        assert_eq!(parts(duration(1.0e30)).len(), 1);
        for bad in [f64::NAN, f64::INFINITY, -0.001] {
            assert_eq!(duration(bad), None);
        }
    }

    #[test]
    fn every_unit_has_a_distinct_key_and_symbol() {
        let keys: std::collections::HashSet<_> = Unit::ALL.iter().map(|u| u.key()).collect();
        assert_eq!(keys.len(), 15);
        assert!(
            Unit::ALL
                .iter()
                .all(|u| u.key().starts_with("unit-") && !u.english_symbol().is_empty())
        );
    }
}
