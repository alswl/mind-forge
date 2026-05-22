---
name: mind-forge
description: "Rust CLI for mind 0.3.0-compatible local knowledge repos. Manage projects, directory or file articles, sources, assets, glossary terms, builds, publishing, and publish targets using mind-format YAML."
---

# mind-forge — Knowledge Repo CLI

## Overview

`mf` manages local mind-format knowledge repos: content sources, assets, articles (file or directory), glossary terms, builds, publishing, publish targets, and configuration.

**Key concepts:**
- **Mind Repo**: A directory rooted at `minds.yaml`. Most commands require running inside one.
- **Project**: A subdirectory with `docs/`, `sources/`, `assets/`, and `mind.yaml`. Project identity is a repo-relative path.
- **Index**: `mind-index.yaml` per project — source of truth for articles, sources, assets, terms, and publish records.
- **Output**: Text by default, JSON with `--json` or `--format json`.

## YAML Format

Use `mind` 0.3.0-compatible YAML as the canonical on-disk format:

- `minds.yaml`: write `schema: "1"` and `projects` as repo-relative path strings. Default projects dir is `projects`.
- `mind.yaml`: write `schema: "1"`; accept both top-level mind fields and wrapped `project:` metadata.
- `mind-index.yaml`: write `schema: "1"` with dictionary sections (`articles:`, `sources:`, `assets:`, `terms:`, `publish_records:`).
- `mind.yaml` supports a `plugins` block. Known plugin keys are typed; unknown keys round-trip for forward compatibility.
- The `typora-front-matter` plugin is enabled by default (`enabled: true`). Set `plugins.typora-front-matter.enabled: false` to disable.
- Read compatibility accepts older `schema_version` and list-based shapes. Mutating commands preserve mind semantics.
- Read-only commands must not rewrite YAML.

## Flags

Most commands accept these flags:

| Flag | Description |
|---|---|
| `--root <PATH>` | Mind Repo root directory |
| `--config <PATH>` | Config file path |
| `-p`, `--project <PROJECT>` | Project selector for project-scoped commands |
| `-v`, `--verbose...` | Verbose output (repeatable) |
| `-q`, `--quiet` | Silence non-error output |
| `--format <text\|json>` | Output format (default: `text`) |
| `--json` | Shorthand for `--format json` |
| `--no-color` | Disable colored output |
| `-h`, `--help` | Show help |
| `-V`, `--version` | Show version |

`--project` accepts repo-relative paths or project names. When running inside a project directory, `--project` can be omitted — the CLI auto-detects the current project and normalizes it to its repo-relative canonical identity.

## Commands

### `mf project` — Manage projects

Subcommands: `new`, `list` (alias `ls`), `archive`, `status` (alias `info`), `lint`, `index`, `show`, `import`

**`mf project new <PATH>`**
Create a project. Accepts cwd-relative or repo-relative paths with Unicode, emoji, dates, spaces, and underscores. No `--name`/`--id`/`--title` required.
`--template <TEMPLATE>` — Project template
`--force` — Overwrite existing

**`mf project list`** — List all projects.

**`mf project archive <NAME_OR_PATH>`** — Move project to `_archived/`.

**`mf project status`** (alias `info`)
Show project status. Requires `-p, --project <PROJECT>`.

**`mf project lint`**
Lint project(s). Requires `-p, --project <PROJECT>`.
`--fix` — Auto-fix issues
`--rule <RULE>` — Filter rules (repeatable). Available: `missing_directory`, `stale_index_entry`, `name_convention`, `missing_manifest`, `duplicate_key`

**`mf project index`**
Index projects (mf extension).
`--dry-run` — Preview without writing

**`mf project show <PROJECT>`** — Show project details.

**`mf project import <DIRECTORY>`**
Import a directory as a project.
`--type <TYPE>` — Project type
`--source <DIR>` — Source directory override
`--assets <DIR>` — Assets directory override
`-f`, `--force`
`-y`, `--non-interactive` — Skip prompts

### `mf article` — Manage articles

Subcommands: `new`, `list` (alias `ls`), `lint`, `index`

**`mf article new <TYPE> <TITLE>`**
Create an article. `<TYPE>` is an article type name (e.g. `arch`, `blog`).
`-t`, `--template <S>` — Built-in schema name (`blank` / `arch` / `prd` / `blog`) or path under project root (default: `blank`). Built-in names win on exact case-sensitive match; use a subdirectory prefix (e.g. `./arch`) to force path resolution.
`--file` — Write a single file `docs/{slug}.md` instead of a directory
`--tag <TAG>` — Tag (repeatable)
`--draft` — Mark as draft
`-f`, `--force` — Overwrite existing same-shape artefact (does not cross file/directory shapes)

JSON envelope fields: `template`, `shape` (`directory`|`file`), `path`, `files`, `typora_front_matter_injected` (bool), `typora_copy_images_to` (string|null). When the Typora plugin is enabled, each generated file starts with a YAML front-matter block containing `typora-copy-images-to` pointing to the project assets directory.

**`mf article list`** — List articles.

**`mf article lint`**
Lint articles.
`--fix` — Auto-fix issues

**`mf article index`**
Index articles (mf extension).
`-n`, `--dry-run` — Preview without writing

### `mf source` — Manage content sources

Subcommands: `list` (alias `ls`), `add`, `update`, `index`, `remove` (alias `rm`), `clean`

**`mf source add <INPUT>`**
`-n`, `--name <NAME>` — Source name
`--file-kind <auto|pdf|file|rss|web>` — File kind (mf primary)
`--source-kind <yuque|meeting|misc>` — Source channel type (mind primary)
`-t`, `--type <KIND>` — Deprecated: use `--file-kind` or `--source-kind` instead
`--link` — Symlink instead of copy (local files)
`-f`, `--force` — Overwrite existing

**`mf source list`**
`--filter <PATTERN>` — Filter by name
`-t`, `--type <auto|pdf|file|rss|web>` — Filter by file kind

**`mf source update <NAME>`**
`--url <URL>` — Update URL
`--rename <NAME>` — Rename source

**`mf source index`**
`--dry-run` — Preview changes without writing

**`mf source remove <NAME_OR_PATH>`** (alias `rm`)
`--keep-file` — Remove from index only, keep file on disk

**`mf source clean`**
`--dry-run` — Preview changes without writing

### `mf asset` — Manage project assets

Subcommands: `list` (alias `ls`), `add`, `update`, `index`, `clean`, `remove` (alias `rm`)

**`mf asset add <PATH>`**
`--name <NAME>` — Asset name
`--tag <TAG>` — Tag (repeatable)
`--copy` — Copy file (mutually exclusive with `--link`)
`--link` — Symlink file (mutually exclusive with `--copy`)
`-f`, `--force` — Overwrite existing

**`mf asset list`**
`--filter <PATTERN>` — Filter by name
`--type <image|video|audio|other>` — Filter by kind

**`mf asset update [PATH]`**
`--set-url <URL>` — Set publish URL
`--channel <CHANNEL>` — Set publish channel
`--all` — Update all assets (mutually exclusive with `PATH`)

**`mf asset index`**
`--dry-run` — Preview without writing
`--refresh-metadata` — Recompute size/hash

**`mf asset clean`**
`--dry-run`

**`mf asset remove <FILE>`** (alias `rm`)
`-f`, `--force` — Skip confirmation

### `mf term` (alias `mf terms`) — Manage terminology

Subcommands: `list` (alias `ls`), `new`, `lint`, `learn`, `fix`, `show`

**`mf term new <TERM>`**
Create a term (mf extension).
`--definition <TEXT>` — Term definition
`--alias <TEXT>` — Alias (repeatable)
`--tag <TAG>` — Tag (repeatable)

**`mf term list`**
`--filter <PATTERN>` — Filter by name
`--term <NAME>` — Look up a single term by name (deprecated: use `term show <NAME>`)

**`mf term lint [PATH]`**
Lint term consistency in project docs.
`--fix` — Auto-correct term usage in docs
`--dry-run` — Preview fixes without writing (requires `--fix`)

**`mf term learn`**
Learn a term correction.
`--term <CANONICAL>` — Canonical term name (mind primary)
`--alias <VARIANT>` — Variant/alias for the term (mind primary)
`--original <TEXT>` — Deprecated: use `--term` instead
`--correct <TEXT>` — Deprecated: use `--alias` instead

**`mf term fix <TERM>`**
Fix a term metadata (mf extension).
`--definition <TEXT>` — Update definition
`--alias <TEXT>` — Add alias (repeatable)
`--tag <TAG>` — Add tag (repeatable)

**`mf term show <NAME>`** — Show term details.

### `mf build <ARTICLE>` — Build/assemble an article

`-o`, `--output <PATH>` — Output file path
`--dry-run` — Show build plan without rendering

`ARTICLE` may be an indexed article name/slug or a repo-relative path prefixed with `@`, such as `@projects/blog/docs/2026-03-review/`. Directory articles are built by merging Markdown files in filename order.

### `mf publish` — Publish articles

Subcommands: `run`, `update`

**`mf publish run <ARTICLE>`**
Publish article to a target (supported: `local`, `yuque-prompt`).
`--target <TARGET>` — Publish target
`--dry-run`
`-f`, `--force`

**`mf publish update <ARTICLE>`**
Update a publish_records entry in mind-index.yaml.
`--target <TARGET>` — Target name (required)
`--status <draft|published|archived>` — Set status
`--target-url <URL>` — Set target URL
`--set <KEY=VALUE>` — Set arbitrary field (repeatable)
`--dry-run`

### `mf publisher` — Manage repo-wide publishers

Subcommands: `list`

**`mf publisher list`** — List publishers and diagnostics.

### `mf config` — Manage configuration

Subcommands: `schema`, `show`, `compile`, `generate`, `default`, `init`

**`mf config schema`** — Show config JSON schema. `--output-format <json|yaml>` (default: `json`)

**`mf config show`** — Show effective config. `--output-format <json|yaml>` (default: `yaml`)

**`mf config compile`** — Alias of `config show`.

**`mf config generate`** — Generate effective config file. `--output-format <json|yaml>` (default: `yaml`), `-o, --output <PATH>`

**`mf config default`** — Show default config values. `--output-format <json|yaml>` (default: `yaml`). Generated config includes a `plugins.typora-front-matter` block with `enabled: true`.

**`mf config init`** — Initialize config file (mf extension). `--output <PATH>`, `--target <project|repo>` (default: `project`), `--force`

### `mf completion <SHELL>` — Generate shell completion scripts

Supported shells: `bash`, `zsh`, `fish`, `powershell`, `elvish`

### `mf version` — Show version information

Accepts `--json` for machine-readable output.

## Common Workflows

```bash
# Repo lifecycle
mf config init                     # initialize a Mind Repo

# Project management (path-based identity, Unicode/emoji/dates supported)
mf project new my-project
mf project new workspaces/team/projects/2026-W21
mf project new 2026-W21             # from inside workspaces/team/projects/
mf project list
mf project lint --project my-project --fix
mf project status --project my-project
mf project show my-project
mf project import /path/to/existing --force

# Sources
mf source add https://example.com/article --name ref-a --file-kind web --project my-project
mf source add paper.pdf --file-kind pdf --project my-project
mf source list --project my-project
mf source index --project my-project
mf source update old-name --rename new-name --project my-project
mf source remove sources/yuque/foo.md --keep-file --project my-project
mf source clean --dry-run --project my-project

# Assets
mf asset add image.png --project my-project
mf asset list --project my-project
mf asset update --all --project my-project
mf asset index --refresh-metadata --project my-project
mf asset remove diagram.png --project my-project
mf asset clean --project my-project

# Articles
mf article new blog "My First Post" --project my-project
mf article new arch "Auth Rewrite" --template arch --project my-project
mf article new blog "Quick Note" --template blog --project my-project
mf article new blog "Single Page" --file --project my-project
mf article list --project my-project
mf article index --project my-project
mf article lint --fix --project my-project

# Build & publish
mf build my-first-post --project my-project
mf build @projects/my-project/docs/2026-03-review/ --output ./_build/review.md
mf build my-first-post --dry-run --project my-project
mf publish run "My First Post" --target local --project my-project
mf publish run "My First Post" --target yuque-prompt --project my-project
mf publish update "My First Post" --target local --status published --project my-project

# Terms
mf term new "Zettelkasten" --definition "A note-taking method" --project my-project
mf term new "API" --alias "Application Programming Interface" --tag tech
mf term list
mf term show Zettelkasten --project my-project
mf term learn --term "API" --alias "Application Programming Interface" --project my-project
mf term fix "API" --definition "Updated definition" --project my-project
mf term lint --project my-project --fix

# Publishers & config
mf publisher list
mf config show
mf config default
mf config generate -o minds.yaml

# Shell & diagnostics
mf completion zsh
mf version --json
```

## Notes

- Commands that modify project state require a Mind Repo context. `config`, `completion`, `version`, and help can run outside repos. `project index` can also run outside repos (scans from cwd).
- `--project` accepts repo-relative paths or project names. When running inside a project directory, it can be omitted — the CLI auto-detects the current project.
- Project identity is path-based: `mf project new` accepts cwd-relative or repo-relative paths with Unicode, emoji, dates, spaces, and underscores.
- Index subcommands reconcile `mind-index.yaml` with the filesystem; run them after manual file changes.
- Prefer `schema` over `schema_version` in docs, examples, and generated YAML.
- Global terms (created without `--project`) are stored in `minds-terms.yaml` at the repo root. Project-scoped terms are stored in each project's `mind-index.yaml`.
- `term lint` requires a project context — it scans project docs for term usage.
- `config init` creates a project-level or repo-level config; use it to bootstrap a Mind Repo.
