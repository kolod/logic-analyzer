//! Session files written before feature 014 open with nothing lost, and the optional `LAYT`
//! section travels through code that does not know it (spec 014, task T050).
//!
//! The two fixtures in `tests/fixtures/` were written by the code before the feature
//! (`make_compat_fixtures.rs`).
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::panic
)]

mod common;

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use common::{TempDir, finished_shell, wait_operation};
use la_decoders::decoders::i2c::ID;
use la_host::presentation::Colour;
use la_host::{Command, Outcome, Shell, ShellConfig};
use la_session::{Control, LoadLimits, SessionData};

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

fn open(dir: &TempDir, path: &Path) -> Shell {
    let mut shell = Shell::new(ShellConfig::for_tests(dir.path()));
    let outcome = shell.apply(Command::OpenSession(path.to_path_buf()));
    assert!(matches!(outcome, Outcome::Done(_)), "{outcome:?}");
    wait_operation(&mut shell);
    let deadline = Instant::now() + Duration::from_secs(30);
    while shell
        .state()
        .decoders
        .instances
        .iter()
        .any(|i| i.state == la_host::decoders::InstanceState::Running)
    {
        shell.poll(Instant::now());
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(2));
    }
    shell
}

fn save(shell: &mut Shell, path: &Path) {
    let outcome = shell.apply(Command::SaveSessionAs(path.to_path_buf()));
    assert!(matches!(outcome, Outcome::Done(_)), "{outcome:?}");
    wait_operation(shell);
}

fn extras(path: &Path) -> Vec<(String, Vec<u8>)> {
    let session = la_session::load_blocking(path, &LoadLimits::default(), &Control::new()).unwrap();
    session
        .extras()
        .iter()
        .map(|s| (s.tag.as_str().to_owned(), s.payload.clone()))
        .collect()
}

/// What the fixture of feature 006 holds, checked on a shell that opened it.
fn assert_view_fixture(shell: &Shell) {
    let state = shell.state();
    assert!(state.capture.is_some());
    let order: Vec<u8> = state
        .presentation
        .channels
        .iter()
        .map(|c| c.index)
        .collect();
    assert_eq!(&order[..4], &[3, 0, 1, 2], "the order");
    let style = |index: u8| {
        state
            .presentation
            .channels
            .iter()
            .find(|c| c.index == index)
            .unwrap()
            .clone()
    };
    assert_eq!(style(1).name, "Clock");
    assert_eq!(style(2).colour, Colour::Rgb(10, 200, 30));
    assert!(
        state.presentation.channels.iter().all(|c| c.height == 0),
        "automatic heights"
    );
    assert_eq!(state.markers.list.len(), 1);
    // Placed at 1,234 and snapped to the nearest edge (1,207) when the fixture was written.
    assert_eq!(state.markers.list[0].position, 1_207);
    assert!(!state.auto_fit);
    assert_eq!((state.window.start, state.window.length), (1_000, 4_000));
    assert!(state.notices.is_empty(), "{:?}", state.notices);
}

#[test]
fn a_file_of_feature_006_opens_with_nothing_lost_and_no_warning() {
    let dir = TempDir::new("compat-006");
    let shell = open(&dir, &fixture("feature_006_view.lasession"));
    assert_view_fixture(&shell);
}

#[test]
fn saving_such_a_file_keeps_everything_and_writes_no_layout_section() {
    let dir = TempDir::new("compat-006-save");
    let mut shell = open(&dir, &fixture("feature_006_view.lasession"));
    let copy = dir.path().join("copy.lasession");
    save(&mut shell, &copy);
    assert!(
        extras(&copy).iter().all(|(tag, _)| tag != "LAYT"),
        "no custom height, no section"
    );
    let reopened = open(&dir, &copy);
    assert_view_fixture(&reopened);
}

#[test]
fn a_file_of_feature_008_opens_with_its_decoder_and_saved_results() {
    let dir = TempDir::new("compat-008");
    let shell = open(&dir, &fixture("feature_008_decoder.lasession"));
    let state = shell.state();
    assert_eq!(state.decoders.instances.len(), 1);
    let instance = &state.decoders.instances[0];
    assert_eq!(instance.setup.decoder, ID);
    assert!(instance.loaded, "the results came from the file");
    assert!(!instance.outdated);
    assert!(state.notices.is_empty(), "{:?}", state.notices);
    assert!(
        state.presentation.channels.iter().all(|c| c.height == 0),
        "automatic heights"
    );
}

#[test]
fn heights_come_back_and_the_section_survives_a_reader_that_does_not_know_it() {
    let dir = TempDir::new("compat-layt");
    let mut shell = finished_shell(&dir, 0b111, 5_000, 9);
    let outcome = shell.apply(Command::SetChannelHeight {
        channel: 1,
        height: 90,
    });
    assert!(matches!(outcome, Outcome::Done(_)), "{outcome:?}");
    let with_heights = dir.path().join("heights.lasession");
    save(&mut shell, &with_heights);
    let written = extras(&with_heights);
    let layt = written
        .iter()
        .find(|(tag, _)| tag == "LAYT")
        .expect("a custom height is saved");

    // A reader that only passes extras on (what a build without the feature does) writes it back
    // byte for byte.
    let session =
        la_session::load_blocking(&with_heights, &LoadLimits::default(), &Control::new()).unwrap();
    let mut data = SessionData::new(session.description().clone(), session.snapshot());
    for section in session.extras() {
        data = data.with_section(section.clone()).unwrap();
    }
    let passed_on = dir.path().join("passed-on.lasession");
    la_session::save_blocking(&passed_on, &data, &Control::new()).unwrap();
    let again = extras(&passed_on);
    let layt_again = again.iter().find(|(tag, _)| tag == "LAYT").unwrap();
    assert_eq!(layt_again.1, layt.1, "the payload is kept byte for byte");

    // The feature reads it: the height is back.
    let reopened = open(&dir, &passed_on);
    let height_of = |index: u8| {
        reopened
            .state()
            .presentation
            .channels
            .iter()
            .find(|c| c.index == index)
            .unwrap()
            .height
    };
    assert_eq!(height_of(1), 90);
    assert_eq!(height_of(0), 0);
}
