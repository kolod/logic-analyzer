//! Logging initialization: structured JSON lines into a size-bounded local file.
//!
//! Logging never prevents the application from starting or running. If the log
//! directory is unusable, or a write fails later, output falls back to standard error
//! and the problem is reported once.

use std::collections::BTreeSet;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::{SystemTime, UNIX_EPOCH};

use tracing::Dispatch;
use tracing_subscriber::fmt::MakeWriter;

use crate::bounded_writer::{BoundedLogWriter, timestamp_now};
use crate::config::LogConfig;

static INSTANCE_SEQ: AtomicU32 = AtomicU32::new(0);

/// Where log output currently goes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LogStatus {
    /// Entries are written to bounded files in `directory`.
    File {
        /// The log directory.
        directory: PathBuf,
    },
    /// File logging is unavailable; entries go to standard error.
    Fallback {
        /// Why file logging is unavailable.
        reason: String,
    },
}

enum Sink {
    File {
        writer: BoundedLogWriter,
        directory: PathBuf,
    },
    Stderr {
        reason: String,
    },
}

impl Sink {
    fn write_record(&mut self, record: &[u8]) {
        let failure = match self {
            Self::File { writer, .. } => writer.write_record(record).err(),
            Self::Stderr { .. } => {
                let _ = io::stderr().write_all(record);
                None
            }
        };
        if let Some(error) = failure {
            let reason = format!("write to the log file failed: {error}");
            report_unavailable(&reason);
            let _ = io::stderr().write_all(record);
            *self = Self::Stderr { reason };
        }
    }

    fn status(&self) -> LogStatus {
        match self {
            Self::File { directory, .. } => LogStatus::File {
                directory: directory.clone(),
            },
            Self::Stderr { reason } => LogStatus::Fallback {
                reason: reason.clone(),
            },
        }
    }
}

fn lock_sink(sink: &Mutex<Sink>) -> MutexGuard<'_, Sink> {
    // A poisoned lock only means another thread panicked while logging; the sink is
    // still usable, and logging must keep working.
    sink.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Reports, once, that file logging is unavailable (stable code `logging.unavailable`).
fn report_unavailable(reason: &str) {
    let reason = reason.replace('\\', "\\\\").replace('"', "\\\"");
    let line = format!(
        "{{\"timestamp\":\"{}\",\"level\":\"WARN\",\"target\":\"la_diagnostics::logging\",\"fields\":{{\"message\":\"file logging unavailable, falling back to stderr\",\"code\":\"logging.unavailable\",\"reason\":\"{reason}\"}}}}\n",
        timestamp_now()
    );
    let _ = io::stderr().write_all(line.as_bytes());
}

#[derive(Clone)]
struct SharedSink(Arc<Mutex<Sink>>);

struct SinkWriter(Arc<Mutex<Sink>>);

impl Write for SinkWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        lock_sink(&self.0).write_record(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl<'a> MakeWriter<'a> for SharedSink {
    type Writer = SinkWriter;

    fn make_writer(&'a self) -> SinkWriter {
        SinkWriter(Arc::clone(&self.0))
    }
}

/// Keeps logging alive; hold it for the lifetime of the program.
#[must_use = "dropping the guard does not stop logging, but it is the handle for status()"]
pub struct LogGuard {
    sink: Arc<Mutex<Sink>>,
}

impl LogGuard {
    /// Where log output currently goes (it can change to `Fallback` at run time).
    #[must_use]
    pub fn status(&self) -> LogStatus {
        lock_sink(&self.sink).status()
    }
}

/// Builds the logging pipeline and returns its guard and a [`Dispatch`], without
/// installing it globally. Tests and embedders use this to scope logging.
///
/// Never fails: an unusable log location results in [`LogStatus::Fallback`].
pub fn build_logging(config: LogConfig) -> (LogGuard, Dispatch) {
    let config = config.normalized();
    let prefix = instance_prefix();

    let sink = match open_writer(&config, &prefix) {
        Ok(writer) => {
            prune_old_instances(&config.directory, &prefix, config.retained_instances);
            Sink::File {
                writer,
                directory: config.directory.clone(),
            }
        }
        Err(error) => {
            let reason = format!(
                "cannot open the log directory {}: {error}",
                config.directory.display()
            );
            report_unavailable(&reason);
            Sink::Stderr { reason }
        }
    };
    let sink = Arc::new(Mutex::new(sink));

    let subscriber = tracing_subscriber::fmt()
        .json()
        .with_max_level(config.level)
        .with_writer(SharedSink(Arc::clone(&sink)))
        .finish();
    (LogGuard { sink }, Dispatch::new(subscriber))
}

/// Installs logging as the process-wide default and returns its guard.
///
/// Never fails the caller: on an unusable log location it falls back to standard
/// error. If a global subscriber is already installed, the new pipeline is not
/// installed and this is reported on standard error.
pub fn init_logging(config: LogConfig) -> LogGuard {
    let (guard, dispatch) = build_logging(config);
    if tracing::dispatcher::set_global_default(dispatch).is_err() {
        let _ = writeln!(
            io::stderr(),
            "logging already initialized; keeping the existing subscriber"
        );
    }
    guard
}

fn open_writer(config: &LogConfig, prefix: &str) -> io::Result<BoundedLogWriter> {
    BoundedLogWriter::open(
        &config.directory,
        prefix,
        usize::from(config.segments),
        config.max_total_bytes,
    )
}

/// `la-<unix milliseconds, 13 digits>-<pid>-<sequence>`: sorts chronologically and is
/// unique per logging pipeline, even inside one process.
fn instance_prefix() -> String {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| u64::try_from(d.as_millis()).unwrap_or(0));
    let seq = INSTANCE_SEQ.fetch_add(1, Ordering::Relaxed);
    format!("la-{millis:013}-{}-{seq}", std::process::id())
}

/// The instance prefix of a log file name, or `None` for files that are not ours.
fn instance_of(file_name: &str) -> Option<&str> {
    if !file_name.starts_with("la-") || !file_name.ends_with(".log") {
        return None;
    }
    file_name.rsplitn(3, '.').nth(2)
}

/// Deletes the logs of the oldest instances so that at most `retained` remain,
/// counting the current one. Files that are not instance logs are never touched.
/// Failures are ignored: housekeeping must never block start-up.
fn prune_old_instances(dir: &Path, current: &str, retained: u8) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let names: Vec<String> = entries
        .filter_map(Result::ok)
        .filter_map(|e| e.file_name().into_string().ok())
        .collect();
    let others: BTreeSet<&str> = names
        .iter()
        .filter_map(|n| instance_of(n))
        .filter(|p| *p != current)
        .collect();
    let keep = usize::from(retained).saturating_sub(1);
    let excess = others.len().saturating_sub(keep);
    for prefix in others.into_iter().take(excess) {
        let dotted = format!("{prefix}.");
        for name in names.iter().filter(|n| n.starts_with(&dotted)) {
            let _ = std::fs::remove_file(dir.join(name));
        }
    }
}
