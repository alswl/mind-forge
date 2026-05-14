---
name: mf
description: "Local-first CLI for card-based writing and document-as-code knowledge management. Manage projects, articles, sources, assets, glossary terms, builds, and publishing."
---

# mf — Mind Forge CLI

## Overview

`mf` is a personal knowledge management toolkit that treats your writing as a codebase. Articles are composed from reusable Blocks, documents are managed as code, and everything stays on your machine.

**Key concepts:**
- **Mind Repo**: A directory tree rooted at `minds.yaml`. All commands (except `config`, `completion`, `version`) require running inside a mind repo.
- **Project**: A subdirectory under the repo containing `docs/`, `sources/`, `assets/`, and `mind.yaml`.
- **Index**: `mind-index.yaml` per project — the single source of truth for filesystem state. Reconcile with `index` subcommands.
- **Block**: An atomic unit of content (quote, thought, code snippet). Articles are assembled from blocks.
- **Output envelope**: All commands output either text (default) or JSON with `--json` flag.

## Global Flags

| Flag | Description |
|------|-------------|
| `--root <PATH>` | Mind repo root directory |
| `--config <PATH>` | Config file path |
| `-v` / `--verbose` | Verbose output (counted) |
| `-q` / `--quiet` | Silence non-error output |
| `--format <FORMAT>` | `text` (default) or `json` |
| `--json` | Shorthand for `--format json` |
| `--no-color` | Disable colored output |

## Command Reference

### `mf source` — Manage source materials

Sources are input materials (files, URLs, RSS feeds, PDFs) that feed into articles.

| Subcommand | Description | Key Flags |
|------------|-------------|-----------|
| `list` / `ls` | List sources | `--filter`, `--type`, `--project` |
| `add <INPUT>` | Add a source (file or URL) | `--name`, `--file-kind`, `--source-kind`, `--link`, `--force`, `--project` |
| `update <NAME>` | Rename or change URL of a source | `--rename`, `--url`, `--project` |
| `index` | Scan `sources/` dir and reconcile index | `--dry-run`, `--project` |
| `remove <PATH>` | Remove a source from index | `--keep-file`, `--project` |
| `clean` | Remove stale index entries | `--dry-run`, `--project` |

Source file kinds: `auto`, `pdf`, `file`, `rss`, `web`
Source channel types: `yuque`, `meeting`, `misc`

### `mf asset` — Manage media assets

Assets are media files (images, video, audio) attached to projects.

| Subcommand | Description | Key Flags |
|------------|-------------|-----------|
| `list` / `ls` | List assets | `--filter`, `--type`, `--project` |
| `add <PATH>` | Add an asset | `--name`, `--tag`, `--copy`/`--link`, `--force`, `--project` |
| `update [PATH]` | Update asset metadata | `--all`, `--set-url`, `--channel`, `--project` |
| `index` | Index assets from filesystem | `--dry-run`, `--refresh-metadata`, `--project` |
| `clean` | Remove stale asset index entries | `--dry-run`, `--project` |
| `remove <FILE>` | Remove an asset | `--force`, `--project` |

### `mf project` — Manage projects

Projects are writing containers with their own config, docs, sources, and assets.

| Subcommand | Description | Key Flags |
|------------|-------------|-----------|
| `new <NAME>` | Scaffold a new project | `--template`, `--force` |
| `list` / `ls` | List all projects | — |
| `status` / `info` | Show project snapshot | `--project` |
| `lint` | Lint a project for issues | `--project`, `--fix`, `--rule` |
| `index` | Reconcile `minds.yaml` with filesystem | `--dry-run` |
| `show <PROJECT>` | Show project details | — |
| `archive <NAME>` | Archive project to `_archived/` | — |
| `import <DIR>` | Import a directory as a project | `--type`, `--source`, `--assets`, `--force`, `--non-interactive` |

Lint rules: `missing_directory`, `stale_index_entry`, `name_convention`, `missing_manifest`

### `mf article` — Manage articles

Articles are your core writing units, composed from blocks and assembled via build.

| Subcommand | Description | Key Flags |
|------------|-------------|-----------|
| `new <TYPE> <TITLE>` | Create a new article | `--project`, `--template`, `--tag`, `--draft` (default true), `--force` |
| `list` / `ls` | List articles | `--project` |
| `lint` | Lint articles for issues | `--fix` |
| `index` | Scan `docs/` and reconcile index | `--dry-run`, `--project` |

### `mf term` — Manage glossary terms

Terms enforce consistent terminology across your writing.

| Subcommand | Description | Key Flags |
|------------|-------------|-----------|
| `list` / `ls` | List terms | `--filter`, `--project` |
| `new <TERM>` | Create a glossary term | `--definition`, `--alias`, `--tag`, `--project` |
| `lint [<path>]` | Check document term consistency | `--fix`, `--dry-run`, `--project` |
| `learn` | Learn a term correction | `--term`, `--alias`, `--project` |
| `fix <TERM>` | Update existing term metadata | `--definition`, `--alias`, `--tag`, `--project` |
| `show <NAME>` | Show term details | `--project` |

### `mf build` — Assemble articles

Builds an article from its constituent blocks.

| Subcommand | Description | Key Flags |
|------------|-------------|-----------|
| `build <ARTICLE>` | Build/assemble an article | `--project`, `--output`, `--dry-run` |

`--dry-run` shows a build plan: input sources, merge order, estimated size.

### `mf publish` — Publish articles

Publish articles to configured targets and track publication records.

| Subcommand | Description | Key Flags |
|------------|-------------|-----------|
| `run <ARTICLE>` | Publish an article to a target (resolves repo-wide publishers first, falls back to mind.yaml targets) | `--target`, `--project`, `--dry-run`, `--force` |
| `update <ARTICLE>` | Update a publish record in index | `--target`, `--set KEY=VALUE`, `--project`, `--dry-run` |

Supported targets: `local`, `yuque-prompt`

When `--target <NAME>` is specified, the resolution order is:
1. `.mind-forge/publisher/<name>.yaml` (repo-wide publisher)
2. `mind.yaml publish.targets[]` (project-local target)

### `mf publisher` — Manage repo-wide publishers

Define and diagnose content delivery channels via `.mind-forge/publisher/*.yaml` files at the Mind Repo root.

| Subcommand | Description | Key Flags |
|------------|-------------|-----------|
| `list` | List available publishers and diagnostics | `--format`, `--json` |

Publisher YAML schema (placed under `.mind-forge/publisher/<name>.yaml`):

```yaml
name: my-channel       # optional, defaults to filename stem
label: My Channel      # optional display name
description: ...       # optional description
type: local            # required: local | yuque-prompt
enabled: true          # optional, defaults to true
config:                # type-specific configuration
  path: ./output       # local type requires config.path
required_inputs: []    # declared non-secret inputs only
```

Relative paths in `config` resolve from the Mind Repo root. Do not store secrets in YAML — use environment variables or CLI parameters instead.

### `mf config` — Manage configuration

Configuration merges three layers: defaults → `mf.yaml` → `mind.yaml`.

| Subcommand | Description | Key Flags |
|------------|-------------|-----------|
| `schema` | Show JSON Schema | `--output-format` (json/yaml) |
| `show` | Show effective merged config | `--output-format` |
| `compile` | Alias of `show` | — |
| `generate` | Show + write config to file | `--output-format`, `--output` |
| `default` | Show default config values | `--output-format` |
| `init` | Initialize config file | `--output`, `--target` (project/user), `--force` |

### `mf completion` — Shell completions

```bash
mf completion <SHELL>    # bash, zsh, fish, powershell, elvish
mf --install-completion <SHELL>
mf --show-completion <SHELL>
```

### `mf version`

Show version information.

## Workflows

### Daily Writing Workflow

```bash
# 1. List your projects
mf project list

# 2. Check project status
mf project status --project my-project

# 3. Add a source (research material)
mf source add https://example.com/article --name "reference-article" --file-kind web --project my-project

# 4. Create a new article from a template
mf article new essay "My New Essay" --project my-project

# 5. Index any new files
mf source index --project my-project
mf article index --project my-project

# 6. Build the article to see the assembled output
mf build "My New Essay" --project my-project --output ./_build
```

### Research & Source Management

```bash
# Add a PDF as a research source
mf source add paper.pdf --name "research-paper" --file-kind pdf --project my-project

# Index all sources in a project
mf source index --project my-project

# List all sources with a filter
mf source list --filter "pdf" --project my-project

# Clean stale source entries
mf source clean --project my-project --dry-run
```

### Glossary & Terminology

```bash
# Create a term with definition and alias
mf term new "Zettelkasten" --definition "A note-taking method..." --alias "slip-box" --project my-project

# Check term consistency across documents
mf term lint --project my-project

# Auto-fix inconsistencies
mf term lint --project my-project --fix

# Learn a correction
mf term learn --term "Zettelkasten" --alias "zettelkasten" --project my-project
```

### Project Lifecycle

```bash
# Create a new project
mf project new my-article-series --template standard

# Import existing content as a project
mf project import ./my-notes --type essay --source --assets --force

# Lint the project
mf project lint --project my-article-series --fix

# Archive when done
mf project archive my-article-series
```

### Publishing

```bash
# List available publishers
mf publisher list

# List publishers with diagnostics (JSON)
mf publisher list --json

# Publish to a repo-wide publisher
mf publish run "My New Essay" --target my-channel --project my-project

# Publish to local output
mf publish run "My New Essay" --target local --project my-project

# Dry-run to preview
mf publish run "My New Essay" --target yuque-prompt --dry-run --project my-project
```

### Diagnostics

```bash
# Verbose output
mf source list -vv --project my-project

# JSON output for scripting
mf project list --json

# Show effective config
mf config show --output-format yaml

# Initialize a new config
mf config init --target project
```

## Index Reconciliation Pattern

The `index` subcommands are central to mf's "document as code" model. They reconcile what's on disk with the `mind-index.yaml` state file. This is needed after:
- Adding files manually (outside of `mf add`)
- Removing files manually
- Renaming files
- Syncing via git

Run the relevant index command after any manual filesystem operation.

## Exit Codes

| Code | Meaning |
|------|---------|
| 0 | Success |
| 1 | General error (NotInMindRepo, NotFound, ParseError, etc.) |
| 2 | Usage error (invalid input) |
| 3 | Not implemented |
