//! Zoom and pan arithmetic of the visible window (data-model "View window and zoom").
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::manual_range_contains
)]

use la_host::window::{ViewWindow, WindowLimits};

fn lim(capture_length: u64, pixels: u32) -> WindowLimits {
    WindowLimits {
        capture_length,
        pixels,
    }
}

fn w(start: u64, length: u64) -> ViewWindow {
    ViewWindow { start, length }
}

#[test]
fn the_length_limits_follow_the_pixel_width() {
    // At most 64 pixels per sample: ceil(800 / 64) = 13.
    assert_eq!(lim(1000, 800).min_length(), 13);
    assert_eq!(lim(1000, 64).min_length(), 1);
    assert_eq!(lim(1000, 65).min_length(), 2);
    assert_eq!(lim(1000, 1).min_length(), 1);
    assert_eq!(lim(1000, 0).min_length(), 1);
    assert_eq!(lim(1000, 800).max_length(), 1000);
    // A capture shorter than the minimum: the capture wins.
    assert_eq!(lim(5, 800).min_length(), 5);
    assert_eq!(lim(5, 800).max_length(), 5);
    // An empty capture has a nominal window of the minimum length.
    assert_eq!(lim(0, 800).min_length(), 13);
    assert_eq!(lim(0, 800).max_length(), 13);
}

#[test]
fn fit_all_shows_the_whole_capture() {
    assert_eq!(ViewWindow::fit_all(&lim(1000, 800)), w(0, 1000));
    assert_eq!(ViewWindow::fit_all(&lim(1, 800)), w(0, 1));
    assert_eq!(ViewWindow::fit_all(&lim(0, 800)), w(0, 13));
}

#[test]
fn zoom_in_keeps_the_anchor_where_it_is() {
    let l = lim(1000, 800);
    let all = ViewWindow::fit_all(&l);
    // Centre anchor.
    assert_eq!(all.zoom(&l, 1, 2, (1, 2)), w(250, 500));
    // Left edge anchor.
    assert_eq!(all.zoom(&l, 1, 2, (0, 1)), w(0, 500));
    // Right edge anchor.
    assert_eq!(all.zoom(&l, 1, 2, (1, 1)), w(500, 500));
    // A quarter of the way: sample 250 stays a quarter of the way into the window.
    assert_eq!(all.zoom(&l, 1, 2, (1, 4)), w(125, 500));
}

#[test]
fn zoom_in_stops_at_the_minimum_length() {
    let l = lim(1000, 800);
    let mut window = ViewWindow::fit_all(&l);
    let mut lengths = Vec::new();
    for _ in 0..12 {
        window = window.zoom(&l, 1, 2, (1, 2));
        lengths.push(window.length);
    }
    assert_eq!(&lengths[..7], &[500, 250, 125, 62, 31, 15, 13]);
    assert!(lengths[7..].iter().all(|len| *len == 13));
}

#[test]
fn zoom_out_stops_at_the_whole_capture() {
    let l = lim(1000, 800);
    assert_eq!(w(250, 500).zoom(&l, 2, 1, (1, 2)), w(0, 1000));
    assert_eq!(w(900, 100).zoom(&l, 2, 1, (1, 1)), w(800, 200));
    assert_eq!(w(0, 100).zoom(&l, 2, 1, (0, 1)), w(0, 200));
    assert_eq!(w(0, 1000).zoom(&l, 2, 1, (1, 2)), w(0, 1000));
}

#[test]
fn pan_moves_and_clamps_at_both_ends() {
    let l = lim(1000, 800);
    assert_eq!(w(250, 500).pan_samples(&l, 100), w(350, 500));
    assert_eq!(w(250, 500).pan_samples(&l, -100), w(150, 500));
    assert_eq!(w(250, 500).pan_samples(&l, 300), w(500, 500));
    assert_eq!(w(250, 500).pan_samples(&l, -1000), w(0, 500));
    assert_eq!(w(250, 500).pan_samples(&l, i64::MAX), w(500, 500));
    assert_eq!(w(250, 500).pan_samples(&l, i64::MIN), w(0, 500));
    // The whole capture cannot move.
    assert_eq!(w(0, 1000).pan_samples(&l, 50), w(0, 1000));
}

#[test]
fn pan_by_pixels_scales_with_the_zoom() {
    let l = lim(1000, 800);
    // 500 samples over 800 pixels: 80 pixels are 50 samples.
    assert_eq!(w(250, 500).pan_pixels(&l, 80), w(300, 500));
    assert_eq!(w(250, 500).pan_pixels(&l, -80), w(200, 500));
    assert_eq!(w(250, 500).pan_pixels(&l, 0), w(250, 500));
}

#[test]
fn start_end_and_zoom_to_range() {
    let l = lim(1000, 800);
    assert_eq!(w(250, 500).go_to_start(&l), w(0, 500));
    assert_eq!(w(250, 500).go_to_end(&l), w(500, 500));
    assert_eq!(ViewWindow::zoom_to(&l, 100, 300), w(100, 200));
    // A range shorter than the minimum is widened around its centre.
    assert_eq!(ViewWindow::zoom_to(&l, 500, 502), w(495, 13));
    // A range beyond the capture is clamped; an inverted range is swapped.
    assert_eq!(ViewWindow::zoom_to(&l, 900, 5000), w(900, 100));
    assert_eq!(ViewWindow::zoom_to(&l, 300, 100), w(100, 200));
}

#[test]
fn reveal_moves_the_window_only_when_needed() {
    let l = lim(1000, 800);
    assert_eq!(w(250, 500).reveal(&l, 400), w(250, 500));
    // Outside: the sample is centred (and clamped to the capture).
    assert_eq!(w(250, 500).reveal(&l, 900), w(500, 500));
    assert_eq!(w(250, 500).reveal(&l, 10), w(0, 500));
    assert_eq!(w(0, 100).reveal(&l, 600), w(550, 100));
}

#[test]
fn an_empty_capture_has_a_nominal_window() {
    let l = lim(0, 800);
    let window = ViewWindow::fit_all(&l);
    assert!(window.is_valid(&l));
    assert_eq!(window.zoom(&l, 1, 2, (1, 2)), window);
    assert_eq!(window.pan_samples(&l, 10), window);
    assert_eq!(window.go_to_end(&l), window);
}

#[test]
fn nothing_overflows_at_the_top_of_the_range() {
    let big = u64::MAX / 2;
    for pixels in [1, 800, 4096] {
        let l = lim(big, pixels);
        let all = ViewWindow::fit_all(&l);
        assert!(all.is_valid(&l));
        let zoomed = all.zoom(&l, 1, 2, (1, 2));
        assert!(zoomed.is_valid(&l));
        assert_eq!(zoomed.length, big / 2);
        let panned = zoomed.pan_samples(&l, i64::MAX).pan_samples(&l, i64::MAX);
        assert!(panned.is_valid(&l));
        assert_eq!(panned.start + panned.length, big);
        let deep = (0..80).fold(all, |w, _| w.zoom(&l, 1, 2, (1, 3)));
        assert!(deep.is_valid(&l));
        assert_eq!(deep.length, l.min_length());
    }
}

struct Lcg(u64);

impl Lcg {
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        self.0 >> 20
    }
}

#[test]
fn a_random_walk_of_100_000_operations_keeps_the_invariants() {
    let mut rng = Lcg(42);
    for (capture_length, pixels) in [
        (0_u64, 800_u32),
        (1, 1),
        (7, 800),
        (1000, 800),
        (10_u64.pow(9), 4096),
    ] {
        let l = lim(capture_length, pixels);
        let mut window = ViewWindow::fit_all(&l);
        for step in 0..20_000_u32 {
            window = match rng.next() % 8 {
                0 => window.zoom(&l, 1, 1 + rng.next() % 4, (rng.next() % 101, 100)),
                1 => window.zoom(&l, 1 + rng.next() % 4, 1, (rng.next() % 101, 100)),
                2 => window.pan_samples(&l, i64::try_from(rng.next() % 100_000).unwrap() - 50_000),
                3 => window.pan_pixels(&l, i64::try_from(rng.next() % 2000).unwrap() - 1000),
                4 => ViewWindow::zoom_to(
                    &l,
                    rng.next() % (capture_length + 10),
                    rng.next() % (capture_length + 10),
                ),
                5 => window.reveal(&l, rng.next() % (capture_length + 10)),
                6 => window.go_to_start(&l),
                _ => window.go_to_end(&l),
            };
            assert!(
                window.is_valid(&l),
                "{capture_length} {pixels} step {step}: {window:?}"
            );
        }
    }
}
