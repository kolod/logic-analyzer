//! The file-system seam: everything the session code does to the disk goes through
//! [`FileSystem`], so tests can inject a failure at every step (see `fault.rs`).

use std::ffi::OsString;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::Path;
use std::time::{Duration, SystemTime};

/// What a path is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PathKind {
    /// A regular file.
    File,
    /// A directory.
    Directory,
    /// Anything else (a device, a socket, ...).
    Other,
}

/// One entry of a directory listing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DirEntryInfo {
    /// The file name.
    pub name: OsString,
    /// `true` for a regular file.
    pub is_file: bool,
    /// Time since the last modification, if known.
    pub age: Option<Duration>,
}

/// A file opened for writing.
pub trait WriteSink: Send {
    /// Writes all of `data`.
    ///
    /// # Errors
    ///
    /// Any I/O error of the underlying write.
    fn write_all(&mut self, data: &[u8]) -> io::Result<()>;

    /// Flushes buffered data to the operating system.
    ///
    /// # Errors
    ///
    /// Any I/O error of the underlying flush.
    fn flush(&mut self) -> io::Result<()>;

    /// Forces the data to stable storage.
    ///
    /// # Errors
    ///
    /// Any I/O error of the underlying sync.
    fn sync_all(&mut self) -> io::Result<()>;
}

/// The operations of the session code on the file system.
pub trait FileSystem: Send + Sync {
    /// Creates a new file, failing with `AlreadyExists` if the name is taken.
    ///
    /// # Errors
    ///
    /// The I/O error of the creation.
    fn create_exclusive(&self, path: &Path) -> io::Result<Box<dyn WriteSink>>;

    /// Opens a file for reading and reports its length at that moment.
    ///
    /// # Errors
    ///
    /// The I/O error of the open.
    fn open_read(&self, path: &Path) -> io::Result<(Box<dyn Read + Send>, u64)>;

    /// Atomically moves `from` over `to`, replacing it.
    ///
    /// # Errors
    ///
    /// The I/O error of the rename.
    fn rename(&self, from: &Path, to: &Path) -> io::Result<()>;

    /// Removes a file.
    ///
    /// # Errors
    ///
    /// The I/O error of the removal.
    fn remove_file(&self, path: &Path) -> io::Result<()>;

    /// Makes a rename inside `dir` durable where the platform allows it (no effect on Windows,
    /// where the standard library cannot synchronise a directory).
    ///
    /// # Errors
    ///
    /// The I/O error of the synchronisation.
    fn sync_dir(&self, dir: &Path) -> io::Result<()>;

    /// Lists a directory.
    ///
    /// # Errors
    ///
    /// The I/O error of the listing.
    fn list_dir(&self, dir: &Path) -> io::Result<Vec<DirEntryInfo>>;

    /// What `path` is, or `None` if it does not exist.
    ///
    /// # Errors
    ///
    /// An I/O error other than "not found".
    fn kind(&self, path: &Path) -> io::Result<Option<PathKind>>;
}

/// The real file system.
#[derive(Debug, Default, Clone, Copy)]
pub struct RealFileSystem;

struct RealSink(File);

impl WriteSink for RealSink {
    fn write_all(&mut self, data: &[u8]) -> io::Result<()> {
        self.0.write_all(data)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.0.flush()
    }

    fn sync_all(&mut self) -> io::Result<()> {
        self.0.sync_all()
    }
}

impl FileSystem for RealFileSystem {
    fn create_exclusive(&self, path: &Path) -> io::Result<Box<dyn WriteSink>> {
        let file = OpenOptions::new().write(true).create_new(true).open(path)?;
        Ok(Box::new(RealSink(file)))
    }

    fn open_read(&self, path: &Path) -> io::Result<(Box<dyn Read + Send>, u64)> {
        let file = File::open(path)?;
        let length = file.metadata()?.len();
        Ok((Box::new(file), length))
    }

    fn rename(&self, from: &Path, to: &Path) -> io::Result<()> {
        fs::rename(from, to)
    }

    fn remove_file(&self, path: &Path) -> io::Result<()> {
        fs::remove_file(path)
    }

    #[cfg(unix)]
    fn sync_dir(&self, dir: &Path) -> io::Result<()> {
        File::open(dir)?.sync_all()
    }

    #[cfg(not(unix))]
    fn sync_dir(&self, _dir: &Path) -> io::Result<()> {
        // The standard library cannot open a directory for synchronisation on Windows. The
        // rename is still atomic; only its durability across a power failure is not forced.
        Ok(())
    }

    fn list_dir(&self, dir: &Path) -> io::Result<Vec<DirEntryInfo>> {
        let now = SystemTime::now();
        let mut entries = Vec::new();
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let metadata = entry.metadata().ok();
            entries.push(DirEntryInfo {
                name: entry.file_name(),
                is_file: metadata.as_ref().is_some_and(fs::Metadata::is_file),
                age: metadata
                    .and_then(|m| m.modified().ok())
                    .and_then(|modified| now.duration_since(modified).ok()),
            });
        }
        Ok(entries)
    }

    fn kind(&self, path: &Path) -> io::Result<Option<PathKind>> {
        match fs::metadata(path) {
            Ok(m) if m.is_file() => Ok(Some(PathKind::File)),
            Ok(m) if m.is_dir() => Ok(Some(PathKind::Directory)),
            Ok(_) => Ok(Some(PathKind::Other)),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e),
        }
    }
}

/// A scratch directory for unit tests, removed on drop.
#[cfg(test)]
pub(crate) struct TempDir(pub(crate) std::path::PathBuf);

#[cfg(test)]
impl TempDir {
    pub(crate) fn new(name: &str) -> Self {
        use std::sync::atomic::{AtomicU64, Ordering};
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let unique = COUNTER.fetch_add(1, Ordering::Relaxed);
        let path =
            std::env::temp_dir().join(format!("la-session-{name}-{}-{unique}", std::process::id()));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }

    pub(crate) fn path(&self) -> &Path {
        &self.0
    }
}

#[cfg(test)]
impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[cfg(test)]
#[allow(clippy::arithmetic_side_effects)]
mod tests {
    use super::*;

    #[test]
    fn exclusive_creation_writes_syncs_and_refuses_an_existing_name() {
        let dir = TempDir::new("excl");
        let fs = RealFileSystem;
        let path = dir.path().join("a.tmp");
        let mut sink = fs.create_exclusive(&path).unwrap();
        sink.write_all(b"hello ").unwrap();
        sink.write_all(b"world").unwrap();
        sink.flush().unwrap();
        sink.sync_all().unwrap();
        drop(sink);
        assert_eq!(std::fs::read(&path).unwrap(), b"hello world");
        let again = fs.create_exclusive(&path).err().unwrap();
        assert_eq!(again.kind(), io::ErrorKind::AlreadyExists);
        assert_eq!(
            std::fs::read(&path).unwrap(),
            b"hello world",
            "the file is untouched"
        );
    }

    #[test]
    fn rename_replaces_an_existing_file_completely() {
        let dir = TempDir::new("rename");
        let fs = RealFileSystem;
        let target = dir.path().join("target");
        std::fs::write(&target, b"old old old old").unwrap();
        let temp = dir.path().join("target.tmp");
        std::fs::write(&temp, b"new").unwrap();
        fs.rename(&temp, &target).unwrap();
        assert_eq!(std::fs::read(&target).unwrap(), b"new");
        assert!(!temp.exists());
        fs.sync_dir(dir.path()).unwrap();
    }

    #[test]
    fn reading_reports_the_length_and_removal_and_listing_work() {
        let dir = TempDir::new("list");
        let fs = RealFileSystem;
        let path = dir.path().join("f");
        std::fs::write(&path, b"12345").unwrap();
        std::fs::create_dir(dir.path().join("sub")).unwrap();
        let (mut reader, length) = fs.open_read(&path).unwrap();
        assert_eq!(length, 5);
        let mut text = String::new();
        reader.read_to_string(&mut text).unwrap();
        assert_eq!(text, "12345");

        let mut entries = fs.list_dir(dir.path()).unwrap();
        entries.sort_by(|a, b| a.name.cmp(&b.name));
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].name, "f");
        assert!(entries[0].is_file && !entries[1].is_file);
        assert!(
            entries[0]
                .age
                .is_some_and(|age| age < Duration::from_secs(60))
        );

        assert_eq!(fs.kind(&path).unwrap(), Some(PathKind::File));
        assert_eq!(fs.kind(dir.path()).unwrap(), Some(PathKind::Directory));
        assert_eq!(fs.kind(&dir.path().join("missing")).unwrap(), None);
        fs.remove_file(&path).unwrap();
        assert_eq!(fs.kind(&path).unwrap(), None);
        assert_eq!(
            fs.remove_file(&path).unwrap_err().kind(),
            io::ErrorKind::NotFound
        );
        assert_eq!(
            fs.open_read(&path).err().unwrap().kind(),
            io::ErrorKind::NotFound
        );
    }
}
