//! Capture request 2 and the device information reply (feature 017, T029): every field at its offset, a request
//! one byte short or long refused, the information parsed from the original's lines and from the new ones, an
//! unknown line ignored, every limit.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::panic
)]

use la_proto::request::MAX_CHANNELS;
use la_proto::{
    CaptureRequest2, DeviceInfo, Encoding, InfoError, Memory, REQUEST2_LEN, RequestError,
    RequestField, Role, TriggerType,
};

fn request() -> CaptureRequest2 {
    let mut channels = [0_u8; MAX_CHANNELS];
    for (i, slot) in channels.iter_mut().enumerate() {
        *slot = (31 - i) as u8; // 31, 30, ... 0: every channel, in reverse
    }
    CaptureRequest2 {
        trigger_type: TriggerType::Pattern,
        trigger_channel: 4,
        trigger_length: 3,
        trigger_value: 0b101,
        channel_count: 32,
        channels,
        mode: 3,
        frequency: 100_000_000,
        samples_before: 1_000,
        samples_after: 50_000,
        bursts: 2,
        burst_times: true,
        role: Role::Master,
        encoding: Encoding::Compact,
        memory: Memory::External,
    }
}

const INFO: &str = "LOGIC_ANALYZER_PICO2_V6_0\n\
FREQ:100000000\n\
BLASTFREQ:100000000\n\
BUFFER:96000\n\
CHANNELS:32\n\
FW:1.2.3\n\
PROTO:2\n\
FEATURES:cmd7,compact,psram,mode32,restart,future\n\
PSRAM:8388608\n\
PSRAMRATE:20000000\n\
MAXPRE:1000,500,250,100\n\
MAXPOST:96000,48000,24000,12000\n\
TRIGGERS:edge,pattern:16,fast:5,burst:255,blast\n\
SOMETHING_NEW:1\n";

fn info() -> DeviceInfo {
    DeviceInfo::parse(INFO).unwrap()
}

#[test]
fn every_field_is_at_its_offset() {
    let bytes = request().to_bytes();
    assert_eq!(bytes.len(), REQUEST2_LEN);
    assert_eq!(REQUEST2_LEN, 58);
    assert_eq!(bytes[0], 1, "trigger type: pattern");
    assert_eq!(bytes[1], 4, "trigger channel");
    assert_eq!(bytes[2], 3, "pattern length");
    assert_eq!(&bytes[3..7], &[0b101, 0, 0, 0], "trigger value");
    assert_eq!(bytes[7], 32, "channel count");
    for i in 0..32 {
        assert_eq!(bytes[8 + i], (31 - i) as u8, "channel list {i}");
    }
    assert_eq!(bytes[40], 3, "mode");
    assert_eq!(&bytes[41..45], &100_000_000_u32.to_le_bytes());
    assert_eq!(&bytes[45..49], &1_000_u32.to_le_bytes());
    assert_eq!(&bytes[49..53], &50_000_u32.to_le_bytes());
    assert_eq!(bytes[53], 2, "bursts");
    assert_eq!(bytes[54], 1, "burst times");
    assert_eq!(bytes[55], 1, "role: master");
    assert_eq!(bytes[56], 1, "encoding: compact");
    assert_eq!(bytes[57], 2, "memory: external");
}

#[test]
fn a_request_reads_back_what_was_written() {
    let r = request();
    assert_eq!(CaptureRequest2::from_bytes(&r.to_bytes()).unwrap(), r);
    for trigger_type in [
        TriggerType::Edge,
        TriggerType::Pattern,
        TriggerType::FastPattern,
        TriggerType::Blast,
    ] {
        for (role, encoding, memory) in [
            (Role::Single, Encoding::Raw, Memory::Auto),
            (Role::Slave, Encoding::Compact, Memory::Internal),
        ] {
            let mut r = request();
            r.trigger_type = trigger_type;
            r.role = role;
            r.encoding = encoding;
            r.memory = memory;
            assert_eq!(CaptureRequest2::from_bytes(&r.to_bytes()).unwrap(), r);
        }
    }
}

#[test]
fn a_request_one_byte_short_or_long_is_refused() {
    let bytes = request().to_bytes();
    assert_eq!(
        CaptureRequest2::from_bytes(&bytes[..57]),
        Err(RequestError::WrongLength)
    );
    let mut long = bytes.to_vec();
    long.push(0);
    assert_eq!(
        CaptureRequest2::from_bytes(&long),
        Err(RequestError::WrongLength)
    );
    assert_eq!(
        CaptureRequest2::from_bytes(&[]),
        Err(RequestError::WrongLength)
    );
}

#[test]
fn a_field_no_request_can_have_names_itself() {
    let cases: [(usize, u8, RequestField); 9] = [
        (0, 4, RequestField::TriggerType),
        (1, 32, RequestField::TriggerChannel),
        (7, 0, RequestField::Channels),
        (7, 33, RequestField::Channels),
        (40, 4, RequestField::Mode),
        (54, 2, RequestField::BurstTimes),
        (55, 3, RequestField::Role),
        (56, 2, RequestField::Encoding),
        (57, 3, RequestField::Memory),
    ];
    for (offset, value, field) in cases {
        let mut bytes = request().to_bytes();
        bytes[offset] = value;
        assert_eq!(
            CaptureRequest2::from_bytes(&bytes),
            Err(RequestError::Field(field)),
            "offset {offset}"
        );
    }
    // A channel number of 32 or more, and a channel listed twice.
    let mut bytes = request().to_bytes();
    bytes[8] = 32;
    assert_eq!(
        CaptureRequest2::from_bytes(&bytes),
        Err(RequestError::Field(RequestField::Channels))
    );
    let mut bytes = request().to_bytes();
    bytes[9] = bytes[8];
    assert_eq!(
        CaptureRequest2::from_bytes(&bytes),
        Err(RequestError::Field(RequestField::Channels))
    );
    // Only the first `count` entries are used: junk after them is not looked at.
    let mut bytes = request().to_bytes();
    bytes[7] = 2;
    bytes[20] = 99;
    assert!(CaptureRequest2::from_bytes(&bytes).is_ok());
}

#[test]
fn the_mode_follows_the_highest_channel() {
    for (highest, mode) in [
        (0, 0),
        (7, 0),
        (8, 1),
        (15, 1),
        (16, 2),
        (23, 2),
        (24, 3),
        (31, 3),
    ] {
        let mut r = request();
        r.channel_count = 1;
        r.channels[0] = highest;
        assert_eq!(r.mode_needed(), mode, "channel {highest}");
    }
    let mut r = request();
    r.channel_count = 9;
    assert_eq!(r.bytes_per_sample(), 2);
}

#[test]
fn the_information_is_read_from_the_original_and_the_new_lines() {
    let i = info();
    assert_eq!(i.frequency, 100_000_000);
    assert_eq!(i.blast_frequency, 100_000_000);
    assert_eq!(i.buffer, 96_000);
    assert_eq!(i.channels, 32);
    assert_eq!(i.firmware, Some((1, 2, 3)));
    assert_eq!(i.proto, 2);
    assert!(i.features.cmd7 && i.features.compact && i.features.psram && i.features.mode32);
    assert!(i.features.restart);
    assert_eq!(i.psram_bytes, 8 * 1024 * 1024);
    assert_eq!(i.psram_rate, 20_000_000);
    assert_eq!(i.max_before, [1_000, 500, 250, 100]);
    assert_eq!(i.max_after, [96_000, 48_000, 24_000, 12_000]);
    assert!(i.triggers.edge && i.triggers.blast);
    assert_eq!(
        (i.triggers.pattern, i.triggers.fast, i.triggers.burst),
        (16, 5, 255)
    );
}

#[test]
fn an_original_firmware_reads_with_the_defaults_of_the_original() {
    let text = "LOGIC_ANALYZER_PICO_V1_0\r\nFREQ:100000000\r\nBLASTFREQ:100000000\r\nBUFFER:32000\r\nCHANNELS:24\r\n";
    let i = DeviceInfo::parse(text).unwrap();
    assert_eq!((i.channels, i.proto, i.firmware), (24, 1, None));
    assert!(!i.features.cmd7 && !i.features.compact);
    assert_eq!(i.psram_bytes, 0);
}

#[test]
fn a_reply_that_is_not_an_analyzer_or_is_incomplete_is_refused() {
    assert_eq!(DeviceInfo::parse(""), Err(InfoError::NotAnalyzer));
    assert_eq!(
        DeviceInfo::parse("HELLO\nFREQ:1\nCHANNELS:1\n"),
        Err(InfoError::NotAnalyzer)
    );
    assert_eq!(
        DeviceInfo::parse("LOGIC_ANALYZER_X_1\nCHANNELS:8\n"),
        Err(InfoError::Missing("FREQ"))
    );
    assert_eq!(
        DeviceInfo::parse("LOGIC_ANALYZER_X_1\nFREQ:8\n"),
        Err(InfoError::Missing("CHANNELS"))
    );
    for (line, name) in [
        ("FREQ:fast", "FREQ"),
        ("FW:1.2", "FW"),
        ("FW:1.2.3.4", "FW"),
        ("FW:1.2.300", "FW"),
        ("MAXPRE:1,2,3", "MAXPRE"),
        ("MAXPOST:1,2,3,4,5", "MAXPOST"),
        ("PSRAM:-1", "PSRAM"),
        ("TRIGGERS:pattern:x", "TRIGGERS"),
    ] {
        let text = format!("LOGIC_ANALYZER_X_1\nFREQ:8\nCHANNELS:8\n{line}\n");
        assert_eq!(
            DeviceInfo::parse(&text),
            Err(InfoError::BadValue(name)),
            "{line}"
        );
    }
}

fn refused(mutate: impl Fn(&mut CaptureRequest2)) -> RequestField {
    let mut r = request();
    mutate(&mut r);
    info().check_request(&r).unwrap_err()
}

#[test]
fn a_request_within_every_limit_is_accepted() {
    let mut r = request();
    // The limits of mode 3: 100 before, 12,000 after.
    r.samples_before = 100;
    r.samples_after = 12_000;
    assert_eq!(info().check_request(&r), Ok(()));
}

#[test]
fn every_limit_of_the_device_is_checked_and_named() {
    let ok = |r: &mut CaptureRequest2| {
        r.samples_before = 100;
        r.samples_after = 12_000;
    };
    let base = |mutate: &dyn Fn(&mut CaptureRequest2)| {
        refused(|r| {
            ok(r);
            mutate(r);
        })
    };
    assert_eq!(base(&|r| r.frequency = 0), RequestField::Frequency);
    assert_eq!(
        base(&|r| r.frequency = 100_000_001),
        RequestField::Frequency
    );
    assert_eq!(
        base(&|r| r.samples_before = 101),
        RequestField::SamplesBefore
    );
    assert_eq!(
        base(&|r| r.samples_after = 12_001),
        RequestField::SamplesAfter
    );
    assert_eq!(base(&|r| r.samples_after = 0), RequestField::SamplesAfter);
    assert_eq!(
        base(&|r| r.mode = 2),
        RequestField::Mode,
        "channel 31 needs mode 3"
    );
    assert_eq!(base(&|r| r.trigger_length = 0), RequestField::TriggerLength);
    assert_eq!(
        base(&|r| r.trigger_length = 17),
        RequestField::TriggerLength
    );
    assert_eq!(
        base(&|r| r.trigger_value = 0b1000),
        RequestField::TriggerValue
    );
    assert_eq!(
        base(&|r| {
            r.trigger_channel = 30;
            r.trigger_length = 3;
            r.trigger_value = 0;
        }),
        RequestField::TriggerChannel
    );
    assert_eq!(
        base(&|r| {
            r.trigger_type = TriggerType::FastPattern;
            r.trigger_length = 6;
            r.trigger_value = 0;
        }),
        RequestField::TriggerLength
    );
    assert_eq!(
        base(&|r| r.bursts = 0),
        RequestField::BurstTimes,
        "times need bursts"
    );
    // The limits of the device change what is refused.
    let mut small = info();
    small.triggers.burst = 1;
    small.features.compact = false;
    small.psram_bytes = 0;
    small.features.mode32 = false;
    let mut r = request();
    ok(&mut r);
    assert_eq!(small.check_request(&r), Err(RequestField::Mode));
    small.features.mode32 = true;
    assert_eq!(small.check_request(&r), Err(RequestField::Bursts));
    r.bursts = 1;
    assert_eq!(small.check_request(&r), Err(RequestField::Encoding));
    r.encoding = Encoding::Raw;
    assert_eq!(small.check_request(&r), Err(RequestField::Memory));
    r.memory = Memory::Auto;
    assert_eq!(small.check_request(&r), Ok(()));
    // A trigger the device does not have.
    small.triggers.pattern = 0;
    assert_eq!(small.check_request(&r), Err(RequestField::TriggerType));
}

#[test]
fn an_edge_blast_and_the_field_names_follow_the_contract() {
    let mut r = request();
    r.samples_before = 0;
    r.samples_after = 1_000;
    r.bursts = 0;
    r.burst_times = false;
    r.trigger_type = TriggerType::Edge;
    r.trigger_length = 1;
    r.trigger_value = 0;
    assert_eq!(info().check_request(&r), Ok(()));
    r.trigger_length = 2;
    assert_eq!(info().check_request(&r), Err(RequestField::TriggerLength));
    r.trigger_length = 0;
    r.trigger_type = TriggerType::Blast;
    assert_eq!(info().check_request(&r), Ok(()));
    assert_eq!(RequestField::SamplesBefore.name(), "samples_before");
    assert_eq!(RequestField::Frequency.name(), "frequency");
}

#[test]
fn the_text_the_board_writes_is_the_text_the_driver_reads() {
    let mut text = String::new();
    info().write_text("RP2350B", &mut text).unwrap();
    assert!(text.starts_with("LOGIC_ANALYZER_RP2350B_1_2\n"), "{text}");
    assert_eq!(DeviceInfo::parse(&text).unwrap(), info());
    // A board with nothing beyond the original writes the original's lines and reads back as such.
    let mut plain = info();
    plain.firmware = None;
    plain.proto = 1;
    plain.features = la_proto::Features::default();
    plain.psram_bytes = 0;
    plain.psram_rate = 0;
    plain.max_before = [0; 4];
    plain.max_after = [0; 4];
    plain.triggers = la_proto::info::TriggerSupport::default();
    let mut text = String::new();
    plain.write_text("PICO", &mut text).unwrap();
    assert!(
        text.contains("FEATURES:\n") && text.contains("TRIGGERS:\n"),
        "{text}"
    );
    assert_eq!(DeviceInfo::parse(&text).unwrap(), plain);
}
