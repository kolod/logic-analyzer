//! Helpers for the tests of saving and opening sessions.
#![allow(
    dead_code,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects
)]

use std::io::{self, Read};
use std::path::Path;

use la_host::Shell;
use la_session::harness::{DirEntryInfo, FileSystem, PathKind, RealFileSystem, WriteSink};
use la_storage::Snapshot;

/// Polls until the shell has no save or load running.
pub fn wait_operation(shell: &mut Shell) {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(60);
    while shell.operation_running() {
        shell.poll(std::time::Instant::now());
        assert!(
            std::time::Instant::now() < deadline,
            "the operation did not end"
        );
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
    shell.poll(std::time::Instant::now());
}

/// Asserts that two captures answer every query the same way.
pub fn assert_same_snapshot(a: &Snapshot, b: &Snapshot) {
    assert_eq!(a.length(), b.length(), "length");
    assert_eq!(a.channels(), b.channels(), "channels");
    assert_eq!(a.sample_rate_hz(), b.sample_rate_hz(), "sample rate");
    assert_eq!(a.gaps(), b.gaps(), "gaps");
    for channel in 0..a.channels() {
        let ea: Vec<_> = a.transitions(channel, 0, a.length()).unwrap().collect();
        let eb: Vec<_> = b.transitions(channel, 0, b.length()).unwrap().collect();
        assert_eq!(ea, eb, "transitions of channel {channel}");
    }
}

/// The names of the entries of `dir`, sorted.
pub fn entry_names(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

/// How many temporary files of a save are left in `dir`.
pub fn temp_files(dir: &Path) -> usize {
    entry_names(dir)
        .iter()
        .filter(|n| n.ends_with(".lasave-tmp"))
        .count()
}

/// The real file system made slow: every write or read of a buffer takes a few milliseconds, so
/// that a save or load lasts long enough to be watched and cancelled.
#[derive(Clone)]
pub struct SlowFs;

struct SlowSink(Box<dyn WriteSink>);

impl WriteSink for SlowSink {
    fn write_all(&mut self, data: &[u8]) -> io::Result<()> {
        std::thread::sleep(std::time::Duration::from_millis(3));
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
        std::thread::sleep(std::time::Duration::from_millis(3));
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
