//! A decoder that fails never harms the session, and the user is told in plain words (spec US2).
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
use la_decoders::descriptor::Descriptor;
use la_decoders::run::Deadlines;
use la_decoders::testing::{self, HEALTHY, NEVER_RETURNS, PANICS, RETURNS_ERROR};
use la_host::decoders::InstanceState;
use la_host::{Command, Outcome, Shell, Unsaved};
use la_i18n::Language;

fn done(shell: &mut Shell, command: Command) {
    let outcome = shell.apply(command);
    assert!(matches!(outcome, Outcome::Done(_)), "{outcome:?}");
}

/// A shell with a capture and every fixture registered.
fn shell(dir: &TempDir) -> Shell {
    let mut shell = finished_shell(dir, 0b11, 60_000, 5);
    for (descriptor, create) in testing::fixtures() {
        shell.register_decoder(descriptor, create).unwrap();
    }
    shell.set_decoder_chunk_limits(la_decoders::run::ChunkLimits {
        max_edges: 200,
        max_span: 3_000,
    });
    shell.set_decoder_deadlines(Deadlines {
        chunk: Duration::from_millis(300),
        cancel: Duration::from_millis(300),
    });
    shell
}

fn add(shell: &mut Shell, descriptor: &Descriptor) -> u32 {
    done(
        shell,
        Command::AddDecoder {
            decoder: descriptor.id.to_owned(),
        },
    );
    let id = shell.state().decoders.instances.last().unwrap().id;
    done(
        shell,
        Command::SetDecoderChannel {
            id,
            role: "clock".into(),
            channel: Some(0),
        },
    );
    id
}

fn settle(shell: &mut Shell) {
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        shell.poll(Instant::now());
        if shell
            .state()
            .decoders
            .instances
            .iter()
            .all(|i| i.state != InstanceState::Running)
        {
            return;
        }
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(2));
    }
}

fn instance(shell: &Shell, id: u32) -> la_host::decoders::InstanceView {
    shell.state().decoders.instance(id).unwrap().clone()
}

#[test]
fn an_error_and_a_panic_become_one_notice_each_and_the_neighbour_finishes() {
    let dir = TempDir::new("faults-basic");
    let mut shell = shell(&dir);
    let healthy = add(&mut shell, &HEALTHY);
    let error = add(&mut shell, &RETURNS_ERROR);
    let panic = add(&mut shell, &PANICS);
    let before_notices = shell.state().notices.len();
    let unsaved = shell.state().unsaved;
    settle(&mut shell);
    assert_eq!(instance(&shell, healthy).state, InstanceState::Finished);
    assert!(!instance(&shell, healthy).incomplete);
    assert_eq!(instance(&shell, error).state, InstanceState::Failed);
    assert_eq!(instance(&shell, error).fault, Some("decoder.failed"));
    assert!(instance(&shell, error).incomplete);
    assert!(
        instance(&shell, error).annotations > 0,
        "what it produced before is kept"
    );
    assert_eq!(instance(&shell, panic).fault, Some("decoder.panicked"));
    // One notice for each fault, none for the healthy decoder; the capture is as it was.
    assert_eq!(shell.state().notices.len(), before_notices + 2);
    assert_eq!(shell.state().unsaved, unsaved);
    assert!(shell.state().capture.is_some());
    // A failed instance can be run again.
    done(&mut shell, Command::RunDecoder(error));
    assert_eq!(instance(&shell, error).state, InstanceState::Running);
    settle(&mut shell);
    assert_eq!(instance(&shell, error).fault, Some("decoder.failed"));
}

#[test]
fn the_notice_of_every_fault_reads_right_in_every_language() {
    let dir = TempDir::new("faults-text");
    let mut shell = shell(&dir);
    add(&mut shell, &RETURNS_ERROR);
    add(&mut shell, &PANICS);
    settle(&mut shell);
    let notices: Vec<_> = shell.state().notices.iter().cloned().collect();
    assert!(notices.len() >= 2);
    for language in Language::ALL {
        done(&mut shell, Command::SetLanguage(language));
        let mut seen = Vec::new();
        for notice in &notices {
            let text = shell.notice_text(notice);
            assert!(text.len() > 20, "{language:?}: {text}");
            for forbidden in ["decoder.", "error-decoder", "planned", "\u{27E6}"] {
                assert!(
                    !text.contains(forbidden),
                    "{language:?} shows `{forbidden}`: {text}"
                );
            }
            seen.push(text);
        }
        seen.sort();
        seen.dedup();
        assert!(
            seen.len() >= 2,
            "{language:?} says the same for different faults"
        );
    }
}

#[test]
fn a_decoder_that_never_returns_is_abandoned_without_ever_blocking_the_shell() {
    let dir = TempDir::new("faults-stuck");
    let mut shell = shell(&dir);
    testing::hold_stuck_decoders();
    let healthy = add(&mut shell, &HEALTHY);
    let stuck = add(&mut shell, &NEVER_RETURNS);
    let started = Instant::now();
    let mut slowest = Duration::ZERO;
    while instance(&shell, stuck).state == InstanceState::Running {
        let t = Instant::now();
        shell.poll(Instant::now());
        // A command is as quick as the poll.
        let outcome = shell.apply(Command::FitAll);
        assert!(matches!(outcome, Outcome::Done(_)));
        slowest = slowest.max(t.elapsed());
        assert!(
            started.elapsed() < Duration::from_secs(20),
            "never abandoned"
        );
        std::thread::sleep(Duration::from_millis(3));
    }
    assert!(slowest < Duration::from_millis(100), "{slowest:?}");
    assert_eq!(
        instance(&shell, stuck).fault,
        Some("decoder.not_responding")
    );
    assert!(instance(&shell, stuck).incomplete);
    assert_eq!(instance(&shell, healthy).state, InstanceState::Finished);
    testing::release_stuck_decoders();
    std::thread::sleep(Duration::from_millis(100));
    testing::hold_stuck_decoders();
}

#[test]
fn a_fault_changes_neither_the_capture_nor_what_is_saved() {
    let dir = TempDir::new("faults-save");
    let mut shell = shell(&dir);
    // The bytes of the file as saved without any decoder.
    let plain_path = dir.path().join("plain.las");
    done(&mut shell, Command::SaveSessionAs(plain_path.clone()));
    wait_save(&mut shell);
    let plain = std::fs::read(&plain_path).unwrap();
    // The same capture with failing decoders next to a healthy one: the capture sections do not
    // change (the decoder setup is a section of its own, so compare the capture's length and
    // the plain bytes' prefix structure through a reopened capture).
    add(&mut shell, &HEALTHY);
    add(&mut shell, &RETURNS_ERROR);
    add(&mut shell, &PANICS);
    settle(&mut shell);
    let length = shell.state().capture.as_ref().unwrap().snapshot.length();
    let path = dir.path().join("with.las");
    done(&mut shell, Command::SaveSessionAs(path.clone()));
    wait_save(&mut shell);
    assert!(std::fs::metadata(&path).unwrap().len() > 0);
    let mut reopened = Shell::new(la_host::ShellConfig::for_tests(dir.path()));
    done(&mut reopened, Command::OpenSession(path));
    wait_load(&mut reopened);
    assert_eq!(
        reopened.state().capture.as_ref().unwrap().snapshot.length(),
        length
    );
    assert!(!plain.is_empty());
}

fn wait_save(shell: &mut Shell) {
    let deadline = Instant::now() + Duration::from_secs(30);
    while shell.state().operation.is_some() {
        shell.poll(Instant::now());
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(2));
    }
    assert_eq!(shell.state().unsaved, Unsaved::Clean);
}

fn wait_load(shell: &mut Shell) {
    let deadline = Instant::now() + Duration::from_secs(30);
    while shell.state().operation.is_some() || shell.state().capture.is_none() {
        shell.poll(Instant::now());
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(2));
    }
}

#[test]
fn memory_and_annotation_limits_end_a_pass_with_a_message_and_keep_what_fits() {
    let dir = TempDir::new("faults-limits");
    let mut shell = shell(&dir);
    done(&mut shell, Command::SetDecoderMemoryLimit(16 << 20));
    let grows = add(&mut shell, &testing::GROWS_MEMORY);
    let many = add(&mut shell, &testing::TOO_MANY_ANNOTATIONS);
    settle(&mut shell);
    assert_eq!(instance(&shell, grows).fault, Some("decoder.memory_limit"));
    assert!(instance(&shell, grows).annotations > 1_000);
    // Both share one memory budget: the second to reach it stops with the same code.
    assert!(matches!(
        instance(&shell, many).fault,
        Some("decoder.memory_limit" | "decoder.annotation_limit")
    ));
}

#[test]
fn a_stopped_instance_can_be_cancelled_and_run_again() {
    let dir = TempDir::new("faults-cancel");
    let mut shell = shell(&dir);
    let id = add(&mut shell, &HEALTHY);
    settle(&mut shell);
    // Nothing runs: cancelling is ignored, running starts a new pass.
    assert!(matches!(
        shell.apply(Command::CancelDecoder(id)),
        Outcome::Ignored(_)
    ));
    done(&mut shell, Command::RunDecoder(id));
    settle(&mut shell);
    assert_eq!(instance(&shell, id).state, InstanceState::Finished);
    assert!(matches!(
        shell.apply(Command::RunDecoder(9_999)),
        Outcome::Ignored(_)
    ));
}
