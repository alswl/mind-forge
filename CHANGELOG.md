# Changelog

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
