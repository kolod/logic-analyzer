//! Markers: points in time the user places to read times and distances.
//!
//! A marker is attached to a sample position, so it stays at the same point of the signal while
//! the view zooms and pans. Two markers give a measurement: the distance in time and the
//! frequency of that distance, formatted for the language of the interface. Marker names are user
//! text: sanitized for display and never translated.

use la_i18n::Localizer;
use la_storage::Snapshot;

/// The most markers a capture can hold.
pub const MAX_MARKERS: usize = 64;

/// One marker.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Marker {
    /// Its identity: unique within the session and never reused.
    pub id: u32,
    /// The sample position it is attached to.
    pub position: u64,
    /// The user's name for it (sanitized, possibly empty).
    pub name: String,
}

/// The markers of the capture on screen.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Markers {
    /// The markers in the order they were placed.
    pub list: Vec<Marker>,
    /// The two markers a measurement uses (by default the two most recently placed).
    pub selected: [Option<u32>; 2],
    next_id: u32,
}

impl Markers {
    /// No markers.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Restores markers read from a session file: the ids must be unique.
    #[must_use]
    pub fn restore(list: Vec<Marker>, selected: [Option<u32>; 2]) -> Self {
        let next_id = list.iter().map(|m| m.id).max().unwrap_or(0);
        Self {
            list,
            selected,
            next_id,
        }
    }

    /// Places a marker; `None` when the limit is reached. The new marker and the one placed
    /// before it become the measured pair.
    pub fn place(&mut self, position: u64) -> Option<u32> {
        if self.list.len() >= MAX_MARKERS {
            return None;
        }
        self.next_id = self.next_id.saturating_add(1);
        let id = self.next_id;
        let previous = self.list.last().map(|m| m.id);
        self.list.push(Marker {
            id,
            position,
            name: String::new(),
        });
        self.selected = [previous, Some(id)];
        Some(id)
    }

    /// The marker with this id.
    #[must_use]
    pub fn get(&self, id: u32) -> Option<&Marker> {
        self.list.iter().find(|m| m.id == id)
    }

    /// The marker with this id, for changing.
    pub fn get_mut(&mut self, id: u32) -> Option<&mut Marker> {
        self.list.iter_mut().find(|m| m.id == id)
    }

    /// Removes a marker (and clears it from the selection); `false` if there is none.
    pub fn remove(&mut self, id: u32) -> bool {
        let before = self.list.len();
        self.list.retain(|m| m.id != id);
        for slot in &mut self.selected {
            if *slot == Some(id) {
                *slot = None;
            }
        }
        self.list.len() != before
    }

    /// Removes every marker.
    pub fn clear(&mut self) {
        self.list.clear();
        self.selected = [None, None];
    }

    /// Chooses the measured pair; `false` (and no change) if an id does not exist.
    pub fn select(&mut self, first: Option<u32>, second: Option<u32>) -> bool {
        let known = |id: Option<u32>| id.is_none_or(|id| self.get(id).is_some());
        if !known(first) || !known(second) {
            return false;
        }
        self.selected = [first, second];
        true
    }

    /// The positions of the measured pair, if both are chosen and exist.
    #[must_use]
    pub fn measured_positions(&self) -> Option<(u64, u64)> {
        let [Some(a), Some(b)] = self.selected else {
            return None;
        };
        Some((self.get(a)?.position, self.get(b)?.position))
    }
}

/// The nearest transition of `channel` to `position` within `radius` samples; on a tie the
/// earlier one. `None` if there is none (or the channel does not exist). Costs O(log N) plus the
/// transitions between the position and the nearest one.
#[must_use]
pub fn nearest_transition(
    snapshot: &Snapshot,
    channel: u8,
    position: u64,
    radius: u64,
) -> Option<u64> {
    let length = snapshot.length();
    if length == 0 || channel >= snapshot.channels() {
        return None;
    }
    // At or after the position: the first transition with `p >= position`.
    let after = {
        let end = position.saturating_add(radius).min(length);
        let start = position.saturating_sub(1);
        if start < end {
            snapshot
                .transitions(channel, start, end)
                .ok()?
                .map(|e| e.position)
                .find(|p| *p >= position)
        } else {
            None
        }
    };
    // Before the position: the last transition with `p < position`, searched in growing steps.
    let before = {
        let mut found = None;
        let mut reach = 1_u64;
        while found.is_none() && position > 1 {
            let start = position.saturating_sub(reach.saturating_add(1));
            let end = position.saturating_sub(1);
            if start < end {
                found = snapshot
                    .transitions(channel, start, end)
                    .ok()?
                    .map(|e| e.position)
                    .last();
            }
            if reach >= radius || start == 0 {
                break;
            }
            reach = reach.saturating_mul(2).min(radius);
        }
        found.filter(|p| position.saturating_sub(*p) <= radius)
    };
    match (before, after) {
        (Some(b), Some(a)) => {
            if position.saturating_sub(b) <= a.saturating_sub(position) {
                Some(b)
            } else {
                Some(a)
            }
        }
        (one, other) => one.or(other),
    }
}

/// The distance in time and the frequency between two markers.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Measurement {
    /// The distance in samples.
    pub distance_samples: u64,
    /// The distance in seconds (exact: samples over the sample rate).
    pub seconds: f64,
    /// The frequency of that distance in hertz; `None` for a distance of zero.
    pub hertz: Option<f64>,
}

/// The measurement between the sample positions `a` and `b` (in either order) at
/// `sample_rate_hz`; `None` without a sample rate.
#[must_use]
pub fn measurement(sample_rate_hz: u64, a: u64, b: u64) -> Option<Measurement> {
    if sample_rate_hz == 0 {
        return None;
    }
    let distance = a.abs_diff(b);
    let rate = sample_rate_hz as f64;
    Some(Measurement {
        distance_samples: distance,
        seconds: distance as f64 / rate,
        hertz: (distance > 0).then(|| rate / distance as f64),
    })
}

/// A measurement as text in the language of a localizer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MeasurementText {
    /// The distance in time ("1.5 ms").
    pub distance: String,
    /// The frequency ("667 Hz"), rounded to the digits the distance resolves.
    pub frequency: Option<String>,
}

/// Rounds `value` to `digits` significant digits.
fn round_significant(value: f64, digits: u32) -> f64 {
    if value == 0.0 || !value.is_finite() {
        return value;
    }
    let magnitude = value.abs().log10().floor() as i32;
    let places = i32::try_from(digits)
        .unwrap_or(i32::MAX)
        .saturating_sub(1)
        .saturating_sub(magnitude);
    let scale = 10_f64.powi(places);
    (value * scale).round() / scale
}

/// Formats a measurement. The frequency never claims more digits than one sample period of
/// resolution allows: the distance has `d` decimal digits, so the frequency keeps `d`.
#[must_use]
pub fn measurement_text(localizer: &Localizer, measurement: &Measurement) -> MeasurementText {
    let digits = measurement
        .distance_samples
        .checked_ilog10()
        .map_or(1, |log| log.saturating_add(1));
    MeasurementText {
        distance: localizer.format_duration(measurement.seconds),
        frequency: measurement
            .hertz
            .map(|hertz| localizer.format_frequency(round_significant(hertz, digits))),
    }
}
