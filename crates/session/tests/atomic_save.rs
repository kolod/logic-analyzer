//! Saving never damages or loses data (spec US2, FR-007..012, SC-003).
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
use std::time::{Duration, SystemTime};

use common::{End, TempDir, assert_same_capture, data, record};
use la_session::error as error_codes;
use la_session::harness::{FaultFs, FaultPlan, Op, load_with, save_with};
use la_session::{Control, LoadLimits, SessionData, load_blocking, save_blocking};
use la_storage::Limits;

/// A session of roughly 300 KiB, so a save makes several write calls.
fn session(seed: u64) -> SessionData {
    let capture = record(
        16,
        120,
        &[(None, 55_000), (Some(30), 25_000)],
        End::Closed,
        seed,
        Limits::default(),
    );
    data(&capture, seed % 2 == 0, Some(seed))
}

fn save_clean(path: &Path, data: &SessionData) -> la_session::SaveReport {
    save_blocking(path, data, &Control::new()).unwrap()
}

fn loads_as(path: &Path, expected: &SessionData) {
    let loaded = load_blocking(path, &LoadLimits::default(), &Control::new()).unwrap();
    assert_same_capture(&expected.snapshot(), &loaded.snapshot());
    assert_eq!(loaded.description(), expected.description());
}

fn save_on(
    fs: &FaultFs,
    path: &Path,
    data: &SessionData,
) -> la_core::Result<la_session::SaveReport> {
    save_with(fs, path, data, &Control::new())
}

#[test]
fn a_failure_at_every_step_leaves_the_old_file_intact_and_no_temporary_file() {
    let old = session(1);
    let new = session(2);
    // A clean run to learn how many calls of each kind a save makes.
    let counting = FaultFs::counting();
    let probe = TempDir::new("probe");
    save_on(&counting, &probe.join("target"), &new).unwrap();
    let writes = counting.calls(Op::Write);
    assert!(
        writes >= 3,
        "the session must be large enough to need several writes: {writes}"
    );

    let mut points = 0;
    for op in [Op::Create, Op::Write, Op::Flush, Op::SyncAll, Op::Rename] {
        for nth in 0..counting.calls(op) {
            for kind in [
                ErrorKind::StorageFull,
                ErrorKind::PermissionDenied,
                ErrorKind::Other,
            ] {
                for partial_write in [false, true] {
                    if partial_write && op != Op::Write {
                        continue;
                    }
                    points += 1;
                    let dir = TempDir::new("fault");
                    let target = dir.join("target");
                    save_clean(&target, &old);
                    let before = std::fs::read(&target).unwrap();

                    let fs = FaultFs::failing(FaultPlan {
                        op,
                        nth,
                        kind,
                        partial_write,
                    });
                    let error = save_on(&fs, &target, &new).unwrap_err();
                    let what = format!("{op:?} #{nth} {kind:?} partial={partial_write}");
                    assert_eq!(fs.fired(), 1, "{what}: the fault must fire");
                    assert_eq!(error.code(), error_codes::IO_WRITE, "{what}");
                    assert!(
                        error.detail().contains("saving failed"),
                        "{what}: {}",
                        error.detail()
                    );
                    assert_eq!(
                        std::fs::read(&target).unwrap(),
                        before,
                        "{what}: old file changed"
                    );
                    assert_eq!(dir.names(), ["target"], "{what}: leftovers");
                    loads_as(&target, &old);
                }
            }
        }
    }
    assert!(points >= 20, "only {points} injection points");
}

#[test]
fn a_failed_first_save_leaves_no_file_at_all() {
    let new = session(3);
    for (op, nth) in [
        (Op::Create, 0),
        (Op::Write, 1),
        (Op::SyncAll, 0),
        (Op::Rename, 0),
    ] {
        let dir = TempDir::new("first");
        let target = dir.join("fresh.lasession");
        let fs = FaultFs::failing(FaultPlan {
            op,
            nth,
            kind: ErrorKind::StorageFull,
            partial_write: false,
        });
        assert!(save_on(&fs, &target, &new).is_err());
        assert!(dir.names().is_empty(), "{op:?}: {:?}", dir.names());
    }
}

#[test]
fn a_failure_to_sync_the_directory_is_reported_but_the_new_file_is_in_place() {
    let old = session(1);
    let new = session(2);
    let dir = TempDir::new("syncdir");
    let target = dir.join("target");
    save_clean(&target, &old);
    let fs = FaultFs::failing(FaultPlan {
        op: Op::SyncDir,
        nth: 0,
        kind: ErrorKind::Other,
        partial_write: false,
    });
    let report = save_on(&fs, &target, &new).unwrap();
    assert!(
        !report.directory_synced,
        "the report says the directory entry may not be durable"
    );
    loads_as(&target, &new);
    assert_eq!(dir.names(), ["target"]);
    // A failing stale-file scan never fails the save either.
    let fs = FaultFs::failing(FaultPlan {
        op: Op::List,
        nth: 0,
        kind: ErrorKind::PermissionDenied,
        partial_write: false,
    });
    assert!(save_on(&fs, &target, &old).is_ok());
    loads_as(&target, &old);
}

#[test]
fn a_cancelled_save_leaves_the_old_file_and_no_temporary_file() {
    let old = session(1);
    let new = session(2);
    let dir = TempDir::new("cancel");
    let target = dir.join("target");
    save_clean(&target, &old);
    let before = std::fs::read(&target).unwrap();
    let control = Control::new();
    control.cancel();
    let error = save_with(&FaultFs::counting(), &target, &new, &control).unwrap_err();
    assert_eq!(error.code(), error_codes::CANCELLED);
    assert_eq!(std::fs::read(&target).unwrap(), before);
    assert_eq!(dir.names(), ["target"]);
}

#[test]
fn unusable_paths_fail_before_anything_is_written() {
    let new = session(2);
    let dir = TempDir::new("paths");
    std::fs::create_dir(dir.join("a folder")).unwrap();
    let invalid = error_codes::INVALID_INPUT;
    for (path, why) in [
        (std::path::PathBuf::new(), "empty path"),
        (dir.path().to_path_buf(), "a directory"),
        (dir.join("a folder"), "a folder named like a file"),
        (
            dir.join("missing folder").join("x"),
            "a parent that does not exist",
        ),
        (std::path::PathBuf::from("/"), "the root"),
    ] {
        let error = save_blocking(&path, &new, &Control::new()).unwrap_err();
        assert_eq!(error.code(), invalid, "{why}: {error}");
    }
    assert_eq!(dir.names(), ["a folder"], "nothing was created");
    // A parent that is a file.
    std::fs::write(dir.join("plain"), b"x").unwrap();
    let error = save_blocking(&dir.join("plain").join("x"), &new, &Control::new()).unwrap_err();
    assert_eq!(error.code(), invalid);
    assert_eq!(std::fs::read(dir.join("plain")).unwrap(), b"x");
}

#[test]
fn a_denied_creation_is_a_typed_actionable_error() {
    let new = session(2);
    let dir = TempDir::new("denied");
    let fs = FaultFs::failing(FaultPlan {
        op: Op::Create,
        nth: 0,
        kind: ErrorKind::PermissionDenied,
        partial_write: false,
    });
    let error = save_on(&fs, &dir.join("x"), &new).unwrap_err();
    assert_eq!(error.code(), error_codes::IO_WRITE);
    assert!(error.detail().contains("permission"), "{}", error.detail());
    let fs = FaultFs::failing(FaultPlan {
        op: Op::Write,
        nth: 0,
        kind: ErrorKind::StorageFull,
        partial_write: true,
    });
    let error = save_on(&fs, &dir.join("x"), &new).unwrap_err();
    assert!(
        error.detail().contains("disk is full"),
        "{}",
        error.detail()
    );
    assert!(
        std::error::Error::source(&error).is_some(),
        "the operating system error is kept"
    );
}

fn leftover(dir: &TempDir, name: &str, age: Duration) {
    let path = dir.join(name);
    std::fs::write(&path, b"partial").unwrap();
    let file = std::fs::OpenOptions::new().write(true).open(&path).unwrap();
    file.set_modified(SystemTime::now() - age).unwrap();
}

#[test]
fn leftover_temporary_files_never_block_a_save_and_only_stale_ones_of_this_target_are_removed() {
    let new = session(2);
    let dir = TempDir::new("leftover");
    let target = dir.join("target.lasession");
    let hex = "0123456789abcdef";
    let old = Duration::from_secs(2 * 3600);
    leftover(&dir, &format!(".target.lasession.{hex}.lasave-tmp"), old); // stale, ours
    leftover(
        &dir,
        ".target.lasession.fedcba9876543210.lasave-tmp",
        Duration::from_secs(5),
    ); // young, ours
    leftover(&dir, ".target.lasession.ABCDEF0123456789.lasave-tmp", old); // uppercase: not ours
    leftover(&dir, ".target.lasession.0123.lasave-tmp", old); // wrong length: not ours
    leftover(&dir, &format!(".other.lasession.{hex}.lasave-tmp"), old); // another target
    leftover(&dir, &format!("target.lasession.{hex}.lasave-tmp"), old); // no leading dot
    leftover(&dir, &format!(".target.lasession.{hex}.tmp"), old); // another suffix
    leftover(&dir, "notes.txt", old);
    std::fs::create_dir(dir.join(&format!(".target.lasession.{}.lasave-tmp", "a".repeat(16))))
        .unwrap();

    save_clean(&target, &new);
    loads_as(&target, &new);
    let names = dir.names();
    assert!(
        !names.contains(&format!(".target.lasession.{hex}.lasave-tmp")),
        "stale leftover removed"
    );
    for kept in [
        ".target.lasession.fedcba9876543210.lasave-tmp",
        ".target.lasession.ABCDEF0123456789.lasave-tmp",
        ".target.lasession.0123.lasave-tmp",
        ".other.lasession.0123456789abcdef.lasave-tmp",
        "target.lasession.0123456789abcdef.lasave-tmp",
        ".target.lasession.0123456789abcdef.tmp",
        "notes.txt",
    ] {
        assert!(
            names.iter().any(|n| n == kept),
            "{kept} must be kept: {names:?}"
        );
    }
    // The folder that looks like a temporary file is not touched either.
    assert!(
        dir.join(&format!(".target.lasession.{}.lasave-tmp", "a".repeat(16)))
            .is_dir()
    );
}

#[test]
fn two_saves_to_the_same_path_end_with_exactly_one_complete_file() {
    let a = session(10);
    let b = session(11);
    let dir = TempDir::new("race");
    let target = dir.join("target");
    let reference = TempDir::new("race-ref");
    save_clean(&reference.join("a"), &a);
    save_clean(&reference.join("b"), &b);
    let bytes_a = std::fs::read(reference.join("a")).unwrap();
    let bytes_b = std::fs::read(reference.join("b")).unwrap();
    assert_ne!(bytes_a, bytes_b);
    for round in 0..10 {
        let handles: Vec<_> = [a.clone(), b.clone()]
            .into_iter()
            .map(|session| {
                let target = target.clone();
                std::thread::spawn(move || save_blocking(&target, &session, &Control::new()))
            })
            .collect();
        let results: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();
        // On Windows a rename over a file that is being replaced at the same moment may be
        // refused; a refused save leaves everything intact, so at least one must succeed.
        assert!(
            results.iter().any(Result::is_ok),
            "round {round}: {results:?}"
        );
        let bytes = std::fs::read(&target).unwrap();
        assert!(
            bytes == bytes_a || bytes == bytes_b,
            "round {round}: the file is neither complete version"
        );
        load_blocking(&target, &LoadLimits::default(), &Control::new()).unwrap();
        assert_eq!(
            dir.names(),
            ["target"],
            "round {round}: no temporary file is left"
        );
    }
}

#[test]
fn paths_with_spaces_and_non_ascii_characters_work() {
    let new = session(4);
    let dir = TempDir::new("names");
    for name in [
        "with space.lasession",
        "заголовок — сесія.lasession",
        "日本語のファイル.lasession",
        "ä ö ü ß.lasession",
    ] {
        let path = dir.join(name);
        save_clean(&path, &new);
        loads_as(&path, &new);
        save_clean(&path, &new); // and again over the existing file
    }
    assert_eq!(dir.names().len(), 4);
}

#[test]
fn the_loader_reads_what_a_failed_save_never_touched() {
    // Loading through the same seam works with the counting file system too.
    let new = session(5);
    let dir = TempDir::new("seam");
    let target = dir.join("t");
    save_clean(&target, &new);
    let fs = FaultFs::counting();
    let loaded = load_with(&fs, &target, &LoadLimits::default(), &Control::new()).unwrap();
    assert_same_capture(&new.snapshot(), &loaded.snapshot());
    assert_eq!(fs.calls(Op::OpenRead), 1);
}
