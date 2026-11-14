//! The cost of a frame does not grow with the length of the capture (constitution Principle VIII,
//! spec SC-002). Measured in index probes, so the check does not depend on the machine.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects
)]

mod common;

use common::{TempDir, finished_shell};
use la_host::window::ViewSize;
use la_host::{Command, ConfigEdit, SignalProfile};

fn probes_for(length: u64, window_fraction: (u64, u64)) -> (u64, usize) {
    let dir = TempDir::new("scaling");
    let mut shell = {
        // A sparse 16-channel capture: the same signal statistics at every length.
        let mut shell = la_host::Shell::new(la_host::ShellConfig::for_tests(dir.path()));
        for edit in [
            ConfigEdit::Realtime(false),
            ConfigEdit::Channels(0xffff),
            ConfigEdit::SampleRate(1_000_000),
            ConfigEdit::Length(length),
            ConfigEdit::Seed(Some(11)),
            ConfigEdit::Profile(SignalProfile::Sparse),
            ConfigEdit::AcceptBudgetRisk(true),
        ] {
            shell.apply(Command::EditConfig(edit));
        }
        shell.apply(Command::StartCapture);
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(120);
        while shell.state().activity != la_host::Activity::Idle {
            shell.poll(std::time::Instant::now());
            assert!(std::time::Instant::now() < deadline);
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
        shell
    };
    let _ = &mut shell;
    let plan = shell.frame_plan(ViewSize {
        pixels: 2000,
        scale: 1.0,
    });
    assert_eq!(plan.channels.len(), 16);
    let _ = window_fraction;
    (plan.stats.probes, plan.stats.segments)
}

#[test]
fn a_ten_times_longer_capture_costs_less_than_twice_the_probes() {
    let (short_probes, short_segments) = probes_for(4_000_000, (1, 1));
    let (long_probes, long_segments) = probes_for(40_000_000, (1, 1));
    assert!(short_probes > 0 && long_probes > 0);
    assert!(
        long_probes < short_probes * 2,
        "probes grew from {short_probes} to {long_probes}"
    );
    // The number of drawing primitives is bounded by the pixel count, not by the capture.
    assert!(short_segments <= 16 * 2000 && long_segments <= 16 * 2000);
    let _ = finished_shell;
}
