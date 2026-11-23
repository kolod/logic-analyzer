//! The settings of a richer device through the host (feature 017, stage 2; task T024): the trigger type,
//! channel and pattern, the samples before the trigger and the number of bursts are validated against the
//! capabilities of the selected device and report the typed error; they are kept when the device changes only
//! where they still fit.
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

use common::TempDir;
use la_devices::error::{INVALID_CAPTURE_LENGTH, UNSUPPORTED_BURSTS, UNSUPPORTED_TRIGGER};
use la_devices::harness::{TestProvider, test_descriptor};
use la_devices::{
    ChannelSet, DepthBand, DeviceDescriptor, DeviceId, ModeLimit, Pattern, Trigger, TriggerKind,
    TriggerLimits,
};
use la_host::devices::Setting;
use la_host::{Command, ConfigEdit, Outcome, Shell, ShellConfig};

/// A 32-channel device with patterns, samples before the trigger, depth bands and bursts.
fn rich(unique: &str, most_bursts: u32) -> DeviceDescriptor {
    let mut descriptor = test_descriptor("fake", unique, 32);
    let caps = &mut descriptor.capabilities;
    caps.triggers = vec![
        TriggerKind::RisingEdge,
        TriggerKind::Pattern,
        TriggerKind::FastPattern,
    ];
    caps.trigger_limits = TriggerLimits {
        pattern_width: 16,
        fast_pattern_width: 4,
        chained: false,
        allowed_in_chain: false,
    };
    caps.pre_post = vec![ModeLimit {
        up_to_channels: 32,
        max_before: 1_000,
        max_after: 50_000,
    }];
    caps.buffer_depths = vec![DepthBand {
        from_rate_hz: 1_000,
        to_rate_hz: 100_000_000,
        depth: 60_000,
    }];
    caps.bursts = Some(most_bursts);
    descriptor
}

/// A device with none of the new functions.
fn plain() -> DeviceDescriptor {
    test_descriptor("fake", "plain", 32)
}

fn shell_with(dir: &TempDir, devices: &[DeviceDescriptor]) -> Shell {
    let mut shell = Shell::new(ShellConfig::for_tests(dir.path()));
    let provider = TestProvider::new("fake");
    shell.add_provider(Arc::new(provider.clone()));
    for device in devices {
        provider.plug(device.clone());
    }
    let deadline = Instant::now() + Duration::from_secs(10);
    while !devices
        .iter()
        .all(|d| shell.state().devices.iter().any(|listed| listed.id == d.id))
    {
        shell.poll(Instant::now());
        assert!(Instant::now() < deadline, "the devices were not listed");
        std::thread::sleep(Duration::from_millis(5));
    }
    shell
}

fn select(shell: &mut Shell, id: &DeviceId) {
    assert!(matches!(
        shell.apply(Command::SelectDevice(id.clone())),
        Outcome::Done(_)
    ));
}

fn edit(shell: &mut Shell, edit: ConfigEdit) {
    assert!(matches!(
        shell.apply(Command::EditConfig(edit)),
        Outcome::Done(_)
    ));
}

fn pattern_trigger(shell: &mut Shell, kind: TriggerKind, length: u8) {
    edit(shell, ConfigEdit::Channels(ChannelSet::all(32)));
    edit(shell, ConfigEdit::Length(10_000));
    edit(
        shell,
        ConfigEdit::Trigger(Some(Trigger { channel: 0, kind })),
    );
    edit(
        shell,
        ConfigEdit::Pattern(Some(Pattern {
            base_channel: 0,
            length,
            value: 1,
        })),
    );
}

#[test]
fn a_valid_pattern_trigger_is_accepted_by_the_host() {
    let dir = TempDir::new("rich-pattern-ok");
    let device = rich("a", 4);
    let mut shell = shell_with(&dir, std::slice::from_ref(&device));
    select(&mut shell, &device.id);
    pattern_trigger(&mut shell, TriggerKind::Pattern, 16);
    assert!(shell.state().config_problems.is_empty());
    assert_eq!(shell.state().config.pattern.unwrap().length, 16);
}

#[test]
fn a_pattern_wider_than_the_device_allows_names_the_trigger() {
    let dir = TempDir::new("rich-pattern-wide");
    let device = rich("a", 4);
    let mut shell = shell_with(&dir, std::slice::from_ref(&device));
    select(&mut shell, &device.id);
    pattern_trigger(&mut shell, TriggerKind::FastPattern, 5);
    let problems = &shell.state().config_problems;
    assert_eq!(problems.len(), 1);
    assert_eq!(problems[0].setting, Setting::Trigger);
    assert_eq!(problems[0].code, UNSUPPORTED_TRIGGER);
    // Shortening it fixes the problem.
    pattern_trigger(&mut shell, TriggerKind::FastPattern, 4);
    assert!(shell.state().config_problems.is_empty());
}

#[test]
fn samples_before_the_trigger_are_checked_against_the_mode() {
    let dir = TempDir::new("rich-pre");
    let device = rich("a", 4);
    let mut shell = shell_with(&dir, std::slice::from_ref(&device));
    select(&mut shell, &device.id);
    edit(&mut shell, ConfigEdit::Channels(ChannelSet::all(8)));
    edit(&mut shell, ConfigEdit::Length(10_000));
    edit(
        &mut shell,
        ConfigEdit::Trigger(Some(Trigger {
            channel: 0,
            kind: TriggerKind::RisingEdge,
        })),
    );
    edit(&mut shell, ConfigEdit::PreTrigger(1_000));
    assert!(shell.state().config_problems.is_empty());
    edit(&mut shell, ConfigEdit::PreTrigger(1_001));
    let problems = &shell.state().config_problems;
    assert_eq!(problems.len(), 1);
    assert_eq!(problems[0].setting, Setting::Length);
    assert_eq!(problems[0].code, INVALID_CAPTURE_LENGTH);
}

#[test]
fn bursts_are_checked_against_the_device() {
    let dir = TempDir::new("rich-bursts");
    let device = rich("a", 4);
    let mut shell = shell_with(&dir, std::slice::from_ref(&device));
    select(&mut shell, &device.id);
    edit(&mut shell, ConfigEdit::Length(10_000));
    edit(&mut shell, ConfigEdit::Bursts(4));
    assert!(shell.state().config_problems.is_empty());
    edit(&mut shell, ConfigEdit::Bursts(5));
    let problems = &shell.state().config_problems;
    assert_eq!(problems.len(), 1);
    assert_eq!(problems[0].setting, Setting::Bursts);
    assert_eq!(problems[0].code, UNSUPPORTED_BURSTS);
}

#[test]
fn a_device_without_the_functions_reports_them_as_unsupported() {
    let dir = TempDir::new("rich-plain");
    let device = plain();
    let mut shell = shell_with(&dir, std::slice::from_ref(&device));
    select(&mut shell, &device.id);
    edit(&mut shell, ConfigEdit::Bursts(2));
    assert_eq!(shell.state().config_problems[0].code, UNSUPPORTED_BURSTS);
    edit(&mut shell, ConfigEdit::Bursts(0));
    edit(&mut shell, ConfigEdit::PreTrigger(5));
    assert_eq!(
        shell.state().config_problems[0].code,
        INVALID_CAPTURE_LENGTH
    );
}

#[test]
fn settings_are_kept_on_a_device_change_only_where_they_still_fit() {
    let dir = TempDir::new("rich-switch");
    let (a, b, c) = (rich("a", 8), rich("b", 2), plain());
    let mut shell = shell_with(&dir, &[a.clone(), b.clone(), c.clone()]);
    select(&mut shell, &a.id);
    edit(&mut shell, ConfigEdit::Length(10_000));
    pattern_trigger(&mut shell, TriggerKind::Pattern, 8);
    edit(&mut shell, ConfigEdit::PreTrigger(500));
    edit(&mut shell, ConfigEdit::Bursts(4));
    assert!(shell.state().config_problems.is_empty());

    // The same functions at lower limits: the pattern and the samples before the trigger fit, 4 bursts do not.
    select(&mut shell, &b.id);
    let config = &shell.state().config;
    assert_eq!(config.pattern.unwrap().length, 8);
    assert_eq!(config.trigger.unwrap().kind, TriggerKind::Pattern);
    assert_eq!(config.pre_trigger_samples, 500);
    assert_eq!(config.bursts, 0, "4 bursts do not fit a device of 2");
    assert!(shell.state().config_problems.is_empty());

    // A device with none of it: everything goes back to the defaults of the device.
    select(&mut shell, &c.id);
    let config = &shell.state().config;
    assert_eq!(config.pattern, None);
    assert_eq!(config.trigger, None);
    assert_eq!(config.pre_trigger_samples, 0);
    assert_eq!(config.bursts, 0);
    assert!(shell.state().config_problems.is_empty());
}
