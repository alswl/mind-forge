---
name: mf
description: "Rust CLI for mind 0.3.0-compatible local knowledge repos. Manage projects, directory or file articles, sources, assets, glossary terms, builds, publishing, and repo-wide publishers using mind-format YAML."
---

# mf — Mind Forge CLI

## Overview

`mf` is the Rust implementation of the `mind` CLI for local knowledge repos. Manage content sources, articles, glossary terms, builds, and publication — all controlled by plain files and mind-format YAML.

**Key concepts:**
- **Mind Repo**: A directory rooted at `minds.yaml`. Most commands require running inside one.
- **Project**: A subdirectory with `docs/`, `sources/`, `assets/`, and `mind.yaml`.
- **Index**: `mind-index.yaml` per project — the source of truth for articles, sources, assets, terms, and publish records.
- **Output envelope**: Text by default, JSON with `--json`.

## YAML Format

Use `mind` 0.3.0-compatible YAML as the canonical on-disk format:

- `minds.yaml`: write `schema: "1"` and `projects` as repo-relative path strings.
- `mind.yaml`: write `schema: "1"`; accept both top-level mind fields and wrapped `project:` metadata.
- `mind-index.yaml`: write `schema: "1"` and dictionary sections such as `articles:`, `sources:`, and `assets:` keyed by article slug, source path, or asset name.
- Read compatibility accepts older `schema_version` and list-based mf shapes, but do not prefer them for new files or examples.
- Read-only commands must not rewrite YAML. Mutating commands should preserve mind semantics and avoid silent destructive migrations.

## Global Flags

`--root`, `--config`, `-v`/`--verbose`, `-q`/`--quiet`, `--format <text|json>`, `--json`, `--no-color`, `--install-completion`, `--show-completion`, `-h`/`--help`, `-V`/`--version`

## Commands

### `mf source` — Manage sources
Subcommands: `list`/`ls`, `add`, `update`, `index`, `remove`, `clean`
File kinds: `auto`, `pdf`, `file`, `rss`, `web`

### `mf asset` — Manage assets
Subcommands: `list`/`ls`, `add`, `update`, `index`, `clean`, `remove`

### `mf project` — Manage projects
Subcommands: `new`, `list`/`ls`, `archive`, `status`/`info`, `lint`, `index`, `show`, `import`

### `mf article` — Manage articles
Subcommands: `new`, `list`/`ls`, `lint`, `index`

### `mf term` / `mf terms` — Manage terminology
Subcommands: `list`, `new`, `lint`, `learn`, `fix`, `show`

### `mf build <ARTICLE>` — Build/assemble an article
Flags: `--project`, `--output`, `--dry-run`
`ARTICLE` may be an indexed article name/slug or a repo-relative path prefixed with `@`, such as `@projects/blog/docs/2026-03-review/`. Directory articles are built by merging Markdown files in filename order.

### `mf publish` — Publish articles
Subcommands: `run` (to target like `local`, `yuque-prompt`), `update`

### `mf config` — Manage configuration
Subcommands: `schema`, `show`, `compile` (alias of show), `generate`, `default`, `init`

### `mf publisher` — Manage repo-wide publishers
Subcommands: `list` (with `--json` for diagnostics)

### `mf completion <SHELL>` — Shell completions

### `mf version` — Show version

## Common Workflows

```bash
# Project management
mf project list
mf project new my-project
mf project status --project my-project
mf project lint --project my-project --fix

# Sources & assets
mf source add https://example.com/article --name ref-a --file-kind web --project my-project
mf source list --project my-project
mf asset add image.png --project my-project

# Articles & indexing
mf article new essay "My Essay" --project my-project
mf source index --project my-project
mf article index --project my-project

# Build & publish
mf build my-essay --project my-project
mf build @projects/my-project/docs/2026-03-review/ --output ./_build/review.md
mf publish run "My Essay" --target yuque-prompt --project my-project

# Terms
mf term new "Zettelkasten" --definition "A note-taking method" --project my-project
mf term lint --project my-project --fix

# Config & diagnostics
mf config show
mf publisher list --json
```

## Notes

- Commands that modify project state require a Mind Repo context. `config`, `completion`, `version`, and help can run outside repos.
- Index subcommands reconcile `mind-index.yaml` with the filesystem; run them after manual file changes.
- Prefer `schema` over `schema_version` in docs, examples, and generated YAML.
