//! The firmware's choice of memory (feature 017, T043): a request that fits is started into the memory the choice
//! names, one no memory can take is answered `ERR_NO_MEMORY` and leaves the board ready. The threshold is a
//! parameter of the tables (`PSRAMRATE`), measured on the board later (T048).
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::panic
)]

use la_fw_logic::{Dispatcher, Response, Tables, device_info};
use la_proto::info::TriggerSupport;
use la_proto::memory::Chosen;
use la_proto::{CaptureRequest2, DeviceInfo, Encoding, Event, Memory, Role, TriggerType, command};

/// 96,000 samples of 4 bytes in the internal memory; 8 MiB external, writable up to `psram_rate` for 32 channels.
fn info(psram_rate: u32) -> DeviceInfo {
    device_info(&Tables {
        board: "RP2350B",
        version: (1, 0, 0),
        frequency: 100_000_000,
        blast_frequency: 100_000_000,
        buffer: 96_000,
        psram_bytes: 8 * 1024 * 1024,
        psram_rate,
        max_before: [1_000; 4],
        max_after: [3_000_000; 4],
        triggers: TriggerSupport {
            edge: true,
            pattern: 16,
            fast: 5,
            burst: 8,
            blast: true,
        },
        compact: true,
    })
}

fn request(rate: u32, after: u32, memory: Memory) -> CaptureRequest2 {
    let mut channels = [0_u8; 32];
    for (i, slot) in channels.iter_mut().enumerate() {
        *slot = i as u8;
    }
    CaptureRequest2 {
        trigger_type: TriggerType::Blast,
        trigger_channel: 0,
        trigger_length: 0,
        trigger_value: 0,
        channel_count: 32,
        channels,
        mode: 3,
        frequency: rate,
        samples_before: 0,
        samples_after: after,
        bursts: 0,
        burst_times: false,
        role: Role::Single,
        encoding: Encoding::Raw,
        memory,
    }
}

fn send(d: &mut Dispatcher, info: &DeviceInfo, request: &CaptureRequest2) -> Response {
    let mut body = vec![command::CAPTURE2];
    body.extend_from_slice(&request.to_bytes());
    d.handle(Event::Frame(&body), info)
}

#[test]
fn a_request_that_fits_goes_to_the_internal_memory() {
    let mut d = Dispatcher::new();
    let r = request(50_000_000, 96_000, Memory::Auto);
    assert_eq!(
        send(&mut d, &info(20_000_000), &r),
        Response::StartCapture {
            request: r,
            memory: Chosen::Internal
        }
    );
}

#[test]
fn a_longer_capture_goes_to_the_external_memory_below_the_threshold() {
    let mut d = Dispatcher::new();
    let r = request(20_000_000, 500_000, Memory::Auto);
    assert_eq!(
        send(&mut d, &info(20_000_000), &r),
        Response::StartCapture {
            request: r.clone(),
            memory: Chosen::External
        }
    );
    // The threshold is the one in the tables: a board measured at 10 MHz refuses the same request.
    let mut d = Dispatcher::new();
    assert_eq!(
        send(&mut d, &info(10_000_000), &r),
        Response::Line("ERR_NO_MEMORY")
    );
    assert!(!d.busy(), "a refused request leaves the board ready");
}

#[test]
fn a_request_no_memory_can_take_is_refused_and_the_board_stays_ready() {
    let mut d = Dispatcher::new();
    let info = info(20_000_000);
    for r in [
        request(100_000_000, 500_000, Memory::Auto),
        request(1_000_000, 2_000_000, Memory::Internal),
        request(1_000_000, 2_100_000, Memory::External),
    ] {
        assert_eq!(send(&mut d, &info, &r), Response::Line("ERR_NO_MEMORY"));
        assert!(!d.busy());
    }
    // A board without external memory refuses a request for it, naming the field.
    let mut plain = tables_without_psram();
    plain.psram_rate = 0;
    let none = device_info(&plain);
    let r = request(1_000_000, 1_000, Memory::External);
    assert_eq!(send(&mut d, &none, &r), Response::CaptureError("memory"));
}

fn tables_without_psram() -> Tables {
    Tables {
        board: "RP2350B",
        version: (1, 0, 0),
        frequency: 100_000_000,
        blast_frequency: 100_000_000,
        buffer: 96_000,
        psram_bytes: 0,
        psram_rate: 0,
        max_before: [1_000; 4],
        max_after: [3_000_000; 4],
        triggers: TriggerSupport {
            edge: true,
            pattern: 16,
            fast: 5,
            burst: 8,
            blast: true,
        },
        compact: false,
    }
}
