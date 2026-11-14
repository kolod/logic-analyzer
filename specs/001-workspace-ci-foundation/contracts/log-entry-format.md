# Contract: Log Entry Format (version 1)

Log files contain UTF-8 text, one JSON object per line, terminated by `\n`. The file
names are `la-<start-timestamp>-<pid>.<segment>.log`.

## Entry

```json
{"timestamp":"2026-11-14T10:15:30.123Z","level":"ERROR","target":"la_diagnostics::worker","message":"worker panicked","fields":{"worker":"decoder-i2c","panic_message":"index out of bounds","location":"src/lib.rs:42:9"}}
```

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `timestamp` | string | yes | UTC, RFC 3339, millisecond precision. |
| `level` | string | yes | `TRACE`, `DEBUG`, `INFO`, `WARN`, `ERROR`. |
| `target` | string | yes | Producing component (module path). |
| `message` | string | yes | English text, not translated. |
| `fields` | object | yes | Structured key/values; may be empty. |
| `span` | object | no | Current span name and fields, if any. |

## Rules

- A reader MUST ignore unknown fields and MUST skip a line that is not valid JSON
  (it can only be the last line after an abrupt termination).
- Entries MUST NOT contain captured signal payloads, only metadata (sizes, counts,
  identifiers) (FR-019).
- Format changes bump the version; the version is recorded in the first entry of each
  segment (`message` = `log started`, `fields.format_version` = `1`).
- Total size on disk per process never exceeds `LogConfig.max_total_bytes`.
