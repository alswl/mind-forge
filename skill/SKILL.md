---
name: mind-forge
description: "Rust CLI for mind 0.3.0-compatible local knowledge repos. Manage projects, directory or file articles, sources, assets, glossary terms, builds, publishing, and repo-wide publishers using mind-format YAML."
---

# mind-forge — Knowledge Repo CLI

## Overview

`mf` manages local mind-format knowledge repos: content sources, assets, articles (file or directory), glossary terms, builds, publishing, render prompts, configuration, and repo-wide publishers.

**Key concepts:**
- **Mind Repo**: A directory rooted at `minds.yaml`. Most commands require running inside one.
- **Project**: A subdirectory with `docs/`, `sources/`, `assets/`, and `mind.yaml`.
- **Index**: `mind-index.yaml` per project — source of truth for articles, sources, assets, terms, and publish records.
- **Output**: Text by default, JSON with `--json` or `--format json`.

## YAML Format

Use `mind` 0.3.0-compatible YAML as the canonical on-disk format:

- `minds.yaml`: write `schema: "1"` and `projects` as repo-relative path strings. Default projects dir is `projects`.
- `mind.yaml`: write `schema: "1"`; accept both top-level mind fields and wrapped `project:` metadata.
- `mind-index.yaml`: write `schema: "1"` with dictionary sections (`articles:`, `sources:`, `assets:`, `terms:`, `publish_records:`).
- Read compatibility accepts older `schema_version` and list-based shapes. Mutating commands preserve mind semantics.
- Read-only commands must not rewrite YAML.

## Global Flags

| Flag | Description |
|---|---|
| `--root <PATH>` | Mind Repo root directory |
| `--config <PATH>` | Config file path |
| `-v`, `--verbose` | Verbose output (repeatable) |
| `-q`, `--quiet` | Silence non-error output |
| `--format <text\|json>` | Output format (default: `text`) |
| `--json` | Shorthand for `--format json` |
| `--no-color` | Disable colored output |
| `--install-completion <SHELL>` | Install shell completion script |
| `--show-completion <SHELL>` | Show shell completion script |
| `-h`, `--help` | Show help |
| `-V`, `--version` | Show version |

## Commands

### `mf source` — Manage content sources

Subcommands: `list` (alias `ls`), `add`, `update`, `index`, `remove`, `clean`

**`mf source add <INPUT>`**
`-n`, `--name <NAME>` — Source name
`--file-kind <auto\|pdf\|file\|rss\|web>` — File kind (mf primary)
`--source-kind <yuque\|meeting\|misc>` — Source channel type (mind primary)
`-t`, `--type` — Deprecated; use `--file-kind` or `--source-kind` instead
`--link` — Symlink instead of copy (local files)
`-f`, `--force` — Overwrite existing
`-p`, `--project <NAME>` — Project context

**`mf source list`**
`--filter <PATTERN>` — Filter by name
`-t`, `--type <pdf\|file\|rss\|web>` — Filter by file kind
`-p`, `--project <NAME>`

**`mf source update <NAME>`**
`--rename <NEW>` — Rename the source
`--url <URL>` — Update URL
`-p`, `--project <NAME>`

**`mf source remove <NAME_OR_PATH>`**
`--keep-file` — Remove from index only, keep file on disk
`-p`, `--project <NAME>`

**`mf source index`**
`--dry-run` — Preview changes without writing
`-p`, `--project <NAME>`

**`mf source clean`**
`--dry-run` — Preview changes without writing
`-p`, `--project <NAME>`

### `mf asset` — Manage project assets

Subcommands: `list` (alias `ls`), `add`, `update`, `index`, `clean`, `remove`

**`mf asset add <PATH>`**
`--name <NAME>` — Asset name
`--tag <TAG>` — Tag (repeatable)
`--copy` — Copy file (mutually exclusive with `--link`)
`--link` — Symlink file (mutually exclusive with `--copy`)
`-f`, `--force` — Overwrite existing
`-p`, `--project <NAME>`

**`mf asset list`**
`--filter <PATTERN>` — Filter by name
`--type <image\|document\|archive\|other>` — Filter by kind
`-p`, `--project <NAME>`

**`mf asset update [PATH]`**
`--set-url <URL>` — Set publish URL
`--channel <CHANNEL>` — Set publish channel
`--all` — Update all assets (mutually exclusive with `PATH`)
`-p`, `--project <NAME>`

**`mf asset index`**
`--dry-run` — Preview without writing
`--refresh-metadata` — Recompute size/hash
`-p`, `--project <NAME>`

**`mf asset clean`**
`-p`, `--project <NAME>`
`--dry-run`

**`mf asset remove <FILE>`**
`-f`, `--force` — Skip confirmation
`-p`, `--project <NAME>`

### `mf project` — Manage projects

Subcommands: `new`, `list` (alias `ls`), `archive`, `status` (alias `info`), `lint`, `index`, `show`, `import`, `rename`

**`mf project new <NAME>`**
`--template <NAME>` — Template to use
`--force` — Overwrite existing

**`mf project list`** — No flags.

**`mf project archive <NAME_OR_PATH>`** — Move project to `_archived/`.

**`mf project status`**
`-p`, `--project <NAME>` — Project context

**`mf project lint`**
`-p`, `--project <NAME>` — Target a specific project (default: whole repo)
`--fix` — Auto-fix issues
`--rule <RULE>` — Filter rules (repeatable). Available: `missing_directory`, `stale_index_entry`, `name_convention`, `missing_manifest`, `duplicate_key`

**`mf project index`**
`--dry-run` — Preview without writing

**`mf project show <PROJECT>`** — Show project details.

**`mf project import <DIRECTORY>`**
`--type <TYPE>` — Project type
`--source <DIR>` — Source directory override
`--assets <DIR>` — Assets directory override
`-f`, `--force`
`-y`, `--non-interactive` — Skip prompts

**`mf project rename <OLD> <NEW>`** — Rename a project.

### `mf article` — Manage articles

Subcommands: `new`, `list` (alias `ls`), `lint`, `index`, `rename`

**`mf article new <TITLE> [--template <S>] [--file]`**
`-p`, `--project <NAME>`
`-t`, `--template <S>` — Built-in schema name (`blank` / `arch` / `prd` / `blog`) or path under project root (default: `blank`). Built-in names win on exact case-sensitive match; use a subdirectory prefix (e.g. `./arch`) to force path resolution.
`--file` — Write a single file `docs/{slug}.md` instead of a directory
`--tag <TAG>` — Tag (repeatable)
`--draft` — Mark as draft (default: `true`)
`-f`, `--force` — Overwrite existing same-shape artefact (does not cross file/directory shapes)

**`mf article list`**
`-p`, `--project <NAME>`

**`mf article lint`**
`--fix` — Auto-fix issues

**`mf article index`**
`-p`, `--project <NAME>`
`-n`, `--dry-run` — Preview without writing

**`mf article rename <OLD_TITLE> <NEW_TITLE>`**
`-p`, `--project <NAME>`
`-f`, `--force`

### `mf term` / `mf terms` — Manage terminology

Subcommands: `list`, `new`, `lint`, `learn`, `fix`, `show`

**`mf term new <TERM>`**
`--definition <TEXT>` — Term definition
`--alias <TEXT>` — Alias (repeatable)
`--tag <TAG>` — Tag (repeatable)
`--misrecognition <PATTERN>` — Misrecognition pattern (global terms only, repeatable)
`-p`, `--project <NAME>` — Project-scoped term (no `--misrecognition`)

**`mf term list`**
`--filter <PATTERN>` — Filter by name
`--term <NAME>` — Deprecated; use `mf term show <NAME>` instead
`-p`, `--project <NAME>`

**`mf term lint [PATH]`**
`--fix` — Auto-correct term usage in docs
`--dry-run` — Preview fixes without writing (requires `--fix`)
`-p`, `--project <NAME>`

**`mf term learn`**
`--term <CANONICAL>` — Canonical term name (mind primary)
`--alias <VARIANT>` — Variant/alias (mind primary)
`--original` — Deprecated; use `--alias` instead
`--correct` — Deprecated; use `--term` instead
`-p`, `--project <NAME>`

**`mf term fix <TERM>`**
`--definition <TEXT>` — Update definition
`--alias <TEXT>` — Add alias (repeatable)
`--tag <TAG>` — Add tag (repeatable)
`-p`, `--project <NAME>`

**`mf term show <NAME>`**
`-p`, `--project <NAME>`

### `mf build <ARTICLE>` — Build/assemble an article

`-p`, `--project <NAME>`
`-o`, `--output <PATH>` — Output file path
`--dry-run` — Show build plan without rendering

`ARTICLE` may be an indexed article name/slug or a repo-relative path prefixed with `@`, such as `@projects/blog/docs/2026-03-review/`. Directory articles are built by merging Markdown files in filename order.

### `mf publish` — Publish articles

Subcommands: `run`, `update`

**`mf publish run <ARTICLE>`**
`--target <TARGET>` — Publish target (e.g. `local`, `yuque-prompt`)
`-p`, `--project <NAME>`
`--dry-run`
`-f`, `--force`

**`mf publish update <ARTICLE>`**
`--target <TARGET>` — Target name (required)
`--status <draft\|published\|archived>` — Deprecated; use `--set status=<value>`
`--target-url <URL>` — Deprecated; use `--set url=<value>`
`--set <KEY=VALUE>` — Set arbitrary field (repeatable)
`-p`, `--project <NAME>`
`--dry-run`

### `mf render` — Generate render prompts

`[ARTICLE]` — Article name (omit for project-scope render)
`-p`, `--project <NAME>`
`--template <NAME>` — Render template name
`--html-form <document\|fragment>` — HTML output form

Subcommand: `mf render template list` — List available render templates (built-in and custom).

### `mf config` — Manage configuration

Subcommands: `schema`, `show`, `compile`, `generate`, `default`, `init`

**`mf config schema`** — `--output-format <json\|yaml>` (default: `json`)

**`mf config show`** — `--output-format <json\|yaml>` (default: `yaml`)

**`mf config compile`** — Alias of `show`. `--output-format <json\|yaml>` (default: `yaml`)

**`mf config generate`** — `--output-format <json\|yaml>` (default: `yaml`), `-o`, `--output <PATH>`

**`mf config default`** — `--output-format <json\|yaml>` (default: `yaml`)

**`mf config init`** — `--output <PATH>`, `--target <project\|repo>` (default: `project`), `--force`

### `mf publisher` — Manage repo-wide publishers

Subcommand: `list` — List publishers with status and diagnostics. Use `--json` for machine-readable output.

### `mf completion <SHELL>` — Generate shell completion scripts

Supported shells: `bash`, `zsh`, `fish`, `powershell`, `elvish`

### `mf version` — Show version information

## Common Workflows

```bash
# Project management
mf project list
mf project new my-project
mf project status --project my-project
mf project lint --project my-project --fix
mf project show my-project
mf project rename old-name new-name
mf project import /path/to/existing --force

# Sources
mf source add https://example.com/article --name ref-a --file-kind web --project my-project
mf source add paper.pdf --file-kind pdf --project my-project
mf source list --project my-project
mf source index --project my-project
mf source remove sources/yuque/foo.md --keep-file
mf source clean --dry-run

# Assets
mf asset add image.png --project my-project
mf asset list --project my-project
mf asset update --all --project my-project
mf asset index --refresh-metadata --project my-project
mf asset clean --project my-project

# Articles
mf article new "My First Post" --project my-project
mf article new "Auth Rewrite" --template arch --project my-project
mf article new "Quick Note" --template blog --file --project my-project
mf article list --project my-project
mf article index --project my-project
mf article rename "Old Title" "New Title" --project my-project

# Build & publish
mf build my-first-post --project my-project
mf build @projects/my-project/docs/2026-03-review/ --output ./_build/review.md
mf build my-first-post --dry-run
mf publish run "My First Post" --target local --project my-project
mf publish run "My First Post" --target yuque-prompt --project my-project
mf publish update "My First Post" --target local --set status=published

# Terms
mf term new "Zettelkasten" --definition "A note-taking method" --project my-project
mf term new "API" --alias "Application Programming Interface" --tag tech
mf term list
mf term show Zettelkasten
mf term learn --term "API" --alias "Application Programming Interface"
mf term lint --project my-project --fix
mf term fix "API" --definition "Updated definition"

# Render
mf render "My First Post" --project my-project --template paper --html-form document
mf render template list

# Config & diagnostics
mf config show
mf config init
mf publisher list --json
```

## Notes

- Commands that modify project state require a Mind Repo context. `config`, `completion`, `version`, and help can run outside repos. `project index` can also run outside repos (scans from cwd).
- Index subcommands reconcile `mind-index.yaml` with the filesystem; run them after manual file changes.
- Prefer `schema` over `schema_version` in docs, examples, and generated YAML.
- Global terms (created without `--project`) are stored in `minds-terms.yaml` at the repo root. Project-scoped terms are stored in each project's `mind-index.yaml`.
- `term lint` requires a project context — it scans project docs for term usage.
