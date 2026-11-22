//! The `META` section: the capture description (see `contracts/file-format.md`).

use std::time::{Duration, SystemTime};

use la_core::Result;
use la_devices::{CaptureDescription, ChannelSet, SavedDescription, Trigger, TriggerKind};

use crate::error;
use crate::limits::MAX_TEXT_BYTES;
use crate::varint::{Cursor, encode_into};

const FLAG_SIMULATED: u8 = 1;
const FLAG_SEED: u8 = 2;
const FLAG_TRIGGER: u8 = 4;
const KNOWN_FLAGS: u8 = FLAG_SIMULATED | FLAG_SEED | FLAG_TRIGGER;

fn trigger_code(kind: TriggerKind) -> u8 {
    match kind {
        TriggerKind::Level(false) => 0,
        TriggerKind::Level(true) => 1,
        TriggerKind::RisingEdge => 2,
        TriggerKind::FallingEdge => 3,
        TriggerKind::AnyEdge => 4,
    }
}

fn trigger_kind(code: u8) -> Result<TriggerKind> {
    Ok(match code {
        0 => TriggerKind::Level(false),
        1 => TriggerKind::Level(true),
        2 => TriggerKind::RisingEdge,
        3 => TriggerKind::FallingEdge,
        4 => TriggerKind::AnyEdge,
        _ => return Err(error::damaged("an unknown trigger kind")),
    })
}

fn put_text(text: &str, out: &mut Vec<u8>) -> Result<()> {
    if text.len() > MAX_TEXT_BYTES {
        return Err(error::invalid_input(format!(
            "a text of {} bytes is longer than the {MAX_TEXT_BYTES} bytes a session file can hold",
            text.len()
        )));
    }
    encode_into(u64::try_from(text.len()).unwrap_or(u64::MAX), out);
    out.extend_from_slice(text.as_bytes());
    Ok(())
}

/// Splits a time into signed seconds since the Unix epoch and nanoseconds `0..10^9`.
fn split_time(time: SystemTime) -> Result<(i64, u32)> {
    let too_far =
        || error::invalid_input("the start time is outside the range a session file can hold");
    match time.duration_since(SystemTime::UNIX_EPOCH) {
        Ok(after) => Ok((
            i64::try_from(after.as_secs()).map_err(|_| too_far())?,
            after.subsec_nanos(),
        )),
        Err(before) => {
            let before = before.duration();
            let secs = i64::try_from(before.as_secs()).map_err(|_| too_far())?;
            let nanos = before.subsec_nanos();
            if nanos == 0 {
                Ok((secs.checked_neg().ok_or_else(too_far)?, 0))
            } else {
                // -(s + n/1e9) = -(s + 1) + (1e9 - n)/1e9
                let secs = secs
                    .checked_add(1)
                    .and_then(i64::checked_neg)
                    .ok_or_else(too_far)?;
                Ok((secs, 1_000_000_000_u32.saturating_sub(nanos)))
            }
        }
    }
}

/// Joins signed seconds and nanoseconds into a time, if the platform can represent it.
fn join_time(secs: i64, nanos: u32) -> Option<SystemTime> {
    if nanos >= 1_000_000_000 {
        return None;
    }
    if secs >= 0 {
        SystemTime::UNIX_EPOCH.checked_add(Duration::new(u64::try_from(secs).ok()?, nanos))
    } else {
        let whole = Duration::new(secs.unsigned_abs(), 0);
        let back = whole.checked_sub(Duration::from_nanos(u64::from(nanos)))?;
        SystemTime::UNIX_EPOCH.checked_sub(back)
    }
}

/// Encodes a description.
///
/// # Errors
///
/// `session.invalid_input` for a text longer than the limit or a start time the file cannot hold.
pub fn encode(description: &CaptureDescription) -> Result<Vec<u8>> {
    let saved = description.to_saved();
    let mut out = Vec::new();
    let mut flags = 0_u8;
    if saved.simulated {
        flags |= FLAG_SIMULATED;
    }
    if saved.seed.is_some() {
        flags |= FLAG_SEED;
    }
    if saved.trigger.is_some() {
        flags |= FLAG_TRIGGER;
    }
    out.push(flags);
    if let Some(seed) = saved.seed {
        out.extend_from_slice(&seed.to_le_bytes());
    }
    put_text(&saved.device_id, &mut out)?;
    put_text(&saved.device_name, &mut out)?;
    out.push(saved.channels);
    // The first word is always there; a capture of more than 32 channels adds the other words
    // of the set right after it, so the description of a capture of 32 channels or fewer is
    // byte for byte what it always was.
    for k in 0..la_storage::words_per_sample(saved.channels) {
        out.extend_from_slice(&saved.enabled_mask.word(k).to_le_bytes());
    }
    out.extend_from_slice(&saved.requested_sample_rate_hz.to_le_bytes());
    out.extend_from_slice(&saved.sample_rate_hz.to_le_bytes());
    out.extend_from_slice(&saved.samples_before_trigger.to_le_bytes());
    if let Some(trigger) = saved.trigger {
        out.push(trigger.channel);
        out.push(trigger_code(trigger.kind));
    }
    let (secs, nanos) = split_time(saved.started_at)?;
    out.extend_from_slice(&secs.to_le_bytes());
    out.extend_from_slice(&nanos.to_le_bytes());
    Ok(out)
}

/// Decodes a description.
///
/// # Errors
///
/// `session.damaged` for an encoding violation, `session.inconsistent` for values no real
/// acquisition could have produced.
pub fn decode(payload: &[u8]) -> Result<CaptureDescription> {
    let mut cursor = Cursor::new(payload);
    let flags = cursor.u8()?;
    if flags & !KNOWN_FLAGS != 0 {
        return Err(error::damaged("a reserved description flag is set"));
    }
    let seed = if flags & FLAG_SEED != 0 {
        Some(cursor.u64()?)
    } else {
        None
    };
    let device_id = cursor.string(MAX_TEXT_BYTES)?;
    let device_name = cursor.string(MAX_TEXT_BYTES)?;
    let channels = cursor.u8()?;
    if channels == 0 || channels > la_storage::MAX_CHANNELS {
        return Err(error::inconsistent(
            "the channel count is outside what a capture can have",
        ));
    }
    let mut words = Vec::new();
    for _ in 0..la_storage::words_per_sample(channels) {
        words.push(cursor.u32()?);
    }
    let enabled_mask = ChannelSet::from_words(&words);
    let requested_sample_rate_hz = cursor.u64()?;
    let sample_rate_hz = cursor.u64()?;
    let samples_before_trigger = cursor.u64()?;
    let trigger = if flags & FLAG_TRIGGER != 0 {
        let channel = cursor.u8()?;
        Some(Trigger {
            channel,
            kind: trigger_kind(cursor.u8()?)?,
        })
    } else {
        None
    };
    let secs = cursor.i64()?;
    let nanos = cursor.u32()?;
    cursor.finish()?;
    let started_at = join_time(secs, nanos).ok_or_else(|| {
        error::inconsistent("the start time is not a valid time on this platform")
    })?;
    CaptureDescription::restore(SavedDescription {
        device_id,
        device_name,
        simulated: flags & FLAG_SIMULATED != 0,
        seed,
        requested_sample_rate_hz,
        sample_rate_hz,
        channels,
        enabled_mask,
        trigger,
        samples_before_trigger,
        started_at,
    })
    .map_err(|e| {
        error::inconsistent(format!(
            "the capture description is invalid: {}",
            e.detail()
        ))
    })
}

#[cfg(test)]
#[allow(clippy::arithmetic_side_effects)]
mod tests {
    use super::*;

    fn saved() -> SavedDescription {
        SavedDescription {
            device_id: "usb:ab".to_owned(),
            device_name: "Gerät ж".to_owned(),
            simulated: true,
            seed: Some(0x0102_0304_0506_0708),
            requested_sample_rate_hz: 1_000_000,
            sample_rate_hz: 2_000_000,
            channels: 8,
            enabled_mask: ChannelSet::from_u32(0b0000_0110),
            trigger: Some(Trigger {
                channel: 2,
                kind: TriggerKind::FallingEdge,
            }),
            samples_before_trigger: 9,
            started_at: SystemTime::UNIX_EPOCH + Duration::new(1_000, 500),
        }
    }

    fn description(s: SavedDescription) -> CaptureDescription {
        CaptureDescription::restore(s).unwrap()
    }

    #[test]
    fn the_layout_is_exactly_the_documented_one() {
        let bytes = encode(&description(saved())).unwrap();
        let mut expected = vec![0b111]; // simulated, seed, trigger
        expected.extend_from_slice(&[8, 7, 6, 5, 4, 3, 2, 1]);
        expected.extend_from_slice(&[6, b'u', b's', b'b', b':', b'a', b'b']);
        let name = "Gerät ж".as_bytes();
        expected.push(u8::try_from(name.len()).unwrap());
        expected.extend_from_slice(name);
        expected.push(8);
        expected.extend_from_slice(&6_u32.to_le_bytes());
        expected.extend_from_slice(&1_000_000_u64.to_le_bytes());
        expected.extend_from_slice(&2_000_000_u64.to_le_bytes());
        expected.extend_from_slice(&9_u64.to_le_bytes());
        expected.extend_from_slice(&[2, 3]);
        expected.extend_from_slice(&1_000_i64.to_le_bytes());
        expected.extend_from_slice(&500_u32.to_le_bytes());
        assert_eq!(bytes, expected);
    }

    #[test]
    fn descriptions_round_trip_in_every_variant() {
        for (simulated, seed) in [
            (true, Some(0)),
            (true, Some(u64::MAX)),
            (true, None),
            (false, None),
        ] {
            for kind in [
                None,
                Some(TriggerKind::Level(false)),
                Some(TriggerKind::Level(true)),
                Some(TriggerKind::RisingEdge),
                Some(TriggerKind::FallingEdge),
                Some(TriggerKind::AnyEdge),
            ] {
                let mut s = saved();
                s.simulated = simulated;
                s.seed = seed;
                s.trigger = kind.map(|kind| Trigger { channel: 1, kind });
                let d = description(s);
                assert_eq!(decode(&encode(&d).unwrap()).unwrap(), d);
            }
        }
    }

    #[test]
    fn times_before_and_after_the_epoch_survive_exactly() {
        let times = [
            SystemTime::UNIX_EPOCH,
            SystemTime::UNIX_EPOCH + Duration::new(0, 100),
            SystemTime::UNIX_EPOCH + Duration::new(0, 999_999_900),
            SystemTime::UNIX_EPOCH - Duration::new(0, 100),
            SystemTime::UNIX_EPOCH - Duration::new(1, 0),
            SystemTime::UNIX_EPOCH - Duration::new(86_400 * 365 * 30, 999_999_900),
            SystemTime::UNIX_EPOCH + Duration::new(4_102_444_800, 123_456_700),
        ];
        for time in times {
            let mut s = saved();
            s.started_at = time;
            let d = description(s);
            let back = decode(&encode(&d).unwrap()).unwrap();
            assert_eq!(back.started_at(), time, "{time:?}");
        }
        assert_eq!(
            split_time(SystemTime::UNIX_EPOCH - Duration::new(0, 100)).unwrap(),
            (-1, 999_999_900)
        );
        assert_eq!(
            split_time(SystemTime::UNIX_EPOCH - Duration::new(2, 0)).unwrap(),
            (-2, 0)
        );
    }

    #[test]
    fn long_texts_are_rejected_on_encode() {
        let mut s = saved();
        s.device_name = "x".repeat(4_096);
        assert!(encode(&description(s)).is_ok());
        // `restore` already limits the text, so build the over-long value for the encoder
        // directly through the helper.
        let mut out = Vec::new();
        assert_eq!(
            put_text(&"x".repeat(4_097), &mut out).unwrap_err().code(),
            error::INVALID_INPUT
        );
    }

    #[test]
    fn structural_damage_is_reported_as_damaged() {
        let good = encode(&description(saved())).unwrap();
        let damaged = |bytes: &[u8]| decode(bytes).unwrap_err().code() == error::DAMAGED;
        // Reserved flag bits.
        for bit in 3..8 {
            let mut b = good.clone();
            b[0] |= 1 << bit;
            assert!(damaged(&b), "flag bit {bit}");
        }
        // Seed bit set but the bytes are missing, and every truncation.
        assert!(damaged(&[FLAG_SEED, 1, 2, 3]));
        for len in 0..good.len() {
            assert!(damaged(&good[..len]), "truncated to {len}");
        }
        // Trailing bytes.
        let mut extra = good.clone();
        extra.push(0);
        assert!(damaged(&extra));
        // Text length beyond the limit, beyond the payload, invalid UTF-8.
        let mut long = vec![0_u8];
        encode_into(4_097, &mut long);
        long.extend(std::iter::repeat_n(b'x', 4_097));
        assert!(damaged(&long));
        assert!(damaged(&[0, 50, b'a']));
        assert!(damaged(&[0, 2, 0xFF, 0xFE]));
        // Unknown trigger kind.
        let mut kind = good.clone();
        let at = kind.len() - 8 - 4 - 1;
        kind[at] = 9;
        assert!(damaged(&kind));
    }

    #[test]
    fn values_no_acquisition_could_produce_are_inconsistent() {
        let inconsistent = |mutate: &dyn Fn(&mut Vec<u8>)| {
            let mut b = encode(&description(saved())).unwrap();
            mutate(&mut b);
            decode(&b).unwrap_err().code() == error::INCONSISTENT
        };
        // Layout offsets of the fixed part: flags(1) seed(8) id(1+6) name(1+9) channels(1) mask(4) ...
        let channels_at = 1 + 8 + 7 + 1 + "Gerät ж".len();
        assert!(inconsistent(&|b| b[channels_at] = 0));
        assert!(inconsistent(&|b| b[channels_at] = 161));
        assert!(inconsistent(
            &|b| b[channels_at + 1..channels_at + 5].copy_from_slice(&0_u32.to_le_bytes())
        ));
        assert!(inconsistent(
            &|b| b[channels_at + 1..channels_at + 5].copy_from_slice(&0x100_u32.to_le_bytes())
        ));
        assert!(inconsistent(
            &|b| b[channels_at + 5..channels_at + 13].copy_from_slice(&0_u64.to_le_bytes())
        ));
        // The trigger channel is not enabled.
        let trigger_at = channels_at + 1 + 4 + 8 + 8 + 8;
        assert!(inconsistent(&|b| b[trigger_at] = 0));
        // Nanoseconds out of range.
        let nanos_at = trigger_at + 2 + 8;
        assert!(inconsistent(
            &|b| b[nanos_at..nanos_at + 4].copy_from_slice(&1_000_000_000_u32.to_le_bytes())
        ));
        // A device id without a colon, and an empty name.
        let mut s = saved();
        s.device_id = "usb:ab".to_owned();
        let mut b = encode(&description(s)).unwrap();
        b[1 + 8 + 1 + 3] = b'-';
        assert_eq!(decode(&b).unwrap_err().code(), error::INCONSISTENT);
    }
}
