//! A save that fails at any step never damages the previous file (spec SC-007, task T065).
//!
//! A clean save is counted call by call; then every call of every kind is failed in turn. After
//! each fault the file is either exactly what it was before or the complete new session, no
//! temporary file is left once the next save has run, and the user was told about a failure with
//! one message, with the capture unchanged and still unsaved.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::panic
)]

mod common;

use std::io::ErrorKind;
use std::sync::Arc;

use common::{TempDir, finished_shell, temp_files, wait_operation};
use la_host::notices::NoticeContent;
use la_host::{Command, Outcome, Shell, Unsaved};
use la_session::harness::{FaultFs, FaultPlan, Op};

fn done(shell: &mut Shell, command: Command) {
    match shell.apply(command) {
        Outcome::Done(_) => {}
        other => panic!("{other:?}"),
    }
}

fn codes(shell: &Shell) -> Vec<String> {
    shell
        .state()
        .notices
        .iter()
        .filter_map(|n| match &n.content {
            NoticeContent::Error { code } => Some(code.clone()),
            NoticeContent::Info { .. } => None,
        })
        .collect()
}

#[test]
fn a_fault_at_every_step_of_a_save_keeps_the_previous_file_and_tells_the_user() {
    let dir = TempDir::new("interruptions");
    let mut shell = finished_shell(&dir, 0xffff, 2_500_000, 21);
    let path = dir.path().join("keep.lasession");

    // The previous file: the capture as first saved.
    done(&mut shell, Command::SaveSessionAs(path.clone()));
    wait_operation(&mut shell);
    let previous = std::fs::read(&path).unwrap();

    // The new content differs: a renamed channel adds a VIEW section.
    done(
        &mut shell,
        Command::RenameChannel {
            channel: 0,
            name: "edited".into(),
        },
    );
    assert_eq!(shell.state().unsaved, Unsaved::Dirty);

    // How many calls of each kind a clean save makes.
    let counting = FaultFs::counting();
    shell.set_session_fs(Arc::new(counting.clone()));
    done(&mut shell, Command::SaveSessionAs(path.clone()));
    wait_operation(&mut shell);
    assert!(codes(&shell).is_empty());
    let new_bytes = std::fs::read(&path).unwrap();
    assert_ne!(new_bytes, previous);
    let counts: Vec<(Op, usize)> = Op::ALL
        .iter()
        .map(|op| (*op, counting.calls(*op)))
        .collect();
    let total: usize = counts.iter().map(|(_, n)| n).sum();
    let kinds = [
        ErrorKind::Other,
        ErrorKind::PermissionDenied,
        ErrorKind::WriteZero,
        ErrorKind::TimedOut,
        ErrorKind::NotFound,
        ErrorKind::UnexpectedEof,
    ];
    assert!(
        total * kinds.len() >= 100,
        "only {} fault points: {counts:?}",
        total * kinds.len()
    );

    let mut failed_saves = 0;
    let mut tolerated = 0;
    for (op, count) in counts {
        for (nth, kind) in (0..count).flat_map(|n| kinds.iter().map(move |k| (n, *k))) {
            // Put the previous file back and make the shell dirty again.
            std::fs::write(&path, &previous).unwrap();
            let fs = FaultFs::failing(FaultPlan {
                op,
                nth,
                kind,
                partial_write: op == Op::Write
                    && (nth + kinds.iter().position(|k| *k == kind).unwrap()) % 2 == 0,
            });
            // Dirty again (a clean capture becomes unsaved when its presentation is edited).
            done(
                &mut shell,
                Command::RenameChannel {
                    channel: 0,
                    name: "edited".into(),
                },
            );
            assert_eq!(shell.state().unsaved, Unsaved::Dirty);
            done(&mut shell, Command::DismissAllNotices);
            shell.set_session_fs(Arc::new(fs.clone()));
            let what = format!("{op:?} #{nth} {kind:?}");
            done(&mut shell, Command::SaveSessionAs(path.clone()));
            wait_operation(&mut shell);
            assert_eq!(fs.fired(), 1, "{what}: the fault did not fire");

            let on_disk = std::fs::read(&path).unwrap();
            let failure = codes(&shell);
            if failure.is_empty() {
                // A best-effort step (directory sync, cleanup) failed: the save is complete.
                tolerated += 1;
                assert_eq!(on_disk, new_bytes, "{what}");
                assert_eq!(shell.state().unsaved, Unsaved::Clean, "{what}");
            } else {
                failed_saves += 1;
                assert_eq!(failure, [la_session::error::IO_WRITE], "{what}");
                assert_eq!(on_disk, previous, "{what}: the previous file changed");
                assert_eq!(shell.state().unsaved, Unsaved::Dirty, "{what}");
                assert!(shell.state().capture.is_some(), "{what}");
            }
            // The previous or the new file always loads.
            assert!(
                la_session::load_blocking(
                    &path,
                    &la_session::LoadLimits::default(),
                    &la_session::Control::new()
                )
                .is_ok(),
                "{what}: the file does not load"
            );
            // The next save (on the real file system) leaves no temporary file behind.
            shell.set_session_fs(Arc::new(la_session::harness::RealFileSystem));
            done(&mut shell, Command::SaveSessionAs(path.clone()));
            wait_operation(&mut shell);
            assert_eq!(
                temp_files(dir.path()),
                0,
                "{what}: a temporary file remains"
            );
            assert_eq!(std::fs::read(&path).unwrap(), new_bytes, "{what}");
        }
    }
    assert!(
        failed_saves >= 60,
        "{failed_saves} saves failed, {tolerated} tolerated"
    );
}
