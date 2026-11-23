//! Chains of boards in the host (feature 017, T062): a chain is defined, saved in the settings and read back; the
//! chain leaves the device list while a board of it is missing and comes back with it; two chains cannot use one
//! board; definitions that cannot be used are refused or dropped.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::panic
)]

mod common;

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use common::TempDir;
use la_core::Result;
use la_devices::harness::test_descriptor;
use la_devices::{Device, DeviceChain, DeviceDescriptor, DeviceId, DeviceProvider};
use la_host::settings::{ChainSetting, RealSettingsFs, Settings};
use la_host::{Command, Outcome, Shell, ShellConfig};

/// A provider of boards that can be chained, and of the chains made of the boards that are present.
struct Provider {
    boards: Mutex<Vec<DeviceDescriptor>>,
    chains: Mutex<Vec<DeviceChain>>,
}

impl Provider {
    fn new(count: usize) -> Arc<Self> {
        let boards = (1..=count)
            .map(|k| {
                let mut board = test_descriptor("fake", &format!("B{k}"), 32);
                board.capabilities.chainable = true;
                board
            })
            .collect();
        Arc::new(Self {
            boards: Mutex::new(boards),
            chains: Mutex::new(Vec::new()),
        })
    }

    fn unplug(&self, unique: &str) -> DeviceDescriptor {
        let mut boards = self.boards.lock().unwrap();
        let at = boards
            .iter()
            .position(|b| b.id.as_str() == format!("fake:{unique}"))
            .unwrap();
        boards.remove(at)
    }

    fn plug(&self, board: DeviceDescriptor) {
        self.boards.lock().unwrap().push(board);
    }
}

impl DeviceProvider for Provider {
    fn name(&self) -> &str {
        "fake"
    }

    fn scan(&self) -> Result<Vec<DeviceDescriptor>> {
        let boards = self.boards.lock().unwrap().clone();
        let mut list = boards.clone();
        for chain in self.chains.lock().unwrap().iter() {
            if chain
                .boards
                .iter()
                .all(|id| boards.iter().any(|b| b.id == *id))
            {
                list.push(test_descriptor(
                    "fake",
                    &format!("chain-{}", chain.name),
                    64,
                ));
            }
        }
        Ok(list)
    }

    fn open(&self, id: &DeviceId) -> Result<Box<dyn Device>> {
        Err(la_devices::error::not_found(id.as_str()))
    }

    fn set_chains(&self, chains: &[DeviceChain]) {
        *self.chains.lock().unwrap() = chains.to_vec();
    }
}

fn shell_with(dir: &TempDir, provider: &Arc<Provider>, boards: usize) -> Shell {
    let mut shell = Shell::new(ShellConfig::for_tests(dir.path()));
    shell.add_provider(provider.clone());
    wait(&mut shell, |s| s.state().devices.len() > boards);
    shell
}

fn wait(shell: &mut Shell, done: impl Fn(&Shell) -> bool) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while !done(shell) {
        shell.poll(Instant::now());
        assert!(Instant::now() < deadline, "timed out");
        std::thread::sleep(Duration::from_millis(5));
    }
}

fn ids(unique: &[&str]) -> Vec<DeviceId> {
    unique.iter().map(|u| DeviceId::new("fake", u)).collect()
}

fn define(shell: &mut Shell, name: &str, boards: &[&str], shifts: &[u32]) -> Outcome {
    shell.apply(Command::DefineChain {
        name: name.to_owned(),
        boards: ids(boards),
        shifts: shifts.to_vec(),
    })
}

fn listed(shell: &Shell, unique: &str) -> bool {
    shell
        .state()
        .devices
        .iter()
        .any(|d| d.id.as_str() == format!("fake:{unique}"))
}

fn stored(dir: &TempDir) -> Settings {
    Settings::load(&RealSettingsFs, &dir.path().join("settings.json")).settings
}

#[test]
fn a_chain_is_defined_saved_and_read_back_on_the_next_start() {
    let dir = TempDir::new("chain-saved");
    let provider = Provider::new(3);
    let mut shell = shell_with(&dir, &provider, 3);
    assert!(matches!(
        define(&mut shell, "lab", &["B1", "B2"], &[0, 3]),
        Outcome::Done(_)
    ));
    wait(&mut shell, |s| listed(s, "chain-lab"));
    let expected = ChainSetting {
        name: "lab".to_owned(),
        boards: vec!["fake:B1".to_owned(), "fake:B2".to_owned()],
        shifts: vec![0, 3],
    };
    assert_eq!(shell.state().settings.chains, vec![expected.clone()]);
    assert_eq!(
        stored(&dir).chains,
        vec![expected.clone()],
        "written to the settings file"
    );
    drop(shell);

    // The next start reads it back and gives it to the provider before anything is asked.
    let provider = Provider::new(3);
    let mut shell = shell_with(&dir, &provider, 3);
    assert_eq!(shell.state().settings.chains, vec![expected]);
    wait(&mut shell, |s| listed(s, "chain-lab"));
}

#[test]
fn a_chain_can_be_redefined_by_its_name_and_removed() {
    let dir = TempDir::new("chain-edit");
    let provider = Provider::new(3);
    let mut shell = shell_with(&dir, &provider, 3);
    define(&mut shell, "lab", &["B1", "B2"], &[0, 0]);
    // The same name takes the new boards and shifts.
    assert!(matches!(
        define(&mut shell, "lab", &["B1", "B2", "B3"], &[0, 1, 2]),
        Outcome::Done(_)
    ));
    assert_eq!(shell.state().settings.chains.len(), 1);
    assert_eq!(shell.state().settings.chains[0].shifts, vec![0, 1, 2]);
    wait(&mut shell, |s| listed(s, "chain-lab"));
    // Removing it takes the device away and keeps the boards.
    assert!(matches!(
        shell.apply(Command::RemoveChain("lab".into())),
        Outcome::Done(_)
    ));
    wait(&mut shell, |s| !listed(s, "chain-lab"));
    assert!(listed(&shell, "B1") && listed(&shell, "B3"));
    assert!(stored(&dir).chains.is_empty());
    // A name that is not there is refused.
    assert!(!matches!(
        shell.apply(Command::RemoveChain("lab".into())),
        Outcome::Done(_)
    ));
}

#[test]
fn two_chains_cannot_use_one_board() {
    let dir = TempDir::new("chain-shared");
    let provider = Provider::new(4);
    let mut shell = shell_with(&dir, &provider, 4);
    assert!(matches!(
        define(&mut shell, "a", &["B1", "B2"], &[0, 0]),
        Outcome::Done(_)
    ));
    let second = define(&mut shell, "b", &["B2", "B3"], &[0, 0]);
    assert!(!matches!(second, Outcome::Done(_)), "{second:?}");
    assert_eq!(shell.state().settings.chains.len(), 1);
    // Boards that are free make a second chain.
    assert!(matches!(
        define(&mut shell, "b", &["B3", "B4"], &[0, 0]),
        Outcome::Done(_)
    ));
    assert_eq!(shell.state().settings.chains.len(), 2);
}

#[test]
fn the_chain_leaves_the_list_while_a_board_is_missing_and_comes_back_with_it() {
    let dir = TempDir::new("chain-missing");
    let provider = Provider::new(3);
    let mut shell = shell_with(&dir, &provider, 3);
    define(&mut shell, "lab", &["B1", "B2", "B3"], &[0, 0, 0]);
    wait(&mut shell, |s| listed(s, "chain-lab"));

    let board = provider.unplug("B2");
    shell.apply(Command::RefreshDevices);
    wait(&mut shell, |s| !listed(s, "chain-lab"));
    // The definition is kept: unplugging a board does not lose what the user set up.
    assert_eq!(shell.state().settings.chains.len(), 1);
    assert_eq!(stored(&dir).chains.len(), 1);

    provider.plug(board);
    wait(&mut shell, |s| listed(s, "chain-lab"));
}

#[test]
fn definitions_that_cannot_be_used_are_refused_and_change_nothing() {
    let dir = TempDir::new("chain-refused");
    let provider = Provider::new(6);
    let mut shell = shell_with(&dir, &provider, 6);
    let cases: Vec<(&str, Vec<&str>, Vec<u32>)> = vec![
        ("one", vec!["B1"], vec![0]),
        ("six", vec!["B1", "B2", "B3", "B4", "B5", "B6"], vec![0; 6]),
        ("twice", vec!["B1", "B1"], vec![0, 0]),
        ("shifts", vec!["B1", "B2"], vec![0]),
        ("not-listed", vec!["B1", "B9"], vec![0, 0]),
        ("", vec!["B1", "B2"], vec![0, 0]),
        ("virtual", vec!["B1", "virtual"], vec![0, 0]),
    ];
    for (name, boards, shifts) in cases {
        let outcome = define(&mut shell, name, &boards, &shifts);
        assert!(
            !matches!(outcome, Outcome::Done(_)),
            "{name:?}: {outcome:?}"
        );
    }
    assert!(shell.state().settings.chains.is_empty());
    assert!(stored(&dir).chains.is_empty());
}

#[test]
fn a_settings_file_with_chains_that_cannot_be_used_keeps_only_the_good_ones() {
    let dir = TempDir::new("chain-file");
    let chain = |name: &str, boards: &[&str], shifts: &[u32]| ChainSetting {
        name: name.to_owned(),
        boards: boards.iter().map(|b| (*b).to_owned()).collect(),
        shifts: shifts.to_vec(),
    };
    let settings = Settings {
        chains: vec![
            chain("good", &["fake:B1", "fake:B2"], &[0, 1]),
            chain("good", &["fake:B3", "fake:B4"], &[0, 0]), // the name again
            chain("shares", &["fake:B2", "fake:B3"], &[0, 0]), // a board of the first
            chain("short", &["fake:B3"], &[0]),
            chain("no-id", &["B3", "B4"], &[0, 0]),
            chain("other", &["fake:B3", "fake:B4"], &[0, 0]),
        ],
        ..Settings::default()
    };
    let kept = settings.normalized().chains;
    let names: Vec<&str> = kept.iter().map(|c| c.name.as_str()).collect();
    assert_eq!(names, ["good", "other"]);
    drop(dir);
}
