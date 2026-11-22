//! A capture of up to 160 channels through the host (feature 017, FR-025; task T011).
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

use common::{TempDir, assert_same_snapshot, wait_operation};
use la_devices::{ChannelSet, WideVirtualProvider, virtual_descriptor_with_channels};
use la_host::presentation::Colour;
use la_host::window::ViewSize;
use la_host::{Activity, Command, ConfigEdit, Outcome, Shell, ShellConfig};

const CHANNELS: u8 = 160;

fn done(shell: &mut Shell, command: Command) {
    match shell.apply(command) {
        Outcome::Done(_) => {}
        other => panic!("{other:?}"),
    }
}

/// A shell whose selected device has 160 channels, with every channel enabled and captured.
fn wide_shell(dir: &TempDir, length: u64) -> Shell {
    let mut shell = Shell::new(ShellConfig::for_tests(dir.path()));
    shell.add_provider(Arc::new(WideVirtualProvider::new(CHANNELS)));
    let id = virtual_descriptor_with_channels(CHANNELS).id;
    let deadline = Instant::now() + Duration::from_secs(10);
    while !shell.state().devices.iter().any(|d| d.id == id) {
        shell.poll(Instant::now());
        assert!(Instant::now() < deadline, "the wide device was not listed");
        std::thread::sleep(Duration::from_millis(5));
    }
    done(&mut shell, Command::SelectDevice(id));
    for edit in [
        ConfigEdit::Realtime(false),
        ConfigEdit::Channels(ChannelSet::all(CHANNELS)),
        ConfigEdit::SampleRate(1_000_000),
        ConfigEdit::Length(length),
        ConfigEdit::Seed(Some(11)),
        ConfigEdit::AcceptBudgetRisk(true),
    ] {
        done(&mut shell, Command::EditConfig(edit));
    }
    done(&mut shell, Command::StartCapture);
    let deadline = Instant::now() + Duration::from_secs(60);
    while shell.state().activity != Activity::Idle {
        shell.poll(Instant::now());
        assert!(Instant::now() < deadline, "the capture did not end");
        std::thread::sleep(Duration::from_millis(2));
    }
    done(
        &mut shell,
        Command::SetViewSize(ViewSize {
            pixels: 800,
            scale: 1.0,
        }),
    );
    shell
}

#[test]
fn a_capture_of_160_channels_is_saved_and_opened_with_its_presentation() {
    let dir = TempDir::new("wide-roundtrip");
    let mut shell = wide_shell(&dir, 20_000);
    let capture = shell.state().capture.as_ref().unwrap();
    assert_eq!(capture.snapshot.channels(), CHANNELS);
    assert_eq!(
        capture.description.enabled_mask(),
        ChannelSet::all(CHANNELS)
    );
    assert_eq!(
        shell.state().presentation.channels.len(),
        usize::from(CHANNELS)
    );
    assert_eq!(
        shell.state().presentation.visible_mask(),
        ChannelSet::all(CHANNELS)
    );
    assert!(capture.snapshot.transition_count(150).unwrap() > 0);

    // Names, colours and heights of channels above 31 and a hidden one survive a save.
    done(
        &mut shell,
        Command::RenameChannel {
            channel: 150,
            name: "far".to_owned(),
        },
    );
    done(
        &mut shell,
        Command::SetChannelColour {
            channel: 120,
            colour: Colour::Rgb(1, 2, 3),
        },
    );
    done(
        &mut shell,
        Command::SetChannelHeight {
            channel: 159,
            height: 60,
        },
    );
    done(
        &mut shell,
        Command::SetChannelVisible {
            channel: 140,
            visible: false,
        },
    );
    done(&mut shell, Command::MoveChannel { from: 150, to: 3 });
    let path = dir.path().join("wide.lasession");
    done(&mut shell, Command::SaveSessionAs(path.clone()));
    wait_operation(&mut shell);

    let other = TempDir::new("wide-roundtrip-b");
    let mut fresh = Shell::new(ShellConfig::for_tests(other.path()));
    done(&mut fresh, Command::OpenSession(path));
    wait_operation(&mut fresh);
    let (a, b) = (shell.state(), fresh.state());
    assert_same_snapshot(
        &a.capture.as_ref().unwrap().snapshot,
        &b.capture.as_ref().unwrap().snapshot,
    );
    assert_eq!(
        a.capture.as_ref().unwrap().description.enabled_mask(),
        b.capture.as_ref().unwrap().description.enabled_mask()
    );
    assert_eq!(a.presentation.channels, b.presentation.channels);
    assert!(!b.presentation.visible_mask().contains(140));
    assert_eq!(
        b.presentation
            .channels
            .iter()
            .find(|s| s.index == 150)
            .unwrap()
            .name,
        "far"
    );
}

#[test]
fn a_decoder_assigned_to_a_channel_above_31_runs() {
    let dir = TempDir::new("wide-decoder");
    let mut shell = wide_shell(&dir, 5_000);
    done(
        &mut shell,
        Command::AddDecoder {
            decoder: "i2c".to_owned(),
        },
    );
    let id = shell.state().decoders.instances[0].id;
    for (role, channel) in [("scl", Some(150_u8)), ("sda", Some(151))] {
        done(
            &mut shell,
            Command::SetDecoderChannel {
                id,
                role: role.to_owned(),
                channel,
            },
        );
    }
    let deadline = Instant::now() + Duration::from_secs(60);
    loop {
        shell.poll(Instant::now());
        let instance = shell.state().decoders.instance(id).unwrap();
        if matches!(
            instance.state,
            la_host::decoders::InstanceState::Finished | la_host::decoders::InstanceState::Failed
        ) {
            assert_eq!(
                instance.state,
                la_host::decoders::InstanceState::Finished,
                "{instance:?}"
            );
            break;
        }
        assert!(Instant::now() < deadline, "the decoder did not finish");
        std::thread::sleep(Duration::from_millis(2));
    }
}
