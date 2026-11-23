//! Stacked decoders in the host (spec US4): the source of an instance, removal with dependants,
//! lane order and propagation.
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
use la_decoders::error;
use la_decoders::run::ChunkLimits;
use la_decoders::setup::Source;
use la_decoders::testing::{self, LOWER_BYTES, UPPER_WORDS};
use la_host::decoders::InstanceState;
use la_host::window::ViewSize;
use la_host::{Activity, Command, Decision, Guarded, Outcome, Shell};

fn done(shell: &mut Shell, command: Command) {
    let outcome = shell.apply(command);
    assert!(matches!(outcome, Outcome::Done(_)), "{outcome:?}");
}

fn settle(shell: &mut Shell) {
    let deadline = Instant::now() + Duration::from_secs(30);
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

/// A capture, with the stacking fixtures registered; the upper instance is added first.
fn shell(dir: &TempDir) -> (Shell, u32, u32) {
    let mut shell = finished_shell(dir, 0b11, 60_000, 6);
    for (descriptor, create) in testing::fixtures() {
        shell.register_decoder(descriptor, create).unwrap();
    }
    shell.set_decoder_thread_cap(4);
    shell.set_decoder_chunk_limits(ChunkLimits {
        max_edges: 300,
        max_span: 4_000,
    });
    done(
        &mut shell,
        Command::AddDecoder {
            decoder: UPPER_WORDS.id.into(),
        },
    );
    let upper = shell.state().decoders.instances[0].id;
    done(
        &mut shell,
        Command::AddDecoder {
            decoder: LOWER_BYTES.id.into(),
        },
    );
    let lower = shell.state().decoders.instances[1].id;
    done(
        &mut shell,
        Command::SetDecoderChannel {
            id: lower,
            role: "clock".into(),
            channel: Some(0),
        },
    );
    done(
        &mut shell,
        Command::StackDecoder {
            id: upper,
            source: Source::Instance(lower),
        },
    );
    settle(&mut shell);
    (shell, lower, upper)
}

fn view(shell: &Shell, id: u32) -> la_host::decoders::InstanceView {
    shell.state().decoders.instance(id).unwrap().clone()
}

#[test]
fn an_upper_instance_reads_the_one_below_and_both_finish() {
    let dir = TempDir::new("stack-basic");
    let (shell, lower, upper) = shell(&dir);
    assert_eq!(view(&shell, lower).state, InstanceState::Finished);
    assert_eq!(view(&shell, upper).state, InstanceState::Finished);
    assert!(!view(&shell, upper).incomplete);
    assert!(view(&shell, upper).annotations > 0);
    assert_eq!(view(&shell, upper).setup.source, Source::Instance(lower));
}

#[test]
fn a_refused_source_names_the_cause_and_changes_nothing() {
    let dir = TempDir::new("stack-refuse");
    let (mut shell, lower, upper) = shell(&dir);
    let before = view(&shell, lower);
    // The lower decoder reads signals, not another instance.
    done(
        &mut shell,
        Command::StackDecoder {
            id: lower,
            source: Source::Instance(upper),
        },
    );
    assert_eq!(
        shell.state().decoders.refusal.clone().unwrap().code,
        error::STACK_INCOMPATIBLE
    );
    assert_eq!(view(&shell, lower), before);
    // The upper decoder reads items, not the capture.
    done(
        &mut shell,
        Command::StackDecoder {
            id: upper,
            source: Source::Capture,
        },
    );
    assert_eq!(
        shell.state().decoders.refusal.clone().unwrap().code,
        error::STACK_INCOMPATIBLE
    );
    assert_eq!(view(&shell, upper).setup.source, Source::Instance(lower));
    // An instance cannot read itself.
    done(
        &mut shell,
        Command::StackDecoder {
            id: upper,
            source: Source::Instance(upper),
        },
    );
    assert_eq!(
        shell.state().decoders.refusal.clone().unwrap().code,
        error::STACK_CYCLE
    );
}

#[test]
fn the_possible_sources_are_the_ones_that_would_be_accepted() {
    let dir = TempDir::new("stack-sources");
    let (shell, lower, upper) = shell(&dir);
    assert_eq!(shell.decoder_sources(upper), vec![Source::Instance(lower)]);
    assert_eq!(shell.decoder_sources(lower), vec![Source::Capture]);
}

#[test]
fn removing_an_instance_with_dependants_asks_first() {
    let dir = TempDir::new("stack-remove");
    let (mut shell, lower, upper) = shell(&dir);
    let outcome = shell.apply(Command::RemoveDecoder(lower));
    assert_eq!(
        outcome,
        Outcome::NeedsDecision(Guarded::RemoveDecoder(lower))
    );
    // Saving makes no sense for this question; cancelling keeps everything.
    assert!(matches!(
        shell.apply(Command::Decide(Decision::Save)),
        Outcome::Ignored(_)
    ));
    done(&mut shell, Command::Decide(Decision::Cancel));
    assert_eq!(shell.state().decoders.instances.len(), 2);
    // Discarding removes it and everything above it; the capture is untouched and idle.
    let outcome = shell.apply(Command::RemoveDecoder(lower));
    assert!(matches!(outcome, Outcome::NeedsDecision(_)));
    done(&mut shell, Command::Decide(Decision::Discard));
    assert!(shell.state().decoders.instances.is_empty());
    assert!(shell.decoder_results(lower).is_none() && shell.decoder_results(upper).is_none());
    assert!(shell.state().capture.is_some());
    assert_eq!(shell.state().activity, Activity::Idle);
    assert!(shell.state().pending_decision.is_none());
}

#[test]
fn removing_the_top_instance_needs_no_question() {
    let dir = TempDir::new("stack-remove-top");
    let (mut shell, lower, upper) = shell(&dir);
    done(&mut shell, Command::RemoveDecoder(upper));
    assert_eq!(shell.state().decoders.instances.len(), 1);
    assert_eq!(view(&shell, lower).state, InstanceState::Finished);
}

#[test]
fn the_lanes_come_in_stack_order() {
    let dir = TempDir::new("stack-lanes");
    let (mut shell, lower, upper) = shell(&dir);
    // The upper instance was created first, but its lane comes after the one it reads from.
    assert_eq!(shell.state().decoders.instances[0].id, upper);
    let plan = shell.frame_plan(ViewSize {
        pixels: 800,
        scale: 1.0,
    });
    let order: Vec<u32> = plan.lanes.iter().map(|l| l.instance).collect();
    assert_eq!(order, vec![lower, upper]);
    done(&mut shell, Command::FitAll);
}

#[test]
fn running_the_lower_instance_again_makes_the_upper_one_outdated_until_new_results_arrive() {
    let dir = TempDir::new("stack-outdated");
    let (mut shell, lower, upper) = shell(&dir);
    let before = shell.decoder_results(upper).unwrap().lane.len();
    done(&mut shell, Command::RunDecoder(lower));
    assert!(
        view(&shell, upper).outdated,
        "it shows old results, flagged"
    );
    assert_eq!(shell.decoder_results(upper).unwrap().lane.len(), before);
    settle(&mut shell);
    assert!(!view(&shell, upper).outdated);
    assert_eq!(view(&shell, upper).state, InstanceState::Finished);
}

#[test]
fn a_fault_below_marks_the_upper_instance_incomplete_without_failing_it() {
    let dir = TempDir::new("stack-fault");
    let (mut shell, lower, upper) = shell(&dir);
    // Cancel the lower pass as it starts again: what it made is partial.
    done(&mut shell, Command::RunDecoder(lower));
    done(&mut shell, Command::CancelDecoder(lower));
    settle(&mut shell);
    let above = view(&shell, upper);
    assert_ne!(above.state, InstanceState::Failed);
    assert!(above.fault.is_none());
    if view(&shell, lower).state == InstanceState::Cancelled {
        assert!(above.incomplete);
    }
}

#[test]
fn a_source_that_does_not_exist_is_refused_even_while_the_setup_is_incomplete() {
    // Found by the random walk: an instance still missing its required channels was given a
    // source that is not there, because the missing channel was reported first.
    let dir = TempDir::new("stack-missing-source");
    let (mut shell, _lower, _upper) = shell(&dir);
    done(
        &mut shell,
        Command::AddDecoder {
            decoder: UPPER_WORDS.id.into(),
        },
    );
    done(
        &mut shell,
        Command::AddDecoder {
            decoder: LOWER_BYTES.id.into(),
        },
    );
    let incomplete = shell.state().decoders.instances.last().unwrap().id;
    assert!(!view(&shell, incomplete).ready);
    done(
        &mut shell,
        Command::StackDecoder {
            id: incomplete,
            source: Source::Instance(999),
        },
    );
    assert_eq!(view(&shell, incomplete).setup.source, Source::Capture);
    assert!(shell.state().decoders.refusal.is_some());
}

#[test]
fn an_upper_instance_on_an_incomplete_lower_one_waits_and_runs_when_the_lower_one_is_complete() {
    // Found by the random walk: such an instance is `Waiting` and `ready` (its own setup is complete) and
    // nothing runs it, and that is right: it has nothing to read until the instance below starts. Whoever
    // waits for the decoders to settle must not take it for one that is about to start.
    let dir = TempDir::new("stack-waits-for-lower");
    let (mut shell, _lower, _upper) = shell(&dir);
    for decoder in [LOWER_BYTES.id, UPPER_WORDS.id] {
        done(
            &mut shell,
            Command::AddDecoder {
                decoder: decoder.into(),
            },
        );
    }
    let ids: Vec<u32> = shell
        .state()
        .decoders
        .instances
        .iter()
        .map(|i| i.id)
        .collect();
    let (new_lower, new_upper) = (ids[ids.len() - 2], ids[ids.len() - 1]);
    assert!(
        !view(&shell, new_lower).ready,
        "the lower one lacks its clock"
    );
    done(
        &mut shell,
        Command::StackDecoder {
            id: new_upper,
            source: Source::Instance(new_lower),
        },
    );
    for _ in 0..50 {
        shell.poll(Instant::now());
        std::thread::sleep(Duration::from_millis(2));
    }
    let waiting = view(&shell, new_upper);
    assert_eq!(waiting.state, InstanceState::Waiting);
    assert!(waiting.ready, "its own setup is complete");
    assert_eq!(view(&shell, new_lower).state, InstanceState::Waiting);
    // The lower one gets its clock: both run and finish.
    done(
        &mut shell,
        Command::SetDecoderChannel {
            id: new_lower,
            role: "clock".into(),
            channel: Some(0),
        },
    );
    settle(&mut shell);
    assert_eq!(view(&shell, new_lower).state, InstanceState::Finished);
    assert_eq!(view(&shell, new_upper).state, InstanceState::Finished);
}
