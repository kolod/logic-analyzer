//! The driver against recorded exchanges (feature 017, T031): listing from the information reply, a capture of
//! 32 channels and one of channels 24 to 31 only, the request the driver sends, a stable identity, and a
//! listing that does not wait on a port that stays silent. The recordings are in `tests/fixtures/`; they are
//! synthetic until the firmware runs on a board (T039).
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::panic
)]

use std::sync::Arc;
use std::time::{Duration, Instant};

use la_devices::{
    ChannelSet, DeviceConfig, DeviceOptions, DeviceProvider, ReadOutcome, Trigger, TriggerKind,
    validate,
};
use la_driver_rp2350b::link::PortInfo;
use la_driver_rp2350b::testing::{ScriptedPorts, Step, parse_recording};
use la_driver_rp2350b::{PROVIDER_NAME, Rp2350bProvider, USB_PID, USB_VID};
use la_proto::{CaptureRequest2, Event, FrameParser, TriggerType};
use la_storage::Limits;

const PORT: &str = "COM7";
const SERIAL: &str = "E6614103E7123456";

fn fixture(name: &str) -> Vec<Step> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(name);
    parse_recording(&std::fs::read_to_string(path).unwrap()).unwrap()
}

fn port(name: &str, vid: u16, pid: u16, serial: Option<&str>) -> PortInfo {
    PortInfo {
        name: name.to_owned(),
        vid,
        pid,
        serial: serial.map(str::to_owned),
    }
}

/// A provider with one board on `COM7` whose sessions are the information reply, then `captures`.
fn provider_with(captures: &[&str]) -> (Rp2350bProvider, Arc<ScriptedPorts>) {
    let ports = Arc::new(ScriptedPorts::new());
    let mut scripts = vec![fixture("info_32ch.rec")];
    scripts.extend(captures.iter().map(|name| fixture(name)));
    ports.add_port(port(PORT, USB_VID, USB_PID, Some(SERIAL)), scripts);
    (Rp2350bProvider::with_backend(ports.clone()), ports)
}

fn config(channels: ChannelSet, length: u64, trigger: Option<Trigger>) -> DeviceConfig {
    DeviceConfig {
        enabled_channels: channels,
        sample_rate_hz: 1_000_000,
        trigger,
        pattern: None,
        capture_length: Some(length),
        pre_trigger_samples: 0,
        bursts: 0,
        options: DeviceOptions::None,
        accept_budget_risk: false,
    }
}

/// Opens the one board, starts a capture and reads it to the end; returns the words.
fn capture(
    provider: &Rp2350bProvider,
    config: &DeviceConfig,
    max_samples: usize,
) -> (Vec<u32>, usize) {
    let descriptor = provider.scan().unwrap().remove(0);
    let resolved = validate(&descriptor.capabilities, config, &Limits::default()).unwrap();
    let mut device = provider.open(&descriptor.id).unwrap();
    device.start(&resolved).unwrap();
    let (mut words, mut reads) = (Vec::new(), 0);
    loop {
        let mut piece = Vec::new();
        reads += 1;
        assert!(reads < 10_000, "the capture did not end");
        match device.read(max_samples, &mut piece).unwrap() {
            ReadOutcome::Samples => {
                assert!(piece.len() <= max_samples, "{} words", piece.len());
                words.extend(piece);
            }
            ReadOutcome::Finished => break,
            ReadOutcome::Idle => {}
            ReadOutcome::Lost(_) => panic!("no sample is lost in this recording"),
        }
    }
    device.stop().unwrap();
    (words, reads)
}

/// The request frame the host wrote to the board in the second session.
fn request_sent(ports: &ScriptedPorts) -> CaptureRequest2 {
    let writes = ports.writes(PORT);
    let frame = writes.get(1).expect("the capture request was written");
    let mut parser = FrameParser::new();
    let mut body = None;
    parser.feed(frame, |event| {
        if let Event::Frame(b) = event {
            body = Some(b.to_vec());
        }
    });
    let body = body.expect("a whole frame");
    assert_eq!(body[0], la_proto::command::CAPTURE2);
    CaptureRequest2::from_bytes(&body[1..]).unwrap()
}

#[test]
fn the_device_is_listed_from_its_information_reply() {
    let (provider, ports) = provider_with(&[]);
    let list = provider.scan().unwrap();
    assert_eq!(list.len(), 1);
    let d = &list[0];
    assert_eq!(d.id.as_str(), format!("{PROVIDER_NAME}:{SERIAL}"));
    assert!(d.name.contains(SERIAL));
    assert!(
        !d.simulated,
        "a capture from the board is never marked simulated"
    );
    d.validate().unwrap();
    let caps = &d.capabilities;
    assert_eq!(caps.channels, 32);
    assert_eq!(caps.buffer_depth, Some(96_000));
    assert_eq!(caps.bursts, Some(8));
    assert_eq!(caps.trigger_limits.pattern_width, 16);
    assert_eq!(caps.trigger_limits.fast_pattern_width, 5);
    assert!(caps.sample_rates.contains(100_000_000));
    assert!(!caps.sample_rates.contains(100_000_001));
    assert!(caps.triggers.contains(&TriggerKind::RisingEdge));
    assert!(caps.triggers.contains(&TriggerKind::Pattern));
    // The board was asked once and the answer is kept: a second listing does not open the port.
    assert_eq!(ports.opens(PORT), 1);
    assert_eq!(provider.scan().unwrap()[0].id, d.id);
    assert_eq!(ports.opens(PORT), 1);
}

#[test]
fn only_ports_with_the_ids_of_the_firmware_are_asked() {
    let ports = Arc::new(ScriptedPorts::new());
    ports.add_port(
        port("COM1", 0x0403, 0x6001, Some("FT")),
        vec![fixture("info_32ch.rec")],
    );
    ports.add_port(
        port("COM2", USB_VID, 0x000A, None),
        vec![fixture("info_32ch.rec")],
    );
    let provider = Rp2350bProvider::with_backend(ports.clone());
    assert!(provider.scan().unwrap().is_empty());
    assert_eq!(ports.opens("COM1") + ports.opens("COM2"), 0);
}

#[test]
fn a_port_without_a_serial_number_gets_an_identity_from_its_name() {
    let ports = Arc::new(ScriptedPorts::new());
    ports.add_port(
        port("ttyACM0", USB_VID, USB_PID, None),
        vec![fixture("info_32ch.rec")],
    );
    let list = Rp2350bProvider::with_backend(ports).scan().unwrap();
    assert!(list[0].id.as_str().contains("ttyACM0"));
}

#[test]
fn a_capture_of_32_channels_returns_exactly_the_recorded_words() {
    let (provider, ports) = provider_with(&["capture_32ch.rec"]);
    let (words, _) = capture(&provider, &config(ChannelSet::all(32), 16, None), 1000);
    let expected: Vec<u32> = (0..16_u32)
        .map(|i| i.wrapping_mul(0x0101_0101) ^ if i % 2 == 1 { 0x8000_0001 } else { 0 })
        .collect();
    assert_eq!(words, expected, "one word per sample, every channel kept");
    let request = request_sent(&ports);
    assert_eq!(request.channel_count, 32);
    assert_eq!(request.mode, 3);
    assert_eq!(
        request.trigger_type,
        TriggerType::Blast,
        "no trigger is blast"
    );
    assert_eq!(request.frequency, 1_000_000);
    assert_eq!((request.samples_before, request.samples_after), (0, 16));
}

#[test]
fn the_eight_upper_channels_are_not_lost_or_mixed() {
    let (provider, ports) = provider_with(&["capture_24_31.rec"]);
    let upper = (24..32).fold(ChannelSet::empty(), |mut set, c| {
        set.insert(c);
        set
    });
    let (words, _) = capture(&provider, &config(upper, 8, None), 1000);
    let expected: Vec<u32> = (0..8).map(|i| 1_u32 << (24 + i)).collect();
    assert_eq!(words, expected);
    let request = request_sent(&ports);
    assert_eq!(
        request.used_channels(),
        (24..32).collect::<Vec<u8>>().as_slice()
    );
    assert_eq!(request.mode, 3, "channel 31 needs mode 3");
    assert_eq!(request.bytes_per_sample(), 1);
}

#[test]
fn a_trigger_goes_into_the_request() {
    let (provider, ports) = provider_with(&["capture_32ch.rec"]);
    let trigger = Trigger {
        channel: 5,
        kind: TriggerKind::FallingEdge,
    };
    let _ = capture(
        &provider,
        &config(ChannelSet::all(32), 16, Some(trigger)),
        1000,
    );
    let request = request_sent(&ports);
    assert_eq!(request.trigger_type, TriggerType::Edge);
    assert_eq!((request.trigger_channel, request.trigger_length), (5, 1));
}

#[test]
fn read_returns_in_pieces_and_never_blocks() {
    let (provider, _) = provider_with(&["capture_32ch.rec"]);
    let started = Instant::now();
    let (words, reads) = capture(&provider, &config(ChannelSet::all(32), 16, None), 3);
    assert_eq!(words.len(), 16);
    assert!(reads >= 6, "pieces of at most 3 samples: {reads} reads");
    assert!(started.elapsed() < Duration::from_secs(2));
}

#[test]
fn listing_does_not_wait_on_a_port_that_stays_silent() {
    let ports = Arc::new(ScriptedPorts::new());
    ports.add_port(
        port(PORT, USB_VID, USB_PID, Some(SERIAL)),
        vec![vec![Step::Write(None)]],
    );
    let provider = Rp2350bProvider::with_backend(ports.clone());
    let started = Instant::now();
    assert!(
        provider.scan().unwrap().is_empty(),
        "a silent port is not listed"
    );
    assert!(started.elapsed() < Duration::from_secs(1));
    // And listing does not fail because of it.
    assert_eq!(ports.opens(PORT), 1);
}

#[test]
fn a_board_that_went_away_is_no_longer_listed() {
    let (provider, ports) = provider_with(&[]);
    assert_eq!(provider.scan().unwrap().len(), 1);
    ports.remove_port(PORT);
    assert!(provider.scan().unwrap().is_empty());
    assert_eq!(
        provider
            .open(&la_devices::DeviceId::new(PROVIDER_NAME, SERIAL))
            .err()
            .map(|e| e.code()),
        Some(la_devices::error::NOT_FOUND)
    );
}
