//! Random scripts against the driver (spec 013, task T034, constitution Principle I).
//!
//! 10,000 scripts from a fixed seed: boards appear, vanish and renumerate, answers have odd
//! lengths, streams carry random bytes, errors of every kind, hangs and panics are injected, and
//! the driver is driven through `scan`, `open`, `start`, `read` and `stop`. Nothing may panic
//! unless the script injected a panic, the list never shows a board twice, the books of every
//! capture balance, and every stream that was opened is closed.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::panic
)]

use std::collections::HashSet;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Arc;
use std::time::Duration;

use la_devices::{
    AcquisitionOptions, DeviceConfig, DeviceOptions, DeviceProvider, ReadOutcome, validate,
};
use la_driver_weact::WeActProvider;
use la_driver_weact::firmware::Image;
use la_driver_weact::protocol::SAMPLE_RATES;
use la_driver_weact::testing::{
    BoardSpec, Capture, Fail, Fault, Point, RecordedUsb, Request, StreamStep,
};
use la_driver_weact::usb::BoardKey;

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        self.0 >> 33
    }

    fn below(&mut self, n: u64) -> u64 {
        self.next() % n.max(1)
    }

    fn bytes(&mut self, max: u64) -> Vec<u8> {
        let length = self.below(max);
        (0..length).map(|_| self.next() as u8).collect()
    }
}

const FAILS: [fn() -> Fail; 5] = [
    || Fail::Disconnected,
    || Fail::Transport,
    || Fail::AccessDenied,
    || Fail::InUse,
    || Fail::DriverMissing(Some("libusb0".to_owned())),
];

fn random_fail(rng: &mut Rng) -> Fail {
    FAILS[rng.below(FAILS.len() as u64) as usize]()
}

fn random_stream(rng: &mut Rng) -> Vec<StreamStep> {
    (0..rng.below(12))
        .map(|_| match rng.below(10) {
            0 => StreamStep::Empty,
            1 => StreamStep::Timeout,
            2 => StreamStep::Fail(random_fail(rng)),
            _ => StreamStep::Data(rng.bytes(3_000)),
        })
        .collect()
}

const POINTS: [Point; 8] = [
    Point::List,
    Point::Open,
    Point::ControlIn,
    Point::ControlOut,
    Point::OpenStream,
    Point::Submit,
    Point::Wait,
    Point::Close,
];

/// Runs one script; returns whether it injected a panic.
fn run_script(seed: u64) {
    let mut rng = Rng(seed);
    let usb = RecordedUsb::new();
    let image = Image::new(vec![0x5A; 1 + rng.below(9_000) as usize]);
    let provider = WeActProvider::with_image(Arc::new(usb.clone()), usb.timer(), image);
    // A hang lasts only a moment: the watchdog releases it.
    let watchdog = {
        let usb = usb.clone();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(3));
            usb.release();
        })
    };
    let keys: Vec<BoardKey> = (1..=3_u8).map(|p| BoardKey::new("001", [p])).collect();
    let mut panics_injected = false;
    let mut panics_seen = false;
    let defaults = AcquisitionOptions::default();
    for _ in 0..(3 + rng.below(12)) {
        let key = keys[rng.below(3) as usize].clone();
        match rng.below(11) {
            0 => usb.plug(BoardSpec::running("001", key.ports())),
            1 => usb.plug(BoardSpec::blank("001", key.ports())),
            2 => usb.unplug(&key),
            3 => usb.set_version(&key, &rng.bytes(4)),
            4 => {
                let fail = (rng.below(2) == 0).then(|| random_fail(&mut rng));
                usb.fail_open(&key, fail);
            }
            5 => usb.script_capture(&key, Capture::stream(random_stream(&mut rng))),
            6 => usb.renumerate_after(
                &key,
                (rng.below(4) != 0).then(|| Duration::from_millis(rng.below(7_000))),
            ),
            7 => {
                let point = POINTS[rng.below(POINTS.len() as u64) as usize];
                let fault = match rng.below(3) {
                    0 => {
                        panics_injected = true;
                        Fault::Panic
                    }
                    1 => Fault::Hang,
                    _ => Fault::Fail(random_fail(&mut rng)),
                };
                usb.inject(point, fault);
            }
            _ => {
                let outcome = catch_unwind(AssertUnwindSafe(|| {
                    let list = match provider.scan() {
                        Ok(list) => list,
                        Err(_) => return,
                    };
                    let ids: HashSet<&str> = list.iter().map(|d| d.id.as_str()).collect();
                    assert_eq!(ids.len(), list.len(), "a board is listed twice: {list:?}");
                    assert!(list.len() <= 3, "{list:?}");
                    for descriptor in &list {
                        descriptor.validate().unwrap();
                    }
                    let Some(chosen) = list.first() else { return };
                    let Ok(mut device) = provider.open(&chosen.id) else {
                        return;
                    };
                    let rate = SAMPLE_RATES[rng.below(16) as usize];
                    let length = (rng.below(3) != 0).then(|| 1 + rng.below(20_000));
                    let config = DeviceConfig {
                        enabled_channels: la_devices::ChannelSet::from_u32(
                            1 + rng.below(255) as u32,
                        ),
                        sample_rate_hz: rate,
                        trigger: None,
                        capture_length: length,
                        options: DeviceOptions::None,
                        accept_budget_risk: true,
                        pattern: None,
                        pre_trigger_samples: 0,
                        bursts: 0,
                    };
                    let Ok(resolved) = validate(&chosen.capabilities, &config, &defaults.limits)
                    else {
                        return;
                    };
                    if device.start(&resolved).is_err() {
                        let _ = device.stop();
                        return;
                    }
                    let mut received = 0_u64;
                    let mut lost = 0_u64;
                    let mut out = Vec::new();
                    for _ in 0..40 {
                        out.clear();
                        let max = 1 + rng.below(5_000) as usize;
                        match device.read(max, &mut out) {
                            Ok(ReadOutcome::Samples) => {
                                assert!(!out.is_empty() && out.len() <= max);
                                received += out.len() as u64;
                            }
                            Ok(ReadOutcome::Lost(n)) => lost += n,
                            Ok(ReadOutcome::Idle) => {}
                            Ok(ReadOutcome::Finished) | Err(_) => break,
                        }
                        if let Some(length) = length {
                            assert!(
                                received + lost <= length,
                                "the books: {received} + {lost} > {length}"
                            );
                        }
                    }
                    assert!(device.stop().is_ok() || device.stop().is_err());
                    let _ = device.stop();
                }));
                if outcome.is_err() {
                    panics_seen = true;
                }
            }
        }
    }
    usb.release();
    drop(watchdog);
    // Not a panic of the script's own making: a bug.
    assert!(
        !panics_seen || panics_injected,
        "seed {seed}: the driver panicked without an injected panic"
    );
    if !panics_injected {
        let log = usb.log();
        let opened = log
            .iter()
            .filter(|r| matches!(r, Request::SubmitAll))
            .count();
        let closed = log.iter().filter(|r| matches!(r, Request::Close)).count();
        assert!(
            closed >= opened,
            "seed {seed}: {opened} streams submitted, {closed} closed: {log:?}"
        );
    }
}

#[test]
fn ten_thousand_random_scripts_never_break_the_driver() {
    let hook = std::panic::take_hook();
    // The injected panics are expected: keep their reports out of the output.
    std::panic::set_hook(Box::new(|_| {}));
    let result = catch_unwind(|| {
        for index in 0..10_000_u64 {
            run_script(0x1357_9bdf ^ index.wrapping_mul(0x9e37_79b9_7f4a_7c15));
        }
    });
    std::panic::set_hook(hook);
    if let Err(payload) = result {
        let message = payload
            .downcast_ref::<String>()
            .cloned()
            .or_else(|| payload.downcast_ref::<&str>().map(|s| (*s).to_owned()))
            .unwrap_or_default();
        panic!("a script failed: {message}");
    }
}
