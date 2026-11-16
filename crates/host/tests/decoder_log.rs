//! What the log says about a decoder fault (spec FR-031, constitution Principle VI).
//!
//! This is a test file of its own: the log of a test is captured with a thread-local dispatcher, and
//! tracing caches whether a call site is enabled the first time it is reached, so a test that
//! reads the log must not share a process with tests that reach the same call sites without one.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::panic
)]

mod common;

use std::time::{Duration, Instant};

use common::{TempDir, finished_shell, read_logs};
use la_decoders::run::{ChunkLimits, Deadlines};
use la_decoders::testing::{self, RETURNS_ERROR};
use la_diagnostics::{LogConfig, build_logging};
use la_host::decoders::InstanceState;
use la_host::{Command, Outcome};

#[test]
fn the_log_names_the_code_and_the_decoder_but_not_the_instance_name_or_annotation_text() {
    let log_dir = TempDir::new("faults-log");
    let (guard, dispatch) = build_logging(LogConfig {
        directory: log_dir.path().to_path_buf(),
        level: tracing::Level::DEBUG,
        ..LogConfig::default()
    });
    tracing::dispatcher::with_default(&dispatch, || {
        let dir = TempDir::new("faults-log-shell");
        let mut shell = finished_shell(&dir, 0b11, 60_000, 5);
        for (descriptor, create) in testing::fixtures() {
            shell.register_decoder(descriptor, create).unwrap();
        }
        shell.set_decoder_chunk_limits(ChunkLimits {
            max_edges: 200,
            max_span: 3_000,
        });
        shell.set_decoder_deadlines(Deadlines::default());
        let outcome = shell.apply(Command::AddDecoder {
            decoder: RETURNS_ERROR.id.into(),
        });
        assert!(matches!(outcome, Outcome::Done(_)));
        let id = shell.state().decoders.instances[0].id;
        shell.apply(Command::SetDecoderChannel {
            id,
            role: "clock".into(),
            channel: Some(0),
        });
        shell.apply(Command::RenameDecoder {
            id,
            name: "INSTANCE-NAME-4711".into(),
        });
        let deadline = Instant::now() + Duration::from_secs(30);
        while shell.state().decoders.instances[0].state == InstanceState::Running {
            shell.poll(Instant::now());
            assert!(Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(2));
        }
    });
    drop(guard);
    let logs = read_logs(log_dir.path());
    assert!(
        logs.contains("decoder.failed"),
        "the code is logged: {logs}"
    );
    assert!(
        logs.contains("test.returns_error"),
        "the decoder id is logged"
    );
    assert!(
        !logs.contains("INSTANCE-NAME-4711"),
        "an instance name was logged"
    );
}
