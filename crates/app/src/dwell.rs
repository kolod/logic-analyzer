//! The clock of the automatic zoom of a crowded snapping zone (`specs/015-markers-rework`,
//! research R4): when the pointer has rested for two seconds, and the 0.4 s animation that
//! follows. The host does the arithmetic of where to zoom and of putting the view back; this is
//! only time, kept as plain data so that it is tested without a screen or a real clock.

use std::time::{Duration, Instant};

use la_host::markers::{Pointer, ZoomTarget};

/// How long the pointer must rest.
pub const DWELL: Duration = Duration::from_secs(2);
/// How far, in pixels, the pointer may move and still be at rest.
pub const STILL: f32 = 3.0;
/// How long the animation of the zoom takes.
pub const ANIMATION: Duration = Duration::from_millis(400);

/// Whether the pointer is resting, and since when.
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct Dwell {
    at: Option<(Pointer, Instant)>,
    fired: bool,
}

impl Dwell {
    /// The pointer is at `pointer` (or has left the view: `None`) at `now`. A move of
    /// `STILL` pixels or more, or into another lane, starts the wait again.
    pub fn moved(&mut self, pointer: Option<Pointer>, now: Instant) {
        let Some(pointer) = pointer else {
            *self = Self::default();
            return;
        };
        let resting = self.at.is_some_and(|(origin, _)| {
            origin.lane == pointer.lane && (origin.x - pointer.x).abs() < STILL
        });
        if !resting {
            self.at = Some((pointer, now));
            self.fired = false;
        }
    }

    /// The pointer that has rested for [`DWELL`], once per rest.
    pub fn due(&mut self, now: Instant) -> Option<Pointer> {
        let (pointer, since) = self.at?;
        if self.fired || now.saturating_duration_since(since) < DWELL {
            return None;
        }
        self.fired = true;
        Some(pointer)
    }

    /// Forgets the rest (the placement ended).
    pub fn reset(&mut self) {
        *self = Self::default();
    }
}

/// The animation from one window to the target.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ZoomAnimation {
    from: (u64, u64),
    to: ZoomTarget,
    started: Instant,
}

impl ZoomAnimation {
    /// An animation from the window `(start, length)` to `to`, started at `now`.
    #[must_use]
    pub fn new(from: (u64, u64), to: ZoomTarget, now: Instant) -> Self {
        Self {
            from,
            to,
            started: now,
        }
    }

    /// The window of the frame at `now` as `(start, length)` and whether it is the last. The
    /// movement slows down towards the end (an ease-out), and the last frame is the target
    /// exactly.
    #[must_use]
    pub fn frame(&self, now: Instant) -> ((u64, u64), bool) {
        let elapsed = now.saturating_duration_since(self.started).as_secs_f64();
        let done = elapsed >= ANIMATION.as_secs_f64();
        if done {
            return ((self.to.start, self.to.length), true);
        }
        let t = elapsed / ANIMATION.as_secs_f64();
        let eased = 1.0 - (1.0 - t).powi(3);
        let mix = |a: u64, b: u64| -> u64 {
            let value = a as f64 + (b as f64 - a as f64) * eased;
            // A finite value between two window numbers.
            #[allow(
                clippy::cast_possible_truncation,
                clippy::cast_sign_loss,
                reason = "an interpolation between two u64 values"
            )]
            let rounded = value.round().max(0.0) as u64;
            rounded
        };
        (
            (
                mix(self.from.0, self.to.start),
                mix(self.from.1, self.to.length),
            ),
            false,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(x: f32, lane: Option<u8>) -> Pointer {
        Pointer { x, lane }
    }

    #[test]
    fn the_pointer_must_rest_for_two_seconds() {
        let t0 = Instant::now();
        let mut dwell = Dwell::default();
        dwell.moved(Some(at(100.0, Some(0))), t0);
        assert_eq!(dwell.due(t0 + Duration::from_millis(1_999)), None);
        assert_eq!(
            dwell.due(t0 + Duration::from_secs(2)),
            Some(at(100.0, Some(0)))
        );
    }

    #[test]
    fn it_fires_once_per_rest() {
        let t0 = Instant::now();
        let mut dwell = Dwell::default();
        dwell.moved(Some(at(100.0, Some(0))), t0);
        assert!(dwell.due(t0 + Duration::from_secs(3)).is_some());
        assert_eq!(dwell.due(t0 + Duration::from_secs(4)), None);
        // A new rest fires again.
        let t1 = t0 + Duration::from_secs(5);
        dwell.moved(Some(at(300.0, Some(0))), t1);
        assert_eq!(dwell.due(t1 + Duration::from_secs(1)), None);
        assert!(dwell.due(t1 + Duration::from_secs(2)).is_some());
    }

    #[test]
    fn a_small_movement_is_still_resting_and_a_big_one_starts_again() {
        let t0 = Instant::now();
        let mut dwell = Dwell::default();
        dwell.moved(Some(at(100.0, Some(0))), t0);
        // Under three pixels from where the rest began, at any time: still resting.
        dwell.moved(Some(at(102.9, Some(0))), t0 + Duration::from_millis(900));
        dwell.moved(Some(at(98.0, Some(0))), t0 + Duration::from_millis(1_500));
        assert!(dwell.due(t0 + Duration::from_secs(2)).is_some());
        // Three pixels or more: the wait starts again from there.
        let mut dwell = Dwell::default();
        dwell.moved(Some(at(100.0, Some(0))), t0);
        dwell.moved(Some(at(103.0, Some(0))), t0 + Duration::from_millis(1_900));
        assert_eq!(dwell.due(t0 + Duration::from_secs(2)), None);
        assert!(dwell.due(t0 + Duration::from_millis(3_900)).is_some());
    }

    #[test]
    fn another_lane_or_leaving_the_view_starts_again() {
        let t0 = Instant::now();
        let mut dwell = Dwell::default();
        dwell.moved(Some(at(100.0, Some(0))), t0);
        dwell.moved(Some(at(100.0, Some(1))), t0 + Duration::from_millis(1_900));
        assert_eq!(dwell.due(t0 + Duration::from_secs(2)), None);
        dwell.moved(None, t0 + Duration::from_secs(3));
        assert_eq!(dwell.due(t0 + Duration::from_secs(30)), None);
        dwell.moved(Some(at(100.0, Some(1))), t0 + Duration::from_secs(31));
        assert!(dwell.due(t0 + Duration::from_secs(33)).is_some());
        dwell.reset();
        assert_eq!(dwell.due(t0 + Duration::from_secs(99)), None);
    }

    #[test]
    fn the_animation_eases_out_and_ends_on_the_target_exactly() {
        let t0 = Instant::now();
        let animation = ZoomAnimation::new(
            (0, 12_000),
            ZoomTarget {
                start: 1_000,
                length: 40,
            },
            t0,
        );
        assert_eq!(animation.frame(t0), ((0, 12_000), false));
        // Halfway in time the movement has covered 1 - (1/2)^3 = 87.5 % of the way.
        let ((start, length), done) = animation.frame(t0 + Duration::from_millis(200));
        assert!(!done);
        assert_eq!(start, 875);
        assert_eq!(length, 12_000 - (11_960_u64 * 7 / 8));
        // It never moves back.
        let mut last = (0_u64, u64::MAX);
        for ms in (0..400).step_by(20) {
            let (frame, _) = animation.frame(t0 + Duration::from_millis(ms));
            assert!(
                frame.0 >= last.0 && frame.1 <= last.1,
                "{frame:?} after {last:?}"
            );
            last = frame;
        }
        assert_eq!(
            animation.frame(t0 + Duration::from_millis(400)),
            ((1_000, 40), true)
        );
        assert_eq!(
            animation.frame(t0 + Duration::from_secs(9)),
            ((1_000, 40), true)
        );
    }
}
