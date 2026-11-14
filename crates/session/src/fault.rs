//! A fault-injecting file system for tests (feature `test-support`).
//!
//! [`FaultFs`] wraps the real file system, counts every operation and can fail exactly one of
//! them — the n-th call of one kind — with a chosen I/O error. A clean run with
//! [`FaultFs::counting`] tells a test how many calls of each kind a save or load makes, so it
//! can fail every one of them in turn.

use std::io::{self, Read};
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use crate::fs::{DirEntryInfo, FileSystem, PathKind, RealFileSystem, WriteSink};

/// The kinds of file-system call that can be counted and failed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Op {
    /// `create_exclusive`.
    Create,
    /// A `write_all` on a file being written.
    Write,
    /// A `flush` on a file being written.
    Flush,
    /// A `sync_all` on a file being written.
    SyncAll,
    /// `rename`.
    Rename,
    /// `sync_dir`.
    SyncDir,
    /// `remove_file`.
    Remove,
    /// `list_dir`.
    List,
    /// `open_read`.
    OpenRead,
    /// A `read` on a file being read.
    Read,
}

impl Op {
    /// Every kind, in a fixed order.
    pub const ALL: [Op; 10] = [
        Op::Create,
        Op::Write,
        Op::Flush,
        Op::SyncAll,
        Op::Rename,
        Op::SyncDir,
        Op::Remove,
        Op::List,
        Op::OpenRead,
        Op::Read,
    ];

    fn index(self) -> usize {
        match self {
            Op::Create => 0,
            Op::Write => 1,
            Op::Flush => 2,
            Op::SyncAll => 3,
            Op::Rename => 4,
            Op::SyncDir => 5,
            Op::Remove => 6,
            Op::List => 7,
            Op::OpenRead => 8,
            Op::Read => 9,
        }
    }
}

/// Which call fails, and how.
#[derive(Debug, Clone, Copy)]
pub struct FaultPlan {
    /// The kind of call.
    pub op: Op,
    /// Which call of that kind (0 = the first).
    pub nth: usize,
    /// The error to return.
    pub kind: io::ErrorKind,
    /// For [`Op::Write`]: write the first half of the data before failing (a full disk).
    pub partial_write: bool,
}

#[derive(Debug)]
struct Shared {
    counts: [AtomicUsize; 10],
    plan: Option<FaultPlan>,
    fired: AtomicUsize,
}

impl Shared {
    /// Counts a call; returns the plan if this very call must fail.
    fn hit(&self, op: Op) -> Option<FaultPlan> {
        let n = self.counts.get(op.index())?.fetch_add(1, Ordering::AcqRel);
        match self.plan {
            Some(plan) if plan.op == op && plan.nth == n => {
                self.fired.fetch_add(1, Ordering::AcqRel);
                Some(plan)
            }
            _ => None,
        }
    }
}

fn injected(plan: &FaultPlan) -> io::Error {
    io::Error::new(plan.kind, "injected fault")
}

/// The real file system with counting and one injectable fault.
#[derive(Debug, Clone)]
pub struct FaultFs {
    inner: RealFileSystem,
    shared: Arc<Shared>,
}

impl FaultFs {
    fn with_plan(plan: Option<FaultPlan>) -> Self {
        Self {
            inner: RealFileSystem,
            shared: Arc::new(Shared {
                counts: std::array::from_fn(|_| AtomicUsize::new(0)),
                plan,
                fired: AtomicUsize::new(0),
            }),
        }
    }

    /// A file system that only counts.
    #[must_use]
    pub fn counting() -> Self {
        Self::with_plan(None)
    }

    /// A file system that fails the call described by `plan`.
    #[must_use]
    pub fn failing(plan: FaultPlan) -> Self {
        Self::with_plan(Some(plan))
    }

    /// How many calls of `op` were made so far.
    #[must_use]
    pub fn calls(&self, op: Op) -> usize {
        self.shared
            .counts
            .get(op.index())
            .map_or(0, |c| c.load(Ordering::Acquire))
    }

    /// How many times the planned fault fired (0 or 1).
    #[must_use]
    pub fn fired(&self) -> usize {
        self.shared.fired.load(Ordering::Acquire)
    }
}

struct FaultSink {
    inner: Box<dyn WriteSink>,
    shared: Arc<Shared>,
}

impl WriteSink for FaultSink {
    fn write_all(&mut self, data: &[u8]) -> io::Result<()> {
        if let Some(plan) = self.shared.hit(Op::Write) {
            if plan.partial_write {
                let half = data.get(..data.len() / 2).unwrap_or(&[]);
                self.inner.write_all(half)?;
            }
            return Err(injected(&plan));
        }
        self.inner.write_all(data)
    }

    fn flush(&mut self) -> io::Result<()> {
        if let Some(plan) = self.shared.hit(Op::Flush) {
            return Err(injected(&plan));
        }
        self.inner.flush()
    }

    fn sync_all(&mut self) -> io::Result<()> {
        if let Some(plan) = self.shared.hit(Op::SyncAll) {
            return Err(injected(&plan));
        }
        self.inner.sync_all()
    }
}

struct FaultReader {
    inner: Box<dyn Read + Send>,
    shared: Arc<Shared>,
}

impl Read for FaultReader {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if let Some(plan) = self.shared.hit(Op::Read) {
            return Err(injected(&plan));
        }
        self.inner.read(buf)
    }
}

impl FileSystem for FaultFs {
    fn create_exclusive(&self, path: &Path) -> io::Result<Box<dyn WriteSink>> {
        if let Some(plan) = self.shared.hit(Op::Create) {
            return Err(injected(&plan));
        }
        let inner = self.inner.create_exclusive(path)?;
        Ok(Box::new(FaultSink {
            inner,
            shared: Arc::clone(&self.shared),
        }))
    }

    fn open_read(&self, path: &Path) -> io::Result<(Box<dyn Read + Send>, u64)> {
        if let Some(plan) = self.shared.hit(Op::OpenRead) {
            return Err(injected(&plan));
        }
        let (inner, length) = self.inner.open_read(path)?;
        Ok((
            Box::new(FaultReader {
                inner,
                shared: Arc::clone(&self.shared),
            }),
            length,
        ))
    }

    fn rename(&self, from: &Path, to: &Path) -> io::Result<()> {
        if let Some(plan) = self.shared.hit(Op::Rename) {
            return Err(injected(&plan));
        }
        self.inner.rename(from, to)
    }

    fn remove_file(&self, path: &Path) -> io::Result<()> {
        if let Some(plan) = self.shared.hit(Op::Remove) {
            return Err(injected(&plan));
        }
        self.inner.remove_file(path)
    }

    fn sync_dir(&self, dir: &Path) -> io::Result<()> {
        if let Some(plan) = self.shared.hit(Op::SyncDir) {
            return Err(injected(&plan));
        }
        self.inner.sync_dir(dir)
    }

    fn list_dir(&self, dir: &Path) -> io::Result<Vec<DirEntryInfo>> {
        if let Some(plan) = self.shared.hit(Op::List) {
            return Err(injected(&plan));
        }
        self.inner.list_dir(dir)
    }

    fn kind(&self, path: &Path) -> io::Result<Option<PathKind>> {
        self.inner.kind(path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fs::TempDir;

    fn plan(op: Op, nth: usize) -> FaultPlan {
        FaultPlan {
            op,
            nth,
            kind: io::ErrorKind::StorageFull,
            partial_write: false,
        }
    }

    /// Runs every kind of call once against `fs`; returns which of them failed.
    fn exercise(fs: &FaultFs, dir: &Path) -> Vec<Op> {
        let mut failed = Vec::new();
        let file = dir.join("f");
        let other = dir.join("g");
        let mut check = |op: Op, ok: bool| {
            if !ok {
                failed.push(op);
            }
        };
        match fs.create_exclusive(&file) {
            Ok(mut sink) => {
                check(Op::Write, sink.write_all(b"data").is_ok());
                check(Op::Flush, sink.flush().is_ok());
                check(Op::SyncAll, sink.sync_all().is_ok());
            }
            Err(_) => check(Op::Create, false),
        }
        check(Op::Rename, fs.rename(&file, &other).is_ok());
        check(Op::SyncDir, fs.sync_dir(dir).is_ok());
        check(Op::List, fs.list_dir(dir).is_ok());
        match fs.open_read(&other) {
            Ok((mut reader, _)) => {
                let mut text = String::new();
                check(Op::Read, reader.read_to_string(&mut text).is_ok());
            }
            Err(_) => check(Op::OpenRead, false),
        }
        check(Op::Remove, fs.remove_file(&other).is_ok());
        failed
    }

    #[test]
    fn a_counting_file_system_counts_and_never_fails() {
        let dir = TempDir::new("fault-count");
        let fs = FaultFs::counting();
        assert!(exercise(&fs, dir.path()).is_empty());
        for op in [
            Op::Create,
            Op::Write,
            Op::Flush,
            Op::SyncAll,
            Op::Rename,
            Op::SyncDir,
            Op::List,
            Op::OpenRead,
            Op::Remove,
        ] {
            assert_eq!(fs.calls(op), 1, "{op:?}");
        }
        assert!(fs.calls(Op::Read) >= 1);
        assert_eq!(fs.fired(), 0);
    }

    #[test]
    fn every_injection_point_fires_exactly_once_and_only_there() {
        for op in Op::ALL {
            let dir = TempDir::new("fault-each");
            let fs = FaultFs::failing(plan(op, 0));
            let failed = exercise(&fs, dir.path());
            assert!(failed.contains(&op), "{op:?} did not fail");
            assert_eq!(fs.fired(), 1, "{op:?} fired {} times", fs.fired());
        }
        // A later call of the same kind is left alone.
        let dir = TempDir::new("fault-nth");
        let fs = FaultFs::failing(plan(Op::Remove, 1));
        assert!(exercise(&fs, dir.path()).is_empty());
        assert_eq!(fs.fired(), 0);
    }

    #[test]
    fn a_partial_write_leaves_half_the_data_behind() {
        let dir = TempDir::new("fault-partial");
        let fs = FaultFs::failing(FaultPlan {
            op: Op::Write,
            nth: 0,
            kind: io::ErrorKind::StorageFull,
            partial_write: true,
        });
        let path = dir.path().join("f");
        let mut sink = fs.create_exclusive(&path).unwrap();
        let error = sink.write_all(b"12345678").unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::StorageFull);
        drop(sink);
        assert_eq!(std::fs::read(&path).unwrap(), b"1234");
    }
}
