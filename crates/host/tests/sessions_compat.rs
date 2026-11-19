//! Session files written before feature 014 open, and the optional `LAYT` section travels through
//! code that does not know it (spec 014, task T050).
//!
//! The two fixtures in `tests/fixtures/` were written by the code before feature 014
//! (`make_compat_fixtures.rs`). The `VIEW` section of the first is of payload version 1, which
//! feature 015 no longer reads: the capture opens with the default look, without markers, and
//! with the `host.view_invalid` warning (there has been no release, so nothing reads such files).
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

/// What the fixture of feature 006 gives now, checked on a shell that opened it: the capture, with
/// the default look, no markers and the warning about the section that is not read.
fn assert_old_view_fixture(shell: &Shell) {
    let state = shell.state();
    assert!(state.capture.is_some());
    assert!(state.markers.list.is_empty());
    assert_eq!(state.markers.origin, 0);
    assert!(state.auto_fit, "no remembered window");
    assert!(
        state
            .presentation
            .channels
            .iter()
            .all(|c| c.name.is_empty() && c.height == 0),
        "the default look"
    );
    let codes: Vec<&str> = state
        .notices
        .iter()
        .filter_map(|n| match &n.content {
            la_host::notices::NoticeContent::Error { code } => Some(code.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(codes, [la_host::error::VIEW_INVALID]);
}

#[test]
fn a_file_of_feature_006_opens_with_the_default_look_no_markers_and_a_warning() {
    let dir = TempDir::new("compat-006");
    let shell = open(&dir, &fixture("feature_006_view.lasession"));
    assert_old_view_fixture(&shell);
}

#[test]
fn saving_such_a_file_drops_the_section_that_is_not_read() {
    let dir = TempDir::new("compat-006-save");
    let mut shell = open(&dir, &fixture("feature_006_view.lasession"));
    let copy = dir.path().join("copy.lasession");
    save(&mut shell, &copy);
    assert!(
        extras(&copy)
            .iter()
            .all(|(tag, _)| tag != "LAYT" && tag != "VIEW"),
        "no custom height, no old view section"
    );
    let reopened = open(&dir, &copy);
    assert!(
        reopened.state().notices.is_empty(),
        "{:?}",
        reopened.state().notices
    );
    assert!(reopened.state().markers.list.is_empty());
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

/// What the file written by feature 015 holds, written out by hand: a single marker `T1` at 1,000
/// (comment "start"), a pair `P1` at 5,000..9,000 (colour 4, comment "bit 3"), time counted from `T1`.
#[test]
fn a_file_with_markers_of_both_kinds_opens_with_all_of_them() {
    use la_host::markers::Shape;
    let dir = TempDir::new("compat-015");
    let shell = open(&dir, &fixture("markers_v2.lasession"));
    let state = shell.state();
    assert!(state.notices.is_empty(), "{:?}", state.notices);
    let list = &state.markers.list;
    assert_eq!(list.len(), 2);
    assert_eq!(list[0].name(), "T1");
    assert_eq!(list[0].shape, Shape::Point(1_000));
    assert_eq!((list[0].colour, list[0].comment.as_str()), (0, "start"));
    assert_eq!(list[1].name(), "P1");
    assert_eq!(
        list[1].shape,
        Shape::Interval {
            start: 5_000,
            end: 9_000
        }
    );
    assert_eq!((list[1].colour, list[1].comment.as_str()), (4, "bit 3"));
    assert_eq!(state.markers.origin, 1_000);
    assert_eq!(state.markers.next_numbers(), [2, 2]);
}
