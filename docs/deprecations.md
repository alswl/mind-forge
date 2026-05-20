# Deprecations

This document lists CLI usages that are deprecated in this release. Existing
scripts using these forms will continue to work but will emit a stderr warning.

## Breaking changes

### BC1: `mf article new <TYPE> <TITLE>` positional removed

| | |
|---|---|
| **Old form** | `mf article new <TYPE> <TITLE>` (e.g. `mf article new blog "Hello"`) |
| **New form** | `mf article new <TITLE> [--template <S>] [--file|--single-file]` (e.g. `mf article new "Hello" --template blog`) |
| **Error** | Passing two positionals exits 2 with a usage message naming the new signature and pointing at `--template` |
| **Migration** | `mf article new arch "T"` → `mf article new "T" --template arch` |
| **JSON envelope** | The legacy `data.type` field is removed; use `data.template` instead |

## Deprecation classes

### D1a: `publish update --status <VALUE>`

| | |
|---|---|
| **Old form** | `mf publish update <ARTICLE> --target <TARGET> --status <VALUE>` |
| **New form** | `mf publish update <ARTICLE> --target <TARGET> --set status=<VALUE>` |
| **Warning** | `[deprecated] --status is deprecated, use --set status=<value> instead` |

### D1b: `publish update --target-url <URL>`

| | |
|---|---|
| **Old form** | `mf publish update <ARTICLE> --target <TARGET> --target-url <URL>` |
| **New form** | `mf publish update <ARTICLE> --target <TARGET> --set url=<URL>` |
| **Warning** | `[deprecated] --target-url is deprecated, use --set url=<value> instead` |

### D2: `source add --type <VALUE>`

| | |
|---|---|
| **Old form** | `mf source add <INPUT> --type auto\|pdf\|rss\|web\|file` |
| **New form** | `mf source add <INPUT> --source-kind yuque\|meeting\|misc` or `--file-kind auto\|pdf\|file\|rss\|web` |
| **Warning** | `[deprecated] --type is deprecated, use --file-kind or --source-kind instead` |

### D3: `source remove <NAME>` (positional NAME)

| | |
|---|---|
| **Old form** | `mf source remove <NAME>` |
| **New form** | `mf source remove <PATH>` (e.g. `sources/yuque/my-doc.md`) |
| **Warning** | `[deprecated] positional NAME is deprecated, use full PATH (e.g., sources/yuque/foo.md) instead` |

### D4a: `term learn --original <VALUE>`

| | |
|---|---|
| **Old form** | `mf term learn --original <variant> --correct <canonical>` |
| **New form** | `mf term learn --term <canonical> --alias <variant>` |
| **Warning** | `[deprecated] --original is deprecated, use --alias <variant> instead` |

### D4b: `term learn --correct <VALUE>`

| | |
|---|---|
| **Old form** | `mf term learn --original <variant> --correct <canonical>` |
| **New form** | `mf term learn --term <canonical> --alias <variant>` |
| **Warning** | `[deprecated] --correct is deprecated, use --term <canonical> instead` |

### D5: `term list --term <NAME>`

| | |
|---|---|
| **Old form** | `mf term list --term <NAME>` |
| **New form** | `mf term show <NAME>` |
| **Warning** | `[deprecated] term list --term <X> is deprecated, use term show <X> instead` |

## CLI Command UX Review (2026-05-20)

### D6: Top-level `mf publisher` removed

| | |
|---|---|
| **Old form** | `mf publisher list` |
| **New form** | `mf publish target list` |
| **Behavior** | The old form is unrecognized (unrecognized subcommand error). Update scripts immediately. |

### D7: `mf project status` → `mf project show`

| | |
|---|---|
| **Old form** | `mf project status [-p PROJECT]` |
| **New form** | `mf project show <PROJECT>` |
| **Behavior** | `project status` remains a hidden compatibility alias. Warning: `[deprecated] project status is deprecated, use project show instead` |
| **Removal** | The `project info` alias is removed immediately (unrecognized subcommand). |

### D8: `mf term learn` → `mf term add`

| | |
|---|---|
| **Old form** | `mf term learn --term <C> --alias <V>` |
| **New form** | `mf term add --term <C> --alias <V>` |
| **Behavior** | `term learn` remains a hidden compatibility alias. Warning: `[deprecated] term learn is deprecated, use term add instead` |

### D9: `mf term fix` → `mf term update`

| | |
|---|---|
| **Old form** | `mf term fix <TERM> [OPTIONS]` |
| **New form** | `mf term update <TERM> [OPTIONS]` |
| **Behavior** | `term fix` remains a hidden compatibility alias. Warning: `[deprecated] term fix is deprecated, use term update instead` |

### D10: `mf config compile` deprecated

| | |
|---|---|
| **Old form** | `mf config compile [OPTIONS]` |
| **New form** | `mf config show [OPTIONS]` |
| **Behavior** | `config compile` remains a hidden compatibility alias. Warning: `[deprecated] config compile is deprecated, use config show instead` |

### D11: Global `--install-completion` and `--show-completion` deprecated

| | |
|---|---|
| **Old form** | `mf --install-completion bash` or `mf --show-completion bash` |
| **New form** | `mf completion bash` |
| **Warning** | `[deprecated] --install-completion is deprecated, use mf completion instead` |

### D12: `mf config init` deprecated

| | |
|---|---|
| **Old form** | `mf config init [OPTIONS]` |
| **New form** | `mf init [PATH]` |
| **Warning** | `[deprecated] config init is deprecated, use mf init instead` |

## Behavior

- Deprecation warnings are emitted to **stderr only** — they never pollute stdout
  or JSON output.
- Using `--quiet` does not suppress deprecation warnings.
- Using `--no-color` strips ANSI color codes from warnings.
- Multiple warnings per command are each emitted on their own line.
- Hidden compatibility aliases are invisible in help output but remain functional
  with stderr warnings.
