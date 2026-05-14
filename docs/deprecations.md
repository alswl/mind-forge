# Deprecations

This document lists CLI usages that are deprecated in this release. Existing
scripts using these forms will continue to work but will emit a stderr warning.

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

## Behavior

- Deprecation warnings are emitted to **stderr only** — they never pollute stdout
  or JSON output.
- Using `--quiet` does not suppress deprecation warnings.
- Using `--no-color` strips ANSI color codes from warnings.
- Multiple warnings per command are each emitted on their own line.
