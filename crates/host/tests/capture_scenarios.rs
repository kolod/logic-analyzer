//! Starting, running, stopping and ending a capture through the host (spec US1).
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::manual_range_contains
)]

mod common;

use std::time::{Duration, Instant};

use common::TempDir;
use la_devices::{FailureKind, ScriptedFailure, Trigger, TriggerKind};
use la_host::capture::CaptureEnd;
use la_host::devices::Setting;
use la_host::notices::NoticeContent;
use la_host::window::ViewSize;
use la_host::{
    Activity, Changes, Command, ConfigEdit, Decision, Guarded, Outcome, Shell, ShellConfig,
    SignalProfile, Tick, Unsaved, error,
};

fn shell_in(dir: &TempDir) -> Shell {
    Shell::new(ShellConfig::for_tests(dir.path()))
}

fn edit(shell: &mut Shell, edit: ConfigEdit) {
    let outcome = shell.apply(Command::EditConfig(edit));
    assert!(matches!(outcome, Outcome::Done(_)), "{outcome:?}");
}

/// Fast (unpaced) capture of `length` samples on `channels` at `rate` with `seed`.
fn configure(shell: &mut Shell, rate: u64, length: u64, channels: u32, seed: u64) {
    edit(shell, ConfigEdit::Realtime(false));
    edit(shell, ConfigEdit::Channels(channels));
    edit(shell, ConfigEdit::SampleRate(rate));
    edit(shell, ConfigEdit::Length(length));
    edit(shell, ConfigEdit::Seed(Some(seed)));
}

/// Polls until the shell is idle again; fails after `seconds`.
fn wait_idle(shell: &mut Shell, seconds: u64) {
    let deadline = Instant::now() + Duration::from_secs(seconds);
    loop {
        shell.poll(Instant::now());
        if shell.state().activity == Activity::Idle {
            return;
        }
        assert!(Instant::now() < deadline, "the capture did not end in time");
        std::thread::sleep(Duration::from_millis(5));
    }
}

fn wait_for(shell: &mut Shell, seconds: u64, mut condition: impl FnMut(&Shell) -> bool) {
    let deadline = Instant::now() + Duration::from_secs(seconds);
    loop {
        shell.poll(Instant::now());
        if condition(shell) {
            return;
        }
        assert!(Instant::now() < deadline, "timed out");
        std::thread::sleep(Duration::from_millis(5));
    }
}

fn error_codes(shell: &Shell) -> Vec<String> {
    shell
        .state()
        .notices
        .iter()
        .filter_map(|n| match &n.content {
            NoticeContent::Error { code } => Some(code.clone()),
            NoticeContent::Info { .. } => None,
        })
        .collect()
}

#[test]
fn the_virtual_device_is_listed_selected_and_marked_simulated() {
    let dir = TempDir::new("devices");
    let shell = shell_in(&dir);
    let state = shell.state();
    assert!(!state.devices.is_empty());
    let virtual_device = state.devices.iter().find(|d| d.simulated).unwrap();
    assert_eq!(state.selected_device.as_ref(), Some(&virtual_device.id));
    assert!(state.config_problems.is_empty());
    assert_eq!(state.activity, Activity::Idle);
    assert!(state.capture.is_none());
}

#[test]
fn the_default_configuration_is_valid_and_fits_the_device() {
    let dir = TempDir::new("defaults");
    let shell = shell_in(&dir);
    let config = &shell.state().config;
    assert_eq!(config.enabled_channels, 0xff);
    assert_eq!(config.sample_rate_hz, 1_000_000);
    assert_eq!(config.length, 1_000_000);
    assert!(config.realtime);
    assert_eq!(config.seed, None);
    assert_eq!(config.profile, SignalProfile::Normal);
}

#[test]
fn an_invalid_setting_is_reported_with_its_code_and_start_is_refused() {
    let dir = TempDir::new("problems");
    let mut shell = shell_in(&dir);
    let cases: Vec<(ConfigEdit, Setting, &str)> = vec![
        (
            ConfigEdit::SampleRate(0),
            Setting::SampleRate,
            la_devices::error::UNSUPPORTED_SAMPLE_RATE,
        ),
        (
            ConfigEdit::SampleRate(600_000_000),
            Setting::SampleRate,
            la_devices::error::UNSUPPORTED_SAMPLE_RATE,
        ),
        (
            ConfigEdit::Channels(0),
            Setting::Channels,
            la_devices::error::INVALID_CHANNEL,
        ),
        (
            ConfigEdit::Length(0),
            Setting::Length,
            la_devices::error::INVALID_CAPTURE_LENGTH,
        ),
        (
            ConfigEdit::Trigger(Some(Trigger {
                channel: 20,
                kind: TriggerKind::RisingEdge,
            })),
            Setting::Trigger,
            la_devices::error::UNSUPPORTED_TRIGGER,
        ),
    ];
    for (bad, setting, code) in cases {
        let fresh = TempDir::new("problem-case");
        shell = shell_in(&fresh);
        edit(&mut shell, bad.clone());
        let problems = &shell.state().config_problems;
        assert_eq!(problems.len(), 1, "{bad:?}");
        assert_eq!(problems[0].setting, setting, "{bad:?}");
        assert_eq!(problems[0].code, code, "{bad:?}");
        let version = shell.state().version;
        assert_eq!(
            shell.apply(Command::StartCapture),
            Outcome::Ignored(error::INVALID_COMMAND),
            "{bad:?}"
        );
        assert_eq!(shell.state().version, version);
        assert_eq!(shell.state().activity, Activity::Idle);
    }
    // Fixing the setting clears the problem.
    edit(&mut shell, ConfigEdit::Trigger(None));
    assert!(shell.state().config_problems.is_empty());
}

#[test]
fn a_capture_with_a_fixed_seed_ends_with_the_configured_samples_and_is_repeatable() {
    let mut plans = Vec::new();
    for _ in 0..2 {
        let dir = TempDir::new("seeded");
        let mut shell = shell_in(&dir);
        configure(&mut shell, 1_000_000, 100_000, 0b1111, 7);
        let outcome = shell.apply(Command::StartCapture);
        let Outcome::Done(changes) = outcome else {
            panic!("{outcome:?}");
        };
        assert!(changes.contains(Changes::ACTIVITY));
        assert_ne!(shell.state().activity, Activity::Idle);
        assert_eq!(shell.state().unsaved, Unsaved::Running);
        wait_idle(&mut shell, 20);
        let capture = shell.state().capture.as_ref().unwrap();
        assert_eq!(capture.snapshot.length(), 100_000);
        assert_eq!(capture.end, Some(CaptureEnd::Completed));
        assert!(!capture.incomplete);
        assert!(capture.simulated);
        assert_eq!(shell.state().unsaved, Unsaved::Dirty);
        let plan = shell.frame_plan(ViewSize {
            pixels: 500,
            scale: 1.0,
        });
        assert_eq!(plan.channels.len(), 4);
        plans.push(plan);
        assert!(error_codes(&shell).is_empty());
    }
    assert_eq!(plans[0], plans[1], "the same seed gives the same signals");

    // A different seed gives different signals.
    let dir = TempDir::new("other-seed");
    let mut shell = shell_in(&dir);
    configure(&mut shell, 1_000_000, 100_000, 0b1111, 8);
    shell.apply(Command::StartCapture);
    wait_idle(&mut shell, 20);
    let other = shell.frame_plan(ViewSize {
        pixels: 500,
        scale: 1.0,
    });
    assert_ne!(plans[0].channels, other.channels);
}

#[test]
fn polling_reports_progress_while_running_and_nothing_when_idle() {
    let dir = TempDir::new("progress");
    let mut shell = shell_in(&dir);
    assert_eq!(shell.wants_ticks(), Tick::Slow);
    assert!(shell.poll(Instant::now()).is_empty());
    // Real-time pacing at 10 kHz keeps the capture running for a while.
    edit(&mut shell, ConfigEdit::Channels(0b11));
    edit(&mut shell, ConfigEdit::SampleRate(10_000));
    edit(&mut shell, ConfigEdit::Length(100_000));
    edit(&mut shell, ConfigEdit::Seed(Some(3)));
    shell.apply(Command::StartCapture);
    assert_eq!(shell.wants_ticks(), Tick::Fast);
    let mut saw_progress = false;
    wait_for(&mut shell, 10, |shell| {
        if let Some(progress) = &shell.state().progress {
            saw_progress |= progress.samples > 0;
            return progress.samples > 100;
        }
        false
    });
    assert!(saw_progress);
    let progress = shell.state().progress.clone().unwrap();
    assert_eq!(
        progress.memory_limit,
        shell.state().settings.memory_limit_bytes
    );
    assert!(progress.memory_used > 0, "memory in use is reported");
    assert_eq!(progress.planned, Some(100_000));
    shell.apply(Command::StopCapture);
    wait_idle(&mut shell, 5);
    assert_eq!(shell.wants_ticks(), Tick::Slow);
    assert!(shell.poll(Instant::now()).is_empty());
}

#[test]
fn stop_ends_a_running_capture_within_a_second_and_keeps_the_data() {
    let dir = TempDir::new("stop");
    let mut shell = shell_in(&dir);
    edit(&mut shell, ConfigEdit::Channels(0b11));
    edit(&mut shell, ConfigEdit::SampleRate(10_000));
    edit(&mut shell, ConfigEdit::Length(1_000_000));
    shell.apply(Command::StartCapture);
    wait_for(&mut shell, 10, |s| {
        s.state()
            .capture
            .as_ref()
            .is_some_and(|c| c.snapshot.length() > 200)
    });
    let stopped_at = Instant::now();
    let outcome = shell.apply(Command::StopCapture);
    assert!(matches!(outcome, Outcome::Done(_)), "{outcome:?}");
    wait_idle(&mut shell, 5);
    assert!(stopped_at.elapsed() < Duration::from_secs(1));
    let capture = shell.state().capture.as_ref().unwrap();
    assert_eq!(capture.end, Some(CaptureEnd::Stopped));
    assert!(capture.snapshot.length() > 200);
    assert!(capture.snapshot.length() < 1_000_000);
    assert_eq!(shell.state().unsaved, Unsaved::Dirty);
    assert!(error_codes(&shell).is_empty(), "a stop is not a failure");
}

#[test]
fn a_failing_device_keeps_the_data_captured_before_the_failure() {
    let cases = [
        (
            FailureKind::Disconnect,
            CaptureEnd::Disconnected,
            la_devices::error::DISCONNECTED,
        ),
        (
            FailureKind::TransportError,
            CaptureEnd::DeviceError(la_devices::error::TRANSPORT),
            la_devices::error::TRANSPORT,
        ),
    ];
    for (kind, end, code) in cases {
        let dir = TempDir::new("failure");
        let mut shell = shell_in(&dir);
        configure(&mut shell, 1_000_000, 200_000, 0b11, 5);
        shell.script_failures(vec![ScriptedFailure {
            position: 50_000,
            kind,
        }]);
        shell.apply(Command::StartCapture);
        wait_idle(&mut shell, 20);
        let capture = shell.state().capture.as_ref().unwrap();
        assert_eq!(capture.end, Some(end), "{kind:?}");
        let length = capture.snapshot.length();
        assert!(length > 0 && length < 200_000, "{kind:?}: {length}");
        assert!(capture.incomplete, "{kind:?}");
        assert_eq!(error_codes(&shell), [code], "{kind:?}");
        assert_eq!(shell.state().unsaved, Unsaved::Dirty);
        // The data that was captured is drawable.
        let plan = shell.frame_plan(ViewSize {
            pixels: 300,
            scale: 1.0,
        });
        assert_eq!(plan.channels.len(), 2);
    }
}

#[test]
fn lost_samples_end_the_capture_as_incomplete_with_the_samples_lost_reason() {
    let dir = TempDir::new("loss");
    let mut shell = shell_in(&dir);
    configure(&mut shell, 1_000_000, 100_000, 0b11, 5);
    shell.script_failures(vec![ScriptedFailure {
        position: 20_000,
        kind: FailureKind::Loss(500),
    }]);
    shell.apply(Command::StartCapture);
    wait_idle(&mut shell, 20);
    let capture = shell.state().capture.as_ref().unwrap();
    assert_eq!(capture.end, Some(CaptureEnd::SamplesLost));
    assert!(capture.incomplete);
    assert_eq!(capture.snapshot.length(), 100_500);
    assert!(error_codes(&shell).is_empty());
}

#[test]
fn the_memory_limit_ends_the_capture_with_its_reason_and_keeps_the_data() {
    let dir = TempDir::new("limit");
    std::fs::write(
        dir.path().join("settings.json"),
        br#"{"version":1,"memory_limit_bytes":1048576}"#,
    )
    .unwrap();
    let mut shell = shell_in(&dir);
    assert_eq!(shell.state().settings.memory_limit_bytes, 1_048_576);
    configure(&mut shell, 1_000_000, 4_000_000, 0xff, 9);
    edit(&mut shell, ConfigEdit::Profile(SignalProfile::Dense));
    // The worst case does not fit the budget: a problem until the user accepts the risk.
    assert_eq!(
        shell.state().config_problems[0].code,
        la_devices::error::MEMORY_BUDGET
    );
    assert_eq!(shell.state().config_problems[0].setting, Setting::Length);
    edit(&mut shell, ConfigEdit::AcceptBudgetRisk(true));
    assert!(shell.state().config_problems.is_empty());
    shell.apply(Command::StartCapture);
    wait_idle(&mut shell, 30);
    let capture = shell.state().capture.as_ref().unwrap();
    assert_eq!(capture.end, Some(CaptureEnd::MemoryLimit));
    assert!(capture.incomplete);
    assert!(capture.snapshot.length() > 0);
    assert!(capture.snapshot.length() < 4_000_000);
    assert_eq!(error_codes(&shell), [la_storage_memory_limit()]);
}

fn la_storage_memory_limit() -> &'static str {
    "storage.memory_limit"
}

#[test]
fn a_second_start_while_running_is_ignored() {
    let dir = TempDir::new("twice");
    let mut shell = shell_in(&dir);
    edit(&mut shell, ConfigEdit::Channels(0b1));
    edit(&mut shell, ConfigEdit::SampleRate(10_000));
    edit(&mut shell, ConfigEdit::Length(1_000_000));
    assert!(matches!(
        shell.apply(Command::StartCapture),
        Outcome::Done(_)
    ));
    let outcome = shell.apply(Command::StartCapture);
    assert!(
        matches!(
            outcome,
            Outcome::Ignored(error::INVALID_COMMAND) | Outcome::NeedsDecision(_)
        ),
        "{outcome:?}"
    );
    shell.apply(Command::StopCapture);
    wait_idle(&mut shell, 5);
}

#[test]
fn a_new_capture_asks_before_discarding_unsaved_data() {
    let dir = TempDir::new("new");
    let mut shell = shell_in(&dir);
    // Nothing to lose: it runs at once.
    assert!(matches!(shell.apply(Command::NewCapture), Outcome::Done(_)));
    configure(&mut shell, 1_000_000, 10_000, 0b1, 1);
    shell.apply(Command::StartCapture);
    wait_idle(&mut shell, 20);
    assert_eq!(shell.state().unsaved, Unsaved::Dirty);

    let outcome = shell.apply(Command::NewCapture);
    assert_eq!(outcome, Outcome::NeedsDecision(Guarded::NewCapture));
    assert!(shell.state().capture.is_some(), "nothing was discarded");
    assert_eq!(
        shell.apply(Command::Decide(Decision::Cancel)),
        Outcome::Done(Changes::DECISION)
    );
    assert!(shell.state().capture.is_some());

    assert_eq!(
        shell.apply(Command::NewCapture),
        Outcome::NeedsDecision(Guarded::NewCapture)
    );
    let outcome = shell.apply(Command::Decide(Decision::Discard));
    assert!(matches!(outcome, Outcome::Done(_)), "{outcome:?}");
    assert!(shell.state().capture.is_none());
    assert_eq!(shell.state().unsaved, Unsaved::Clean);
}

#[test]
fn selecting_an_unknown_device_is_ignored_and_keeps_the_selection() {
    let dir = TempDir::new("unknown-device");
    let mut shell = shell_in(&dir);
    let outcome = shell.apply(Command::SelectDevice(la_devices::DeviceId::new(
        "nothing", "x",
    )));
    assert_eq!(outcome, Outcome::Ignored(error::INVALID_COMMAND));
    assert!(shell.state().selected_device.is_some());
}
