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

## Project layout migration

`mf` introduces a canonical `layout` block in `mind.yaml` that replaces the
historical `paths` and `build.output_dir` fields with a unified directory map.

### Default layout

When no `layout` is configured, effective defaults are:

```yaml
layout:
  articles: docs
  sources: sources
  assets: assets
  templates: templates
  build_output: outputs
```

### Compatibility

Historical fields are still accepted and map to the effective layout:

| Historical field | Effective layout category |
|------------------|---------------------------|
| `paths.docs` | `layout.articles` |
| `paths.sources` | `layout.sources` |
| `paths.assets` | `layout.assets` |
| `build.output_dir` | `layout.build_output` |

**Precedence**: Canonical `layout` values win over historical fields.
Conflicts produce a diagnostic naming both fields.

### Migration steps

1. **Inspect** the current effective layout: `mf config show --output-format json`
2. **Add** a `layout` block to `mind.yaml` with your desired directory names
3. **Move** existing files to the new directories manually — layout changes never
   move, delete, or rewrite existing user files
4. **Run** commands like `mf article new`, `mf build`, etc. — they will use the
   new directories

### What's not in layout

- **Terms**: Stored in `minds-terms.yaml` (project or repo level), not a layout directory
- **Archive**: Out of scope; existing `paths.archive` is unchanged but not a
  layout category
- **Publish**: Post-build action, not a layout directory

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
