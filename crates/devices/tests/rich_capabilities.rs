//! The device abstraction describes the functions of a richer capture device (feature 017, stage 2):
//! pattern triggers, samples before the trigger by capture mode, buffer depth by rate band, bursts and the
//! highest rate by channel count. A value at the limit is accepted, one past it is refused with the typed
//! `device.*` error that names the limit; a device that declares none of this behaves as before.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::panic
)]

use la_devices::error::{
    INVALID_CAPTURE_LENGTH, INVALID_CHANNEL, UNSUPPORTED_BURSTS, UNSUPPORTED_SAMPLE_RATE,
    UNSUPPORTED_TRIGGER,
};
use la_devices::{
    Capabilities, ChannelSet, DepthBand, DeviceConfig, DeviceOptions, ModeLimit, Pattern,
    SampleRates, Trigger, TriggerKind, TriggerLimits, validate,
};
use la_storage::Limits;

/// A 32-channel device with every new function: patterns of 16 and fast patterns of 4 channels, samples
/// before the trigger by mode, buffer depth by rate band, up to 8 bursts and a lower rate for more channels.
fn rich() -> Capabilities {
    Capabilities {
        channels: 32,
        sample_rates: SampleRates::Range {
            min: 1_000,
            max: 200_000_000,
        },
        triggers: vec![
            TriggerKind::RisingEdge,
            TriggerKind::Pattern,
            TriggerKind::FastPattern,
        ],
        buffer_depth: None,
        trigger_limits: TriggerLimits {
            pattern_width: 16,
            fast_pattern_width: 4,
            chained: false,
            allowed_in_chain: false,
        },
        pre_post: vec![
            ModeLimit {
                up_to_channels: 8,
                max_before: 1_000,
                max_after: 100_000,
            },
            ModeLimit {
                up_to_channels: 32,
                max_before: 500,
                max_after: 50_000,
            },
        ],
        buffer_depths: vec![
            DepthBand {
                from_rate_hz: 1_000,
                to_rate_hz: 100_000_000,
                depth: 80_000,
            },
            DepthBand {
                from_rate_hz: 100_000_001,
                to_rate_hz: 200_000_000,
                depth: 40_000,
            },
        ],
        bursts: Some(8),
        max_rates: vec![(16, 200_000_000), (32, 100_000_000)],
        actions: Vec::new(),
    }
}

fn base() -> DeviceConfig {
    DeviceConfig {
        enabled_channels: ChannelSet::from_u32(0xFF),
        sample_rate_hz: 1_000_000,
        trigger: None,
        pattern: None,
        capture_length: Some(1_000),
        pre_trigger_samples: 0,
        bursts: 0,
        options: DeviceOptions::None,
        accept_budget_risk: false,
    }
}

fn code(caps: &Capabilities, config: &DeviceConfig) -> &'static str {
    validate(caps, config, &Limits::default())
        .unwrap_err()
        .code()
}

fn detail(caps: &Capabilities, config: &DeviceConfig) -> String {
    validate(caps, config, &Limits::default())
        .unwrap_err()
        .detail()
        .to_owned()
}

fn pattern_config(kind: TriggerKind, base_channel: u8, length: u8) -> DeviceConfig {
    DeviceConfig {
        trigger: Some(Trigger {
            channel: base_channel,
            kind,
        }),
        pattern: Some(Pattern {
            base_channel,
            length,
            value: 0b1,
        }),
        ..base()
    }
}

#[test]
fn the_plain_configuration_is_accepted() {
    assert!(validate(&rich(), &base(), &Limits::default()).is_ok());
}

#[test]
fn a_pattern_trigger_is_accepted_up_to_the_widest_pattern() {
    let ok = pattern_config(TriggerKind::Pattern, 0, 8);
    let resolved = validate(&rich(), &ok, &Limits::default()).unwrap();
    assert_eq!(resolved.pattern().unwrap().length, 8);

    // 16 channels need channels 0..16, so enable them.
    let mut at_limit = pattern_config(TriggerKind::Pattern, 0, 16);
    at_limit.enabled_channels = ChannelSet::all(16);
    assert!(validate(&rich(), &at_limit, &Limits::default()).is_ok());

    let mut past = pattern_config(TriggerKind::Pattern, 0, 17);
    past.enabled_channels = ChannelSet::all(17);
    assert_eq!(code(&rich(), &past), UNSUPPORTED_TRIGGER);
    assert!(detail(&rich(), &past).contains("16"), "names the limit");
}

#[test]
fn a_fast_pattern_has_its_own_narrower_limit() {
    let mut at_limit = pattern_config(TriggerKind::FastPattern, 0, 4);
    at_limit.enabled_channels = ChannelSet::all(4);
    assert!(validate(&rich(), &at_limit, &Limits::default()).is_ok());

    let mut past = pattern_config(TriggerKind::FastPattern, 0, 5);
    past.enabled_channels = ChannelSet::all(5);
    assert_eq!(code(&rich(), &past), UNSUPPORTED_TRIGGER);
    assert!(detail(&rich(), &past).contains('4'));
}

#[test]
fn a_pattern_must_be_complete_and_consistent() {
    // A pattern kind without a pattern.
    let mut missing = pattern_config(TriggerKind::Pattern, 0, 4);
    missing.pattern = None;
    assert_eq!(code(&rich(), &missing), UNSUPPORTED_TRIGGER);

    // A pattern with a kind that does not use one.
    let mut stray = pattern_config(TriggerKind::RisingEdge, 0, 4);
    stray.trigger = Some(Trigger {
        channel: 0,
        kind: TriggerKind::RisingEdge,
    });
    assert_eq!(code(&rich(), &stray), UNSUPPORTED_TRIGGER);

    // An empty pattern.
    let empty = pattern_config(TriggerKind::Pattern, 0, 0);
    assert_eq!(code(&rich(), &empty), UNSUPPORTED_TRIGGER);

    // The trigger channel is the pattern's first channel.
    let mut other = pattern_config(TriggerKind::Pattern, 0, 4);
    other.trigger = Some(Trigger {
        channel: 2,
        kind: TriggerKind::Pattern,
    });
    assert_eq!(code(&rich(), &other), UNSUPPORTED_TRIGGER);

    // A channel of the pattern that is not enabled.
    let mut disabled = pattern_config(TriggerKind::Pattern, 6, 4);
    disabled.enabled_channels = ChannelSet::all(8);
    assert_eq!(code(&rich(), &disabled), UNSUPPORTED_TRIGGER);

    // A channel of the pattern that the device does not have.
    let mut beyond = pattern_config(TriggerKind::Pattern, 30, 4);
    beyond.enabled_channels = ChannelSet::all(32);
    assert_eq!(code(&rich(), &beyond), UNSUPPORTED_TRIGGER);

    // A value with bits above the pattern's length.
    let mut wide_value = pattern_config(TriggerKind::Pattern, 0, 4);
    wide_value.pattern = Some(Pattern {
        base_channel: 0,
        length: 4,
        value: 0b1_0000,
    });
    assert_eq!(code(&rich(), &wide_value), UNSUPPORTED_TRIGGER);
}

#[test]
fn a_pattern_trigger_in_a_chain_is_refused_where_the_capabilities_say() {
    let mut chain = rich();
    chain.trigger_limits.chained = true;
    chain.trigger_limits.allowed_in_chain = false;
    let config = pattern_config(TriggerKind::Pattern, 0, 4);
    assert_eq!(code(&chain, &config), UNSUPPORTED_TRIGGER);
    assert!(detail(&chain, &config).contains("chain"));

    chain.trigger_limits.allowed_in_chain = true;
    let mut ok = config.clone();
    ok.enabled_channels = ChannelSet::all(8);
    assert!(validate(&chain, &ok, &Limits::default()).is_ok());
}

#[test]
fn a_device_without_pattern_triggers_refuses_them() {
    let mut caps = rich();
    caps.triggers = vec![TriggerKind::RisingEdge];
    caps.trigger_limits = TriggerLimits::NONE;
    let config = pattern_config(TriggerKind::Pattern, 0, 4);
    assert_eq!(code(&caps, &config), UNSUPPORTED_TRIGGER);
}

#[test]
fn samples_before_the_trigger_are_limited_by_the_capture_mode() {
    // 8 enabled channels fall in the first mode: up to 1,000 before, 100,000 after.
    let mut at_limit = base();
    at_limit.trigger = Some(Trigger {
        channel: 0,
        kind: TriggerKind::RisingEdge,
    });
    at_limit.pre_trigger_samples = 1_000;
    at_limit.capture_length = Some(1_000 + 70_000);
    let resolved = validate(&rich(), &at_limit, &Limits::default()).unwrap();
    assert_eq!(resolved.pre_trigger_samples(), 1_000);

    let mut past = at_limit.clone();
    past.pre_trigger_samples = 1_001;
    assert_eq!(code(&rich(), &past), INVALID_CAPTURE_LENGTH);
    assert!(detail(&rich(), &past).contains("1000"));

    // 20 enabled channels fall in the second mode: up to 500 before.
    let mut wide = at_limit.clone();
    wide.enabled_channels = ChannelSet::all(20);
    wide.pre_trigger_samples = 500;
    wide.capture_length = Some(500 + 10_000);
    assert!(validate(&rich(), &wide, &Limits::default()).is_ok());
    wide.pre_trigger_samples = 501;
    assert_eq!(code(&rich(), &wide), INVALID_CAPTURE_LENGTH);
}

#[test]
fn samples_after_the_trigger_are_limited_by_the_capture_mode() {
    let mut config = base();
    config.trigger = Some(Trigger {
        channel: 0,
        kind: TriggerKind::RisingEdge,
    });
    config.pre_trigger_samples = 100;
    // The depth at 1 MHz is 80,000, which is below the mode's 100,000 after, so use the depth to set the end.
    config.capture_length = Some(80_000);
    assert!(validate(&rich(), &config, &Limits::default()).is_ok());

    // 20 channels: at most 50,000 after the trigger.
    config.enabled_channels = ChannelSet::all(20);
    config.capture_length = Some(100 + 50_000);
    assert!(validate(&rich(), &config, &Limits::default()).is_ok());
    config.capture_length = Some(100 + 50_001);
    assert_eq!(code(&rich(), &config), INVALID_CAPTURE_LENGTH);
    assert!(detail(&rich(), &config).contains("50000"));
}

#[test]
fn samples_before_the_trigger_need_a_trigger_and_fit_the_length() {
    let mut no_trigger = base();
    no_trigger.pre_trigger_samples = 10;
    assert_eq!(code(&rich(), &no_trigger), INVALID_CAPTURE_LENGTH);

    let mut long = base();
    long.trigger = Some(Trigger {
        channel: 0,
        kind: TriggerKind::RisingEdge,
    });
    long.pre_trigger_samples = 900;
    long.capture_length = Some(800);
    assert_eq!(code(&rich(), &long), INVALID_CAPTURE_LENGTH);
}

#[test]
fn a_device_without_pre_trigger_support_refuses_samples_before_the_trigger() {
    let mut caps = rich();
    caps.pre_post = Vec::new();
    let mut config = base();
    config.trigger = Some(Trigger {
        channel: 0,
        kind: TriggerKind::RisingEdge,
    });
    config.pre_trigger_samples = 1;
    assert_eq!(code(&caps, &config), INVALID_CAPTURE_LENGTH);
    config.pre_trigger_samples = 0;
    assert!(validate(&caps, &config, &Limits::default()).is_ok());
}

#[test]
fn the_buffer_depth_follows_the_rate_band() {
    // Up to 100 MHz the depth is 80,000.
    let mut config = base();
    config.sample_rate_hz = 100_000_000;
    config.capture_length = Some(80_000);
    assert!(validate(&rich(), &config, &Limits::default()).is_ok());
    config.capture_length = Some(80_001);
    assert_eq!(code(&rich(), &config), INVALID_CAPTURE_LENGTH);
    assert!(detail(&rich(), &config).contains("80000"));

    // Above it the depth is 40,000.
    config.sample_rate_hz = 100_000_001;
    config.capture_length = Some(40_000);
    assert!(validate(&rich(), &config, &Limits::default()).is_ok());
    config.capture_length = Some(40_001);
    assert_eq!(code(&rich(), &config), INVALID_CAPTURE_LENGTH);
    assert!(detail(&rich(), &config).contains("40000"));
}

#[test]
fn a_buffered_device_needs_a_finite_length_when_it_has_bands() {
    let mut config = base();
    config.capture_length = None;
    assert_eq!(code(&rich(), &config), INVALID_CAPTURE_LENGTH);
}

#[test]
fn the_highest_rate_depends_on_the_channel_count() {
    // Up to 16 enabled channels: 200 MHz; more: 100 MHz.
    let mut config = base();
    config.sample_rate_hz = 200_000_000;
    config.capture_length = Some(40_000);
    config.enabled_channels = ChannelSet::all(16);
    assert!(validate(&rich(), &config, &Limits::default()).is_ok());

    config.enabled_channels = ChannelSet::all(17);
    assert_eq!(code(&rich(), &config), UNSUPPORTED_SAMPLE_RATE);
    assert!(detail(&rich(), &config).contains("100000000"));

    config.sample_rate_hz = 100_000_000;
    // 17 channels allow 50,000 samples after the trigger.
    config.capture_length = Some(50_000);
    assert!(validate(&rich(), &config, &Limits::default()).is_ok());
}

#[test]
fn bursts_are_accepted_up_to_the_most_the_device_supports() {
    let mut config = base();
    config.bursts = 8;
    assert_eq!(
        validate(&rich(), &config, &Limits::default())
            .unwrap()
            .bursts(),
        8
    );
    config.bursts = 9;
    assert_eq!(code(&rich(), &config), UNSUPPORTED_BURSTS);
    assert!(detail(&rich(), &config).contains('8'));

    let mut none = rich();
    none.bursts = None;
    config.bursts = 2;
    assert_eq!(code(&none, &config), UNSUPPORTED_BURSTS);
    config.bursts = 0;
    assert!(validate(&none, &config, &Limits::default()).is_ok());
}

#[test]
fn the_capabilities_of_a_rich_device_validate() {
    use la_devices::DeviceDescriptor;
    use la_devices::DeviceId;
    let good = DeviceDescriptor {
        id: DeviceId::new("test", "rich"),
        name: "Rich".to_owned(),
        simulated: false,
        capabilities: rich(),
    };
    assert!(good.validate().is_ok());

    type Mutation = Box<dyn Fn(&mut Capabilities)>;
    let bad: Vec<Mutation> = vec![
        // A pattern wider than the device.
        Box::new(|c| c.trigger_limits.pattern_width = 33),
        // A mode that goes backwards.
        Box::new(|c| {
            c.pre_post = vec![
                ModeLimit {
                    up_to_channels: 16,
                    max_before: 1,
                    max_after: 1,
                },
                ModeLimit {
                    up_to_channels: 8,
                    max_before: 1,
                    max_after: 1,
                },
            ]
        }),
        // Rate bands that overlap.
        Box::new(|c| {
            c.buffer_depths = vec![
                DepthBand {
                    from_rate_hz: 1,
                    to_rate_hz: 10,
                    depth: 5,
                },
                DepthBand {
                    from_rate_hz: 10,
                    to_rate_hz: 20,
                    depth: 5,
                },
            ]
        }),
        // A zero depth.
        Box::new(|c| {
            c.buffer_depths = vec![DepthBand {
                from_rate_hz: 1,
                to_rate_hz: 10,
                depth: 0,
            }]
        }),
        // Zero bursts supported is the same as none.
        Box::new(|c| c.bursts = Some(0)),
        // A rate limit for a channel count the device does not have.
        Box::new(|c| c.max_rates = vec![(33, 1_000)]),
        // A rate limit that goes up with the channel count.
        Box::new(|c| c.max_rates = vec![(8, 1_000), (16, 2_000)]),
    ];
    for (i, mutate) in bad.iter().enumerate() {
        let mut d = good.clone();
        mutate(&mut d.capabilities);
        assert_eq!(
            d.validate().unwrap_err().code(),
            la_devices::error::INVALID_CAPABILITIES,
            "case {i}"
        );
    }
}

#[test]
fn a_device_that_declares_none_of_it_behaves_as_before() {
    let plain = Capabilities {
        channels: 8,
        sample_rates: SampleRates::List(vec![1_000_000]),
        triggers: vec![TriggerKind::RisingEdge],
        buffer_depth: Some(1_000),
        trigger_limits: TriggerLimits::NONE,
        pre_post: Vec::new(),
        buffer_depths: Vec::new(),
        bursts: None,
        max_rates: Vec::new(),
        actions: Vec::new(),
    };
    let mut config = base();
    config.capture_length = Some(1_000);
    assert!(validate(&plain, &config, &Limits::default()).is_ok());
    config.capture_length = Some(1_001);
    assert_eq!(code(&plain, &config), INVALID_CAPTURE_LENGTH);
    config.capture_length = Some(10);
    config.bursts = 1;
    assert_eq!(code(&plain, &config), UNSUPPORTED_BURSTS);
    config.bursts = 0;
    config.enabled_channels = ChannelSet::from_u32(1 << 9);
    assert_eq!(code(&plain, &config), INVALID_CHANNEL);
}
