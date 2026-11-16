//! The layout of the lanes of the signal view: rectangles, the zones of the legend strip, scrolling
//! and the landing place of a dragged lane. Pure functions, so that drawing, hit tests and the
//! in-place editor all read the same numbers (`specs/014-gui-improvements/research.md` R4).

/// Width of the legend strip (the column left of the waveforms), in logical pixels.
pub const GUTTER: f32 = 112.0;
/// Height of the time axis, in logical pixels.
pub const AXIS_HEIGHT: f32 = 28.0;
/// Width of the colour bar at the left edge of a lane.
pub const BAR_WIDTH: f32 = 8.0;
/// The shortest height a user can give a signal.
pub const MIN_HEIGHT: u16 = 24;
/// The longest height a user can give a signal.
pub const MAX_HEIGHT: u16 = 240;
/// The automatic height is kept above this (the rule of feature 006).
pub const AUTO_MIN: f32 = 30.0;
/// The automatic height is kept below this.
pub const AUTO_MAX: f32 = 72.0;
/// The border band reaches this far above the bottom edge of a lane...
const BORDER_ABOVE: f32 = 3.0;
/// ...and this far below it.
const BORDER_BELOW: f32 = 4.0;

/// What a lane shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LaneKind {
    /// The signal of a channel: it has a colour bar, a name and a border.
    Channel,
    /// The lane of a decoder: automatic height, no zones, not draggable.
    Decoder,
}

/// One lane to lay out.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LaneSpec {
    /// What it shows.
    pub kind: LaneKind,
    /// The height the user chose; 0 is automatic (and the only value a decoder lane uses).
    pub height: u16,
}

/// The place of one lane, in logical pixels from the top of the signal view.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Lane {
    /// The top edge (below the axis, or above it while scrolled).
    pub top: f32,
    /// The height.
    pub height: f32,
}

impl Lane {
    /// The bottom edge.
    #[must_use]
    pub fn bottom(&self) -> f32 {
        self.top + self.height
    }
}

/// The part of the legend strip a point is in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Zone {
    /// The colour bar: a click changes the colour.
    Bar,
    /// The name: a click edits it, a drag moves the lane.
    Name,
    /// The border below the lane: a drag changes the height of the lane above it.
    Border,
}

/// A point in the legend strip.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Hit {
    /// The lane (an index into the specs).
    pub lane: usize,
    /// The zone.
    pub zone: Zone,
}

/// The layout of all lanes for one area height and one scroll offset.
#[derive(Debug, Clone)]
pub struct Layout {
    kinds: Vec<LaneKind>,
    lanes: Vec<Lane>,
    area: f32,
    offset: f32,
    total: f32,
}

impl Layout {
    /// Lays out `specs` in an area `area_height` high (the axis included), scrolled by `offset`
    /// (limited to what there is to scroll).
    #[must_use]
    pub fn new(specs: &[LaneSpec], area_height: f32, offset: f32) -> Self {
        let count = specs.len();
        let automatic = if count == 0 {
            AUTO_MAX
        } else {
            ((area_height - AXIS_HEIGHT) / count as f32).clamp(AUTO_MIN, AUTO_MAX)
        };
        let heights: Vec<f32> = specs
            .iter()
            .map(|spec| match spec.kind {
                LaneKind::Channel if spec.height != 0 => {
                    f32::from(clamp_height(f32::from(spec.height)))
                }
                _ => automatic,
            })
            .collect();
        let total: f32 = heights.iter().sum();
        let visible = (area_height - AXIS_HEIGHT).max(0.0);
        let max_offset = (total - visible).max(0.0);
        let offset = if offset.is_nan() {
            0.0
        } else {
            offset.clamp(0.0, max_offset)
        };
        let mut top = AXIS_HEIGHT - offset;
        let lanes = heights
            .iter()
            .map(|height| {
                let lane = Lane {
                    top,
                    height: *height,
                };
                top += *height;
                lane
            })
            .collect();
        Self {
            kinds: specs.iter().map(|s| s.kind).collect(),
            lanes,
            area: area_height,
            offset,
            total,
        }
    }

    /// The place of a lane.
    #[must_use]
    pub fn lane(&self, index: usize) -> Option<Lane> {
        self.lanes.get(index).copied()
    }

    /// The sum of the heights of all lanes.
    #[must_use]
    pub fn total_height(&self) -> f32 {
        self.total
    }

    /// How far the lanes can be scrolled (0 when they all fit).
    #[must_use]
    pub fn max_offset(&self) -> f32 {
        (self.total - (self.area - AXIS_HEIGHT).max(0.0)).max(0.0)
    }

    /// The scroll offset in use.
    #[must_use]
    pub fn offset(&self) -> f32 {
        self.offset
    }

    /// What the point `(x, y)` is on in the legend strip, if anything: the border bands win over
    /// the colour bar and the name, nothing is above the axis edge, and a decoder lane has no zones.
    #[must_use]
    pub fn zone_at(&self, x: f32, y: f32) -> Option<Hit> {
        if !(0.0..GUTTER).contains(&x) || y < AXIS_HEIGHT {
            return None;
        }
        for (index, lane) in self.lanes.iter().enumerate() {
            if self.kinds.get(index) != Some(&LaneKind::Channel) {
                continue;
            }
            let bottom = lane.bottom();
            if y >= bottom - BORDER_ABOVE && y < bottom + BORDER_BELOW {
                return Some(Hit {
                    lane: index,
                    zone: Zone::Border,
                });
            }
        }
        for (index, lane) in self.lanes.iter().enumerate() {
            if self.kinds.get(index) != Some(&LaneKind::Channel) {
                continue;
            }
            if y >= lane.top && y < lane.bottom() {
                let zone = if x < BAR_WIDTH { Zone::Bar } else { Zone::Name };
                return Some(Hit { lane: index, zone });
            }
        }
        None
    }

    /// Where a dragged lane lands when released at `y`: the number of channel lanes whose middle is
    /// above `y` (only channels count; a lane never lands among decoder lanes).
    #[must_use]
    pub fn landing(&self, y: f32) -> usize {
        self.lanes
            .iter()
            .zip(&self.kinds)
            .filter(|(lane, kind)| **kind == LaneKind::Channel && lane.top + lane.height / 2.0 < y)
            .count()
    }

    /// The `to` of `MoveChannel { from, to }` for lane `from` released at `y`: positions are counted
    /// after the lane has been taken out.
    #[must_use]
    pub fn move_target(&self, from: usize, y: f32) -> usize {
        let landing = self.landing(y);
        if landing > from {
            landing.saturating_sub(1)
        } else {
            landing
        }
    }

    /// The vertical shift of every one of the first `count` lanes while lane `from` is carried
    /// over the slot of `to` (the `to` of [`Layout::move_target`]): the lanes between the two slots
    /// move by the height of the carried lane, so that the hole closes where it was and opens where
    /// the lane will land. The carried lane itself has no shift.
    #[must_use]
    pub fn shifts(&self, from: usize, to: usize, count: usize) -> Vec<f32> {
        let carried = self.lane(from).map_or(0.0, |l| l.height);
        (0..count)
            .map(|i| {
                if i == from {
                    0.0
                } else if from < to && i > from && i <= to {
                    -carried
                } else if to < from && i >= to && i < from {
                    carried
                } else {
                    0.0
                }
            })
            .collect()
    }

    /// The y of the line that shows where a lane landing at `landing` will be: the top of the lane it
    /// goes before, or the bottom of the last channel lane.
    #[must_use]
    pub fn landing_line(&self, landing: usize) -> f32 {
        let channels = self
            .kinds
            .iter()
            .filter(|k| **k == LaneKind::Channel)
            .count();
        if landing < channels {
            self.lanes.get(landing).map_or(AXIS_HEIGHT, |l| l.top)
        } else {
            channels
                .checked_sub(1)
                .and_then(|last| self.lanes.get(last))
                .map_or(AXIS_HEIGHT, Lane::bottom)
        }
    }
}

/// A height chosen by dragging, rounded and limited to the allowed range (NaN gives the minimum).
#[must_use]
pub fn clamp_height(height: f32) -> u16 {
    if height.is_nan() {
        return MIN_HEIGHT;
    }
    let clamped = height
        .round()
        .clamp(f32::from(MIN_HEIGHT), f32::from(MAX_HEIGHT));
    // The value is inside the range of `u16` after the clamp.
    clamped as u16
}

/// The height of a lane whose border is dragged from `start_y` to `y`, starting from `start_height`.
#[must_use]
pub fn resized_height(start_height: f32, start_y: f32, y: f32) -> u16 {
    clamp_height(start_height + (y - start_y))
}

/// How long a lane takes to cover about two thirds of the way to its place (seconds).
const EASE_SECONDS: f32 = 0.07;

/// One step of the movement of a lane that makes room: `current` moves toward `target` by the
/// share of the way that `dt` seconds make, never past it.
#[must_use]
pub fn ease(current: f32, target: f32, dt: f32) -> f32 {
    let share = 1.0 - (-dt.max(0.0) / EASE_SECONDS).exp();
    current + (target - current) * share.clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    const GUTTER_W: f32 = 112.0;

    fn channel(height: u16) -> LaneSpec {
        LaneSpec {
            kind: LaneKind::Channel,
            height,
        }
    }

    fn decoder() -> LaneSpec {
        LaneSpec {
            kind: LaneKind::Decoder,
            height: 0,
        }
    }

    /// Three channels of heights [automatic, 60, 40] in an area of 300 px: the axis takes 28, so 272
    /// remain and the automatic lane is clamp(272 / 3, 30, 72) = 72 high.
    fn three() -> Layout {
        Layout::new(&[channel(0), channel(60), channel(40)], 300.0, 0.0)
    }

    #[test]
    fn the_rectangles_follow_the_heights_below_the_axis() {
        let layout = three();
        let lanes: Vec<(f32, f32)> = (0..3)
            .map(|i| {
                let lane = layout.lane(i).unwrap();
                (lane.top, lane.height)
            })
            .collect();
        assert_eq!(lanes, vec![(28.0, 72.0), (100.0, 60.0), (160.0, 40.0)]);
        assert_eq!(layout.lane(3), None);
        assert!((layout.total_height() - 172.0).abs() < f32::EPSILON);
        assert!(layout.max_offset().abs() < f32::EPSILON, "everything fits");
    }

    #[test]
    fn the_automatic_height_is_the_rule_of_feature_006() {
        // clamp((area - axis) / lanes, 30, 72), with every lane counted, decoder lanes too.
        let high = Layout::new(&[channel(0), channel(0), channel(0)], 300.0, 0.0);
        assert!((high.lane(0).unwrap().height - 72.0).abs() < 1e-4);
        let middle = Layout::new(&[channel(0), channel(0), channel(0)], 150.0, 0.0);
        assert!((middle.lane(0).unwrap().height - 122.0 / 3.0).abs() < 1e-4);
        let low = Layout::new(&[channel(0), channel(0), channel(0)], 100.0, 0.0);
        assert!((low.lane(0).unwrap().height - 30.0).abs() < 1e-4);
        let with_decoder = Layout::new(&[channel(0), decoder()], 128.0, 0.0);
        assert!((with_decoder.lane(1).unwrap().height - 50.0).abs() < 1e-4);
        assert!((with_decoder.lane(0).unwrap().height - 50.0).abs() < 1e-4);
        // No lanes at all: the maximum.
        assert!((Layout::new(&[], 300.0, 0.0).total_height()).abs() < f32::EPSILON);
    }

    #[test]
    fn a_decoder_lane_ignores_a_height() {
        let layout = Layout::new(
            &[
                channel(0),
                LaneSpec {
                    kind: LaneKind::Decoder,
                    height: 200,
                },
            ],
            300.0,
            0.0,
        );
        assert!((layout.lane(1).unwrap().height - 72.0).abs() < 1e-4);
    }

    fn hit(layout: &Layout, x: f32, y: f32) -> Option<(usize, Zone)> {
        layout.zone_at(x, y).map(|h| (h.lane, h.zone))
    }

    #[test]
    fn the_zones_of_a_lane_are_the_colour_bar_the_name_and_the_border() {
        let layout = three();
        // The colour bar: x 0..8.
        assert_eq!(hit(&layout, 0.0, 30.0), Some((0, Zone::Bar)));
        assert_eq!(hit(&layout, 7.9, 96.0), Some((0, Zone::Bar)));
        // The name: x 8..112.
        assert_eq!(hit(&layout, 8.0, 30.0), Some((0, Zone::Name)));
        assert_eq!(hit(&layout, 50.0, 50.0), Some((0, Zone::Name)));
        assert_eq!(hit(&layout, GUTTER_W - 0.1, 96.0), Some((0, Zone::Name)));
        // The border below lane 0 (bottom at 100): y 97 up to but not including 104, whole strip.
        assert_eq!(hit(&layout, 3.0, 97.0), Some((0, Zone::Border)));
        assert_eq!(hit(&layout, 50.0, 100.0), Some((0, Zone::Border)));
        assert_eq!(hit(&layout, 50.0, 103.9), Some((0, Zone::Border)));
        assert_eq!(hit(&layout, 50.0, 104.0), Some((1, Zone::Name)));
        assert_eq!(hit(&layout, 50.0, 96.9), Some((0, Zone::Name)));
        // The last lane (bottom at 200) has a border too.
        assert_eq!(hit(&layout, 3.0, 203.0), Some((2, Zone::Border)));
        // Outside: the waveforms, above the axis edge, below the last border, left of the strip.
        assert_eq!(hit(&layout, GUTTER_W, 50.0), None);
        assert_eq!(hit(&layout, 50.0, 27.9), None);
        assert_eq!(hit(&layout, 50.0, 204.0), None);
        assert_eq!(hit(&layout, -1.0, 50.0), None);
    }

    #[test]
    fn a_decoder_lane_has_no_zones_and_no_border_of_its_own() {
        let layout = Layout::new(&[channel(40), decoder()], 300.0, 0.0);
        // The channel lane is 28..68; the decoder lane 68..140.
        assert_eq!(hit(&layout, 3.0, 66.0), Some((0, Zone::Border)));
        assert_eq!(hit(&layout, 50.0, 100.0), None);
        assert_eq!(hit(&layout, 3.0, 139.0), None);
        assert_eq!(hit(&layout, 3.0, 142.0), None);
    }

    #[test]
    fn scrolling_moves_the_lanes_under_the_axis_and_is_limited() {
        // 32 channels of 60 px: 1,920 px in an area that shows 272.
        let specs: Vec<LaneSpec> = (0..32).map(|_| channel(60)).collect();
        let layout = Layout::new(&specs, 300.0, 0.0);
        assert!((layout.total_height() - 1_920.0).abs() < f32::EPSILON);
        assert!((layout.max_offset() - 1_648.0).abs() < f32::EPSILON);
        let scrolled = Layout::new(&specs, 300.0, 100.0);
        assert!((scrolled.lane(0).unwrap().top - (28.0 - 100.0)).abs() < f32::EPSILON);
        assert!((scrolled.lane(2).unwrap().top - (28.0 + 120.0 - 100.0)).abs() < f32::EPSILON);
        // A point above the axis edge is outside even if a lane reaches under the axis.
        assert_eq!(hit(&scrolled, 50.0, 20.0), None);
        // y = 28 is lane 1 (lane 1 spans -12+60=... lane 0 spans -72..-12, lane 1 spans -12..48).
        assert_eq!(hit(&scrolled, 50.0, 28.0), Some((1, Zone::Name)));
        // The offset is limited to [0, max].
        let too_far = Layout::new(&specs, 300.0, 5_000.0);
        assert!((too_far.offset() - 1_648.0).abs() < f32::EPSILON);
        let negative = Layout::new(&specs, 300.0, -30.0);
        assert!(negative.offset().abs() < f32::EPSILON);
    }

    #[test]
    fn a_dragged_lane_lands_between_the_others_by_midpoints() {
        let layout = three(); // lanes 28..100, 100..160, 160..200: midpoints 64, 130, 180
        assert_eq!(layout.landing(10.0), 0);
        assert_eq!(layout.landing(64.0), 0);
        assert_eq!(layout.landing(65.0), 1);
        assert_eq!(layout.landing(131.0), 2);
        assert_eq!(layout.landing(1_000.0), 3);
        // The move target of `MoveChannel { from, to }` counts positions after the lane was taken out.
        assert_eq!(layout.move_target(0, 1_000.0), 2);
        assert_eq!(layout.move_target(2, 10.0), 0);
        assert_eq!(layout.move_target(1, 70.0), 1, "released where it was");
        assert_eq!(layout.move_target(1, 140.0), 1);
        assert_eq!(layout.move_target(1, 1_000.0), 2);
        // The line that shows where it will land: the top of the lane it goes before, or the
        // bottom of the last one.
        assert!((layout.landing_line(0) - 28.0).abs() < f32::EPSILON);
        assert!((layout.landing_line(2) - 160.0).abs() < f32::EPSILON);
        assert!((layout.landing_line(3) - 200.0).abs() < f32::EPSILON);
    }

    #[test]
    fn the_lanes_between_the_two_slots_make_room_by_the_height_of_the_carried_lane() {
        let layout = three(); // heights 72, 60, 40
        // Carrying lane 0 down to the slot of lane 2: lanes 1 and 2 move up by 72.
        assert_eq!(layout.shifts(0, 2, 3), vec![0.0, -72.0, -72.0]);
        assert_eq!(layout.shifts(0, 1, 3), vec![0.0, -72.0, 0.0]);
        // Carrying lane 2 up to the top: lanes 0 and 1 move down by 40.
        assert_eq!(layout.shifts(2, 0, 3), vec![40.0, 40.0, 0.0]);
        // Released where it was: nothing moves.
        assert_eq!(layout.shifts(1, 1, 3), vec![0.0, 0.0, 0.0]);
        // A count beyond the lanes is harmless.
        assert_eq!(layout.shifts(0, 2, 5).len(), 5);
    }

    #[test]
    fn easing_moves_toward_the_target_without_overshooting_and_arrives() {
        let mut value = 0.0_f32;
        let mut last = 0.0_f32;
        for _ in 0..200 {
            value = ease(value, 50.0, 0.016);
            assert!(value >= last && value <= 50.0, "{value}");
            last = value;
        }
        assert!((value - 50.0).abs() < 0.01, "{value}");
        // A long pause jumps to the target; a negative target works the same way.
        assert!((ease(0.0, -30.0, 10.0) + 30.0).abs() < 0.01);
        assert!((ease(5.0, 5.0, 0.016) - 5.0).abs() < f32::EPSILON);
    }

    #[test]
    fn dragging_never_lands_among_decoder_lanes() {
        let layout = Layout::new(
            &[channel(40), channel(40), decoder(), decoder()],
            400.0,
            0.0,
        );
        // Only the two channel lanes count, however far down the pointer is.
        assert_eq!(layout.landing(10_000.0), 2);
        assert_eq!(layout.move_target(0, 10_000.0), 1);
    }

    #[test]
    fn a_height_is_limited_to_the_range_and_follows_the_pointer() {
        assert_eq!(clamp_height(10.0), 24);
        assert_eq!(clamp_height(24.0), 24);
        assert_eq!(clamp_height(60.4), 60);
        assert_eq!(clamp_height(60.5), 61);
        assert_eq!(clamp_height(240.0), 240);
        assert_eq!(clamp_height(300.0), 240);
        assert_eq!(clamp_height(f32::NAN), 24);
        assert_eq!(clamp_height(-5.0), 24);
        // The border moves with the pointer from the height it started at.
        assert_eq!(resized_height(60.0, 100.0, 130.0), 90);
        assert_eq!(resized_height(60.0, 100.0, 50.0), 24);
        assert_eq!(resized_height(60.0, 100.0, 900.0), 240);
    }
}
