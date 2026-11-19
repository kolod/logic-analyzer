//! Markers: points and intervals in time the user places to read times and distances.
//!
//! There are two kinds. A **single marker** (`T1`, `T2`, ...) is one sample position; a **pair
//! marker** (`P1`, `P2`, ...) is an interval with a start and an end. Both are attached to sample
//! positions, so they stay at the same point of the signal while the view zooms and pans. A pair
//! gives a measurement: the duration and the frequency of that duration, formatted for the language
//! of the interface. Comments are user text: sanitized for display and never translated.
//!
//! The rules here (numbering, ordering, limits, conversions, what a cut leaves) are pure data
//! operations, testable without a shell (`specs/015-markers-rework/data-model.md`).

use crate::sanitize::sanitize_text;
use la_i18n::Localizer;

/// The most markers a capture can hold (a pair counts as one).
pub const MAX_MARKERS: usize = 64;
/// How much room, in logical pixels, the marks of a pair need beside each end of its interval when
/// the view is fitted to it: a square of 12 pixels and a gap of 4.
pub const MARK_MARGIN_PX: u32 = 16;
/// The longest comment, in characters.
pub const MAX_COMMENT_CHARS: usize = 80;
/// How many colours a marker can have (an index below this).
pub const MARKER_COLOURS: u8 = 8;

/// The two kinds of marker.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Kind {
    /// One point in time (`T`).
    Single,
    /// An interval: a start and an end (`P`).
    Pair,
}

impl Kind {
    /// The letter in front of the number in the name.
    #[must_use]
    pub fn prefix(self) -> char {
        match self {
            Kind::Single => 'T',
            Kind::Pair => 'P',
        }
    }

    fn index(self) -> usize {
        match self {
            Kind::Single => 0,
            Kind::Pair => 1,
        }
    }
}

/// Where a marker is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shape {
    /// A single marker at a sample position.
    Point(u64),
    /// A pair marker; `start <= end`.
    Interval {
        /// The earlier end.
        start: u64,
        /// The later end.
        end: u64,
    },
}

impl Shape {
    /// An interval between two positions given in either order.
    #[must_use]
    pub fn interval(a: u64, b: u64) -> Self {
        Shape::Interval {
            start: a.min(b),
            end: a.max(b),
        }
    }

    /// The kind this shape belongs to.
    #[must_use]
    pub fn kind(&self) -> Kind {
        match self {
            Shape::Point(_) => Kind::Single,
            Shape::Interval { .. } => Kind::Pair,
        }
    }

    /// The position of a single marker, the start of a pair.
    #[must_use]
    pub fn start(&self) -> u64 {
        match self {
            Shape::Point(p) => *p,
            Shape::Interval { start, .. } => *start,
        }
    }

    /// The position of a single marker, the end of a pair.
    #[must_use]
    pub fn end(&self) -> u64 {
        match self {
            Shape::Point(p) => *p,
            Shape::Interval { end, .. } => *end,
        }
    }
}

/// Which end of a marker a command means.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum End {
    /// The only position of a single marker.
    Point,
    /// The earlier end of a pair.
    Start,
    /// The later end of a pair.
    End,
}

/// One marker.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Marker {
    /// Its identity: unique within the session and never reused.
    pub id: u32,
    /// Its number within its kind (`T3` is a single marker with number 3); never given twice.
    pub number: u32,
    /// Its colour, an index below [`MARKER_COLOURS`].
    pub colour: u8,
    /// The user's comment (sanitized, possibly empty).
    pub comment: String,
    /// Where it is.
    pub shape: Shape,
}

impl Marker {
    /// Its kind.
    #[must_use]
    pub fn kind(&self) -> Kind {
        self.shape.kind()
    }

    /// Its name, `T3` or `P1`: never translated.
    #[must_use]
    pub fn name(&self) -> String {
        format!("{}{}", self.kind().prefix(), self.number)
    }
}

/// What a cut or a trim did to the markers.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RemapReport {
    /// Markers removed because they lay outside the kept range.
    pub removed: usize,
    /// Pairs removed because only one of their ends was inside the kept range.
    pub split_pairs: usize,
}

/// The markers of the capture on screen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Markers {
    /// The markers in the order they were created.
    pub list: Vec<Marker>,
    /// The selected marker.
    pub selected: Option<u32>,
    /// The position that counts as time zero for the axis and the marker times.
    pub origin: u64,
    next_id: u32,
    next_number: [u32; 2],
}

impl Default for Markers {
    fn default() -> Self {
        Self {
            list: Vec::new(),
            selected: None,
            origin: 0,
            next_id: 0,
            next_number: [1, 1],
        }
    }
}

impl Markers {
    /// No markers.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Restores markers read from a session file. The reader has checked that the ids and the
    /// numbers are unique; the counters are raised above what is in use whatever they were.
    #[must_use]
    pub fn restore(
        list: Vec<Marker>,
        selected: Option<u32>,
        origin: u64,
        next_id: u32,
        next_number: [u32; 2],
    ) -> Self {
        let highest_id = list.iter().map(|m| m.id).max().unwrap_or(0);
        let highest = |kind: Kind| {
            list.iter()
                .filter(|m| m.kind() == kind)
                .map(|m| m.number)
                .max()
                .unwrap_or(0)
        };
        let next_number = [
            next_number[0]
                .max(highest(Kind::Single).saturating_add(1))
                .max(1),
            next_number[1]
                .max(highest(Kind::Pair).saturating_add(1))
                .max(1),
        ];
        Self {
            list,
            selected,
            origin,
            next_id: next_id.max(highest_id),
            next_number,
        }
    }

    /// The identity the next marker gets, minus one (what the section stores).
    #[must_use]
    pub fn last_id(&self) -> u32 {
        self.next_id
    }

    /// The next number of each kind, `[single, pair]`.
    #[must_use]
    pub fn next_numbers(&self) -> [u32; 2] {
        self.next_number
    }

    fn allocate(&mut self, kind: Kind) -> Option<(u32, u32)> {
        let id = self.next_id.checked_add(1)?;
        let number_slot = self.next_number.get_mut(kind.index())?;
        let number = *number_slot;
        *number_slot = number.checked_add(1)?;
        self.next_id = id;
        Some((id, number))
    }

    /// Adds a marker of the kind of `shape` with colour 0 and no comment; `None` at the limit.
    /// The new marker is not selected (the command that adds it decides).
    pub fn add(&mut self, shape: Shape) -> Option<u32> {
        if self.list.len() >= MAX_MARKERS {
            return None;
        }
        let (id, number) = self.allocate(shape.kind())?;
        self.list.push(Marker {
            id,
            number,
            colour: 0,
            comment: String::new(),
            shape,
        });
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

    /// Removes a marker (and clears the selection if it was selected); `false` if there is none.
    /// Its number is not given again.
    pub fn remove(&mut self, id: u32) -> bool {
        let before = self.list.len();
        self.list.retain(|m| m.id != id);
        if self.selected == Some(id) {
            self.selected = None;
        }
        self.list.len() != before
    }

    /// Removes every marker; the numbering and the origin go on.
    pub fn clear(&mut self) {
        self.list.clear();
        self.selected = None;
    }

    /// Selects a marker (`None` clears the selection); `false` (and no change) if there is no
    /// such marker.
    pub fn select(&mut self, id: Option<u32>) -> bool {
        if id.is_some_and(|id| self.get(id).is_none()) {
            return false;
        }
        self.selected = id;
        true
    }

    /// Sets the colour; `false` for an unknown marker or a colour out of range.
    pub fn set_colour(&mut self, id: u32, colour: u8) -> bool {
        if colour >= MARKER_COLOURS {
            return false;
        }
        match self.get_mut(id) {
            Some(marker) => {
                marker.colour = colour;
                true
            }
            None => false,
        }
    }

    /// Sets the comment (sanitized and cut to [`MAX_COMMENT_CHARS`]); `false` for an unknown
    /// marker.
    pub fn set_comment(&mut self, id: u32, comment: &str) -> bool {
        match self.get_mut(id) {
            Some(marker) => {
                marker.comment = sanitize_text(comment, MAX_COMMENT_CHARS);
                true
            }
            None => false,
        }
    }

    /// Moves one end of a marker to `position`. A pair whose ends cross is put back in order, so
    /// the end that was moved may now be the other one: the result is the role the moved end has
    /// now. `None` (and no change) for an unknown marker or an end the shape does not have.
    pub fn move_end(&mut self, id: u32, end: End, position: u64) -> Option<End> {
        let marker = self.get_mut(id)?;
        match (&mut marker.shape, end) {
            (Shape::Point(p), End::Point) => {
                *p = position;
                Some(End::Point)
            }
            (Shape::Interval { start, end: stop }, End::Start) => {
                let (a, b) = (position, *stop);
                *start = a.min(b);
                *stop = a.max(b);
                Some(if a <= b { End::Start } else { End::End })
            }
            (Shape::Interval { start, end: stop }, End::End) => {
                let (a, b) = (*start, position);
                *start = a.min(b);
                *stop = a.max(b);
                Some(if b >= a { End::End } else { End::Start })
            }
            _ => None,
        }
    }

    /// Makes the position of a single marker the origin of time; `false` for anything else.
    pub fn set_origin(&mut self, id: u32) -> bool {
        match self.get(id).map(|m| m.shape) {
            Some(Shape::Point(position)) => {
                self.origin = position;
                true
            }
            _ => false,
        }
    }

    /// Replaces a pair by two single markers at its ends, with new single numbers, its colour and
    /// (on the first) its comment. The two ids, or `None` for anything but a pair or when the
    /// limit would be exceeded.
    pub fn split(&mut self, id: u32) -> Option<(u32, u32)> {
        let index = self.list.iter().position(|m| m.id == id)?;
        let marker = self.list.get(index)?.clone();
        let Shape::Interval { start, end } = marker.shape else {
            return None;
        };
        if self.list.len() >= MAX_MARKERS {
            return None;
        }
        let (first_id, first_number) = self.allocate(Kind::Single)?;
        let (second_id, second_number) = self.allocate(Kind::Single)?;
        let first = Marker {
            id: first_id,
            number: first_number,
            colour: marker.colour,
            comment: marker.comment,
            shape: Shape::Point(start),
        };
        let second = Marker {
            id: second_id,
            number: second_number,
            colour: marker.colour,
            comment: String::new(),
            shape: Shape::Point(end),
        };
        self.list.splice(index..=index, [first, second]);
        if self.selected == Some(id) {
            self.selected = None;
        }
        Some((first_id, second_id))
    }

    /// Replaces two different single markers by one pair between them, with a new pair number
    /// and the colour and comment of the earlier one. The id of the pair, or `None` for anything
    /// but two different single markers.
    pub fn join(&mut self, a: u32, b: u32) -> Option<u32> {
        if a == b {
            return None;
        }
        let first = self.get(a)?.clone();
        let second = self.get(b)?.clone();
        let (Shape::Point(pa), Shape::Point(pb)) = (first.shape, second.shape) else {
            return None;
        };
        let earlier = if pb < pa { &second } else { &first };
        let (id, number) = self.allocate(Kind::Pair)?;
        let pair = Marker {
            id,
            number,
            colour: earlier.colour,
            comment: earlier.comment.clone(),
            shape: Shape::interval(pa, pb),
        };
        let index = self
            .list
            .iter()
            .position(|m| m.id == a || m.id == b)
            .unwrap_or(self.list.len());
        self.list.retain(|m| m.id != a && m.id != b);
        let index = index.min(self.list.len());
        self.list.insert(index, pair);
        self.selected = Some(id);
        Some(id)
    }

    /// Keeps the part `start..end` of the capture: every position `p` inside becomes `p - start`.
    /// A single marker outside is removed; a pair is removed unless both its ends are inside. The
    /// origin keeps its point if that point is inside, else it goes back to the start.
    pub fn remap(&mut self, start: u64, end: u64) -> RemapReport {
        let inside = |p: u64| p >= start && p < end;
        let mut report = RemapReport::default();
        let mut kept = Vec::with_capacity(self.list.len());
        for mut marker in std::mem::take(&mut self.list) {
            let keep = match marker.shape {
                Shape::Point(p) => {
                    if inside(p) {
                        marker.shape = Shape::Point(p.saturating_sub(start));
                        true
                    } else {
                        false
                    }
                }
                Shape::Interval { start: a, end: b } => {
                    if inside(a) && inside(b) {
                        marker.shape = Shape::Interval {
                            start: a.saturating_sub(start),
                            end: b.saturating_sub(start),
                        };
                        true
                    } else {
                        if inside(a) || inside(b) {
                            report.split_pairs = report.split_pairs.saturating_add(1);
                        }
                        false
                    }
                }
            };
            if keep {
                kept.push(marker);
            } else {
                report.removed = report.removed.saturating_add(1);
                if self.selected == Some(marker.id) {
                    self.selected = None;
                }
            }
        }
        self.list = kept;
        self.origin = if inside(self.origin) {
            self.origin.saturating_sub(start)
        } else {
            0
        };
        report
    }

    /// The markers by the position of their (start) point, then by id.
    #[must_use]
    pub fn ordered(&self) -> Vec<&Marker> {
        let mut ordered: Vec<&Marker> = self.list.iter().collect();
        ordered.sort_by_key(|m| (m.shape.start(), m.id));
        ordered
    }

    /// The marker to jump to from `from`: the next one with a start after it (or the first, past
    /// the end) when `forward`, else the previous one with a start before it (or the last).
    #[must_use]
    pub fn jump_target(&self, from: u64, forward: bool) -> Option<u32> {
        let ordered = self.ordered();
        let found = if forward {
            ordered.iter().find(|m| m.shape.start() > from)
        } else {
            ordered.iter().rev().find(|m| m.shape.start() < from)
        };
        found
            .or(if forward {
                ordered.first()
            } else {
                ordered.last()
            })
            .map(|m| m.id)
    }

    /// Checks every invariant for a capture of `length` positions; the text names the first one
    /// that does not hold.
    ///
    /// # Errors
    ///
    /// The description of the broken invariant.
    pub fn check(&self, length: u64) -> Result<(), String> {
        if self.list.len() > MAX_MARKERS {
            return Err("more markers than the limit".to_owned());
        }
        let limit = length.max(1);
        for (i, marker) in self.list.iter().enumerate() {
            if self
                .list
                .iter()
                .skip(i.saturating_add(1))
                .any(|o| o.id == marker.id)
            {
                return Err(format!("the id {} repeats", marker.id));
            }
            if self
                .list
                .iter()
                .skip(i.saturating_add(1))
                .any(|o| o.kind() == marker.kind() && o.number == marker.number)
            {
                return Err(format!("the number of {} repeats", marker.name()));
            }
            if marker.colour >= MARKER_COLOURS {
                return Err(format!("{} has a colour out of range", marker.name()));
            }
            if marker.id > self.next_id {
                return Err(format!("the id of {} is above the counter", marker.name()));
            }
            let numbers = self.next_number.get(marker.kind().index()).copied();
            if numbers.is_none_or(|next| marker.number >= next) {
                return Err(format!(
                    "the number of {} is above the counter",
                    marker.name()
                ));
            }
            match marker.shape {
                Shape::Point(p) if p >= limit => {
                    return Err(format!("{} is beyond the capture", marker.name()));
                }
                Shape::Interval { start, end } if start > end || end >= limit => {
                    return Err(format!(
                        "{} is not inside the capture in order",
                        marker.name()
                    ));
                }
                _ => {}
            }
        }
        if self.origin >= limit {
            return Err("the origin is beyond the capture".to_owned());
        }
        if self.selected.is_some_and(|id| self.get(id).is_none()) {
            return Err("the selected marker does not exist".to_owned());
        }
        Ok(())
    }
}

/// A placement under way: a marker that is being put in place with the mouse.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Placement {
    /// Which kind is being placed.
    pub kind: Kind,
    /// The half of a pair that is already fixed (always `None` for a single marker).
    pub first: Option<u64>,
    /// The colour the marker will have.
    pub colour: u8,
}

/// Where the pointer is over the signal area.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Pointer {
    /// Pixels from the left of the signal area.
    pub x: f32,
    /// The channel whose band the pointer is in; `None` over the ruler, the legend, empty space
    /// or a decoder lane.
    pub lane: Option<u8>,
}

/// A transition a marker snaps to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Snapped {
    /// The channel.
    pub channel: u8,
    /// The position of the transition.
    pub position: u64,
}

/// What a pointer means: the sample position and the transition it snapped to, if any.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Resolved {
    /// The sample position under the pointer, or of the transition it snapped to.
    pub position: u64,
    /// The transition to highlight.
    pub snapped: Option<Snapped>,
    /// Whether the zone of the pointer holds more than one transition of its lane.
    pub crowded: bool,
}

/// The view an automatic zoom replaced, to be put back.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SavedView {
    /// First visible sample.
    pub start: u64,
    /// Number of visible samples.
    pub length: u64,
    /// Whether the window showed the whole capture.
    pub auto_fit: bool,
    /// Whether the view followed the newest data.
    pub follow_latest: bool,
}

/// The window of the automatic zoom of a crowded zone.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ZoomTarget {
    /// First visible sample.
    pub start: u64,
    /// Number of visible samples.
    pub length: u64,
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

/// The texts of one marker in the language of the interface (times are relative to the origin).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MarkerText {
    /// The marker.
    pub id: u32,
    /// Its name (`T3`, `P1`): never translated.
    pub name: String,
    /// The time of the marker, or of the start of a pair.
    pub time_a: String,
    /// The time of the end of a pair.
    pub time_b: Option<String>,
    /// The duration of a pair.
    pub duration: Option<String>,
    /// The frequency of that duration, for a pair whose duration is not zero.
    pub frequency: Option<String>,
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
