# CLI output and safety

JSON commands use `{ "status", "command", "data" }` envelopes. Exit codes are:

| Code | Meaning |
|---|---|
| `0` | Success |
| `1` | Runtime/storage failure |
| `2` | Invalid input or rejected operation |

Global flags: `--root`, `--config`, `-p`/`--project`, `-o`/`--output text|json`,
`--json`, `-q`, `-v`, `--no-color`.

Read-only retrieval does not modify authored files.
For commands that change content, check their confirmation options.
Use `-n`/`--dry-run` to preview supported operations before writing.
Use `-f`/`--force` only where the command supports that safety override.

`-q` silences successful output while preserving diagnostics and exit codes.
Use it for byte-silent automation.
