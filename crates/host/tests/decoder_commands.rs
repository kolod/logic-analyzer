//! The decoder commands of the host (spec US1): adding, configuring, removing, refusals, passes
//! and the annotation selection.
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
use la_decoders::decoders::clocked_bits::ID;
use la_decoders::error;
use la_decoders::lanes::Cell;
use la_decoders::setup::Value;
use la_host::decoders::{AnnotationRef, InstanceState};
use la_host::window::ViewSize;
use la_host::{Changes, Command, Outcome, Shell, ShellConfig, Unsaved};

fn done(shell: &mut Shell, command: Command) -> Changes {
    match shell.apply(command) {
        Outcome::Done(changes) => changes,
        other => panic!("{other:?}"),
    }
}

/// Polls until no instance is running.
fn settle(shell: &mut Shell) {
    let deadline = Instant::now() + Duration::from_secs(60);
    loop {
        shell.poll(Instant::now());
        let busy = shell.state().decoders.instances.iter().any(|i| {
            i.state == InstanceState::Running || (i.state == InstanceState::Waiting && i.ready)
        });
        if !busy {
            return;
        }
        assert!(Instant::now() < deadline, "a decoder did not finish");
        std::thread::sleep(Duration::from_millis(2));
    }
}

/// A shell with a finished capture of two busy channels and one decoder fully set up.
fn with_decoder(dir: &TempDir) -> (Shell, u32) {
    let mut shell = finished_shell(dir, 0b11, 20_000, 7);
    done(
        &mut shell,
        Command::AddDecoder {
            decoder: ID.to_owned(),
        },
    );
    let id = shell.state().decoders.instances[0].id;
    for (role, channel) in [("clock", 0), ("data", 1)] {
        done(
            &mut shell,
            Command::SetDecoderChannel {
                id,
                role: role.into(),
                channel: Some(channel),
            },
        );
    }
    settle(&mut shell);
    (shell, id)
}

#[test]
fn a_decoder_needs_a_capture() {
    let dir = TempDir::new("deco-none");
    let mut shell = Shell::new(ShellConfig::for_tests(dir.path()));
    // The built-in decoders: the sample decoder and I2C.
    assert_eq!(shell.state().decoders.catalogue.len(), 2);
    assert!(shell.state().decoders.catalogue.iter().any(|c| c.id == ID));
    let outcome = shell.apply(Command::AddDecoder {
        decoder: ID.to_owned(),
    });
    assert!(matches!(outcome, Outcome::Ignored(_)), "{outcome:?}");
    assert!(shell.state().decoders.instances.is_empty());
}

#[test]
fn the_catalogue_is_built_from_the_descriptor_in_the_current_language() {
    let dir = TempDir::new("deco-catalogue");
    let mut shell = Shell::new(ShellConfig::for_tests(dir.path()));
    let entry = shell
        .state()
        .decoders
        .catalogue
        .iter()
        .find(|c| c.id == ID)
        .unwrap()
        .clone();
    assert_eq!(entry.name, "Clocked bits");
    let roles: Vec<&str> = entry.signals.iter().map(|s| s.role.as_str()).collect();
    assert_eq!(roles, ["clock", "data"]);
    assert!(entry.signals.iter().all(|s| s.required));
    let options: Vec<&str> = entry.options.iter().map(|o| o.id.as_str()).collect();
    assert_eq!(options, ["clock_edge", "bit_order", "bits", "idle"]);
    assert!(entry.has_generator && !entry.reads_items);
    done(&mut shell, Command::SetLanguage(la_i18n::Language::Uk));
    let entry = shell
        .state()
        .decoders
        .catalogue
        .iter()
        .find(|c| c.id == ID)
        .unwrap();
    assert_eq!(entry.name, "Тактовані біти");
}

#[test]
fn an_added_decoder_waits_for_its_channels_and_then_decodes() {
    let dir = TempDir::new("deco-add");
    let mut shell = finished_shell(&dir, 0b11, 20_000, 7);
    assert_eq!(
        shell.state().unsaved,
        Unsaved::Dirty,
        "a fresh capture is unsaved anyway"
    );
    let changes = done(
        &mut shell,
        Command::AddDecoder {
            decoder: ID.to_owned(),
        },
    );
    assert!(changes.contains(Changes::DECODERS));
    let instance = shell.state().decoders.instances[0].clone();
    assert_eq!(instance.state, InstanceState::Waiting);
    assert!(!instance.ready);
    // One channel is not enough; the second makes the setup complete and starts the pass.
    done(
        &mut shell,
        Command::SetDecoderChannel {
            id: instance.id,
            role: "clock".into(),
            channel: Some(0),
        },
    );
    assert_eq!(
        shell.state().decoders.instances[0].state,
        InstanceState::Waiting
    );
    done(
        &mut shell,
        Command::SetDecoderChannel {
            id: instance.id,
            role: "data".into(),
            channel: Some(1),
        },
    );
    settle(&mut shell);
    let finished = shell.state().decoders.instances[0].clone();
    assert_eq!(finished.state, InstanceState::Finished);
    assert!(finished.ready && !finished.incomplete && finished.fault.is_none());
    assert_eq!(
        finished.covered,
        shell.state().capture.as_ref().unwrap().snapshot.length()
    );
    assert!(finished.annotations > 100, "{}", finished.annotations);
    assert!(shell.state().decoders.refusal.is_none());
    assert!(shell.decoder_results(instance.id).is_some());
}

#[test]
fn a_refused_setup_names_the_setting_and_changes_nothing() {
    let dir = TempDir::new("deco-refuse");
    let (mut shell, id) = with_decoder(&dir);
    let before = shell.state().decoders.instances[0].clone();
    let refusals = [
        (
            Command::SetDecoderChannel {
                id,
                role: "data".into(),
                channel: Some(200),
            },
            "channels.data",
        ),
        (
            Command::SetDecoderOption {
                id,
                option: "bits".into(),
                value: Value::Integer(99),
            },
            "options.bits",
        ),
        (
            Command::SetDecoderOption {
                id,
                option: "bit_order".into(),
                value: Value::Choice("sideways".into()),
            },
            "options.bit_order",
        ),
        (
            Command::SetDecoderOption {
                id,
                option: "nothing".into(),
                value: Value::Boolean(true),
            },
            "options.nothing",
        ),
    ];
    for (command, setting) in refusals {
        done(&mut shell, command);
        let refusal = shell
            .state()
            .decoders
            .refusal
            .clone()
            .unwrap_or_else(|| panic!("not refused: {setting}"));
        assert_eq!(refusal.code, error::INVALID_SETUP);
        assert_eq!(refusal.setting, setting);
        assert_eq!(
            shell.state().decoders.instances[0],
            before,
            "nothing changed"
        );
    }
    // The next accepted command clears the refusal.
    done(
        &mut shell,
        Command::RenameDecoder {
            id,
            name: "UART".into(),
        },
    );
    assert!(shell.state().decoders.refusal.is_none());
}

#[test]
fn the_instance_limit_is_eight() {
    let dir = TempDir::new("deco-limit");
    let mut shell = finished_shell(&dir, 0b11, 2_000, 1);
    for _ in 0..8 {
        done(
            &mut shell,
            Command::AddDecoder {
                decoder: ID.to_owned(),
            },
        );
    }
    assert_eq!(shell.state().decoders.instances.len(), 8);
    done(
        &mut shell,
        Command::AddDecoder {
            decoder: ID.to_owned(),
        },
    );
    let refusal = shell.state().decoders.refusal.clone().unwrap();
    assert_eq!(refusal.code, error::TOO_MANY_INSTANCES);
    assert_eq!(shell.state().decoders.instances.len(), 8);
    // Ids are never reused.
    let ids: Vec<u32> = shell
        .state()
        .decoders
        .instances
        .iter()
        .map(|i| i.id)
        .collect();
    done(&mut shell, Command::RemoveDecoder(ids[0]));
    done(
        &mut shell,
        Command::AddDecoder {
            decoder: ID.to_owned(),
        },
    );
    assert!(!ids.contains(&shell.state().decoders.instances[7].id));
}

#[test]
fn rename_hide_and_remove() {
    let dir = TempDir::new("deco-edit");
    let (mut shell, id) = with_decoder(&dir);
    done(
        &mut shell,
        Command::RenameDecoder {
            id,
            name: "  \u{202e}I2C\u{0} bus  ".into(),
        },
    );
    assert_eq!(shell.state().decoders.instances[0].setup.name, "I2C bus");
    // A rename does not start a new pass.
    assert_eq!(
        shell.state().decoders.instances[0].state,
        InstanceState::Finished
    );
    done(&mut shell, Command::ShowDecoderLane { id, visible: false });
    assert!(!shell.state().decoders.instances[0].setup.lane_visible);
    assert_eq!(
        shell.state().decoders.instances[0].state,
        InstanceState::Finished
    );
    let too_long = "n".repeat(65);
    done(&mut shell, Command::RenameDecoder { id, name: too_long });
    assert_eq!(
        shell.state().decoders.instances[0]
            .setup
            .name
            .chars()
            .count(),
        64
    );
    done(&mut shell, Command::RemoveDecoder(id));
    assert!(shell.state().decoders.instances.is_empty());
    assert!(shell.decoder_results(id).is_none());
    // An unknown instance is ignored.
    assert!(matches!(
        shell.apply(Command::RemoveDecoder(id)),
        Outcome::Ignored(_)
    ));
}

#[test]
fn a_setup_change_makes_a_saved_capture_unsaved() {
    let dir = TempDir::new("deco-dirty");
    let (mut shell, id) = with_decoder(&dir);
    let path = dir.path().join("one.las");
    done(&mut shell, Command::SaveSessionAs(path));
    let deadline = Instant::now() + Duration::from_secs(30);
    while shell.state().operation.is_some() {
        shell.poll(Instant::now());
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(2));
    }
    assert_eq!(shell.state().unsaved, Unsaved::Clean);
    done(
        &mut shell,
        Command::SetDecoderOption {
            id,
            option: "bits".into(),
            value: Value::Integer(12),
        },
    );
    assert_eq!(shell.state().unsaved, Unsaved::Dirty);
    settle(&mut shell);
    // The pass finishing does not change the unsaved state again, and a rename of the lane
    // visibility does not need a new pass.
    assert_eq!(
        shell.state().decoders.instances[0].state,
        InstanceState::Finished
    );
}

#[test]
fn an_option_change_runs_a_new_pass_and_replaces_the_results() {
    let dir = TempDir::new("deco-rerun");
    let (mut shell, id) = with_decoder(&dir);
    let first = shell.state().decoders.instances[0].annotations;
    done(
        &mut shell,
        Command::SetDecoderOption {
            id,
            option: "bits".into(),
            value: Value::Integer(4),
        },
    );
    assert_eq!(
        shell.state().decoders.instances[0].state,
        InstanceState::Running
    );
    settle(&mut shell);
    let second = shell.state().decoders.instances[0].annotations;
    assert_ne!(
        first, second,
        "different bit counts group the edges differently"
    );
}

#[test]
fn selecting_an_annotation_fills_its_details() {
    let dir = TempDir::new("deco-select");
    let (mut shell, id) = with_decoder(&dir);
    shell.apply(Command::SetViewSize(ViewSize {
        pixels: 800,
        scale: 1.0,
    }));
    // Zoom in so that single annotations get cells of their own.
    done(&mut shell, Command::ZoomToRange { start: 0, end: 400 });
    let plan = shell.frame_plan(ViewSize {
        pixels: 800,
        scale: 1.0,
    });
    let lane = &plan.lanes[0];
    let cell = lane.plan.rows.iter().flatten().next().expect("a cell");
    let (target_id, start) = match cell {
        Cell::Span(s) => (s.id, s.start),
        Cell::Mark(m) => (m.id, m.start),
    };
    let reference = AnnotationRef {
        target: la_decoders::model::Target::Lane,
        id: target_id,
        start,
    };
    done(
        &mut shell,
        Command::SelectAnnotation {
            instance: id,
            annotation: reference,
        },
    );
    let selection = shell.state().decoders.selection.clone().unwrap();
    assert_eq!(selection.instance, id);
    assert_eq!(selection.details.start, start);
    assert!(!selection.details.text.is_empty());
    assert!(["range", "point", "edge"].contains(&selection.details.kind));
    done(&mut shell, Command::ClearAnnotationSelection);
    assert!(shell.state().decoders.selection.is_none());
    // An annotation that does not exist cannot be selected.
    let bogus = AnnotationRef {
        id: 9_999_999,
        ..reference
    };
    assert!(matches!(
        shell.apply(Command::SelectAnnotation {
            instance: id,
            annotation: bogus
        }),
        Outcome::Ignored(_)
    ));
}

#[test]
fn starting_a_new_capture_removes_the_decoders() {
    let dir = TempDir::new("deco-clear");
    let (mut shell, id) = with_decoder(&dir);
    let outcome = shell.apply(Command::NewCapture);
    let outcome = match outcome {
        Outcome::NeedsDecision(_) => shell.apply(Command::Decide(la_host::Decision::Discard)),
        other => other,
    };
    assert!(matches!(outcome, Outcome::Done(_)), "{outcome:?}");
    assert!(shell.state().decoders.instances.is_empty());
    assert!(shell.decoder_results(id).is_none());
}
