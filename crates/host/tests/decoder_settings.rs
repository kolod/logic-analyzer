//! The memory limit of decoder results (spec FR-011, US2).
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::panic
)]

mod common;

use std::time::{Duration, Instant};

use common::TempDir;
use la_host::{Command, Outcome, Shell, ShellConfig};

const MIB: u64 = 1 << 20;

fn done(shell: &mut Shell, command: Command) {
    let outcome = shell.apply(command);
    assert!(matches!(outcome, Outcome::Done(_)), "{outcome:?}");
}

#[test]
fn the_limit_is_validated_from_16_mib_to_4_gib() {
    let dir = TempDir::new("dlimit-valid");
    let mut shell = Shell::new(ShellConfig::for_tests(dir.path()));
    assert_eq!(shell.state().settings.decoder_memory_limit_bytes, 256 * MIB);
    for bad in [0, 15 * MIB, (4 << 30) + 1, u64::MAX] {
        assert!(
            matches!(
                shell.apply(Command::SetDecoderMemoryLimit(bad)),
                Outcome::Ignored(_)
            ),
            "{bad}"
        );
        assert_eq!(shell.state().settings.decoder_memory_limit_bytes, 256 * MIB);
    }
    for good in [16 * MIB, 1 << 30, 4 << 30] {
        done(&mut shell, Command::SetDecoderMemoryLimit(good));
        assert_eq!(shell.state().settings.decoder_memory_limit_bytes, good);
        assert_eq!(shell.state().decoders.limits.memory_bytes, good);
    }
}

#[test]
fn the_limit_is_remembered_and_a_damaged_value_falls_back_to_the_default() {
    let dir = TempDir::new("dlimit-remember");
    {
        let mut shell = Shell::new(ShellConfig::for_tests(dir.path()));
        done(&mut shell, Command::SetDecoderMemoryLimit(64 * MIB));
        shell.save_settings();
    }
    let shell = Shell::new(ShellConfig::for_tests(dir.path()));
    assert_eq!(shell.state().settings.decoder_memory_limit_bytes, 64 * MIB);
    // A value outside the range in the file is replaced by the default.
    let path = dir.path().join("settings.json");
    let text = std::fs::read_to_string(&path).unwrap();
    let damaged = text.replace(&(64 * MIB).to_string(), "5");
    assert_ne!(text, damaged);
    std::fs::write(&path, damaged).unwrap();
    let shell = Shell::new(ShellConfig::for_tests(dir.path()));
    assert_eq!(shell.state().settings.decoder_memory_limit_bytes, 256 * MIB);
}

#[test]
fn a_new_limit_applies_to_the_next_pass_and_never_to_a_running_one() {
    use la_decoders::testing::{self, GROWS_MEMORY, HEALTHY};
    let dir = TempDir::new("dlimit-running");
    let mut shell = common::finished_shell(&dir, 0b11, 60_000, 3);
    for (descriptor, create) in testing::fixtures() {
        shell.register_decoder(descriptor, create).unwrap();
    }
    shell.set_decoder_chunk_limits(la_decoders::run::ChunkLimits {
        max_edges: 200,
        max_span: 3_000,
    });
    // A healthy pass that is still running when the limit changes keeps going.
    done(
        &mut shell,
        Command::AddDecoder {
            decoder: HEALTHY.id.into(),
        },
    );
    let id = shell.state().decoders.instances[0].id;
    done(
        &mut shell,
        Command::SetDecoderChannel {
            id,
            role: "clock".into(),
            channel: Some(0),
        },
    );
    done(&mut shell, Command::SetDecoderMemoryLimit(16 * MIB));
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        shell.poll(Instant::now());
        if shell.state().decoders.instances[0].state != la_host::decoders::InstanceState::Running {
            break;
        }
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(2));
    }
    assert_eq!(shell.state().decoders.instances[0].fault, None);
    // The next pass uses the new, small limit and runs into it.
    done(
        &mut shell,
        Command::AddDecoder {
            decoder: GROWS_MEMORY.id.into(),
        },
    );
    let grows = shell.state().decoders.instances[1].id;
    done(
        &mut shell,
        Command::SetDecoderChannel {
            id: grows,
            role: "clock".into(),
            channel: Some(0),
        },
    );
    let deadline = Instant::now() + Duration::from_secs(30);
    while shell.state().decoders.instances[1].state == la_host::decoders::InstanceState::Running {
        shell.poll(Instant::now());
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(2));
    }
    assert_eq!(
        shell.state().decoders.instances[1].fault,
        Some("decoder.memory_limit")
    );
}
