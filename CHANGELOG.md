# Changelog

## 029 (2026-05-19)

### Features

- **version**: add `mf version` command with text (`mf <version>`) and JSON envelope output, works without a Mind Repo, and is fully read-only
- **release workflow**: add version tag validation (`v<MAJOR>.<MINOR>.<PATCH>` with optional prerelease/build metadata)
- **release workflow**: releases are now created as GitHub Releases drafts for maintainer review before publishing
- **release workflow**: add explicit duplicate release handling (`allowUpdates`) and minimum `contents: write` permissions

## 028 (2026-05-19)

### Breaking Changes

- **article new**: the `<TYPE>` positional is removed. The new signature is `mf article new <TITLE> [--template <S>] [--file]`. Migration: `mf article new arch "T"` → `mf article new "T" --template arch`. The legacy `data.type` JSON envelope field is replaced by `data.template`.

### Features

- **article new**: default to `blank` template and directory-article output shape. `mf article new "X"` creates `docs/x/00-head.md` from the minimal blank scaffold.
- **article new**: `--template <S>` accepts built-in names (`blank`, `arch`, `prd`, `blog`) or a project-root-relative path to a custom template file. Built-in names win on name clash.
- **article new**: `--file` opts into the single-file output shape `docs/{slug}.md` for compatibility with legacy workflows.
- **article new**: directory articles are split on `## ` headings — `00-head.md` + one block file per H2 section.
- **article new**: atomic group write for directory articles — N+1 files committed via a single `rename`.
- **article new**: JSON envelope gains four fields (`data.template`, `data.shape`, `data.path`, `data.files`); legacy `data.type` is removed.
- **article new**: three new error kinds — `unknown_template`, `duplicate_block_slug`, `shape_conflict`.

## 024 (2026-05-18)

### Bug Fixes

- **publish**: fix generated-article identity in `mind-index.yaml` — article keys now use `<template-name>/<slot-value>` instead of derived source-path keys
- **publish**: fix artifact resolution for generated articles — `mf publish run` reads the canonical `source_path` from the index entry instead of string-joining the CLI argument
- **publish**: fix declared-article index gaps — `build.articles.<id>` (typed) and compat `articles.<id>` entries are now discoverable in `mf article list` and publishable via `mf publish run <id>`

### Features

- **article list**: add `id`, `origin` (`declared`|`docs`|`generated`), and `source_present` fields to JSON output; text mode appends `(missing source)` marker
- **errors**: add `build_artifact_missing` error kind (renamed from generic `not_found` for missing-build-artifact cases)
- **errors**: add `no_source_files` error kind for declared articles with missing source files
- **errors**: all variants now carry user-facing hints

### Re-verified

- **BUG-4**: date path-template expansion and `prefix` rendering confirmed working for both generated and docs articles on local publish targets
