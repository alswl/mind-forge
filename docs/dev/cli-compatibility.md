# CLI compatibility during Go layering

The comparison baseline is the Go implementation at `6c4344c`, before the
command and package layering refactor. The command catalogue moves from
`internal/cli/spec_data.json` to `cmd/mf/spec_data.json` without a byte change.

| Surface | Before | After | Added / removed / modified |
| --- | ---: | ---: | --- |
| Command nodes | 118 | 118 | 0 / 0 / 0 |
| Leaf commands | 99 | 99 | 0 / 0 / 0 |
| Canonical and alias invocation paths | 149 | 149 | 0 / 0 / 0 |
| Command alias declarations | 13 | 13 | 0 / 0 / 0 |
| Global flags including help/version | 11 | 11 | 0 / 0 / 0 |
| Local flag definitions | 277 | 277 | 0 / 0 / 0 |
| Positional argument definitions | 88 | 88 | 0 / 0 / 0 |
| Flag alias declarations | 1 | 1 | 0 / 0 / 0 |

Global flag counts include the generated `--help/-h` and `--version/-V`.
Local definitions and positional arguments count each declaration in its command
context; repeated flag names across commands are intentionally counted again.

Both catalogues have SHA-256 `94b763c7843a939bdc7d5d28d0908f541550729323f527de51d0cc2023c5dcd1`.
This compares command paths, aliases, positional arity/requirements, flag names,
short forms, defaults, choices, repeatability, conflicts, dependencies, repository
requirements, access classification and frozen help text. Direct parser component
checks and the independent external public-parser cases verify construction and
actual execution; catalogue equality alone is not a runtime acceptance claim.

The eighteen root families remain `init`, `project`, `article`, `mind`,
`additional`, `lifecycle`, `output`, `prompt`, `thinking`, `source`, `asset`,
`term` (alias `terms`), `build`, `publish`, `render`, `config`, `completion`,
and `version`.

## Skill reference corrections

Skill updates describe the already-existing CLI; they do not change its surface:

- Source builds use Go 1.26.11 and Make; development checks add the pinned linter.
- `mf init` initializes repositories. `mf config init` is not a command.
- `mf config generate --out PATH` selects the destination; `-o/--output` still
  selects the response format.
- Noninteractive term fixes use `--yes`; `--force` is not exposed on those commands.
  Other destructive commands retain their existing command-specific confirmation
  flags and exit codes; agents must not invent a universal flag policy.
- Version data already exposes Go compiler metadata and retains `rustc` as
  `"not applicable"`. Commit/build-date values naturally identify each candidate.
- The skill fixture preparer keeps `--workspace`; it now reads the selected
  binary from workspace `bin/mf` and validates the hash frozen in the run manifest.
  It no longer selects a retired Rust build path or a source-embedded old hash.

No Viper, configuration path/precedence, storage format or normal CLI I/O migration
is introduced. Operational cancellation and debug logging are the separately
requested behavior changes; they do not add or rename commands or arguments.
