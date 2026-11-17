//! Opt-in tests with the real analyzer wired to the Arduino Nano rig (spec 013, task T028).
//!
//! They are ignored by default and never run in CI. To run them:
//!
//! ```text
//! LA_RIG_PORT=COM7 cargo test -p la-driver-weact --test rig -- --ignored --nocapture
//! ```
//!
//! Without `LA_RIG_PORT`, or without the board, each test prints `SKIPPED: <reason>` and passes.
//! `LA_RIG_RECORD=<directory>` also writes the USB exchange of each test as a recording.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::panic
)]

#[path = "rig/mod.rs"]
mod support;

use std::sync::Arc;
use std::time::Duration;

use la_devices::{
    Acquisition, AcquisitionOptions, DeviceConfig, DeviceId, DeviceOptions, DeviceProvider,
    DeviceRegistry, EndReason, RegistryConfig,
};
use la_driver_weact::WeActProvider;
use la_driver_weact::protocol::SAMPLE_RATES;
use la_driver_weact::recording::{RecordingBackend, Sink, recording_of};
use la_driver_weact::system_backend;
use la_driver_weact::usb::SystemTimer;

use support::patterns::{check_counter, check_hold, check_square, check_walk};
use support::{Guard, RECORD_VARIABLE, Rig, skip};

/// Everything a rig test needs: the Nano, the registry over the real bus, the board's identity.
struct Fixture {
    guard: Guard,
    registry: DeviceRegistry,
    id: DeviceId,
    sink: Sink,
    name: &'static str,
}

impl Fixture {
    /// `None` after printing why the test is skipped.
    fn open(name: &'static str) -> Option<Self> {
        let mut rig = match Rig::connect() {
            Ok(rig) => rig,
            Err(reason) => {
                skip(&reason);
                return None;
            }
        };
        // Start from silence: the analyzer sees no pattern until the test asks for one.
        if let Err(reason) = rig.expect_ok("STOP") {
            skip(&reason);
            return None;
        }
        let sink: Sink = Arc::default();
        let backend = Arc::new(RecordingBackend::new(system_backend(), Arc::clone(&sink)));
        let provider = Arc::new(WeActProvider::with_backend(
            backend,
            Arc::new(SystemTimer::new()),
        ));
        let registry = DeviceRegistry::new(RegistryConfig {
            scan_interval: Duration::from_millis(200),
            scan_timeout: Duration::from_secs(2),
        });
        registry.add_provider(provider.clone() as Arc<dyn DeviceProvider>);
        // A blank board is prepared by the provider: allow for it.
        let started = std::time::Instant::now();
        let id = loop {
            if let Some(device) = registry
                .list()
                .into_iter()
                .find(|d| d.id.as_str().starts_with("weact:"))
            {
                break device.id;
            }
            if started.elapsed() > Duration::from_secs(20) {
                skip("no WeAct analyzer board was found within 20 s");
                return None;
            }
            std::thread::sleep(Duration::from_millis(100));
        };
        Some(Self {
            guard: Guard { rig },
            registry,
            id,
            sink,
            name,
        })
    }

    /// Captures `length` samples at `rate` with all eight channels and returns one byte a sample.
    fn capture(&self, rate: u64, length: u64) -> Result<Vec<u8>, String> {
        let config = DeviceConfig {
            enabled_channels: 0xFF,
            sample_rate_hz: rate,
            trigger: None,
            capture_length: Some(length),
            options: DeviceOptions::None,
            accept_budget_risk: false,
        };
        // The board may still be preparing for a moment.
        let started = std::time::Instant::now();
        let acquisition: Acquisition = loop {
            match self.registry.start(
                &self.id,
                config.clone(),
                AcquisitionOptions {
                    channel_capacity: 1_000,
                    ..AcquisitionOptions::default()
                },
            ) {
                Ok(acquisition) => break acquisition,
                Err(error)
                    if started.elapsed() < Duration::from_secs(20)
                        && error.detail().contains("weact.preparing") =>
                {
                    std::thread::sleep(Duration::from_millis(100));
                }
                Err(error) => return Err(format!("start failed: {error}")),
            }
        };
        let result = acquisition
            .wait_timeout(Duration::from_secs(60))
            .ok_or("the capture did not end in 60 s")?;
        if result.end != EndReason::Completed {
            // The board may still be preparing when the capture opens it: try again.
            let preparing = result
                .error
                .as_ref()
                .is_some_and(|e| e.detail().contains("weact.preparing"));
            if preparing && started.elapsed() < Duration::from_secs(20) {
                std::thread::sleep(Duration::from_millis(200));
                return self.capture(rate, length);
            }
            return Err(format!("the capture ended with {result:?}"));
        }
        let snapshot = acquisition.capture().snapshot();
        let length = usize::try_from(snapshot.length()).map_err(|e| e.to_string())?;
        let mut bytes = vec![0_u8; length];
        for channel in 0..8_u8 {
            let mut level = snapshot.initial_high(channel).map_err(|e| e.to_string())?;
            let mut at = 0_usize;
            let edges = snapshot
                .transitions(channel, 0, length as u64)
                .map_err(|e| e.to_string())?;
            for edge in edges {
                let to = usize::try_from(edge.position).map_err(|e| e.to_string())?;
                if level {
                    for byte in &mut bytes[at..to] {
                        *byte |= 1 << channel;
                    }
                }
                level = !level;
                at = to;
            }
            if level {
                for byte in &mut bytes[at..] {
                    *byte |= 1 << channel;
                }
            }
        }
        Ok(bytes)
    }

    fn command(&mut self, text: &str) {
        self.guard.rig.expect_ok(text).unwrap();
        // Let the pattern settle before the capture starts.
        std::thread::sleep(Duration::from_millis(50));
    }

    /// Writes the exchange of the test when `LA_RIG_RECORD` names a directory.
    fn finish(self) {
        let Ok(dir) = std::env::var(RECORD_VARIABLE) else {
            return;
        };
        let recording = recording_of(
            &self.sink,
            &format!("recorded from a real board on the rig, test {}", self.name),
            "recorded",
        );
        let path = std::path::Path::new(&dir).join(format!("{}.rec", self.name));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(&path, recording.to_text()).unwrap();
        println!("recording written to {}", path.display());
    }
}

const IGNORE: &str = "needs the rig: set LA_RIG_PORT and run with --ignored";

#[test]
#[ignore = "needs the rig: set LA_RIG_PORT and run with --ignored"]
fn hold_on_each_of_the_eight_channels() {
    let _ = IGNORE;
    let Some(mut fixture) = Fixture::open("hold_each_channel") else {
        return;
    };
    for channel in 0..8_u8 {
        let value = 1_u8 << channel;
        fixture.command(&format!("HOLD {value:02X}"));
        let bytes = fixture.capture(1_000_000, 2_000).unwrap();
        check_hold(&bytes, value).unwrap_or_else(|m| panic!("channel {channel}: {m}"));
    }
    fixture.finish();
}

#[test]
#[ignore = "needs the rig: set LA_RIG_PORT and run with --ignored"]
fn walk_at_one_megasample_shows_every_channel_in_order() {
    let Some(mut fixture) = Fixture::open("walk_1mhz") else {
        return;
    };
    fixture.command("WALK 100");
    let bytes = fixture.capture(1_000_000, 20_000).unwrap();
    check_walk(&bytes, 1_000_000, 100).unwrap();
    fixture.finish();
}

#[test]
#[ignore = "needs the rig: set LA_RIG_PORT and run with --ignored"]
fn counter_over_three_hundred_milliseconds_counts_up_without_skipping() {
    let Some(mut fixture) = Fixture::open("counter_1mhz") else {
        return;
    };
    fixture.command("COUNTER 100");
    let bytes = fixture.capture(1_000_000, 300_000).unwrap();
    check_counter(&bytes, 1_000_000, 100).unwrap();
    fixture.finish();
}

#[test]
#[ignore = "needs the rig: set LA_RIG_PORT and run with --ignored"]
fn square_wave_toggles_all_channels_together() {
    let Some(mut fixture) = Fixture::open("square_500khz") else {
        return;
    };
    fixture.command("SQUARE 100");
    let bytes = fixture.capture(500_000, 50_000).unwrap();
    check_square(&bytes, 500_000, 100).unwrap();
    fixture.finish();
}

#[test]
#[ignore = "needs the rig: set LA_RIG_PORT and run with --ignored"]
fn every_rate_from_one_hundred_kilosamples_up_to_one_megasample_shows_the_walk() {
    let Some(mut fixture) = Fixture::open("walk_rates") else {
        return;
    };
    fixture.command("WALK 100");
    for rate in SAMPLE_RATES
        .iter()
        .copied()
        .filter(|r| (100_000..=1_000_000).contains(r))
    {
        // About forty steps of the walk at this rate.
        let length = rate * 100 * 40 / 1_000_000;
        let bytes = fixture.capture(rate, length).unwrap();
        check_walk(&bytes, rate, 100).unwrap_or_else(|m| panic!("{rate} S/s: {m}"));
    }
    fixture.finish();
}

#[test]
#[ignore = "needs the rig: set LA_RIG_PORT and run with --ignored"]
fn two_captures_in_a_row_give_the_same_structure() {
    let Some(mut fixture) = Fixture::open("two_captures") else {
        return;
    };
    fixture.command("WALK 100");
    for round in 0..2 {
        let bytes = fixture.capture(1_000_000, 20_000).unwrap();
        check_walk(&bytes, 1_000_000, 100).unwrap_or_else(|m| panic!("capture {round}: {m}"));
    }
    fixture.finish();
}
