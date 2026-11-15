//! Saving and opening a capture with its decoders (spec US5, SC-008).
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::panic
)]

mod common;

use std::io::ErrorKind;
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

use common::{TempDir, finished_shell, wait_operation};
use la_decoders::decoders::clocked_bits::ID;
use la_decoders::persist::deco;
use la_decoders::run::ChunkLimits;
use la_decoders::setup::{Setup, Source, Value};
use la_decoders::testing::{self, LOWER_BYTES, UPPER_WORDS};
use la_host::decoders::InstanceState;
use la_host::notices::NoticeContent;
use la_host::{Command, Outcome, Shell, ShellConfig, Tick, Unsaved};
use la_session::harness::{FaultFs, FaultPlan, Op};
use la_session::{Control, LoadLimits, Section, SectionTag, SessionData};

fn done(shell: &mut Shell, command: Command) {
    let outcome = shell.apply(command);
    assert!(matches!(outcome, Outcome::Done(_)), "{outcome:?}");
}

fn prepare(shell: &mut Shell) {
    for (descriptor, create) in testing::fixtures() {
        shell.register_decoder(descriptor, create).unwrap();
    }
    shell.set_decoder_chunk_limits(ChunkLimits {
        max_edges: 300,
        max_span: 4_000,
    });
}

fn settle(shell: &mut Shell) {
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        shell.poll(Instant::now());
        let busy = shell.state().decoders.instances.iter().any(|i| {
            i.state == InstanceState::Running || (i.state == InstanceState::Waiting && i.ready)
        });
        if !busy && shell.state().operation.is_none() {
            return;
        }
        assert!(Instant::now() < deadline, "did not settle");
        std::thread::sleep(Duration::from_millis(2));
    }
}

/// A capture with a stack (`lower_bytes` below `upper_words`) and the sample decoder named "Bus".
fn shell_with_decoders(dir: &TempDir) -> Shell {
    let mut shell = finished_shell(dir, 0b11, 60_000, 4);
    prepare(&mut shell);
    done(
        &mut shell,
        Command::AddDecoder {
            decoder: LOWER_BYTES.id.into(),
        },
    );
    let lower = shell.state().decoders.instances[0].id;
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
        Command::AddDecoder {
            decoder: UPPER_WORDS.id.into(),
        },
    );
    let upper = shell.state().decoders.instances[1].id;
    done(
        &mut shell,
        Command::StackDecoder {
            id: upper,
            source: Source::Instance(lower),
        },
    );
    done(&mut shell, Command::AddDecoder { decoder: ID.into() });
    let sample = shell.state().decoders.instances[2].id;
    for (role, channel) in [("clock", 0), ("data", 1)] {
        done(
            &mut shell,
            Command::SetDecoderChannel {
                id: sample,
                role: role.into(),
                channel: Some(channel),
            },
        );
    }
    done(
        &mut shell,
        Command::SetDecoderOption {
            id: sample,
            option: "bits".into(),
            value: Value::Integer(12),
        },
    );
    done(
        &mut shell,
        Command::RenameDecoder {
            id: sample,
            name: "Bus".into(),
        },
    );
    done(
        &mut shell,
        Command::ShowDecoderLane {
            id: sample,
            visible: false,
        },
    );
    settle(&mut shell);
    shell
}

fn save(shell: &mut Shell, path: &Path) {
    done(shell, Command::SaveSessionAs(path.to_path_buf()));
    wait_operation(shell);
}

fn open(dir: &TempDir, path: &Path) -> Shell {
    let mut shell = Shell::new(ShellConfig::for_tests(dir.path()));
    prepare(&mut shell);
    done(&mut shell, Command::OpenSession(path.to_path_buf()));
    wait_operation(&mut shell);
    shell
}

fn setups(shell: &Shell) -> Vec<Setup> {
    shell
        .state()
        .decoders
        .instances
        .iter()
        .map(|i| i.setup.clone())
        .collect()
}

fn codes(shell: &Shell) -> Vec<String> {
    shell
        .state()
        .notices
        .iter()
        .map(|n| match &n.content {
            NoticeContent::Error { code } => code.clone(),
            NoticeContent::Info { key, .. } => (*key).to_owned(),
        })
        .collect()
}

fn sections(path: &Path) -> Vec<(String, Vec<u8>)> {
    let session = la_session::load_blocking(path, &LoadLimits::default(), &Control::new()).unwrap();
    session
        .extras()
        .iter()
        .map(|s| (s.tag.as_str().to_owned(), s.payload.clone()))
        .collect()
}

/// Writes a file like `source` but with these decoder sections instead of its own.
fn rewrite(source: &Path, target: &Path, extra: Vec<(&str, Vec<u8>)>) {
    let session =
        la_session::load_blocking(source, &LoadLimits::default(), &Control::new()).unwrap();
    let mut data = SessionData::new(session.description().clone(), session.snapshot());
    for section in session.extras() {
        if section.tag.as_str() != "DECO" && section.tag.as_str() != "DANN" {
            data = data.with_section(section.clone()).unwrap();
        }
    }
    for (tag, payload) in extra {
        data = data
            .with_section(Section {
                tag: SectionTag::new(tag).unwrap(),
                critical: false,
                payload,
            })
            .unwrap();
    }
    la_session::save_blocking(target, &data, &Control::new()).unwrap();
}

#[test]
fn the_setup_is_restored_and_the_passes_start_again() {
    let dir = TempDir::new("sess-setup");
    let mut shell = shell_with_decoders(&dir);
    let before = setups(&shell);
    let path = dir.path().join("a.lasession");
    save(&mut shell, &path);
    let mut reopened = open(&dir, &path);
    // Same decoders, channels, options, stack and lane visibility, in the same order.
    assert_eq!(setups(&reopened), before);
    assert!(
        reopened
            .state()
            .decoders
            .instances
            .iter()
            .all(|i| !i.loaded)
    );
    settle(&mut reopened);
    assert!(
        reopened
            .state()
            .decoders
            .instances
            .iter()
            .all(|i| i.state == InstanceState::Finished)
    );
    for i in &reopened.state().decoders.instances {
        // `lower_bytes` only makes items, no annotations.
        assert!(
            i.annotations > 0 || i.setup.decoder == "test.lower_bytes",
            "{i:?}"
        );
    }
    // The file has the setup and no results.
    let tags: Vec<String> = sections(&path).into_iter().map(|(t, _)| t).collect();
    assert!(
        tags.contains(&"DECO".to_owned()) && !tags.contains(&"DANN".to_owned()),
        "{tags:?}"
    );
    // The next ids go on after the saved ones.
    done(&mut reopened, Command::AddDecoder { decoder: ID.into() });
    let ids: Vec<u32> = reopened
        .state()
        .decoders
        .instances
        .iter()
        .map(|i| i.id)
        .collect();
    assert_eq!(ids.len(), 4);
    assert!(ids[3] > *ids[..3].iter().max().unwrap());
}

#[test]
fn saved_results_are_shown_at_once_flagged_loaded_until_a_new_pass_replaces_them() {
    let dir = TempDir::new("sess-results");
    let mut shell = shell_with_decoders(&dir);
    done(&mut shell, Command::SetSaveDecoderResults(true));
    let path = dir.path().join("b.lasession");
    save(&mut shell, &path);
    let ids: Vec<u32> = shell
        .state()
        .decoders
        .instances
        .iter()
        .map(|i| i.id)
        .collect();
    let saved: Vec<(usize, Vec<_>)> = ids
        .iter()
        .map(|id| {
            let r = shell.decoder_results(*id).unwrap();
            (
                r.lane.len(),
                r.lane
                    .window(0, u64::MAX)
                    .iter()
                    .map(|e| e.annotation.clone())
                    .collect(),
            )
        })
        .collect();
    let mut reopened = open(&dir, &path);
    assert_eq!(reopened.wants_ticks(), Tick::Slow, "no pass starts");
    for (id, (len, annotations)) in ids.iter().zip(&saved) {
        let view = reopened.state().decoders.instance(*id).unwrap().clone();
        assert!(
            view.loaded && view.state == InstanceState::Finished,
            "{view:?}"
        );
        let results = reopened.decoder_results(*id).unwrap();
        assert_eq!(results.lane.len(), *len);
        let again: Vec<_> = results
            .lane
            .window(0, u64::MAX)
            .iter()
            .map(|e| e.annotation.clone())
            .collect();
        assert_eq!(&again, annotations, "identical to what was saved");
    }
    // Saved again, the results are the same and the file is byte for byte the same.
    let again_path = dir.path().join("c.lasession");
    done(&mut reopened, Command::SetSaveDecoderResults(true));
    save(&mut reopened, &again_path);
    let a = sections(&path);
    let b = sections(&again_path);
    assert_eq!(
        a.iter().find(|(t, _)| t == "DECO"),
        b.iter().find(|(t, _)| t == "DECO")
    );
    // A new pass replaces loaded results.
    let first = ids[2];
    done(&mut reopened, Command::RunDecoder(first));
    settle(&mut reopened);
    assert!(!reopened.state().decoders.instance(first).unwrap().loaded);
}

#[test]
fn a_file_without_decoder_sections_opens_as_it_did() {
    let dir = TempDir::new("sess-old");
    let mut shell = finished_shell(&dir, 0b11, 20_000, 3);
    let path = dir.path().join("plain.lasession");
    save(&mut shell, &path);
    let reopened = open(&dir, &path);
    assert!(reopened.state().decoders.instances.is_empty());
    assert!(reopened.state().capture.is_some());
    assert!(codes(&reopened).is_empty());
}

#[test]
fn a_damaged_deco_opens_with_a_warning_and_the_capture() {
    let dir = TempDir::new("sess-bad-deco");
    let mut shell = shell_with_decoders(&dir);
    let path = dir.path().join("a.lasession");
    save(&mut shell, &path);
    let damaged = dir.path().join("damaged.lasession");
    rewrite(&path, &damaged, vec![("DECO", vec![1, 200, 7, 7])]);
    let reopened = open(&dir, &damaged);
    assert!(reopened.state().capture.is_some());
    assert!(reopened.state().decoders.instances.is_empty());
    assert!(codes(&reopened).contains(&"decoder.section_invalid".to_owned()));
    // The invalid section is not written back.
    let mut reopened = reopened;
    let again = dir.path().join("again.lasession");
    save(&mut reopened, &again);
    assert!(sections(&again).iter().all(|(t, _)| t != "DECO"));
}

#[test]
fn a_damaged_dann_keeps_the_setup_and_the_decoders_run_again() {
    let dir = TempDir::new("sess-bad-dann");
    let mut shell = shell_with_decoders(&dir);
    done(&mut shell, Command::SetSaveDecoderResults(true));
    let path = dir.path().join("a.lasession");
    save(&mut shell, &path);
    let deco_payload = sections(&path)
        .into_iter()
        .find(|(t, _)| t == "DECO")
        .unwrap()
        .1;
    let damaged = dir.path().join("damaged.lasession");
    rewrite(
        &path,
        &damaged,
        vec![("DECO", deco_payload), ("DANN", vec![1, 0, 3, 9, 9, 9])],
    );
    let mut reopened = open(&dir, &damaged);
    assert_eq!(setups(&reopened).len(), 3);
    assert!(codes(&reopened).contains(&"decoder.section_invalid".to_owned()));
    settle(&mut reopened);
    assert!(
        reopened
            .state()
            .decoders
            .instances
            .iter()
            .all(|i| i.state == InstanceState::Finished && !i.loaded)
    );
}

#[test]
fn an_unknown_decoders_record_survives_a_save_byte_for_byte() {
    let dir = TempDir::new("sess-unknown");
    let mut shell = shell_with_decoders(&dir);
    let path = dir.path().join("a.lasession");
    save(&mut shell, &path);
    // A record of a decoder that does not exist, between the two others.
    let mut future = Setup::new(40, "vendor.future");
    future.channels.insert("clock".into(), 1);
    future
        .options
        .insert("mode".into(), Value::Choice("fast".into()));
    let unknown = deco::encode_instance(&future, 7);
    let session =
        la_session::load_blocking(&path, &LoadLimits::default(), &Control::new()).unwrap();
    let original = session
        .extras()
        .iter()
        .find(|s| s.tag.as_str() == "DECO")
        .unwrap()
        .payload
        .clone();
    // Rebuild the payload with the unknown record in the middle.
    let records: Vec<Vec<u8>> = {
        let mut r = Reader::new(&original);
        r.records()
    };
    let mixed: Vec<Vec<u8>> = vec![
        records[0].clone(),
        unknown.clone(),
        records[1].clone(),
        records[2].clone(),
    ];
    let with_unknown = dir.path().join("future.lasession");
    rewrite(&path, &with_unknown, vec![("DECO", deco::assemble(&mixed))]);
    let mut reopened = open(&dir, &with_unknown);
    assert_eq!(
        setups(&reopened).len(),
        3,
        "the unknown decoder is not created"
    );
    assert!(codes(&reopened).contains(&"decoder.unknown_decoder".to_owned()));
    let saved = dir.path().join("saved.lasession");
    save(&mut reopened, &saved);
    let payload = sections(&saved)
        .into_iter()
        .find(|(t, _)| t == "DECO")
        .unwrap()
        .1;
    assert_eq!(
        payload,
        deco::assemble(&mixed),
        "byte for byte, in its place"
    );
}

/// A reader of the framing of a `DECO` payload, written for this test.
struct Reader<'a> {
    bytes: &'a [u8],
}

impl<'a> Reader<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes }
    }

    fn varint(&mut self) -> usize {
        let mut value = 0_usize;
        let mut shift = 0;
        loop {
            let byte = self.bytes[0];
            self.bytes = &self.bytes[1..];
            value |= usize::from(byte & 0x7f) << shift;
            if byte & 0x80 == 0 {
                return value;
            }
            shift += 7;
        }
    }

    fn records(&mut self) -> Vec<Vec<u8>> {
        self.bytes = &self.bytes[1..];
        let count = self.varint();
        (0..count)
            .map(|_| {
                let length = self.varint();
                let record = self.bytes[..length].to_vec();
                self.bytes = &self.bytes[length..];
                record
            })
            .collect()
    }
}

#[test]
fn a_newer_version_gives_an_information_notice_and_is_kept() {
    let dir = TempDir::new("sess-newer");
    let mut shell = shell_with_decoders(&dir);
    let path = dir.path().join("a.lasession");
    save(&mut shell, &path);
    let newer = dir.path().join("newer.lasession");
    let payload = vec![2, 1, 2, 3, 4];
    rewrite(&path, &newer, vec![("DECO", payload.clone())]);
    let mut reopened = open(&dir, &newer);
    assert!(reopened.state().decoders.instances.is_empty());
    assert!(codes(&reopened).contains(&"notice-decoders-newer".to_owned()));
    assert!(!codes(&reopened).contains(&"decoder.section_invalid".to_owned()));
    let saved = dir.path().join("saved.lasession");
    save(&mut reopened, &saved);
    let deco = sections(&saved)
        .into_iter()
        .find(|(t, _)| t == "DECO")
        .unwrap()
        .1;
    assert_eq!(deco, payload, "kept untouched");
}

#[test]
fn a_setup_change_makes_the_session_unsaved_and_a_finishing_pass_does_not() {
    let dir = TempDir::new("sess-dirty");
    let mut shell = shell_with_decoders(&dir);
    let path = dir.path().join("a.lasession");
    save(&mut shell, &path);
    let mut reopened = open(&dir, &path);
    settle(&mut reopened);
    assert_eq!(
        reopened.state().unsaved,
        Unsaved::Clean,
        "the passes that start after opening do not dirty it"
    );
    let sample = reopened.state().decoders.instances[2].id;
    done(
        &mut reopened,
        Command::SetDecoderOption {
            id: sample,
            option: "bits".into(),
            value: Value::Integer(9),
        },
    );
    assert_eq!(reopened.state().unsaved, Unsaved::Dirty);
    settle(&mut reopened);
    assert_eq!(reopened.state().unsaved, Unsaved::Dirty);
}

#[test]
fn a_save_with_decoders_that_fails_at_any_step_keeps_the_previous_file() {
    let dir = TempDir::new("sess-faults");
    let mut shell = finished_shell(&dir, 0xffff, 2_500_000, 9);
    prepare(&mut shell);
    done(&mut shell, Command::AddDecoder { decoder: ID.into() });
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
    done(&mut shell, Command::SetSaveDecoderResults(true));
    settle(&mut shell);
    let path = dir.path().join("keep.lasession");
    save(&mut shell, &path);
    let previous = std::fs::read(&path).unwrap();
    done(
        &mut shell,
        Command::RenameDecoder {
            id,
            name: "changed".into(),
        },
    );
    let counting = FaultFs::counting();
    shell.set_session_fs(Arc::new(counting.clone()));
    done(&mut shell, Command::SaveSessionAs(path.clone()));
    wait_operation(&mut shell);
    let new_bytes = std::fs::read(&path).unwrap();
    assert_ne!(new_bytes, previous);
    let counts: Vec<(Op, usize)> = Op::ALL
        .iter()
        .map(|op| (*op, counting.calls(*op)))
        .collect();
    let kinds = [
        ErrorKind::Other,
        ErrorKind::PermissionDenied,
        ErrorKind::WriteZero,
        ErrorKind::TimedOut,
        ErrorKind::NotFound,
        ErrorKind::UnexpectedEof,
    ];
    let total: usize = counts.iter().map(|(_, n)| n).sum();
    assert!(
        total * kinds.len() >= 100,
        "only {} fault points",
        total * kinds.len()
    );
    for (op, count) in counts {
        for nth in 0..count {
            for kind in kinds {
                std::fs::write(&path, &previous).unwrap();
                let fs = FaultFs::failing(FaultPlan {
                    op,
                    nth,
                    kind,
                    partial_write: op == Op::Write && nth % 2 == 0,
                });
                done(
                    &mut shell,
                    Command::RenameDecoder {
                        id,
                        name: format!("changed {nth}"),
                    },
                );
                shell.set_session_fs(Arc::new(fs.clone()));
                done(&mut shell, Command::SaveSessionAs(path.clone()));
                wait_operation(&mut shell);
                let now = std::fs::read(&path).unwrap();
                assert!(
                    now == previous || now.len() > 100,
                    "{op:?} #{nth}: neither old nor new"
                );
                if now == previous {
                    assert_eq!(shell.state().unsaved, Unsaved::Dirty, "{op:?} #{nth}");
                } else {
                    // The fault was one the save could live with: the new file is whole.
                    la_session::load_blocking(&path, &LoadLimits::default(), &Control::new())
                        .unwrap();
                }
            }
        }
    }
}
