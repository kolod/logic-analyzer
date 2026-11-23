//! The firmware's handling of the host's commands (feature 017, T032): every request field against the limits
//! (the first failure named), the mode for every channel set, the answer to every kind of input, and the
//! information the board writes.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::panic
)]

use la_fw_logic::{Dispatcher, Response, Tables, device_info, mode_for_mask};
use la_proto::info::TriggerSupport;
use la_proto::memory::Chosen;
use la_proto::{
    CaptureRequest2, DeviceInfo, Encoding, Event, FrameParser, Memory, REQUEST2_LEN, Role,
    TriggerType, command, encode_frame,
};

fn tables() -> Tables {
    Tables {
        board: "RP2350B",
        version: (1, 0, 0),
        frequency: 100_000_000,
        blast_frequency: 100_000_000,
        buffer: 96_000,
        psram_bytes: 8 * 1024 * 1024,
        psram_rate: 20_000_000,
        max_before: [1_000, 800, 600, 400],
        max_after: [96_000, 48_000, 32_000, 24_000],
        triggers: TriggerSupport {
            edge: true,
            pattern: 16,
            fast: 5,
            burst: 8,
            blast: true,
        },
        compact: true,
    }
}

fn info() -> DeviceInfo {
    device_info(&tables())
}

fn request() -> CaptureRequest2 {
    let mut channels = [0_u8; 32];
    for (i, slot) in channels.iter_mut().enumerate() {
        *slot = i as u8;
    }
    CaptureRequest2 {
        trigger_type: TriggerType::Pattern,
        trigger_channel: 2,
        trigger_length: 4,
        trigger_value: 0b1010,
        channel_count: 32,
        channels,
        mode: 3,
        frequency: 50_000_000,
        samples_before: 400,
        samples_after: 24_000,
        bursts: 2,
        burst_times: true,
        role: Role::Single,
        encoding: Encoding::Raw,
        memory: Memory::Auto,
    }
}

/// Sends a frame of `body` through the parser and the dispatcher, as the board does.
fn send(dispatcher: &mut Dispatcher, body: &[u8]) -> Response {
    let mut frame = vec![0_u8; 4 + 2 * body.len()];
    let n = encode_frame(body, &mut frame).unwrap();
    let mut parser = FrameParser::new();
    let mut response = Response::None;
    let info = info();
    parser.feed(&frame[..n], |event| {
        response = dispatcher.handle(event, &info)
    });
    response
}

fn capture_body(request: &CaptureRequest2) -> Vec<u8> {
    let mut body = vec![command::CAPTURE2];
    body.extend_from_slice(&request.to_bytes());
    body
}

#[test]
fn the_information_the_board_writes_is_what_the_driver_reads() {
    let info = info();
    assert_eq!(info.channels, 32);
    assert_eq!(info.proto, 2);
    assert!(
        info.features.cmd7 && info.features.mode32 && info.features.compact && info.features.psram
    );
    let mut text = String::new();
    info.write_text(tables().board, &mut text).unwrap();
    assert_eq!(DeviceInfo::parse(&text).unwrap(), info);
    // A board without external memory or the compact coding does not announce them.
    let mut plain = tables();
    plain.psram_bytes = 0;
    plain.compact = false;
    let info = device_info(&plain);
    assert!(!info.features.psram && !info.features.compact);
}

#[test]
fn the_mode_of_a_channel_set_follows_its_highest_channel() {
    assert_eq!(mode_for_mask(0), 0);
    for (channel, mode) in [
        (0, 0),
        (7, 0),
        (8, 1),
        (15, 1),
        (16, 2),
        (23, 2),
        (24, 3),
        (31, 3),
    ] {
        assert_eq!(mode_for_mask(1 << channel), mode, "channel {channel}");
        // Lower channels in the set do not change it.
        assert_eq!(
            mode_for_mask((1 << channel) | 1),
            mode,
            "channel {channel} and 0"
        );
    }
    assert_eq!(mode_for_mask(u32::MAX), 3);
    assert_eq!(mode_for_mask(0x00FF_FFFF), 2);
    assert_eq!(mode_for_mask(0x0000_FFFF), 1);
    assert_eq!(mode_for_mask(0x0000_00FF), 0);
}

#[test]
fn a_valid_request_starts_a_capture_and_the_board_is_busy_until_it_is_done() {
    let mut d = Dispatcher::new();
    assert!(!d.busy());
    assert_eq!(
        send(&mut d, &capture_body(&request())),
        Response::StartCapture {
            request: request(),
            memory: Chosen::Internal
        }
    );
    assert!(d.busy());
    assert_eq!(
        send(&mut d, &capture_body(&request())),
        Response::Line("ERR_BUSY")
    );
    assert_eq!(send(&mut d, &[command::INFO]), Response::Line("ERR_BUSY"));
    d.capture_done();
    assert_eq!(send(&mut d, &[command::INFO]), Response::Info);
}

#[test]
fn each_field_against_its_limit_names_itself_and_leaves_the_board_ready() {
    type Mutation = Box<dyn Fn(&mut CaptureRequest2)>;
    let cases: Vec<(&str, Mutation)> = vec![
        ("frequency", Box::new(|r| r.frequency = 100_000_001)),
        ("frequency", Box::new(|r| r.frequency = 0)),
        ("samples_before", Box::new(|r| r.samples_before = 401)),
        ("samples_after", Box::new(|r| r.samples_after = 24_001)),
        ("samples_after", Box::new(|r| r.samples_after = 0)),
        ("trigger_length", Box::new(|r| r.trigger_length = 17)),
        ("trigger_length", Box::new(|r| r.trigger_length = 0)),
        ("trigger_value", Box::new(|r| r.trigger_value = 0b1_0000)),
        ("trigger_channel", Box::new(|r| r.trigger_channel = 30)),
        ("bursts", Box::new(|r| r.bursts = 9)),
        ("burst_times", Box::new(|r| r.bursts = 0)),
        ("mode", Box::new(|r| r.mode = 2)),
    ];
    for (field, mutate) in cases {
        let mut d = Dispatcher::new();
        let mut r = request();
        mutate(&mut r);
        assert_eq!(
            send(&mut d, &capture_body(&r)),
            Response::CaptureError(field),
            "{field}"
        );
        assert!(
            !d.busy(),
            "{field}: a refused request leaves the board ready"
        );
    }
    // The fast pattern has its own, narrower limit.
    let mut d = Dispatcher::new();
    let mut r = request();
    r.trigger_type = TriggerType::FastPattern;
    r.trigger_length = 6;
    r.trigger_value = 0;
    assert_eq!(
        send(&mut d, &capture_body(&r)),
        Response::CaptureError("trigger_length")
    );
    // The external memory is refused by a board that has none.
    let mut plain = tables();
    plain.psram_bytes = 0;
    let info = device_info(&plain);
    let mut r = request();
    r.memory = Memory::External;
    assert_eq!(info.check_request(&r), Err(la_proto::RequestField::Memory));
}

#[test]
fn the_other_commands_get_their_answers() {
    let mut d = Dispatcher::new();
    assert_eq!(send(&mut d, &[command::INFO]), Response::Info);
    assert_eq!(
        send(&mut d, &[command::NETWORK]),
        Response::Line("ERR_UNSUPPORTED")
    );
    assert_eq!(
        send(&mut d, &[command::POWER]),
        Response::Line("ERR_UNSUPPORTED")
    );
    assert_eq!(send(&mut d, &[command::BOOTLOADER]), Response::Bootloader);
    assert_eq!(send(&mut d, &[command::RESTART]), Response::Restart);
    assert_eq!(
        send(&mut d, &[command::INDICATOR_ON]),
        Response::Indicator(true)
    );
    assert_eq!(
        send(&mut d, &[command::INDICATOR_OFF]),
        Response::Indicator(false)
    );
    assert_eq!(send(&mut d, &[99]), Response::Line("ERR_UNKNOWN_MSG"));
    // The original's capture request is not provided yet (a gap recorded in verification.md).
    assert_eq!(
        send(&mut d, &[command::CAPTURE, 0, 0]),
        Response::Line("ERR_UNSUPPORTED")
    );
}

#[test]
fn the_restarts_are_accepted_while_a_capture_runs() {
    let mut d = Dispatcher::new();
    send(&mut d, &capture_body(&request()));
    assert!(d.busy());
    assert_eq!(send(&mut d, &[command::BOOTLOADER]), Response::Bootloader);
    assert_eq!(send(&mut d, &[command::RESTART]), Response::Restart);
}

#[test]
fn malformed_input_is_answered_and_leaves_the_board_ready() {
    let mut d = Dispatcher::new();
    let info = info();
    assert_eq!(
        d.handle(Event::Overflow, &info),
        Response::Line("ERR_MSG_OVERFLOW")
    );
    assert_eq!(
        d.handle(Event::Damaged, &info),
        Response::Line("ERR_BAD_REQUEST")
    );
    assert_eq!(
        d.handle(Event::Frame(&[]), &info),
        Response::Line("ERR_UNKNOWN_MSG")
    );
    // A payload where none belongs.
    for cmd in [
        command::INFO,
        command::BOOTLOADER,
        command::RESTART,
        command::INDICATOR_ON,
        command::INDICATOR_OFF,
    ] {
        assert_eq!(
            d.handle(Event::Frame(&[cmd, 1]), &info),
            Response::Line("ERR_BAD_REQUEST"),
            "command {cmd}"
        );
    }
    // A capture request one byte short or long, and one with a field no request can have.
    let body = capture_body(&request());
    assert_eq!(
        d.handle(Event::Frame(&body[..REQUEST2_LEN]), &info),
        Response::Line("ERR_BAD_REQUEST")
    );
    let mut long = body.clone();
    long.push(0);
    assert_eq!(
        d.handle(Event::Frame(&long), &info),
        Response::Line("ERR_BAD_REQUEST")
    );
    let mut bad = body.clone();
    bad[1] = 9; // an unknown trigger type
    assert_eq!(
        d.handle(Event::Frame(&bad), &info),
        Response::Line("ERR_BAD_REQUEST")
    );
    assert!(!d.busy());
    // And the board still takes a good request afterwards.
    assert_eq!(
        d.handle(Event::Frame(&body), &info),
        Response::StartCapture {
            request: request(),
            memory: Chosen::Internal
        }
    );
}

#[test]
fn any_bytes_in_any_chunking_never_stop_the_board() {
    let mut state = 5_u64;
    let mut next = move || {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        state
    };
    let mut d = Dispatcher::new();
    let mut parser = FrameParser::new();
    let info = info();
    let mut answered = 0_u32;
    for _ in 0..2_000 {
        let len = (next() % 300) as usize;
        let bytes: Vec<u8> = (0..len)
            .map(|_| match next() % 5 {
                0 => 0x55,
                1 => 0xAA,
                2 => 0xF0,
                _ => next() as u8,
            })
            .collect();
        parser.feed(&bytes, |event| {
            let _ = d.handle(event, &info);
            answered += 1;
        });
        d.capture_done();
    }
    assert!(answered > 0);
}
