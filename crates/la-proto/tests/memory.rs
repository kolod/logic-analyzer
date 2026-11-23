//! The choice of memory (feature 017, T043): the internal memory takes what fits; the external one takes captures it
//! can write at the rate and that do not fit the internal one; a request no memory can take is refused with the
//! reason. The numbers are worked out by hand.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::panic
)]

use la_proto::memory::{Chosen, MemoryError, MemoryLimits, choose, needed_bytes};
use la_proto::{CaptureRequest2, Encoding, Memory, Role, TriggerType};

/// 32 channels (4 bytes a sample) unless `count` says less.
fn request(count: u8, rate: u32, before: u32, after: u32, memory: Memory) -> CaptureRequest2 {
    let mut channels = [0_u8; 32];
    for (i, slot) in channels.iter_mut().enumerate() {
        *slot = i as u8;
    }
    CaptureRequest2 {
        trigger_type: TriggerType::Blast,
        trigger_channel: 0,
        trigger_length: 0,
        trigger_value: 0,
        channel_count: count,
        channels,
        mode: 3,
        frequency: rate,
        samples_before: before,
        samples_after: after,
        bursts: 0,
        burst_times: false,
        role: Role::Single,
        encoding: Encoding::Raw,
        memory,
    }
}

/// An internal buffer of 96,000 samples of 4 bytes (384,000 bytes), 8 MiB of external memory that takes up to
/// 20 MHz of 32 channels (80 MB/s).
fn limits() -> MemoryLimits {
    MemoryLimits {
        internal_bytes: 96_000 * 4,
        psram_bytes: 8 * 1024 * 1024,
        psram_rate_hz: 20_000_000,
    }
}

#[test]
fn the_bytes_a_request_needs_follow_its_samples_and_channels() {
    assert_eq!(
        needed_bytes(&request(32, 1_000_000, 0, 1_000, Memory::Auto)),
        4_000
    );
    assert_eq!(
        needed_bytes(&request(8, 1_000_000, 100, 900, Memory::Auto)),
        1_000
    );
    assert_eq!(
        needed_bytes(&request(9, 1_000_000, 0, 10, Memory::Auto)),
        20,
        "9 channels take 2 bytes"
    );
    assert_eq!(
        needed_bytes(&request(32, 1, u32::MAX, u32::MAX, Memory::Auto)),
        2 * u64::from(u32::MAX) * 4,
        "no overflow"
    );
}

#[test]
fn auto_takes_the_internal_memory_when_the_capture_fits() {
    let l = limits();
    assert_eq!(
        choose(&request(32, 100_000_000, 0, 96_000, Memory::Auto), &l),
        Ok(Chosen::Internal)
    );
    // Narrower samples hold more: 8 channels, 1 byte a sample, 384,000 samples.
    assert_eq!(
        choose(&request(8, 100_000_000, 0, 384_000, Memory::Auto), &l),
        Ok(Chosen::Internal)
    );
}

#[test]
fn auto_takes_the_external_memory_only_when_it_does_not_fit_and_the_rate_is_low_enough() {
    let l = limits();
    // One sample more than the internal memory holds, at a rate the external one can take.
    assert_eq!(
        choose(&request(32, 20_000_000, 0, 96_001, Memory::Auto), &l),
        Ok(Chosen::External)
    );
    // The same capture at a higher rate cannot be written: it fits nowhere.
    assert_eq!(
        choose(&request(32, 20_000_001, 0, 96_001, Memory::Auto), &l),
        Err(MemoryError::FitsNowhere)
    );
    // Fewer bytes a sample raise the rate the external memory takes: 8 channels at 80 MHz are 80 MB/s.
    assert_eq!(
        choose(&request(8, 80_000_000, 0, 400_000, Memory::Auto), &l),
        Ok(Chosen::External)
    );
    assert_eq!(
        choose(&request(8, 80_000_001, 0, 400_000, Memory::Auto), &l),
        Err(MemoryError::FitsNowhere)
    );
    // Larger than the external memory too.
    assert_eq!(
        choose(&request(32, 1_000, 0, 2_097_153, Memory::Auto), &l),
        Err(MemoryError::FitsNowhere)
    );
    assert_eq!(
        choose(&request(32, 1_000, 0, 2_097_152, Memory::Auto), &l),
        Ok(Chosen::External),
        "8 MiB is 2,097,152 samples of 4 bytes"
    );
}

#[test]
fn a_named_memory_is_used_or_refused_with_the_reason() {
    let l = limits();
    assert_eq!(
        choose(&request(32, 1_000_000, 0, 1_000, Memory::Internal), &l),
        Ok(Chosen::Internal)
    );
    assert_eq!(
        choose(&request(32, 1_000_000, 0, 96_001, Memory::Internal), &l),
        Err(MemoryError::TooLargeForInternal)
    );
    assert_eq!(
        choose(&request(32, 1_000_000, 0, 1_000, Memory::External), &l),
        Ok(Chosen::External),
        "asking for the external memory gets it even for a small capture"
    );
    assert_eq!(
        choose(&request(32, 30_000_000, 0, 1_000, Memory::External), &l),
        Err(MemoryError::TooFastForExternal)
    );
    assert_eq!(
        choose(&request(32, 1_000_000, 0, 2_097_153, Memory::External), &l),
        Err(MemoryError::TooLargeForExternal)
    );
}

#[test]
fn a_board_without_external_memory_uses_the_internal_one_only() {
    let l = MemoryLimits {
        psram_bytes: 0,
        psram_rate_hz: 0,
        ..limits()
    };
    assert_eq!(
        choose(&request(32, 1_000, 0, 96_000, Memory::Auto), &l),
        Ok(Chosen::Internal)
    );
    assert_eq!(
        choose(&request(32, 1_000, 0, 96_001, Memory::Auto), &l),
        Err(MemoryError::FitsNowhere)
    );
    assert_eq!(
        choose(&request(32, 1_000, 0, 10, Memory::External), &l),
        Err(MemoryError::NoExternal)
    );
}

#[test]
fn the_depth_follows_the_rate_and_the_bytes_of_a_sample() {
    let l = limits();
    // At a rate the external memory takes, it is the deeper one: 2,097,152 samples of 4 bytes.
    assert_eq!(l.depth_samples(4, 20_000_000), 2_097_152);
    // Above it, only the internal memory: 96,000 samples.
    assert_eq!(l.depth_samples(4, 20_000_001), 96_000);
    // One byte a sample: the external memory takes up to 80 MHz and holds 8 Mi samples.
    assert_eq!(l.depth_samples(1, 80_000_000), 8 * 1024 * 1024);
    assert_eq!(l.depth_samples(1, 80_000_001), 384_000);
    let none = MemoryLimits {
        psram_bytes: 0,
        ..l
    };
    assert_eq!(none.depth_samples(4, 1_000), 96_000);
}
