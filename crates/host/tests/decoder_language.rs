//! The texts of decoders follow the language at once; text from outside does not (spec US7, SC-010).
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
use la_decoders::model::Text;
use la_decoders::testing::{self, RETURNS_ERROR};
use la_host::decoders::{InstanceState, annotation_text};
use la_host::window::ViewSize;
use la_host::{Command, Outcome, Shell};
use la_i18n::Language;

fn done(shell: &mut Shell, command: Command) {
    let outcome = shell.apply(command);
    assert!(matches!(outcome, Outcome::Done(_)), "{outcome:?}");
}

fn settle(shell: &mut Shell) {
    let deadline = Instant::now() + Duration::from_secs(30);
    while shell.state().decoders.instances.iter().any(|i| {
        i.state == InstanceState::Running || (i.state == InstanceState::Waiting && i.ready)
    }) {
        shell.poll(Instant::now());
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(2));
    }
}

fn shell_with_sample(dir: &TempDir) -> (Shell, u32) {
    let mut shell = finished_shell(dir, 0b11, 30_000, 5);
    for (descriptor, create) in testing::fixtures() {
        shell.register_decoder(descriptor, create).unwrap();
    }
    shell.set_decoder_thread_cap(4);
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
    settle(&mut shell);
    (shell, id)
}

#[test]
fn the_catalogue_names_and_labels_change_with_the_language() {
    let dir = TempDir::new("lang-catalogue");
    let (mut shell, _) = shell_with_sample(&dir);
    let mut names = Vec::new();
    for language in Language::ALL {
        done(&mut shell, Command::SetLanguage(language));
        let entry = shell
            .state()
            .decoders
            .catalogue
            .iter()
            .find(|c| c.id == ID)
            .unwrap()
            .clone();
        for text in [&entry.name, &entry.description]
            .into_iter()
            .chain(entry.signals.iter().map(|s| &s.label))
            .chain(entry.options.iter().map(|o| &o.label))
        {
            assert!(
                !text.is_empty() && !text.contains('\u{27E6}'),
                "{language:?}: {text}"
            );
            assert!(
                !text.starts_with("decoder-"),
                "{language:?}: an unresolved key {text}"
            );
        }
        names.push(entry.name);
    }
    let mut unique = names.clone();
    unique.sort();
    unique.dedup();
    assert_eq!(
        unique.len(),
        6,
        "every language has its own name: {names:?}"
    );
}

#[test]
fn the_lane_name_and_the_standard_annotation_texts_follow_the_language() {
    let dir = TempDir::new("lang-annotations");
    let (mut shell, id) = shell_with_sample(&dir);
    done(&mut shell, Command::ZoomToRange { start: 0, end: 600 });
    let byte = Text::Message {
        key: "ann-sample-clocked-bits-byte",
        args: vec!["0x5A".into()],
    };
    let literal = Text::Literal("From outside".into());
    let mut texts = Vec::new();
    let mut lane_names = Vec::new();
    for language in Language::ALL {
        done(&mut shell, Command::SetLanguage(language));
        let text = annotation_text(shell.localizer(), &byte);
        assert!(text.contains("0x5A"), "{language:?}: {text}");
        assert!(!text.contains("ann-sample"), "{language:?}: {text}");
        // Literal text is shown as it is, in every language.
        assert_eq!(annotation_text(shell.localizer(), &literal), "From outside");
        assert_eq!(shell.annotation_text(&byte), text);
        texts.push(text);
        let plan = shell.frame_plan(ViewSize {
            pixels: 800,
            scale: 1.0,
        });
        lane_names.push(plan.lanes[0].name.clone());
    }
    let distinct = |mut v: Vec<String>| {
        v.sort();
        v.dedup();
        v.len()
    };
    // English, German and Spanish happen to say "Byte"; the others differ.
    assert!(distinct(texts) >= 4);
    assert_eq!(
        distinct(lane_names),
        6,
        "the lane carries the localized decoder name"
    );
    let _ = id;
}

#[test]
fn an_instance_name_given_by_the_user_does_not_change() {
    let dir = TempDir::new("lang-instance-name");
    let (mut shell, id) = shell_with_sample(&dir);
    done(
        &mut shell,
        Command::RenameDecoder {
            id,
            name: "My bus ñ シリアル".into(),
        },
    );
    for language in Language::ALL {
        done(&mut shell, Command::SetLanguage(language));
        assert_eq!(
            shell.state().decoders.instance(id).unwrap().setup.name,
            "My bus ñ シリアル"
        );
        let plan = shell.frame_plan(ViewSize {
            pixels: 800,
            scale: 1.0,
        });
        assert_eq!(plan.lanes[0].name, "My bus ñ シリアル");
    }
}

#[test]
fn the_notice_of_a_fault_follows_the_language() {
    let dir = TempDir::new("lang-notice");
    let (mut shell, _) = shell_with_sample(&dir);
    shell.set_decoder_chunk_limits(la_decoders::run::ChunkLimits {
        max_edges: 100,
        max_span: 1_000,
    });
    done(
        &mut shell,
        Command::AddDecoder {
            decoder: RETURNS_ERROR.id.into(),
        },
    );
    let id = shell.state().decoders.instances.last().unwrap().id;
    done(
        &mut shell,
        Command::SetDecoderChannel {
            id,
            role: "clock".into(),
            channel: Some(0),
        },
    );
    settle(&mut shell);
    let notice = shell.state().notices.iter().last().cloned().unwrap();
    let mut texts = Vec::new();
    for language in Language::ALL {
        done(&mut shell, Command::SetLanguage(language));
        texts.push(shell.notice_text(&notice));
    }
    texts.sort();
    texts.dedup();
    assert_eq!(texts.len(), 6);
}
