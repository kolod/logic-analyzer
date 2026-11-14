# Contract: Log Entry Format (version 1)

Log files contain UTF-8 text, one JSON object per line, terminated by `\n`. File names
are `la-<unix-milliseconds, 13 digits>-<pid>-<sequence>.<segment>.log`; the zero-padded
timestamp makes names sort chronologically, and the sequence number keeps several
logging pipelines in one process apart.

## Entry

```json
{"timestamp":"2026-11-14T10:15:30.123456Z","level":"ERROR","target":"la_diagnostics::worker","fields":{"message":"worker panicked","worker":"decoder-i2c","panic_message":"index out of bounds","location":"src/lib.rs:42:9"}}
```

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `timestamp` | string | yes | UTC, RFC 3339, fractional seconds (microseconds). |
| `level` | string | yes | `TRACE`, `DEBUG`, `INFO`, `WARN`, `ERROR`. |
| `target` | string | yes | Producing component (module path or explicit target). |
| `fields` | object | yes | Structured key/values. `fields.message` holds the English message text. |
| `span` / `spans` | object / array | no | Current span(s), if any. |

## Rules

- A reader MUST ignore unknown fields and MUST skip a line that is not valid JSON
  (it can only be the last line after an abrupt termination).
- Entries MUST NOT contain captured signal payloads, only metadata (sizes, counts,
  identifiers) (FR-019).
- Format changes bump the version. The first entry of every segment is
  `fields.message` = `log started` with `fields.format_version` = `1`.
- An entry too large for a segment is replaced by a notice
  (`fields.message` = `log entry too large, dropped`, `fields.dropped_bytes`).
- Total size on disk per process never exceeds `LogConfig.max_total_bytes`.
- If file logging is unavailable (unusable directory, failed write), entries go to
  standard error and a single entry with `fields.code` = `logging.unavailable` and
  `fields.reason` is written to standard error.
