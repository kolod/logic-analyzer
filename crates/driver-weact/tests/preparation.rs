//! Preparing a board that has no firmware (spec 013, user story 2, task T016).
//!
//! The download, the order of the requests and the times are worked out by hand from the USB
//! contract. A free-running virtual clock makes the waits instant; a stepped clock lets a test look
//! at a board in the middle of its preparation.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::panic
)]

mod common;

use std::sync::Arc;
use std::time::{Duration, Instant};

use la_core::Error;
use la_devices::{DeviceId, DeviceProvider};
use la_driver_weact::WeActProvider;
use la_driver_weact::firmware::Image;
use la_driver_weact::protocol::{CPUCS, REQUEST_GET_FW_VERSION, REQUEST_LOADER};
use la_driver_weact::testing::{BoardSpec, Fail, Fault, Point, RecordedUsb, Request};
use la_driver_weact::usb::{BoardKey, Timer, UsbBackend};

/// An image of `length` bytes that is easy to tell apart byte by byte.
fn image(length: usize) -> Image {
    Image::new((0..length).map(|i| (i * 7 + 3) as u8).collect())
}

fn make_provider(usb: &RecordedUsb, image: Image) -> Arc<WeActProvider> {
    Arc::new(WeActProvider::with_image(
        Arc::new(usb.clone()),
        usb.timer(),
        image,
    ))
}

fn blank(usb: &RecordedUsb, ports: &[u8]) -> BoardKey {
    let spec = BoardSpec::blank("001", ports);
    let key = spec.key.clone();
    usb.plug(spec);
    key
}

/// Scans and tries to open until the board is ready or its preparation has failed.
fn ready_or_error(provider: &WeActProvider, id: &DeviceId) -> Result<(), Error> {
    let started = Instant::now();
    loop {
        provider.scan().unwrap();
        match provider.open(id) {
            Ok(_) => return Ok(()),
            Err(error) if error.code() == "weact.preparing" => {}
            Err(error) => return Err(error),
        }
        assert!(
            started.elapsed() < Duration::from_secs(10),
            "the board neither became ready nor failed"
        );
        std::thread::sleep(Duration::from_millis(1));
    }
}

fn wait_for(what: &str, mut condition: impl FnMut() -> bool) {
    let started = Instant::now();
    while !condition() {
        assert!(
            started.elapsed() < Duration::from_secs(10),
            "waited for {what}"
        );
        std::thread::sleep(Duration::from_millis(1));
    }
}

/// The `(address, bytes)` of every chunk of the download, in order, and the writes to CPUCS.
type Chunk = (u16, Vec<u8>);

fn download(log: &[Request]) -> (Vec<Chunk>, Vec<Vec<u8>>) {
    let mut chunks = Vec::new();
    let mut cpucs = Vec::new();
    for request in log {
        if let Request::ControlOut(REQUEST_LOADER, value, 0, data) = request {
            if *value == CPUCS {
                cpucs.push(data.clone());
            } else {
                chunks.push((*value, data.clone()));
            }
        }
    }
    (chunks, cpucs)
}

#[test]
fn a_blank_board_is_prepared_and_becomes_ready() {
    let usb = RecordedUsb::new();
    let key = blank(&usb, &[1]);
    let provider = make_provider(&usb, image(10_000));
    ready_or_error(&provider, &key.device_id()).unwrap();
    let log = usb.log();
    // Hold the 8051 in reset, write the image from address 0 in chunks of at most 4,096 bytes
    // (4,096 + 4,096 + 1,808), release the reset.
    let (chunks, cpucs) = download(&log);
    let expected_image: Vec<u8> = (0..10_000_usize).map(|i| (i * 7 + 3) as u8).collect();
    assert_eq!(
        chunks,
        vec![
            (0, expected_image[..4_096].to_vec()),
            (4_096, expected_image[4_096..8_192].to_vec()),
            (8_192, expected_image[8_192..].to_vec()),
        ]
    );
    assert_eq!(cpucs, vec![vec![0x01], vec![0x00]], "hold, then release");
    let position = |wanted: &dyn Fn(&Request) -> bool| log.iter().position(wanted).unwrap();
    let hold = position(
        &|r| matches!(r, Request::ControlOut(REQUEST_LOADER, CPUCS, 0, d) if d == &[0x01]),
    );
    let first = position(&|r| matches!(r, Request::ControlOut(REQUEST_LOADER, 0, 0, _)));
    let release = position(
        &|r| matches!(r, Request::ControlOut(REQUEST_LOADER, CPUCS, 0, d) if d == &[0x00]),
    );
    let version = position(&|r| matches!(r, Request::ControlIn(REQUEST_GET_FW_VERSION, 0, 0, 2)));
    assert!(
        hold < first && first < release && release < version,
        "{log:?}"
    );
    assert_eq!(usb.downloaded_image(&key), expected_image);
    // The board returned after 900 ms of the virtual clock and was found within one poll of it.
    let now = usb.timer().now();
    assert!(
        now >= Duration::from_millis(900) && now <= Duration::from_millis(1_100),
        "{now:?}"
    );
    assert!(usb.runs_firmware(&key));
}

#[test]
fn a_board_that_runs_the_firmware_is_used_without_a_download() {
    let usb = RecordedUsb::new();
    let spec = BoardSpec::running("001", &[1]);
    let key = spec.key.clone();
    usb.plug(spec);
    let provider = make_provider(&usb, image(1_000));
    ready_or_error(&provider, &key.device_id()).unwrap();
    assert!(
        !usb.log()
            .iter()
            .any(|r| matches!(r, Request::ControlOut(REQUEST_LOADER, ..))),
        "nothing may be downloaded: {:?}",
        usb.log()
    );
}

#[test]
fn a_board_that_never_returns_fails_after_five_seconds_and_scan_stays_quick() {
    let usb = RecordedUsb::new();
    let key = blank(&usb, &[1]);
    usb.renumerate_after(&key, None);
    let provider = make_provider(&usb, image(1_000));
    let error = ready_or_error(&provider, &key.device_id()).unwrap_err();
    assert_eq!(error.code(), "weact.not_renumerated");
    let now = usb.timer().now();
    assert!(
        now >= Duration::from_secs(5) && now <= Duration::from_millis(5_100),
        "the limit is 5 s of waiting, not before and not much after: {now:?}"
    );
    // The failure was handed over once; the board is prepared again on the next scan.
    let started = Instant::now();
    provider.scan().unwrap();
    assert!(started.elapsed() < Duration::from_millis(200));
}

#[test]
fn a_refusal_of_the_system_is_reported_with_its_own_code() {
    for (fail, code) in [
        (Fail::AccessDenied, "weact.access_denied"),
        (
            Fail::DriverMissing(Some("libusb0".to_owned())),
            "weact.driver_missing",
        ),
        (Fail::InUse, "weact.in_use"),
    ] {
        let usb = RecordedUsb::new();
        let key = blank(&usb, &[1]);
        usb.fail_open(&key, Some(fail));
        let provider = make_provider(&usb, image(1_000));
        let error = ready_or_error(&provider, &key.device_id()).unwrap_err();
        assert_eq!(error.code(), code);
        if code == "weact.driver_missing" {
            assert!(error.detail().contains("libusb0"), "{}", error.detail());
        }
    }
}

#[test]
fn a_firmware_of_another_major_version_is_rejected() {
    let usb = RecordedUsb::new();
    let key = blank(&usb, &[1]);
    usb.set_version(&key, &[2, 0]);
    let provider = make_provider(&usb, image(1_000));
    let error = ready_or_error(&provider, &key.device_id()).unwrap_err();
    assert_eq!(error.code(), "weact.firmware_rejected");
    // Another minor version is fine.
    let usb = RecordedUsb::new();
    let key = blank(&usb, &[1]);
    usb.set_version(&key, &[1, 9]);
    let provider = make_provider(&usb, image(1_000));
    ready_or_error(&provider, &key.device_id()).unwrap();
}

#[test]
fn an_unusable_image_is_reported_and_nothing_is_sent_to_the_board() {
    for length in [0_usize, 16_385] {
        let usb = RecordedUsb::new();
        let key = blank(&usb, &[1]);
        let provider = make_provider(&usb, image(length));
        let error = ready_or_error(&provider, &key.device_id()).unwrap_err();
        assert_eq!(error.code(), "weact.firmware_invalid", "length {length}");
        assert!(
            !usb.log()
                .iter()
                .any(|r| matches!(r, Request::Open(_) | Request::ControlOut(..))),
            "length {length}: nothing may be sent: {:?}",
            usb.log()
        );
    }
    // The largest image the chip holds is accepted.
    let usb = RecordedUsb::new();
    let key = blank(&usb, &[1]);
    let provider = make_provider(&usb, image(16_384));
    ready_or_error(&provider, &key.device_id()).unwrap();
}

#[test]
fn the_board_stays_listed_once_while_it_is_off_the_bus_and_cannot_be_opened_yet() {
    let usb = RecordedUsb::stepped();
    let key = blank(&usb, &[1]);
    let provider = make_provider(&usb, image(5_000));
    let id = key.device_id();
    provider.scan().unwrap();
    // The download is done and the worker waits out the 300 ms head start of the board.
    wait_for("the release of the reset", || {
        usb.log()
            .iter()
            .any(|r| matches!(r, Request::ControlOut(REQUEST_LOADER, CPUCS, 0, d) if d == &[0x00]))
    });
    assert!(
        UsbBackend::list(&usb).unwrap().is_empty(),
        "the board left the bus"
    );
    // Still one board, the same identity, and not ready.
    for _ in 0..5 {
        let started = Instant::now();
        let list = provider.scan().unwrap();
        assert!(
            started.elapsed() < Duration::from_millis(200),
            "scan must not wait"
        );
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].id, id);
    }
    assert_eq!(provider.open(&id).err().unwrap().code(), "weact.preparing");
    // Time passes; the board comes back after 900 ms and the worker finishes.
    wait_for("the board to run the firmware", || {
        usb.timer().advance(Duration::from_millis(50));
        usb.runs_firmware(&key)
    });
    ready_or_error(&provider, &id).unwrap();
    let list = provider.scan().unwrap();
    assert_eq!(list.len(), 1, "never twice");
    assert_eq!(list[0].id, id, "the identity did not change");
}

#[test]
fn a_failed_board_is_still_listed_hands_over_its_failure_once_and_is_prepared_again() {
    let usb = RecordedUsb::new();
    let key = blank(&usb, &[1]);
    usb.fail_open(&key, Some(Fail::AccessDenied));
    let provider = make_provider(&usb, image(1_000));
    let id = key.device_id();
    let error = ready_or_error(&provider, &id).unwrap_err();
    assert_eq!(error.code(), "weact.access_denied");
    // The cause is gone; the board is prepared again on the next scan.
    usb.fail_open(&key, None);
    ready_or_error(&provider, &id).unwrap();
    assert_eq!(provider.scan().unwrap().len(), 1);
}

#[test]
fn a_failed_board_stays_listed_while_it_is_on_the_bus() {
    let usb = RecordedUsb::new();
    let key = blank(&usb, &[1]);
    usb.fail_open(&key, Some(Fail::InUse));
    let provider = make_provider(&usb, image(1_000));
    let id = key.device_id();
    provider.scan().unwrap();
    // The failure is handed over by `open` once the worker has finished; the board is listed
    // all the time, before and after.
    let started = Instant::now();
    let error = loop {
        assert_eq!(provider.scan().unwrap().len(), 1);
        match provider.open(&id) {
            Err(error) if error.code() != "weact.preparing" => break error,
            Err(_) => {}
            Ok(_) => panic!("the board cannot be opened"),
        }
        assert!(started.elapsed() < Duration::from_secs(10));
        std::thread::sleep(Duration::from_millis(1));
    };
    assert_eq!(error.code(), "weact.in_use");
    assert_eq!(provider.scan().unwrap().len(), 1);
}

#[test]
fn two_blank_boards_are_both_prepared() {
    let usb = RecordedUsb::new();
    let first = blank(&usb, &[1]);
    let second = blank(&usb, &[2]);
    let provider = make_provider(&usb, image(2_000));
    ready_or_error(&provider, &first.device_id()).unwrap();
    ready_or_error(&provider, &second.device_id()).unwrap();
    assert_eq!(provider.scan().unwrap().len(), 2);
    assert!(usb.runs_firmware(&first) && usb.runs_firmware(&second));
}

#[test]
fn a_hung_download_does_not_stop_scan_or_the_other_board() {
    let usb = RecordedUsb::new();
    let first = blank(&usb, &[1]);
    let second = blank(&usb, &[2]);
    // The first write of either board hangs until released.
    usb.inject(Point::ControlOut, Fault::Hang);
    let provider = make_provider(&usb, image(2_000));
    provider.scan().unwrap();
    // One board is prepared in spite of the other being stuck.
    wait_for("one board to be ready", || {
        let started = Instant::now();
        provider.scan().unwrap();
        assert!(
            started.elapsed() < Duration::from_millis(200),
            "scan must not wait"
        );
        usb.runs_firmware(&first) || usb.runs_firmware(&second)
    });
    assert_eq!(provider.scan().unwrap().len(), 2);
    usb.release();
    ready_or_error(&provider, &first.device_id()).unwrap();
    ready_or_error(&provider, &second.device_id()).unwrap();
}

#[test]
fn a_hundred_rapid_plugs_and_unplugs_leave_the_list_equal_to_what_is_present() {
    let usb = RecordedUsb::new();
    let provider = make_provider(&usb, image(500));
    let keys: Vec<BoardKey> = (1..=3_u8).map(|p| BoardKey::new("001", [p])).collect();
    let mut present = [false; 3];
    let mut state = 13_u64;
    for _ in 0..100 {
        state = state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        let which = ((state >> 33) % 3) as usize;
        if present[which] {
            usb.unplug(&keys[which]);
        } else {
            usb.plug(BoardSpec::running("001", &[(which + 1) as u8]));
        }
        present[which] = !present[which];
        let list = provider.scan().unwrap();
        let mut found: Vec<String> = list.iter().map(|d| d.id.as_str().to_owned()).collect();
        found.sort();
        let expected: Vec<String> = (0..3)
            .filter(|i| present[*i])
            .map(|i| keys[i].device_id().as_str().to_owned())
            .collect();
        assert_eq!(found, expected);
    }
}
