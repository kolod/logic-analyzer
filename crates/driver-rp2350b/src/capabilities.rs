//! What the board reports turned into the capabilities of the device abstraction, and a validated
//! configuration turned into the board's capture request.

use la_core::Result;
use la_devices::ModeLimit;
use la_devices::{
    Capabilities, ResolvedConfig, SampleRates, TriggerKind, TriggerLimits, error as device_error,
};
use la_proto::{CaptureRequest2, DeviceInfo, Encoding, Memory, Role, TriggerType};

use crate::error;

/// The lowest sample rate the board takes, in hertz.
pub const MIN_RATE_HZ: u64 = 3_100;

/// The channels of the board.
pub const CHANNELS: u8 = 32;

/// The major version of the firmware this driver speaks to.
pub const SUPPORTED_FIRMWARE_MAJOR: u8 = 1;

/// What this driver supports, as text for the detail of an error.
const SUPPORTED: &str = "firmware 1.x with protocol level 2 and capture request 2";

/// Checks that the board's firmware is one this driver can use.
///
/// # Errors
///
/// `rp2350b.firmware_unsupported`, naming what the board reported.
pub fn check_supported(info: &DeviceInfo) -> Result<()> {
    let found = match info.firmware {
        Some((major, minor, patch)) => format!(
            "firmware {major}.{minor}.{patch}, protocol level {}",
            info.proto
        ),
        None => format!("an unnamed firmware, protocol level {}", info.proto),
    };
    let known = info
        .firmware
        .is_some_and(|(major, _, _)| major == SUPPORTED_FIRMWARE_MAJOR);
    if !known || info.proto < 2 || !info.features.cmd7 {
        return Err(error::firmware_unsupported(&found, SUPPORTED));
    }
    if info.channels != CHANNELS || !info.features.mode32 {
        return Err(error::firmware_unsupported(
            &format!("{found}, {} channels", info.channels),
            "32 channels in mode 3",
        ));
    }
    Ok(())
}

/// The capabilities of the board, as declared to the core. The limits of the modes are the tightest of the
/// four, because the board's mode depends on the highest channel used and the core's on how many are enabled.
#[must_use]
pub fn capabilities(info: &DeviceInfo) -> Capabilities {
    let mut triggers = Vec::new();
    if info.triggers.edge {
        triggers.push(TriggerKind::RisingEdge);
        triggers.push(TriggerKind::FallingEdge);
    }
    if info.triggers.pattern > 0 {
        triggers.push(TriggerKind::Pattern);
    }
    if info.triggers.fast > 0 {
        triggers.push(TriggerKind::FastPattern);
    }
    // Without a trigger the board runs in blast mode, which has its own highest rate.
    let highest = match info.blast_frequency {
        0 => info.frequency,
        blast => blast.min(info.frequency),
    };
    let before = info.max_before.iter().copied().min().unwrap_or(0);
    let after = info.max_after.iter().copied().min().unwrap_or(0);
    Capabilities {
        channels: CHANNELS,
        sample_rates: SampleRates::Range {
            min: MIN_RATE_HZ,
            max: u64::from(highest).max(MIN_RATE_HZ),
        },
        triggers,
        buffer_depth: (info.buffer > 0).then(|| u64::from(info.buffer)),
        trigger_limits: TriggerLimits {
            pattern_width: info.triggers.pattern,
            fast_pattern_width: info.triggers.fast,
            chained: false,
            allowed_in_chain: false,
        },
        pre_post: if after == 0 {
            Vec::new()
        } else {
            vec![ModeLimit {
                up_to_channels: CHANNELS,
                max_before: u64::from(before),
                max_after: u64::from(after),
            }]
        },
        buffer_depths: Vec::new(),
        bursts: (info.triggers.burst > 0).then(|| u32::from(info.triggers.burst)),
        max_rates: Vec::new(),
    }
}

/// The capture request for a configuration the core has validated against [`capabilities`].
///
/// # Errors
///
/// `device.invalid_capture_length` or `device.unsupported_trigger` for a configuration the capabilities
/// should have refused (a defect, not a user mistake: it is reported, never panics).
pub fn build_request(config: &ResolvedConfig) -> Result<CaptureRequest2> {
    let mut channels = [0_u8; 32];
    let mut count = 0_u8;
    for (slot, channel) in channels.iter_mut().zip(config.enabled_channels().iter()) {
        *slot = channel;
        count = count.saturating_add(1);
    }
    let (trigger_type, trigger_channel, trigger_length, trigger_value) = match config.trigger() {
        None => (TriggerType::Blast, 0, 0, 0),
        Some(trigger) => match (trigger.kind, config.pattern()) {
            (TriggerKind::RisingEdge, _) => (TriggerType::Edge, trigger.channel, 0, 0),
            (TriggerKind::FallingEdge, _) => (TriggerType::Edge, trigger.channel, 1, 0),
            (TriggerKind::Pattern, Some(p)) => {
                (TriggerType::Pattern, p.base_channel, p.length, p.value)
            }
            (TriggerKind::FastPattern, Some(p)) => {
                (TriggerType::FastPattern, p.base_channel, p.length, p.value)
            }
            (kind, _) => {
                return Err(device_error::unsupported_trigger(format!(
                    "the board has no trigger of the kind {kind:?} (or it has no pattern)"
                )));
            }
        },
    };
    let too_long = || {
        device_error::invalid_capture_length(
            "the board needs a capture length that fits in 32 bits".to_owned(),
        )
    };
    let total = config.capture_length().ok_or_else(|| {
        device_error::invalid_capture_length("the board needs a finite capture length".to_owned())
    })?;
    let before = config.pre_trigger_samples();
    let after = total.saturating_sub(before);
    let highest = channels
        .get(..usize::from(count))
        .and_then(|used| used.iter().copied().max())
        .unwrap_or(0);
    Ok(CaptureRequest2 {
        trigger_type,
        trigger_channel,
        trigger_length,
        trigger_value,
        channel_count: count,
        channels,
        mode: highest / 8,
        frequency: u32::try_from(config.sample_rate_hz()).map_err(|_| too_long())?,
        samples_before: u32::try_from(before).map_err(|_| too_long())?,
        samples_after: u32::try_from(after).map_err(|_| too_long())?,
        bursts: u8::try_from(config.bursts()).map_err(|_| too_long())?,
        burst_times: false,
        role: Role::Single,
        encoding: Encoding::Raw,
        memory: Memory::Auto,
    })
}
