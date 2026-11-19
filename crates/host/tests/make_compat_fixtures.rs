//! Writes the session files kept in `tests/fixtures/` as the code **before** feature 014 writes
//! them (names, colours, a marker and a window as features 006 and 007 save them, and an I2C decoder
//! with its saved results as feature 008 does). `sessions_compat.rs` opens them.
//!
//! **Do not run this again**: the fixtures are meant to be what an earlier version wrote, and a
//! newer build would write something new. It is `#[ignore]`d for that reason.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::panic
)]

mod common;

use std::time::{Duration, Instant};

use common::{TempDir, finished_shell};
use la_decoders::decoders::i2c::ID;
use la_host::decoders::InstanceState;
use la_host::{Command, Outcome, Shell};

fn done(shell: &mut Shell, command: Command) {
    let outcome = shell.apply(command);
    assert!(matches!(outcome, Outcome::Done(_)), "{outcome:?}");
}

fn settle(shell: &mut Shell) {
    let deadline = Instant::now() + Duration::from_secs(60);
    loop {
        shell.poll(Instant::now());
        let busy = shell.state().decoders.instances.iter().any(|i| {
            i.state == InstanceState::Running || (i.state == InstanceState::Waiting && i.ready)
        });
        if shell.state().operation.is_none() && !busy {
            return;
        }
        assert!(Instant::now() < deadline, "did not settle");
        std::thread::sleep(Duration::from_millis(2));
    }
}

fn fixtures() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

#[test]
#[ignore = "writes the fixtures once; see the module comment"]
fn write_the_fixtures_of_the_code_before_feature_014() {
    std::fs::create_dir_all(fixtures()).unwrap();
    let dir = TempDir::new("compat-fixtures");

    // Names, colours, order, a marker and a window (feature 006).
    let mut shell = finished_shell(&dir, 0b1111, 20_000, 5);
    done(
        &mut shell,
        Command::RenameChannel {
            channel: 1,
            name: "Clock".to_owned(),
        },
    );
    done(
        &mut shell,
        Command::SetChannelColour {
            channel: 2,
            colour: la_host::presentation::Colour::Rgb(10, 200, 30),
        },
    );
    done(&mut shell, Command::MoveChannel { from: 3, to: 0 });
    // (The committed fixture was written with the marker of feature 006 and the `VIEW` section of
    // version 1; this generator is kept compiling for the record and is not to be re-run.)
    done(
        &mut shell,
        Command::BeginPlacement(la_host::markers::Kind::Single),
    );
    done(
        &mut shell,
        Command::FixPlacement {
            pointer: la_host::markers::Pointer {
                x: 12.0,
                lane: None,
            },
        },
    );
    done(
        &mut shell,
        Command::ZoomToRange {
            start: 1_000,
            end: 5_000,
        },
    );
    done(
        &mut shell,
        Command::SaveSessionAs(fixtures().join("feature_006_view.lasession")),
    );
    settle(&mut shell);

    // An I2C decoder with its saved results (features 007 and 008).
    let mut shell = finished_shell(&dir, 0b11, 30_000, 7);
    done(&mut shell, Command::AddDecoder { decoder: ID.into() });
    let id = shell.state().decoders.instances[0].id;
    for (role, channel) in [("scl", 0), ("sda", 1)] {
        done(
            &mut shell,
            Command::SetDecoderChannel {
                id,
                role: role.into(),
                channel: Some(channel),
            },
        );
    }
    done(&mut shell, Command::SetSaveDecoderResults(true));
    settle(&mut shell);
    done(
        &mut shell,
        Command::SaveSessionAs(fixtures().join("feature_008_decoder.lasession")),
    );
    settle(&mut shell);
}

/// Writes `markers_v2.lasession`: a capture with markers of both kinds, as feature 015 saves them
/// (`VIEW` payload version 2). **Do not run this again**, for the reason in the module comment:
/// `sessions_compat.rs` checks that later versions still read what this one wrote.
#[test]
#[ignore = "writes the fixture once; see the module comment"]
fn write_the_fixture_of_the_code_of_feature_015() {
    std::fs::create_dir_all(fixtures()).unwrap();
    let dir = TempDir::new("compat-fixtures-015");
    let mut shell = finished_shell(&dir, 0b11, 20_000, 5);
    let single = common::place_single(&mut shell, 1_000);
    let pair = common::place_pair(&mut shell, 5_000, 9_000);
    done(
        &mut shell,
        Command::SetMarkerColour {
            id: pair,
            colour: 4,
        },
    );
    done(
        &mut shell,
        Command::SetMarkerComment {
            id: single,
            comment: "start".into(),
        },
    );
    done(
        &mut shell,
        Command::SetMarkerComment {
            id: pair,
            comment: "bit 3".into(),
        },
    );
    done(&mut shell, Command::SetTimeOrigin(single));
    done(
        &mut shell,
        Command::SaveSessionAs(fixtures().join("markers_v2.lasession")),
    );
    settle(&mut shell);
}
