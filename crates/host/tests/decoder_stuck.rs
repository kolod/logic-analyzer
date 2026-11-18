//! A pass refused because too many workers are stuck must leave a coherent instance.
//!
//! This test is alone in its file on purpose: the number of stuck workers is one counter for the
//! whole process, and another test that abandons a worker at the same time would disturb it.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::panic
)]

mod common;

use std::time::{Duration, Instant};

use common::{TempDir, finished_shell, wait_operation};
use la_decoders::run::{ChunkLimits, Deadlines, MAX_STUCK_WORKERS, stuck_workers};
use la_decoders::testing::{self, HEALTHY, NEVER_RETURNS};
use la_host::decoders::InstanceState;
use la_host::{Command, Outcome, Shell, ShellConfig};

fn done(shell: &mut Shell, command: Command) {
    let outcome = shell.apply(command);
    assert!(matches!(outcome, Outcome::Done(_)), "{outcome:?}");
}

fn prepare(shell: &mut Shell) {
    for (descriptor, create) in testing::fixtures() {
        shell.register_decoder(descriptor, create).unwrap();
    }
    shell.set_decoder_thread_cap(8);
    shell.set_decoder_chunk_limits(ChunkLimits {
        max_edges: 200,
        max_span: 3_000,
    });
    shell.set_decoder_deadlines(Deadlines {
        chunk: Duration::from_millis(300),
        cancel: Duration::from_millis(300),
    });
}

fn add(shell: &mut Shell, decoder: &str) -> u32 {
    done(
        shell,
        Command::AddDecoder {
            decoder: decoder.to_owned(),
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

fn wait_until(shell: &mut Shell, what: &str, mut condition: impl FnMut(&Shell) -> bool) {
    let deadline = Instant::now() + Duration::from_secs(30);
    while !condition(shell) {
        shell.poll(Instant::now());
        assert!(Instant::now() < deadline, "timed out: {what}");
        std::thread::sleep(Duration::from_millis(3));
    }
}

#[test]
fn a_pass_refused_for_stuck_workers_does_not_leave_the_instance_flagged_loaded() {
    let dir = TempDir::new("stuck-loaded");
    testing::hold_stuck_decoders();

    // A session with the results of one finished pass.
    let mut shell = finished_shell(&dir, 0b11, 60_000, 5);
    prepare(&mut shell);
    let healthy = add(&mut shell, HEALTHY.id);
    wait_until(&mut shell, "the first pass", |s| {
        s.state().decoders.instance(healthy).unwrap().state == InstanceState::Finished
    });
    done(&mut shell, Command::SetSaveDecoderResults(true));
    let path = dir.path().join("loaded.lasession");
    done(&mut shell, Command::SaveSessionAs(path.clone()));
    wait_operation(&mut shell);

    // Opened again, its results are loaded from the file, not decoded.
    let mut shell = Shell::new(ShellConfig::for_tests(dir.path()));
    prepare(&mut shell);
    done(&mut shell, Command::OpenSession(path));
    wait_operation(&mut shell);
    wait_until(&mut shell, "the saved results", |s| {
        s.state().decoders.instance(healthy).unwrap().loaded
    });

    // Workers that never return are abandoned until the process refuses new passes.
    let base = stuck_workers();
    for _ in 0..MAX_STUCK_WORKERS {
        add(&mut shell, NEVER_RETURNS.id);
    }
    wait_until(&mut shell, "the workers to be abandoned", |_| {
        stuck_workers() >= base + MAX_STUCK_WORKERS
    });

    // A new pass is refused; what the instance shows must agree with that.
    done(&mut shell, Command::RunDecoder(healthy));
    let view = shell.state().decoders.instance(healthy).unwrap().clone();
    assert_eq!(view.state, InstanceState::Failed, "{view:?}");
    assert_eq!(view.fault, Some("decoder.too_many_stuck"), "{view:?}");
    assert!(!view.loaded, "results of the file are not shown: {view:?}");
    assert!(!view.outdated, "{view:?}");

    testing::release_stuck_decoders();
    wait_until(&mut shell, "the stuck workers to end", |_| {
        stuck_workers() <= base
    });
    testing::hold_stuck_decoders();
}
