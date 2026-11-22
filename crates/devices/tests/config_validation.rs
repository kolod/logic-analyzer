//! Configuration validation (spec US2, FR-005..FR-008, SC-002).
//!
//! `validate` is pure: it never calls a device. The registry-level check that an invalid
//! configuration never reaches `DeviceProvider::open` is added with `registry.start`.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::panic
)]

mod common;

use la_devices::error::{
    INVALID_CAPTURE_LENGTH, INVALID_CHANNEL, INVALID_STATE, MEMORY_BUDGET, UNSUPPORTED_SAMPLE_RATE,
    UNSUPPORTED_TRIGGER,
};
use la_devices::{
    Capabilities, DeviceConfig, DeviceOptions, SampleRates, Trigger, TriggerKind, VirtualOptions,
    validate,
};
use la_storage::Limits;

/// 8 channels, three discrete rates, edge triggers, an on-board buffer of 1,000,000 samples.
fn buffered() -> Capabilities {
    Capabilities {
        channels: 8,
        sample_rates: SampleRates::List(vec![1_000, 1_000_000, 100_000_000]),
        triggers: vec![TriggerKind::RisingEdge, TriggerKind::FallingEdge],
        buffer_depth: Some(1_000_000),
    }
}

/// The same device but streaming (no buffer depth).
fn streaming() -> Capabilities {
    Capabilities {
        buffer_depth: None,
        ..buffered()
    }
}

fn small_budget() -> Limits {
    Limits {
        memory_budget_bytes: 1024 * 1024,
        ..Limits::default()
    }
}

fn base() -> DeviceConfig {
    DeviceConfig {
        enabled_channels: la_devices::ChannelSet::from_u32(0b11),
        sample_rate_hz: 1_000_000,
        trigger: None,
        capture_length: Some(1_000),
        options: DeviceOptions::None,
        accept_budget_risk: false,
    }
}

fn virtual_options(mean_gap: u32) -> DeviceOptions {
    DeviceOptions::Virtual(VirtualOptions {
        mean_gap,
        ..VirtualOptions::default()
    })
}

type Mutation = Box<dyn Fn(&mut DeviceConfig)>;

struct Case {
    name: &'static str,
    caps: Capabilities,
    limits: Limits,
    mutate: Mutation,
    code: &'static str,
    /// Text the error detail must contain (the offending value or the allowed values).
    detail: &'static str,
}

fn case(
    name: &'static str,
    caps: &Capabilities,
    limits: &Limits,
    mutate: impl Fn(&mut DeviceConfig) + 'static,
    code: &'static str,
    detail: &'static str,
) -> Case {
    Case {
        name,
        caps: caps.clone(),
        limits: *limits,
        mutate: Box::new(mutate),
        code,
        detail,
    }
}

fn invalid_cases() -> Vec<Case> {
    let (b, s, big, small) = (buffered(), streaming(), Limits::default(), small_budget());
    let rate = UNSUPPORTED_SAMPLE_RATE;
    let ch = INVALID_CHANNEL;
    let trig = UNSUPPORTED_TRIGGER;
    let len = INVALID_CAPTURE_LENGTH;
    vec![
        // ---- channels
        case(
            "no channel enabled",
            &b,
            &big,
            |c| c.enabled_channels = la_devices::ChannelSet::from_u32(0),
            ch,
            "no channel",
        ),
        case(
            "ninth channel",
            &b,
            &big,
            |c| c.enabled_channels = la_devices::ChannelSet::from_u32(1 << 8),
            ch,
            "8",
        ),
        case(
            "mask bit 31 on 8 channels",
            &b,
            &big,
            |c| c.enabled_channels = la_devices::ChannelSet::from_u32(1 << 31),
            ch,
            "31",
        ),
        case(
            "all bits set",
            &b,
            &big,
            |c| c.enabled_channels = la_devices::ChannelSet::from_u32(u32::MAX),
            ch,
            "8",
        ),
        case(
            "valid plus channel 9",
            &b,
            &big,
            |c| c.enabled_channels = la_devices::ChannelSet::from_u32(0b11 | (1 << 9)),
            ch,
            "9",
        ),
        // ---- sample rate
        case(
            "rate zero",
            &b,
            &big,
            |c| c.sample_rate_hz = 0,
            rate,
            "1000, 1000000, 100000000",
        ),
        case(
            "rate one",
            &b,
            &big,
            |c| c.sample_rate_hz = 1,
            rate,
            "nearest",
        ),
        case(
            "rate just below a listed one",
            &b,
            &big,
            |c| c.sample_rate_hz = 999,
            rate,
            "1000",
        ),
        case(
            "rate just above a listed one",
            &b,
            &big,
            |c| c.sample_rate_hz = 1_000_001,
            rate,
            "1000000",
        ),
        case(
            "rate between two listed ones",
            &b,
            &big,
            |c| c.sample_rate_hz = 50_000,
            rate,
            "nearest",
        ),
        case(
            "rate above the highest",
            &b,
            &big,
            |c| c.sample_rate_hz = 500_000_000,
            rate,
            "100000000",
        ),
        case(
            "rate u64::MAX",
            &b,
            &big,
            |c| c.sample_rate_hz = u64::MAX,
            rate,
            "supported",
        ),
        // ---- trigger
        case(
            "any-edge trigger not supported",
            &b,
            &big,
            |c| {
                c.trigger = Some(Trigger {
                    channel: 0,
                    kind: TriggerKind::AnyEdge,
                })
            },
            trig,
            "supported",
        ),
        case(
            "level-high trigger not supported",
            &b,
            &big,
            |c| {
                c.trigger = Some(Trigger {
                    channel: 0,
                    kind: TriggerKind::Level(true),
                })
            },
            trig,
            "supported",
        ),
        case(
            "level-low trigger not supported",
            &b,
            &big,
            |c| {
                c.trigger = Some(Trigger {
                    channel: 1,
                    kind: TriggerKind::Level(false),
                })
            },
            trig,
            "supported",
        ),
        case(
            "trigger on a disabled channel",
            &b,
            &big,
            |c| {
                c.trigger = Some(Trigger {
                    channel: 2,
                    kind: TriggerKind::RisingEdge,
                })
            },
            trig,
            "enabled",
        ),
        case(
            "trigger on channel 8",
            &b,
            &big,
            |c| {
                c.trigger = Some(Trigger {
                    channel: 8,
                    kind: TriggerKind::RisingEdge,
                })
            },
            trig,
            "8",
        ),
        case(
            "trigger on channel 255",
            &b,
            &big,
            |c| {
                c.trigger = Some(Trigger {
                    channel: 255,
                    kind: TriggerKind::FallingEdge,
                })
            },
            trig,
            "255",
        ),
        case(
            "trigger on channel 31",
            &b,
            &big,
            |c| {
                c.trigger = Some(Trigger {
                    channel: 31,
                    kind: TriggerKind::RisingEdge,
                })
            },
            trig,
            "31",
        ),
        // ---- capture length and buffer depth
        case(
            "length zero",
            &b,
            &big,
            |c| c.capture_length = Some(0),
            len,
            "zero",
        ),
        case(
            "length zero on a streaming device",
            &s,
            &big,
            |c| c.capture_length = Some(0),
            len,
            "zero",
        ),
        case(
            "length above the buffer depth",
            &b,
            &big,
            |c| c.capture_length = Some(1_000_001),
            len,
            "1000000",
        ),
        case(
            "length u64::MAX on a buffered device",
            &b,
            &big,
            |c| c.capture_length = Some(u64::MAX),
            len,
            "1000000",
        ),
        case(
            "no length on a buffered device",
            &b,
            &big,
            |c| c.capture_length = None,
            len,
            "finite",
        ),
        // ---- memory budget (streaming device, 1 MiB budget)
        case(
            "length over the budget (2 channels)",
            &s,
            &small,
            |c| c.capture_length = Some(300_000),
            MEMORY_BUDGET,
            "budget",
        ),
        case(
            "length u64::MAX over the budget",
            &s,
            &small,
            |c| c.capture_length = Some(u64::MAX),
            MEMORY_BUDGET,
            "budget",
        ),
        case(
            "a billion samples on one channel",
            &s,
            &small,
            |c| {
                c.enabled_channels = la_devices::ChannelSet::from_u32(1);
                c.capture_length = Some(1_000_000_000);
            },
            MEMORY_BUDGET,
            "budget",
        ),
        case(
            "eight channels just over the budget",
            &s,
            &small,
            |c| {
                c.enabled_channels = la_devices::ChannelSet::from_u32(0xFF);
                c.capture_length = Some(40_000);
            },
            MEMORY_BUDGET,
            "budget",
        ),
        // ---- virtual device options
        case(
            "virtual mean_gap zero",
            &b,
            &big,
            |c| c.options = virtual_options(0),
            INVALID_STATE,
            "mean_gap",
        ),
        case(
            "virtual mean_gap too large",
            &b,
            &big,
            |c| c.options = virtual_options(1_000_001),
            INVALID_STATE,
            "mean_gap",
        ),
        case(
            "virtual mean_gap u32::MAX",
            &b,
            &big,
            |c| c.options = virtual_options(u32::MAX),
            INVALID_STATE,
            "mean_gap",
        ),
    ]
}

#[test]
fn every_invalid_configuration_is_rejected_with_the_documented_code_and_an_actionable_detail() {
    let cases = invalid_cases();
    assert!(
        cases.len() >= 30,
        "SC-002 needs at least 30 distinct invalid cases"
    );
    for c in cases {
        let mut config = base();
        (c.mutate)(&mut config);
        let error = validate(&c.caps, &config, &c.limits).unwrap_err();
        assert_eq!(error.code(), c.code, "case `{}`: {error}", c.name);
        assert!(
            error.detail().contains(c.detail),
            "case `{}`: detail `{}` should mention `{}`",
            c.name,
            error.detail(),
            c.detail
        );
    }
}

#[test]
fn valid_configurations_are_accepted_and_resolve_to_exactly_the_request() {
    let (b, s, big) = (buffered(), streaming(), Limits::default());
    let cases: Vec<(Capabilities, Limits, DeviceConfig)> = vec![
        (b.clone(), big, base()),
        (
            b.clone(),
            big,
            DeviceConfig {
                enabled_channels: la_devices::ChannelSet::from_u32(1),
                ..base()
            },
        ),
        (
            b.clone(),
            big,
            DeviceConfig {
                enabled_channels: la_devices::ChannelSet::from_u32(0xFF),
                ..base()
            },
        ),
        (
            b.clone(),
            big,
            DeviceConfig {
                enabled_channels: la_devices::ChannelSet::from_u32(0b1010),
                ..base()
            },
        ),
        (
            b.clone(),
            big,
            DeviceConfig {
                sample_rate_hz: 1_000,
                ..base()
            },
        ),
        (
            b.clone(),
            big,
            DeviceConfig {
                sample_rate_hz: 100_000_000,
                ..base()
            },
        ),
        (
            b.clone(),
            big,
            DeviceConfig {
                capture_length: Some(1),
                ..base()
            },
        ),
        (
            b.clone(),
            big,
            DeviceConfig {
                capture_length: Some(1_000_000),
                ..base()
            },
        ),
        (
            b.clone(),
            big,
            DeviceConfig {
                trigger: Some(Trigger {
                    channel: 1,
                    kind: TriggerKind::RisingEdge,
                }),
                ..base()
            },
        ),
        (
            b.clone(),
            big,
            DeviceConfig {
                trigger: Some(Trigger {
                    channel: 0,
                    kind: TriggerKind::FallingEdge,
                }),
                ..base()
            },
        ),
        (
            s.clone(),
            big,
            DeviceConfig {
                capture_length: None,
                ..base()
            },
        ),
        (
            s.clone(),
            big,
            DeviceConfig {
                capture_length: Some(10_000_000),
                ..base()
            },
        ),
        (
            b.clone(),
            big,
            DeviceConfig {
                options: virtual_options(1),
                ..base()
            },
        ),
        (
            b,
            big,
            DeviceConfig {
                options: virtual_options(1_000_000),
                ..base()
            },
        ),
        // Over the budget, but the caller explicitly accepts the risk.
        (
            s,
            small_budget(),
            DeviceConfig {
                capture_length: Some(u64::MAX),
                accept_budget_risk: true,
                ..base()
            },
        ),
    ];
    for (i, (caps, limits, config)) in cases.into_iter().enumerate() {
        let resolved = validate(&caps, &config, &limits)
            .unwrap_or_else(|e| panic!("case {i} should be valid: {e}"));
        assert_eq!(
            resolved.enabled_channels(),
            config.enabled_channels,
            "case {i}"
        );
        assert_eq!(resolved.sample_rate_hz(), config.sample_rate_hz, "case {i}");
        assert_eq!(resolved.capture_length(), config.capture_length, "case {i}");
        assert_eq!(resolved.trigger(), config.trigger, "case {i}");
        assert_eq!(resolved.channels(), caps.channels, "case {i}");
    }
}

#[test]
fn a_range_capability_accepts_every_rate_inside_the_range_and_rejects_the_edges() {
    let caps = Capabilities {
        sample_rates: SampleRates::Range {
            min: 10,
            max: 500_000_000,
        },
        buffer_depth: None,
        ..buffered()
    };
    for rate in [10_u64, 11, 12_345, 500_000_000 - 1, 500_000_000] {
        let config = DeviceConfig {
            sample_rate_hz: rate,
            ..base()
        };
        assert!(
            validate(&caps, &config, &Limits::default()).is_ok(),
            "rate {rate}"
        );
    }
    for rate in [0_u64, 9, 500_000_001, u64::MAX] {
        let config = DeviceConfig {
            sample_rate_hz: rate,
            ..base()
        };
        let e = validate(&caps, &config, &Limits::default()).unwrap_err();
        assert_eq!(e.code(), UNSUPPORTED_SAMPLE_RATE, "rate {rate}");
        assert!(
            e.detail().contains("10 Hz to 500000000 Hz"),
            "{}",
            e.detail()
        );
    }
}

#[test]
fn thirty_two_channel_devices_accept_every_channel() {
    let caps = Capabilities {
        channels: 32,
        buffer_depth: None,
        ..buffered()
    };
    for mask in [1_u32, 1 << 31, u32::MAX, 0xFFFF_0000] {
        let config = DeviceConfig {
            enabled_channels: la_devices::ChannelSet::from_u32(mask),
            ..base()
        };
        assert!(
            validate(&caps, &config, &Limits::default()).is_ok(),
            "mask {mask:#x}"
        );
    }
    let e = validate(
        &caps,
        &DeviceConfig {
            enabled_channels: la_devices::ChannelSet::from_u32(0),
            ..base()
        },
        &Limits::default(),
    );
    assert_eq!(e.unwrap_err().code(), INVALID_CHANNEL);
}

// ---- through the registry: an invalid configuration never reaches the provider ----------

#[test]
fn registry_start_rejects_every_invalid_case_before_the_provider_is_opened() {
    use std::sync::Arc;
    use std::time::Duration;

    use common::{descriptor, registry_fast, wait_until};
    use la_devices::AcquisitionOptions;
    use la_devices::harness::TestProvider;

    let registry = registry_fast();
    let provider = TestProvider::new("usb");
    // The device in the registry has the capabilities of the table above (8 channels,
    // rates 1 kHz / 1 MHz / 100 MHz, rising and falling edge triggers, streaming).
    let device = descriptor("usb", "dev", 8);
    provider.plug(device.clone());
    registry.add_provider(Arc::new(provider.clone()));
    assert!(wait_until(Duration::from_secs(5), || registry
        .describe(&device.id)
        .is_some()));

    let options = AcquisitionOptions {
        limits: small_budget(),
        ..AcquisitionOptions::default()
    };
    let mut checked = 0;
    for c in invalid_cases() {
        // Only the cases that are invalid for a streaming device with 8 channels apply here
        // (the buffer-depth cases belong to the buffered capability set).
        if c.caps != streaming() && c.caps != buffered() {
            continue;
        }
        if c.caps.buffer_depth.is_some() && c.code == INVALID_CAPTURE_LENGTH {
            continue;
        }
        let mut config = base();
        (c.mutate)(&mut config);
        let error = registry.start(&device.id, config, options).unwrap_err();
        assert_eq!(error.code(), c.code, "case `{}`: {error}", c.name);
        checked += 1;
    }
    assert!(checked >= 25, "checked {checked} cases");
    assert_eq!(
        provider.open_calls(),
        0,
        "no invalid configuration reached the provider"
    );

    // Virtual device options on a device that is not the virtual device are rejected too.
    let config = DeviceConfig {
        options: virtual_options(50),
        ..base()
    };
    let error = registry.start(&device.id, config, options).unwrap_err();
    assert_eq!(error.code(), INVALID_STATE);
    assert_eq!(error.kind(), la_core::ErrorKind::InvalidInput);
    assert_eq!(provider.open_calls(), 0);
}
