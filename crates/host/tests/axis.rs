//! Tick selection and labels of the time axis (research R11).
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::manual_range_contains
)]

use la_host::axis::{AxisPlan, TickStep, build_ticks};
use la_host::window::ViewWindow;

fn axis(start: u64, length: u64, pixels: u32, rate: u64) -> AxisPlan {
    build_ticks(&ViewWindow { start, length }, pixels, rate)
}

fn step(mantissa: u8, exponent: i32) -> TickStep {
    TickStep { mantissa, exponent }
}

#[test]
fn one_second_at_800_pixels_has_tenth_second_ticks() {
    let plan = axis(0, 1000, 800, 1000);
    assert_eq!(plan.step, step(1, -1));
    assert_eq!(plan.ticks.len(), 11);
    // Exact decimal values: 0.3 is the number 0.3, not 0.30000000000000004.
    assert_eq!(plan.ticks[3].seconds, 0.3);
    assert_eq!(plan.ticks[7].seconds, 0.7);
    assert_eq!(plan.ticks[10].seconds, 1.0);
    // 0.3 s is sample 300 of 1000, which is pixel 240 of 800.
    assert!((plan.ticks[3].x - 240.0).abs() < 1e-9);
    let majors: Vec<bool> = plan.ticks.iter().map(|t| t.major).collect();
    assert_eq!(
        majors,
        [
            true, false, false, false, false, false, false, false, false, false, true
        ]
    );
}

#[test]
fn nanosecond_windows_pick_ten_nanosecond_ticks() {
    // 100 ns at 1 GHz over 800 pixels: 80 pixels are exactly 10 ns.
    let plan = axis(0, 100, 800, 1_000_000_000);
    assert_eq!(plan.step, step(1, -8));
    assert_eq!(plan.ticks.len(), 11);
    assert_eq!(plan.ticks[3].seconds, 3e-8);
}

#[test]
fn long_windows_pick_hundred_second_ticks() {
    // 1000 s over 1000 pixels: 80 pixels are 80 s, so the next step is 100 s.
    let plan = axis(0, 1_000_000, 1000, 1000);
    assert_eq!(plan.step, step(1, 2));
    assert_eq!(plan.ticks.len(), 11);
    assert_eq!(plan.ticks[10].seconds, 1000.0);
}

#[test]
fn the_two_and_five_steps_are_used() {
    // 1 s over 400 pixels: 80 pixels are 0.2 s.
    let plan = axis(0, 1000, 400, 1000);
    assert_eq!(plan.step, step(2, -1));
    assert_eq!(plan.ticks.len(), 6);
    let majors: Vec<bool> = plan.ticks.iter().map(|t| t.major).collect();
    assert_eq!(majors, [true, false, false, false, false, true]);

    // 3 s over 800 pixels: 80 pixels are 0.3 s, so 0.5 s is the first step that is wide enough.
    let plan = axis(0, 3000, 800, 1000);
    assert_eq!(plan.step, step(5, -1));
    assert_eq!(plan.ticks.len(), 7);
    let majors: Vec<bool> = plan.ticks.iter().map(|t| t.major).collect();
    assert_eq!(majors, [true, false, true, false, true, false, true]);
}

#[test]
fn the_first_tick_is_at_or_after_the_window_start() {
    let plan = axis(500, 100, 800, 1000);
    assert_eq!(plan.step, step(1, -2));
    assert_eq!(plan.ticks.len(), 11);
    assert_eq!(plan.ticks[0].seconds, 0.5);
    assert!(plan.ticks[0].x.abs() < 1e-9);
    assert_eq!(plan.ticks[3].seconds, 0.53);
    assert_eq!(plan.ticks[10].seconds, 0.6);
    // A window that starts between two ticks starts with the next one.
    let plan = axis(503, 100, 800, 1000);
    assert_eq!(plan.ticks[0].seconds, 0.51);
}

#[test]
fn a_window_of_one_sample_still_has_ticks() {
    let plan = axis(10, 1, 800, 1000);
    assert_eq!(plan.step, step(1, -4));
    assert_eq!(plan.ticks.len(), 11);
    assert_eq!(plan.ticks[0].seconds, 0.01);
    assert_eq!(plan.ticks[10].seconds, 0.011);
}

#[test]
fn ticks_are_never_closer_than_80_pixels_nor_far_apart_over_many_windows() {
    let rates = [
        1_u64,
        1000,
        12_345,
        1_000_000,
        500_000_000,
        1_000_000_000_000,
    ];
    let lengths = [1_u64, 2, 17, 1000, 123_456, 10_u64.pow(9), 10_u64.pow(15)];
    let widths = [200_u32, 400, 800, 1920, 4096];
    for rate in rates {
        for length in lengths {
            for pixels in widths {
                let plan = axis(length / 3, length, pixels, rate);
                assert!(plan.ticks.len() <= usize::try_from(pixels / 80 + 2).unwrap());
                for pair in plan.ticks.windows(2) {
                    let gap = pair[1].x - pair[0].x;
                    assert!(
                        gap >= 79.99 && gap <= 200.1,
                        "gap {gap} (rate {rate}, length {length}, width {pixels}, step {:?})",
                        plan.step
                    );
                }
                for tick in &plan.ticks {
                    assert!(tick.x >= -1e-6 && tick.x <= f64::from(pixels) + 1e-6);
                }
            }
        }
    }
}
