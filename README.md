# mind-forge

**A Local-First CLI for Card-Based Writing, Document as Code.**

`mf` is a personal knowledge management toolkit that treats your writing as a
codebase — articles are composed from composable Blocks, documents are managed
as code, and everything stays on your machine.

## Product Philosophies

### Card-Based Writing

Articles aren't monolithic files. They are assembled from reusable Blocks
(atomic units of content — a quote, a thought, a code snippet). Blocks can be
referenced, remixed, and versioned independently, enabling a Zettelkasten-like
workflow inside a structured document model.

### Document as Code

Your knowledge base is a codebase. Project structure, metadata, build
configuration, and indexing all follow IaC principles: declarative config
files (YAML), schema validation, CLI-first interaction, and auditability via
git. If you manage infrastructure with code, why not your documents?

### Local First

No cloud dependency. No vendor lock-in. Everything is plain files on disk —
markdown, YAML, and assets in a standard directory tree. Your writing belongs
to you, readable by any text editor, extensible via CLI pipelines.

## Current Scope

The framework stage of `mf` is implemented:

- multi-level command tree for `source`, `asset`, `project`, `article`, `term`
- shared global flags such as `--config`, `--verbose`, `--quiet`, `--format`, `--no-color`
- consistent placeholder responses for leaf commands
- shell completion generation via `mf completion <shell>`

## Quickstart

```bash
cargo test
cargo run -- --help
cargo run -- source list
cargo run -- completion zsh
```

## Project Status

See [ROADMAP](specs/002-mf-command-design/ROADMAP.md) for the feature
evolution plan and [specs/](specs/) for detailed specifications.
