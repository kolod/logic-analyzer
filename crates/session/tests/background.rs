//! Saving and loading never block the application (spec US6, FR-028, FR-029, FR-031, FR-032,
//! SC-008).
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::panic
)]

mod common;

use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use common::{End, TempDir, assert_same_capture, data, record};
use la_session::error::{CANCELLED, IO_WRITE, WORKER_FAILED};
use la_session::harness::{
    DirEntryInfo, FaultFs, FaultPlan, FileSystem, Op, PathKind, RealFileSystem, WriteSink,
    start_load_with, start_save_with,
};
use la_session::{LoadLimits, OperationState, SessionData, start_load, start_save};
use la_storage::Limits;

/// How long starting an operation may take.
#[cfg(debug_assertions)]
const START_LIMIT: Duration = Duration::from_millis(50);
#[cfg(not(debug_assertions))]
const START_LIMIT: Duration = Duration::from_millis(10);

/// How long a cancelled operation may take to end.
#[cfg(debug_assertions)]
const CANCEL_LIMIT: Duration = Duration::from_millis(500);
#[cfg(not(debug_assertions))]
const CANCEL_LIMIT: Duration = Duration::from_millis(100);

const WAIT: Duration = Duration::from_secs(30);

fn big_session() -> SessionData {
    let capture = record(
        16,
        200,
        &[(None, 60_000), (None, 60_000), (None, 60_000)],
        End::Closed,
        7,
        Limits::default(),
    );
    data(&capture, true, Some(3))
}

/// The real file system made slow, so that operations last long enough to be observed and
/// cancelled: every 64 KiB write or read takes 3 ms.
#[derive(Clone)]
struct SlowFs;

struct SlowSink(Box<dyn WriteSink>);

impl WriteSink for SlowSink {
    fn write_all(&mut self, data: &[u8]) -> io::Result<()> {
        std::thread::sleep(Duration::from_millis(3));
        self.0.write_all(data)
    }
    fn flush(&mut self) -> io::Result<()> {
        self.0.flush()
    }
    fn sync_all(&mut self) -> io::Result<()> {
        self.0.sync_all()
    }
}

struct SlowReader(Box<dyn Read + Send>);

impl Read for SlowReader {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        std::thread::sleep(Duration::from_millis(3));
        self.0.read(buf)
    }
}

impl FileSystem for SlowFs {
    fn create_exclusive(&self, path: &Path) -> io::Result<Box<dyn WriteSink>> {
        Ok(Box::new(SlowSink(RealFileSystem.create_exclusive(path)?)))
    }
    fn open_read(&self, path: &Path) -> io::Result<(Box<dyn Read + Send>, u64)> {
        let (reader, length) = RealFileSystem.open_read(path)?;
        Ok((Box::new(SlowReader(reader)), length))
    }
    fn rename(&self, from: &Path, to: &Path) -> io::Result<()> {
        RealFileSystem.rename(from, to)
    }
    fn remove_file(&self, path: &Path) -> io::Result<()> {
        RealFileSystem.remove_file(path)
    }
    fn sync_dir(&self, dir: &Path) -> io::Result<()> {
        RealFileSystem.sync_dir(dir)
    }
    fn list_dir(&self, dir: &Path) -> io::Result<Vec<DirEntryInfo>> {
        RealFileSystem.list_dir(dir)
    }
    fn kind(&self, path: &Path) -> io::Result<Option<PathKind>> {
        RealFileSystem.kind(path)
    }
}

/// A file system whose every call panics: a faulty worker.
struct PanicFs;

impl FileSystem for PanicFs {
    fn create_exclusive(&self, _: &Path) -> io::Result<Box<dyn WriteSink>> {
        panic!("deliberate fault injection: create")
    }
    fn open_read(&self, _: &Path) -> io::Result<(Box<dyn Read + Send>, u64)> {
        panic!("deliberate fault injection: open")
    }
    fn rename(&self, _: &Path, _: &Path) -> io::Result<()> {
        panic!("deliberate fault injection: rename")
    }
    fn remove_file(&self, _: &Path) -> io::Result<()> {
        Ok(())
    }
    fn sync_dir(&self, _: &Path) -> io::Result<()> {
        Ok(())
    }
    fn list_dir(&self, _: &Path) -> io::Result<Vec<DirEntryInfo>> {
        Ok(Vec::new())
    }
    fn kind(&self, _: &Path) -> io::Result<Option<PathKind>> {
        panic!("deliberate fault injection: kind")
    }
}

fn saved_file(dir: &TempDir, session: &SessionData) -> PathBuf {
    let path = dir.join("saved.lasession");
    start_save(path.clone(), session.clone())
        .wait_timeout(WAIT)
        .unwrap()
        .unwrap();
    path
}

#[test]
fn starting_returns_at_once_and_the_outcome_is_set_exactly_once() {
    let session = big_session();
    let dir = TempDir::new("start");
    let path = dir.join("s");

    let started = Instant::now();
    let save = start_save(path.clone(), session.clone());
    assert!(
        started.elapsed() < START_LIMIT,
        "start_save took {:?}",
        started.elapsed()
    );
    let report = save.wait_timeout(WAIT).unwrap().unwrap();
    assert_eq!(save.progress().state, OperationState::Succeeded);
    assert_eq!(save.progress().done_bytes, report.bytes_written);
    assert_eq!(save.progress().total_bytes, report.bytes_written);
    // The same outcome on every read.
    for _ in 0..5 {
        assert_eq!(save.outcome().unwrap().unwrap(), report);
    }

    let started = Instant::now();
    let load = start_load(path, LoadLimits::default());
    assert!(
        started.elapsed() < START_LIMIT,
        "start_load took {:?}",
        started.elapsed()
    );
    let loaded = load.wait_timeout(WAIT).unwrap().unwrap();
    assert_same_capture(&session.snapshot(), &loaded.snapshot());
    assert_eq!(load.progress().state, OperationState::Succeeded);
    assert!(Arc::ptr_eq(&loaded, &load.outcome().unwrap().unwrap()));
}

#[test]
fn progress_never_goes_backwards_and_never_exceeds_the_total() {
    let session = big_session();
    let dir = TempDir::new("progress");
    let path = dir.join("s");
    let save = start_save_with(Arc::new(SlowFs), path.clone(), session);
    let mut last = 0;
    let mut samples = 0;
    while save.outcome().is_none() {
        let p = save.progress();
        assert!(
            p.done_bytes >= last,
            "progress went back: {} after {last}",
            p.done_bytes
        );
        assert!(p.done_bytes <= p.total_bytes || p.total_bytes == 0);
        last = p.done_bytes;
        samples += 1;
        std::thread::sleep(Duration::from_micros(200));
    }
    assert!(
        samples >= 20,
        "the save should have been observable ({samples} observations)"
    );
    let p = save.progress();
    assert_eq!(
        (p.state, p.done_bytes),
        (OperationState::Succeeded, p.total_bytes)
    );

    let load = start_load_with(Arc::new(SlowFs), path, LoadLimits::default());
    let mut last = 0;
    while load.outcome().is_none() {
        let p = load.progress();
        assert!(p.done_bytes >= last && p.done_bytes <= p.total_bytes.max(last));
        last = p.done_bytes;
        std::thread::sleep(Duration::from_micros(200));
    }
    assert!(load.outcome().unwrap().is_ok());
}

#[test]
fn cancelling_a_save_is_prompt_and_leaves_the_old_file_and_no_temporary_file() {
    let session = big_session();
    let dir = TempDir::new("cancel-save");
    let path = saved_file(&dir, &session);
    let before = std::fs::read(&path).unwrap();

    let save = start_save_with(Arc::new(SlowFs), path.clone(), session);
    while save.progress().done_bytes == 0 {
        assert!(
            save.outcome().is_none(),
            "the save finished before it could be cancelled"
        );
        std::thread::sleep(Duration::from_millis(1));
    }
    let asked = Instant::now();
    save.cancel();
    assert!(asked.elapsed() < START_LIMIT, "cancel must return at once");
    let outcome = save.wait_timeout(WAIT).unwrap();
    assert!(
        asked.elapsed() < CANCEL_LIMIT,
        "ended after {:?}",
        asked.elapsed()
    );
    assert_eq!(outcome.unwrap_err().code(), CANCELLED);
    assert_eq!(save.progress().state, OperationState::Cancelled);
    assert_eq!(std::fs::read(&path).unwrap(), before);
    assert_eq!(dir.names(), ["saved.lasession"]);
}

#[test]
fn cancelling_a_load_ends_with_no_session() {
    let session = big_session();
    let dir = TempDir::new("cancel-load");
    let path = saved_file(&dir, &session);
    let load = start_load_with(Arc::new(SlowFs), path, LoadLimits::default());
    while load.progress().done_bytes == 0 {
        assert!(
            load.outcome().is_none(),
            "the load finished before it could be cancelled"
        );
        std::thread::sleep(Duration::from_millis(1));
    }
    let asked = Instant::now();
    load.cancel();
    let outcome = load.wait_timeout(WAIT).unwrap();
    assert!(
        asked.elapsed() < CANCEL_LIMIT,
        "ended after {:?}",
        asked.elapsed()
    );
    assert_eq!(outcome.err().unwrap().code(), CANCELLED);
    assert_eq!(load.progress().state, OperationState::Cancelled);
}

#[test]
fn cancelling_early_or_late_is_harmless() {
    let session = big_session();
    let dir = TempDir::new("cancel-edges");
    let path = dir.join("s");
    // Cancelled before the worker can have started: either it was stopped or it had finished.
    let save = start_save(path.clone(), session.clone());
    save.cancel();
    match save.wait_timeout(WAIT).unwrap() {
        Ok(_) => assert!(path.exists()),
        Err(e) => {
            assert_eq!(e.code(), CANCELLED);
            assert!(dir.names().is_empty(), "{:?}", dir.names());
        }
    }
    // Cancelled after it finished: nothing changes.
    let save = start_save(dir.join("t"), session);
    let report = save.wait_timeout(WAIT).unwrap().unwrap();
    save.cancel();
    assert_eq!(save.outcome().unwrap().unwrap(), report);
    assert_eq!(save.progress().state, OperationState::Succeeded);
}

#[test]
fn a_background_failure_is_the_error_a_blocking_call_would_return() {
    let session = big_session();
    let dir = TempDir::new("fail");
    let fs = FaultFs::failing(FaultPlan {
        op: Op::Create,
        nth: 0,
        kind: io::ErrorKind::PermissionDenied,
        partial_write: false,
    });
    let save = start_save_with(Arc::new(fs), dir.join("x"), session.clone());
    let error = save.wait_timeout(WAIT).unwrap().unwrap_err();
    assert_eq!(error.code(), IO_WRITE);
    assert!(error.detail().contains("permission"));
    assert_eq!(save.progress().state, OperationState::Failed);
    let blocking = la_session::save_blocking(&dir.join("y"), &session, &la_session::Control::new());
    assert!(
        blocking.is_ok(),
        "the same data saves fine on a healthy file system"
    );
    // A missing file fails a load with a typed error.
    let load = start_load(dir.join("missing"), LoadLimits::default());
    assert_eq!(
        load.wait_timeout(WAIT).unwrap().err().unwrap().code(),
        "session.io_read"
    );
}

#[test]
fn a_worker_panic_becomes_a_typed_error_and_the_capture_in_memory_is_unaffected() {
    let session = big_session();
    let before = session.snapshot();
    let dir = TempDir::new("panic");
    let save = start_save_with(Arc::new(PanicFs), dir.join("x"), session.clone());
    let error = save.wait_timeout(WAIT).unwrap().unwrap_err();
    assert_eq!(error.code(), WORKER_FAILED);
    assert_eq!(save.progress().state, OperationState::Failed);
    let load = start_load_with(Arc::new(PanicFs), dir.join("x"), LoadLimits::default());
    assert_eq!(
        load.wait_timeout(WAIT).unwrap().err().unwrap().code(),
        WORKER_FAILED
    );
    assert_same_capture(&before, &session.snapshot());
    // And the application goes on: a normal save still works afterwards.
    assert!(
        start_save(dir.join("ok"), session)
            .wait_timeout(WAIT)
            .unwrap()
            .is_ok()
    );
}

#[test]
fn wait_timeout_never_waits_forever_and_operations_run_side_by_side() {
    let session = big_session();
    let dir = TempDir::new("timeout");
    let slow = start_save_with(Arc::new(SlowFs), dir.join("slow"), session.clone());
    let asked = Instant::now();
    assert!(slow.wait_timeout(Duration::from_millis(5)).is_none());
    assert!(asked.elapsed() < Duration::from_millis(500));
    // Two more operations run while the slow one is busy.
    let (a, b) = (
        start_save(dir.join("a"), session.clone()),
        start_save(dir.join("b"), session),
    );
    assert!(a.wait_timeout(WAIT).unwrap().is_ok() && b.wait_timeout(WAIT).unwrap().is_ok());
    assert!(slow.wait_timeout(WAIT).unwrap().is_ok());
    // Dropping a handle detaches the worker without a panic.
    let detached = start_save_with(Arc::new(SlowFs), dir.join("detached"), data_small());
    drop(detached);
    std::thread::sleep(Duration::from_millis(200));
}

fn data_small() -> SessionData {
    let capture = record(2, 50, &[(None, 2_000)], End::Closed, 1, Limits::default());
    data(&capture, false, None)
}
