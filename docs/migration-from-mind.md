# Migration Guide: mind → mf

`mf` aligns its CLI interface with `mind`. Mind users can replace `mind` with
`mf` in their scripts and workflows with minimal changes.

## Quick replacement

For most commands, simply replace the executable name. `article new` is an
exception because `mf` uses a title-first signature and template flag:

```text
# Before (mind)
mind article new blog "My Post"
mind source list --project my-project

# After (mf)
mf article new "My Post" --template blog
mf source list --project my-project
```

## Command mapping table

All 37 mind subcommands are supported in `mf`. Commands marked **(mf extension)**
are mf-only commands with no mind equivalent.

### term/terms

| mind | mf | Notes |
|------|----|-------|
| `terms list` | `term list` or `term ls` | |
| `terms list --term <X>` | `term show <X>` | Deprecated in mind form, use `term show` |
| `terms new` | `term new` (mf extension) | |
| `terms lint` | `term lint [<path>]` | |
| `terms learn --term --alias` | `term learn --term --alias` | |
| `terms learn --original --correct` | `term learn --original --correct` | Deprecated, use `--term --alias` |
| `terms fix` | `term fix` (mf extension) | |

### article

| mind | mf | Notes |
|------|----|-------|
| `article new <TYPE> <TITLE>` | `article new <TYPE> <TITLE>` | |
| `article list` | `article list` or `article ls` | |
| `article lint` | `article lint` | |
| `article index` | `article index` (mf extension) | |

### source

| mind | mf | Notes |
|------|----|-------|
| `source add --source-kind` | `source add --source-kind` | |
| `source add --type` | `source add --type` | Deprecated, use `--source-kind` or `--file-kind` |
| `source list` | `source list` or `source ls` | |
| `source remove <PATH>` | `source remove <PATH>` | |
| `source remove <NAME>` | `source remove <NAME>` | Deprecated, use full PATH |
| `source update` | `source update` (mf extension) | |
| `source index` | `source index` (mf extension) | |
| `source clean` | `source clean` | |

### asset

| mind | mf | Notes |
|------|----|-------|
| `asset add --name` | `asset add --name` | |
| `asset list` | `asset list` or `asset ls` | |
| `asset update --set-url --channel` | `asset update --set-url --channel` | |
| `asset index` | `asset index` (mf extension) | |
| `asset clean` | `asset clean` | |
| `asset remove` | `asset remove` | |

### project

| mind | mf | Notes |
|------|----|-------|
| `project new` | `project new` | |
| `project list` | `project list` or `project ls` | |
| `project show <PROJECT>` | `project show <PROJECT>` or `project info` | |
| `project status` | `project status` | |
| `project lint` | `project lint` | |
| `project index` | `project index` (mf extension) | |
| `project archive <PROJECT>` | `project archive <PROJECT>` | |
| `project import <DIR>` | `project import <DIR>` | |

### config

| mind | mf | Notes |
|------|----|-------|
| `config compile` | `config compile` (alias of `config show`) | |
| `config generate` | `config generate` (alias of `config show`) | |
| `config default` | `config default` | |
| `config schema` | `config schema` | |
| `config init` | `config init` (mf extension) | |

### publish

| mind | mf | Notes |
|------|----|-------|
| `publish update --set` | `publish update --set` | |
| `publish update --status` | `publish update --status` | Deprecated, use `--set status=` |
| `publish update --target-url` | `publish update --target-url` | Deprecated, use `--set url=` |

### Other commands

| mind | mf | Notes |
|------|----|-------|
| `build <ARTICLE>` | `build <ARTICLE>` | |
| `version` | `mf --version` or `mf version` | |

### Global flags

| mind | mf | Notes |
|------|----|-------|
| `--json` | `--json` or `--format json` | |
| `--root <PATH>` | `--root <PATH>` | |
| `--no-color` | `--no-color` | |
| `--install-completion <SHELL>` | `--install-completion <SHELL>` or `completion install` | |
| `--show-completion <SHELL>` | `--show-completion <SHELL>` or `completion show` | |

## What's different from mind

- **Output format**: `mf` uses `--format text|json` (not just `--json`), which is
  more extensible.
- **Completion**: `mf completion install|show|generate` as a subcommand (plus
  global `--install-completion`/`--show-completion` flags).
- **Global flags**: `--config <PATH>`, `--verbose`/`-v`, `--quiet`/`-q` are
  mf extensions.
- **Short flags**: `mf` now supports short flags like `-p` (--project),
  `-t` (--template), `-f` (--force), `-n` (--name), etc.

## See also

- [Deprecations](deprecations.md) — list of deprecated usages
- [mf extensions](mf-extensions.md) — mf-only commands
