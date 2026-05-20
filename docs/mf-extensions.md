# mf extensions

These commands are mf-only — they have no equivalent in the `mind` CLI. They
provide functionality specific to mf's document-as-code model.

## Index commands

Index commands reconcile the filesystem state with the project index
(`mind-index.yaml`). They detect new, removed, and changed files.

- **`article index`** — scan `docs/` directory and update article index
- **`asset index`** — scan assets directory and update asset index
- **`project index`** — scan projects directory and update top-level manifest
- **`source index`** — scan sources directory and update source index

## Config

- **`config init`** — generate a `mind.yaml` configuration file in the current
  or specified directory
- **`config show`** — display the effective merged configuration (the primary
  target of `config compile`/`generate`/`default` aliases)

## Source

- **`source update <NAME> --rename <NEW> --url <URL>`** — rename a source
  or update its URL

## Term

- **`term new <TERM> --definition <DEF> --alias <ALIAS>`** — create a new
  glossary term entry
- **`term fix <TERM> --definition <DEF> --alias <ALIAS> --tag <TAG>`** —
  update metadata on an existing term

## Global flags

- **`--config <PATH>`** — specify a config file path
- **`--verbose` / `-v`** — increase log verbosity
- **`--quiet` / `-q`** — suppress non-essential output

## Article `new` — template system

The `mf article new` command has been redesigned from type-first to title-first:

```text
mf article new <TITLE> [--template <S>] [--file|--single-file]
```

**Default behaviour**: Creates a **directory article** at `docs/{slug}/` using
the `blank` template (H1 + creation date only, no H2 sections).

**`--template <S>`**: Accepts a built-in schema name or a path under the project root.

| Value | Behaviour |
|-------|-----------|
| `blank` (default) | Minimal scaffold — H1 + creation date, no H2 sections |
| `arch` | Architecture Decision Record: `## Context` / `## Decision` / `## Consequence` / `## Alternatives Considered` |
| `prd` | Product Requirements Document: `## Background` / `## Goals` / `## Non-Goals` / `## Requirements` |
| `blog` | Blog post: `## Summary` / `## Content` |
| `<path>` | Custom template read from `{project}/<path>`. Recorded as `article_type: blank`. |

When a value matches both a built-in name and a file path, the **built-in wins**.
Use a leading `./` or subdirectory prefix (e.g. `templates/arch.md`) to force
path resolution.

**Directory-article H2 split rule**: In directory mode, the resolved template
is split on top-level `## ` lines: `01-opening.md` contains everything before the
first H2; each subsequent H2 section becomes `0N-<heading-slug>.md` (2-indexed,
zero-padded to two digits). The concatenation of all block files in filename
order reproduces the template byte-for-byte.

**`--file` / `--single-file`**: Write a single file `docs/{slug}.md` instead
of a directory. No H2 split occurs. Compatible with all template sources.

**JSON envelope**: Four new fields under `data`:
- `template` (`string`) — resolved name (`blank`/`arch`/`prd`/`blog`) or relative path
- `shape` (`"directory"` | `"file"`) — output form
- `path` (`string`) — `docs/{slug}/` (directory mode, trailing slash) or `docs/{slug}.md`
- `files` (`array<string>`) — block filenames in write order

The legacy `data.type` field is removed; use `data.template` instead.
