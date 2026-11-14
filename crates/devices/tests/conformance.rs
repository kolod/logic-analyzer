//! The shared conformance suite (spec US8, FR-032).
//!
//! The suite must pass for well-behaved devices and must fail, naming the broken rule, for
//! deliberately broken ones, so it is not vacuous.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::panic
)]

mod common;

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use common::descriptor;
use la_devices::harness::conformance::{
    Expectations, check_capability_enforcement, check_descriptor, check_failure_containment,
    check_finite_capture, check_start_stop, run_all,
};
use la_devices::harness::{ScriptedDevice, Step, TestProvider};
use la_devices::{DeviceId, DeviceProvider, Flow, VirtualProvider, error};

fn virtual_id() -> DeviceId {
    DeviceId::new("virtual", "0")
}

/// A provider whose single device replays 400,000 samples (and can wait for the store).
fn scripted_provider() -> (Arc<dyn DeviceProvider>, DeviceId) {
    let provider = TestProvider::new("usb");
    let d = descriptor("usb", "scripted", 8);
    let id = d.id.clone();
    let for_factory = d.clone();
    provider.plug_with(
        d,
        Arc::new(move || {
            let steps: Vec<Step> = (0..2_000)
                .map(|i| Step::Samples((0..200_u32).map(|s| (s + i) & 0xF).collect()))
                .collect();
            Box::new(ScriptedDevice::new(for_factory.clone(), steps).with_flow(Flow::Throttleable))
        }),
    );
    (Arc::new(provider), id)
}

fn err_of(result: Result<(), String>) -> String {
    result.expect_err("the check must fail for a broken device")
}

// ---- well-behaved devices pass ---------------------------------------------------------

#[test]
fn the_virtual_device_passes_every_check() {
    run_all(
        Arc::new(VirtualProvider),
        &virtual_id(),
        Expectations::default(),
    );
}

#[test]
fn a_scripted_device_passes_every_check() {
    let (provider, id) = scripted_provider();
    // The script ends by itself and ignores the requested length.
    let expectations = Expectations {
        finite_capture_samples: None,
        ..Expectations::default()
    };
    run_all(provider, &id, expectations);
}

#[test]
fn the_individual_checks_pass_for_the_virtual_device() {
    let provider: Arc<dyn DeviceProvider> = Arc::new(VirtualProvider);
    let id = virtual_id();
    let e = Expectations::default();
    check_descriptor(&provider, &id).unwrap();
    check_capability_enforcement(&provider, &id).unwrap();
    check_start_stop(&provider, &id, &e).unwrap();
    check_finite_capture(&provider, &id, 10_000, &e).unwrap();
    check_failure_containment(&provider, &id, &e).unwrap();
}

// ---- broken devices fail, naming the rule ------------------------------------------------

#[test]
fn a_device_that_claims_zero_channels_fails_the_descriptor_check() {
    let provider = TestProvider::new("usb");
    let mut d = descriptor("usb", "broken", 8);
    d.capabilities.channels = 0;
    let id = d.id.clone();
    provider.plug(d);
    let provider: Arc<dyn DeviceProvider> = Arc::new(provider);
    let message = err_of(check_descriptor(&provider, &id));
    assert!(message.contains("[descriptor]"), "{message}");
    assert!(message.contains("channel count"), "{message}");
}

#[test]
fn a_device_that_is_not_listed_fails_the_descriptor_check() {
    let provider: Arc<dyn DeviceProvider> = Arc::new(TestProvider::new("usb"));
    let message = err_of(check_descriptor(
        &provider,
        &DeviceId::new("usb", "missing"),
    ));
    assert!(message.contains("[descriptor]"), "{message}");
}

#[test]
fn a_device_that_ignores_the_stop_request_fails_the_start_stop_check() {
    let provider = TestProvider::new("usb");
    let d = descriptor("usb", "deaf", 8);
    let id = d.id.clone();
    let for_factory = d.clone();
    provider.plug_with(
        d,
        Arc::new(move || {
            // Blocks in `read` for an hour: it never looks at the stop request.
            let device = ScriptedDevice::new(
                for_factory.clone(),
                vec![
                    Step::Samples(vec![1, 2, 3]),
                    Step::Hang(Duration::from_secs(3600)),
                ],
            );
            Box::new(device)
        }),
    );
    let provider: Arc<dyn DeviceProvider> = Arc::new(provider);
    let message = err_of(check_start_stop(&provider, &id, &Expectations::default()));
    assert!(message.contains("[start-stop]"), "{message}");
}

#[test]
fn a_device_that_cannot_start_fails_the_start_stop_check() {
    let provider = TestProvider::new("usb");
    let d = descriptor("usb", "dead", 8);
    let id = d.id.clone();
    let for_factory = d.clone();
    provider.plug_with(
        d,
        Arc::new(move || {
            Box::new(
                ScriptedDevice::new(for_factory.clone(), vec![])
                    .with_start_error(error::driver_error("the firmware did not answer")),
            )
        }),
    );
    let provider: Arc<dyn DeviceProvider> = Arc::new(provider);
    let message = err_of(check_start_stop(&provider, &id, &Expectations::default()));
    assert!(message.contains("[start-stop]"), "{message}");
    assert!(message.contains(error::DRIVER_ERROR), "{message}");
}

#[test]
fn a_device_that_cannot_be_reopened_after_an_acquisition_fails_the_containment_check() {
    let provider = TestProvider::new("usb");
    let d = descriptor("usb", "once", 8);
    let id = d.id.clone();
    let for_factory = d.clone();
    let opens = Arc::new(AtomicUsize::new(0));
    provider.plug_with(
        d,
        Arc::new(move || {
            let steps: Vec<Step> = (0..100_000).map(|_| Step::Samples(vec![1, 2, 3])).collect();
            let device =
                ScriptedDevice::new(for_factory.clone(), steps).with_flow(Flow::Throttleable);
            // The second open comes back broken: a driver that leaks state across sessions.
            if opens.fetch_add(1, Ordering::AcqRel) >= 1 {
                Box::new(device.with_start_error(error::driver_error("the firmware is wedged")))
            } else {
                Box::new(device)
            }
        }),
    );
    let provider: Arc<dyn DeviceProvider> = Arc::new(provider);
    let message = err_of(check_failure_containment(
        &provider,
        &id,
        &Expectations::default(),
    ));
    assert!(message.contains("[failure-containment]"), "{message}");
    assert!(message.contains("acquisition 2 of 2"), "{message}");
}

#[test]
fn a_device_that_produces_fewer_samples_than_asked_fails_the_finite_capture_check() {
    let provider = TestProvider::new("usb");
    let d = descriptor("usb", "short", 8);
    let id = d.id.clone();
    let for_factory = d.clone();
    provider.plug_with(
        d,
        Arc::new(move || {
            Box::new(ScriptedDevice::new(
                for_factory.clone(),
                vec![Step::Samples(vec![1, 2, 3]), Step::Finish],
            ))
        }),
    );
    let provider: Arc<dyn DeviceProvider> = Arc::new(provider);
    // Three samples were produced, ten thousand were asked for.
    let message = err_of(check_finite_capture(
        &provider,
        &id,
        10_000,
        &Expectations::default(),
    ));
    assert!(message.contains("[finite-capture]"), "{message}");
    assert!(message.contains("10000"), "{message}");
}
