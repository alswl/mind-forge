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

`mf` covers the full knowledge management lifecycle:

- **Project lifecycle**: init, list, status, lint, index, archive
- **Content**: article, source, asset management with CRUD and indexing
- **Glossary**: term lookup, list, lint, fix, learn
- **Build & Publish**: config-driven build, local and yuque-prompt publishing, repo-wide publisher management
- **Render Prompt**: `mf render <article>` generates an Agent-facing HTML rendering prompt from existing built output, with built-in (`report`, `paper`) and custom Markdown templates under `.mind-forge/renders/`
- **Compatibility**: reads and writes mind 0.3.0-compatible YAML (`minds.yaml`, `mind.yaml`, `mind-index.yaml`)
- **Output**: text/JSON envelope (`{ status, command, data }`) with shell completion (`mf completion <shell>`)

## Quickstart

```bash
cargo test
cargo run -- --help
cargo run -- source list
cargo run -- completion zsh
```

## Defaults

`mf build <ARTICLE>` writes to `outputs/<ARTICLE>.md` by default. Projects can
override this in `mind.yaml`:

```yaml
build:
  output_dir: custom-output
  format: md
```

Other project layout defaults are `docs/`, `sources/`, `assets/`, and
`_archived/`; these defaults are centralized in code and exposed through
`mf config show`.

## Migrating from mind

If you're migrating from the `mind` CLI, see:

- [Migration guide](docs/migration-from-mind.md) — command mapping table
- [Deprecations](docs/deprecations.md) — deprecated usages and their replacements
- [mf extensions](docs/mf-extensions.md) — mf-only commands

## Project Status

See [ROADMAP](specs/002-mf-command-design/ROADMAP.md) for the feature
evolution plan and [specs/](specs/) for detailed specifications.
