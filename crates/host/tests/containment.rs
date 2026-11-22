//! A failing background task is contained: reported, logged, and the data stays (spec SC-005,
//! US6 scenarios 2 and 3).
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
use la_devices::harness::{ScriptedDevice, Step, TestProvider, test_descriptor};
use la_devices::{Device, DeviceId};
use la_host::capture::CaptureEnd;
use la_host::notices::NoticeContent;
use la_host::{Activity, Command, ConfigEdit, Outcome, Shell, ShellConfig, Unsaved};
use la_i18n::{Language, ProblemKind};

fn codes(shell: &Shell) -> Vec<String> {
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

fn wait(shell: &mut Shell, what: &str, mut done: impl FnMut(&Shell) -> bool) {
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        shell.poll(Instant::now());
        if done(shell) {
            return;
        }
        assert!(Instant::now() < deadline, "timed out waiting for {what}");
        std::thread::sleep(Duration::from_millis(5));
    }
}

/// A shell with a scripted device that delivers 2,000 samples and then panics in the driver.
fn shell_with_panicking_device(dir: &TempDir) -> Shell {
    let mut shell = Shell::new(ShellConfig::for_tests(dir.path()));
    let provider = TestProvider::new("fake");
    shell.add_provider(Arc::new(provider.clone()));
    let descriptor = test_descriptor("fake", "1", 4);
    let device_descriptor = descriptor.clone();
    let slot = std::sync::Mutex::new(Some(ScriptedDevice::new(
        device_descriptor,
        vec![Step::Samples(vec![0b0101; 2_000]), Step::Panic],
    )));
    provider.plug_with(
        descriptor,
        Arc::new(move || {
            let device = slot.lock().unwrap().take().expect("opened once");
            Box::new(device) as Box<dyn Device>
        }),
    );
    let id = DeviceId::new("fake", "1");
    wait(&mut shell, "the device to be listed", |s| {
        s.state().devices.iter().any(|d| d.id == id)
    });
    assert!(matches!(
        shell.apply(Command::SelectDevice(id)),
        Outcome::Done(_)
    ));
    for edit in [
        ConfigEdit::Channels(la_devices::ChannelSet::from_u32(0b1111)),
        ConfigEdit::SampleRate(1_000_000),
        ConfigEdit::Length(10_000),
    ] {
        shell.apply(Command::EditConfig(edit));
    }
    assert!(shell.state().config_problems.is_empty());
    shell
}

#[test]
fn a_driver_panic_is_reported_and_the_data_captured_before_it_is_kept() {
    let dir = TempDir::new("panic-driver");
    let mut shell = shell_with_panicking_device(&dir);
    assert!(matches!(
        shell.apply(Command::StartCapture),
        Outcome::Done(_)
    ));
    wait(&mut shell, "the capture to end", |s| {
        s.state().activity == Activity::Idle
    });
    let capture = shell.state().capture.as_ref().unwrap();
    assert_eq!(
        capture.end,
        Some(CaptureEnd::DeviceError(la_devices::error::DRIVER_PANIC))
    );
    assert_eq!(
        capture.snapshot.length(),
        2_000,
        "the data before the panic is kept"
    );
    assert!(capture.incomplete);
    assert_eq!(codes(&shell), [la_devices::error::DRIVER_PANIC]);
    assert_eq!(shell.state().unsaved, Unsaved::Dirty);
    // The shell keeps working: the data is drawable and a new capture can be prepared.
    let plan = shell.frame_plan(la_host::window::ViewSize {
        pixels: 200,
        scale: 1.0,
    });
    assert_eq!(plan.channels.len(), 4);
    assert!(matches!(
        shell.apply(Command::EditConfig(ConfigEdit::Length(5_000))),
        Outcome::Done(_)
    ));
}

#[test]
fn repeated_failures_are_grouped_and_the_list_stays_bounded() {
    let dir = TempDir::new("flood");
    let mut shell = Shell::new(ShellConfig::for_tests(dir.path()));
    for _ in 0..10 {
        shell.apply(Command::TestError("session.damaged"));
    }
    assert_eq!(shell.state().notices.len(), 1);
    assert_eq!(shell.state().notices.iter().next().unwrap().count, 10);
    // Alternating failures are not grouped, but the list never exceeds its bound.
    for i in 0..200 {
        let code = if i % 2 == 0 {
            "session.damaged"
        } else {
            "session.truncated"
        };
        shell.apply(Command::TestError(code));
    }
    assert!(shell.state().notices.len() <= la_host::notices::MAX_NOTICES);
}

#[test]
fn a_code_without_a_message_shows_the_generic_text_and_records_a_defect() {
    let dir = TempDir::new("unknown-code");
    let mut shell = Shell::new(ShellConfig::for_tests(dir.path()));
    shell.apply(Command::TestError("host.no_such_code"));
    let notice = shell.state().notices.iter().next().unwrap().clone();
    shell.localizer().set_language(Language::En);
    let text = shell.notice_text(&notice);
    assert!(text.contains("host.no_such_code"), "{text}");
    assert!(
        shell
            .localizer()
            .problems()
            .iter()
            .any(|p| p.kind == ProblemKind::UnknownErrorCode)
    );
}
