//! Restarting a device from the host (feature 017, T054): the command is accepted only for an action the
//! selected device declares, the answer arrives with a later poll and a failure is a notice; the menu entries
//! follow the same rule.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::panic
)]

mod common;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use common::TempDir;
use la_core::Result;
use la_devices::harness::test_descriptor;
use la_devices::{Device, DeviceAction, DeviceDescriptor, DeviceId, DeviceProvider};
use la_host::shortcuts::{Action, table};
use la_host::{Command, Outcome, Shell, ShellConfig};

/// A provider with two devices: one that offers both restarts and one that offers none.
struct Provider {
    offers: DeviceDescriptor,
    plain: DeviceDescriptor,
    done: Mutex<Vec<(DeviceId, DeviceAction)>>,
    fail: AtomicBool,
}

impl DeviceProvider for Provider {
    fn name(&self) -> &str {
        "fake"
    }

    fn scan(&self) -> Result<Vec<DeviceDescriptor>> {
        Ok(vec![self.offers.clone(), self.plain.clone()])
    }

    fn open(&self, id: &DeviceId) -> Result<Box<dyn Device>> {
        Err(la_devices::error::not_found(id.as_str()))
    }

    fn perform(&self, id: &DeviceId, action: DeviceAction) -> Result<()> {
        self.done.lock().unwrap().push((id.clone(), action));
        if self.fail.load(Ordering::Relaxed) {
            Err(la_devices::error::transport("the board did not answer"))
        } else {
            Ok(())
        }
    }
}

fn setup(dir: &TempDir) -> (Shell, Arc<Provider>) {
    let mut offers = test_descriptor("fake", "offers", 8);
    offers.capabilities.actions = vec![DeviceAction::Restart, DeviceAction::RestartIntoBootloader];
    let provider = Arc::new(Provider {
        offers,
        plain: test_descriptor("fake", "plain", 8),
        done: Mutex::new(Vec::new()),
        fail: AtomicBool::new(false),
    });
    let mut shell = Shell::new(ShellConfig::for_tests(dir.path()));
    shell.add_provider(provider.clone());
    let deadline = Instant::now() + Duration::from_secs(10);
    while shell.state().devices.len() < 3 {
        shell.poll(Instant::now());
        assert!(Instant::now() < deadline, "the devices were not listed");
        std::thread::sleep(Duration::from_millis(5));
    }
    (shell, provider)
}

fn select(shell: &mut Shell, unique: &str) {
    let id = DeviceId::new("fake", unique);
    assert!(matches!(
        shell.apply(Command::SelectDevice(id)),
        Outcome::Done(_)
    ));
}

fn wait_for_answer(shell: &mut Shell) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while shell.state().device_action.is_some() {
        shell.poll(Instant::now());
        assert!(Instant::now() < deadline, "the action did not end");
        std::thread::sleep(Duration::from_millis(5));
    }
}

fn menu_enabled(shell: &Shell, action: DeviceAction) -> bool {
    table()
        .iter()
        .find(|e| e.action == Action::Command(Command::DeviceAction(action)))
        .expect("the action is in the menu table")
        .enabled_when
        .is_enabled(shell.state())
}

#[test]
fn a_restart_is_done_by_the_provider_and_ends_with_the_answer() {
    let dir = TempDir::new("action-ok");
    let (mut shell, provider) = setup(&dir);
    select(&mut shell, "offers");
    assert!(menu_enabled(&shell, DeviceAction::Restart));
    assert!(matches!(
        shell.apply(Command::DeviceAction(DeviceAction::Restart)),
        Outcome::Done(_)
    ));
    assert_eq!(shell.state().device_action, Some(DeviceAction::Restart));
    assert!(
        !menu_enabled(&shell, DeviceAction::Restart),
        "not while one is in progress"
    );
    wait_for_answer(&mut shell);
    assert_eq!(
        provider.done.lock().unwrap().as_slice(),
        [(DeviceId::new("fake", "offers"), DeviceAction::Restart)]
    );
    assert_eq!(shell.state().notices.len(), 0, "a success makes no notice");
    assert!(menu_enabled(&shell, DeviceAction::RestartIntoBootloader));
}

#[test]
fn a_second_action_is_refused_while_one_is_in_progress() {
    let dir = TempDir::new("action-twice");
    let (mut shell, provider) = setup(&dir);
    select(&mut shell, "offers");
    shell.apply(Command::DeviceAction(DeviceAction::Restart));
    let second = shell.apply(Command::DeviceAction(DeviceAction::RestartIntoBootloader));
    assert!(!matches!(second, Outcome::Done(_)), "{second:?}");
    wait_for_answer(&mut shell);
    assert_eq!(provider.done.lock().unwrap().len(), 1);
}

#[test]
fn a_device_that_does_not_offer_the_action_is_not_asked() {
    let dir = TempDir::new("action-not-offered");
    let (mut shell, provider) = setup(&dir);
    select(&mut shell, "plain");
    assert!(!menu_enabled(&shell, DeviceAction::Restart));
    assert!(!menu_enabled(&shell, DeviceAction::RestartIntoBootloader));
    let outcome = shell.apply(Command::DeviceAction(DeviceAction::Restart));
    assert!(!matches!(outcome, Outcome::Done(_)), "{outcome:?}");
    assert!(provider.done.lock().unwrap().is_empty());
    assert!(shell.state().device_action.is_none());
}

#[test]
fn a_failure_is_a_notice_and_the_host_is_ready_again() {
    let dir = TempDir::new("action-fails");
    let (mut shell, provider) = setup(&dir);
    provider.fail.store(true, Ordering::Relaxed);
    select(&mut shell, "offers");
    shell.apply(Command::DeviceAction(DeviceAction::RestartIntoBootloader));
    wait_for_answer(&mut shell);
    assert_eq!(shell.state().notices.len(), 1);
    assert!(menu_enabled(&shell, DeviceAction::RestartIntoBootloader));
}
