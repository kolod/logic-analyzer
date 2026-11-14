# Contract: Public Rust API (signatures only)

Non-final sketch of the public surface other crates may rely on. Bodies, private
helpers and exact lifetimes are decided during implementation; changes to this surface
follow semver.

## Crate `la-core`

```rust
pub enum ErrorKind { Io, InvalidInput, Corrupt, Device, Cancelled, Resource, Internal }

pub struct Error { /* kind, code, detail, source */ }
impl Error {
    pub fn new(kind: ErrorKind, code: &'static str, detail: impl Into<String>) -> Self;
    pub fn with_source(self, source: impl std::error::Error + Send + Sync + 'static) -> Self;
    pub fn kind(&self) -> ErrorKind;
    pub fn code(&self) -> &'static str;
    pub fn detail(&self) -> &str;
}
impl std::error::Error for Error {}
impl std::fmt::Display for Error {}

pub type Result<T> = core::result::Result<T, Error>;
```

## Crate `la-diagnostics`

```rust
pub struct LogConfig { /* see data-model.md */ }
impl Default for LogConfig {}

pub struct LogGuard; // flushes on drop; keep alive for the program's lifetime

/// Never fails the caller: on an unusable log location it falls back to stderr.
pub fn init_logging(config: LogConfig) -> LogGuard;

/// Installs the process-wide panic hook that logs message, location, thread.
pub fn install_panic_hook();

pub struct WorkerFailure { pub worker: String, pub message: String, pub location: Option<String> }

pub struct WorkerHandle<T> { /* ... */ }
impl<T> WorkerHandle<T> {
    pub fn join(self) -> core::result::Result<T, WorkerFailure>;
}

/// Runs `f` on a named thread; a panic is caught, logged and returned as a failure.
pub fn spawn_worker<T, F>(name: &str, f: F) -> WorkerHandle<T>
where F: FnOnce() -> T + Send + 'static, T: Send + 'static;

/// Locks a std Mutex, recovering (and logging) if it was poisoned.
pub fn lock_recover<T>(m: &std::sync::Mutex<T>) -> std::sync::MutexGuard<'_, T>;
```
