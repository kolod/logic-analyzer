# Data Model: Workspace and CI Foundation

No persistent user data is introduced. The entities below are in-memory types and one
on-disk format (the log). Spec entities are mapped to their definitions.

## Error (crate `la-core`)

| Field | Type | Notes |
|-------|------|-------|
| `kind` | `ErrorKind` | Closed enum: `Io`, `InvalidInput`, `Corrupt`, `Device`, `Cancelled`, `Resource`, `Internal`. Extended by later features via a new variant (MINOR change). |
| `code` | `&'static str` | Stable dotted identifier (e.g. `io.read`, `input.too_short`). Key for the localization layer; never shown raw to users. |
| `detail` | `String` | English technical description, for the log only. |
| `source` | `Option<Box<dyn Error + Send + Sync>>` | Preserved underlying cause. |

Rules:
- Constructed only through typed constructors (`Error::invalid_input(code, detail)`, …).
- `Display` prints `code` and `detail` (log use); there is no user-message field.
- `Result<T> = core::result::Result<T, Error>`.
- Must be `Send + Sync + 'static` so it can cross worker boundaries.

## LogConfig (crate `la-diagnostics`)

| Field | Type | Default | Validation |
|-------|------|---------|------------|
| `directory` | `PathBuf` | platform log dir | Created if missing; failure → fallback (see below). |
| `max_total_bytes` | `u64` | 8 MiB | Clamped to a minimum of 64 KiB. |
| `segments` | `u8` | 4 | Clamped to 2..=16. |
| `retained_instances` | `u8` | 5 | Clamped to 1..=50. |
| `level` | `Level` | `INFO` (`DEBUG` in debug builds) | Overridable by environment variable. |

## Log entry (on-disk, one JSON object per line)

Format specified in [contracts/log-entry-format.md](contracts/log-entry-format.md).
Fields: `timestamp` (UTC, RFC 3339 with milliseconds), `level`, `target` (component),
`message`, `fields` (object of structured key/values), optional `span`.

## Log store

State machine of the writer:

```text
Segment k active ──(next write would exceed max_total_bytes/segments)──▶
  advance to segment (k+1) mod N, truncate it, make it active
```

Invariants:
- Sum of all segment sizes ≤ `max_total_bytes` at all times.
- A line is never split across segments.
- The newest entries are always retained; the oldest segment is discarded first.
- Fallback when the directory is unusable: write to standard error only, and report
  once (`logging.unavailable` event) — the application continues.

## WorkerFailure (crate `la-diagnostics`)

| Field | Type | Notes |
|-------|------|-------|
| `worker` | `String` | Worker name passed to `spawn_worker`. |
| `message` | `String` | Panic payload rendered as text (non-string payloads → `"<non-string panic payload>"`). |
| `location` | `Option<String>` | `file:line:col` if known. |

Returned by `WorkerHandle::join()` as `Err(WorkerFailure)`; also logged at `ERROR`.

## CI check

Defined in [contracts/ci-checks.md](contracts/ci-checks.md): name, operating system,
command, blocking flag.
