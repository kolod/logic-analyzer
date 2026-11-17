//! The shared conformance suite of the device abstraction against the WeAct driver (spec 013,
//! task T012). The suite is the one of feature 004, not a copy: it checks the descriptor, the
//! enforcement of the capabilities, start and stop, a finite capture and the containment of
//! failures, and names every rule that is broken.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::panic
)]

mod common;

use std::time::Duration;

use la_devices::harness::conformance::{Expectations, run_all};
use la_driver_weact::testing::Capture;

use common::one_ready_board;

#[test]
fn the_driver_passes_the_shared_conformance_suite() {
    let (usb, key, provider) = one_ready_board();
    // The board delivers a pattern forever, as a real board does while it is read.
    usb.set_default_capture(
        &key,
        Capture::endless(vec![0x55, 0x33, 0x0F, 0xF0, 0xAA, 0x00]),
    );
    run_all(
        provider,
        &key.device_id(),
        Expectations {
            finite_capture_samples: Some(10_000),
            timeout: Duration::from_secs(20),
            stop_timeout: Duration::from_secs(5),
        },
    );
}
