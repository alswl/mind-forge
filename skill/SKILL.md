---
name: mind-forge
description: "Rust CLI for mind 0.3.0-compatible local knowledge repos. Manage projects, directory or file articles, sources, assets, glossary terms, builds, publishing, and publish targets using mind-format YAML."
---

# mind-forge — Knowledge Repo CLI

## Overview

`mf` manages local mind-format knowledge repos: content sources, assets, articles (file or directory), glossary terms, builds, publishing, publish targets, render templates, and configuration.

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

## Output Contracts (spec 039)

Every command honors these uniform contracts:

- **JSON envelope:** `{ status, command, data }`. `data` is always a JSON object (never a bare array, string, or `null`).
- **Identity round-trip:** Every list emits `data.<plural-noun>[].identity`; that exact value is accepted as input by the matching `show`/`rename`/`remove`/etc.
- **List layout:** Header row + two-space gap between columns; missing values rendered as `-`; long values truncated with `…` (ASCII fallback `...`).
- **Show layout:** Aligned `Key:  Value` block with optional sub-sections.
- **Create family envelope:** `{ kind, identity, created_at, path?, dry_run, details }` plus text `✓ created|added {kind}: {identity}`.
- **Modify family envelope:** Per-verb `{ kind, identity, ... }`; rename adds `old_identity`/`new_identity`; remove adds `removed: bool`; update adds `changes: { field: {from, to} }`; index adds `{ added, removed, kept_count, scanned_count }`.
- **Lint envelope:** `{ kind, issues, summary: { errors, warnings, info }, dry_run }`.
- **TTY adaptation:** Pipes drop ANSI and headers, preserve row shape; `NO_COLOR` env disables color.
- **Confirmation protocol:** `remove` and `archive` open `/dev/tty` for a `[y/N]` prompt on stderr. Non-TTY without `--yes`/`--force` exits 1 with hint `pass --yes to confirm`.

## Flags

Most commands accept these flags:

| Flag | Description |
|---|---|
| `--root <PATH>` | Mind Repo root directory |
| `--config <PATH>` | Config file path |
| `-p`, `--project <PROJECT>` | Project selector for project-scoped commands |
| `-v`, `--verbose...` | Verbose output (repeatable) |
| `-q`, `--quiet` | Silence non-error output on success |
| `--format <text\|json>` | Output format (default: `text`) |
| `--json` | Shorthand for `--format json` |
| `--no-color` | Disable colored output |
| `-h`, `--help` | Show help |
| `-V`, `--version` | Show version |

Shared flag families (uniform across all commands they apply to):

| Flag family | Applies to | Description |
|---|---|---|
| `--dry-run` | every mutating command (`new`, `add`, `rename`, `remove`, `archive`, `update`, `index`, lint `--fix`) | Preview without writing; JSON envelope sets `dry_run: true` |
| `-f`, `--force` | every `new` / `add` / `rename` / `remove` / `archive` | Overwrite or skip safety checks; on remove/archive also satisfies confirmation |
| `-y`, `--yes` | every `remove` and `archive` | Confirm destructive action non-interactively |
| `--no-headers`, `--no-trunc` | every `list` | Suppress table header / disable column truncation |
| `--fix`, `--rule <RULE>`, `--severity <LEVEL>`, `--max-warnings <N>` | every `lint` | Auto-fix; restrict to one rule kind; filter at-or-above severity (`error`/`warning`/`info`); exit 1 when warnings exceed N |

`--project` accepts repo-relative paths or project names. When running inside a project directory, `--project` can be omitted — the CLI auto-detects the current project and normalizes it to its repo-relative canonical identity.

## Commands

### `mf init [PATH]` — Initialize a Mind Repo

Bootstrap a directory as a Mind Repo: creates `minds.yaml` and the default `projects/` container. Defaults to the current directory.

### `mf project` — Manage projects

Subcommands: `new`, `list` (alias `ls`), `show`, `rename`, `remove` (alias `rm`), `archive`, `lint`, `index`, `import`.

**`mf project new <PATH>`**
Create a project. Accepts cwd-relative or repo-relative paths with Unicode, emoji, dates, spaces, and underscores. No `--name`/`--id`/`--title` required.
`--template <TEMPLATE>` — Project template.

**`mf project list`** (alias `ls`) — List all projects.

**`mf project show <NAME>`** — Show project details (key-value block + JSON envelope `{ kind: "project", identity, ... }`).

**`mf project rename <OLD_NAME> <NEW_NAME>`** — Rename a project.

**`mf project remove <NAME>`** (alias `rm`) — Remove a project. Interactive TTY confirmation unless `--yes` or `--force` is set.

**`mf project archive <NAME_OR_PATH>`** — Move project to `_archived/`. Interactive TTY confirmation unless `--yes` or `--force` is set.

**`mf project lint`**
Lint project(s). Requires `-p, --project <PROJECT>`.
Rule kinds: `missing_directory`, `stale_index_entry`, `name_convention`, `missing_manifest`, `duplicate_key`.

**`mf project index`** — Index projects (mf extension).

**`mf project import <DIRECTORY>`**
Import a directory as a project.
`--type <TYPE>` — Project type
`--source <DIR>` — Source directory override
`--assets <DIR>` — Assets directory override
`-y`, `--non-interactive` — Skip prompts

### `mf article` — Manage articles

Subcommands: `new`, `list` (alias `ls`), `show`, `rename`, `remove` (alias `rm`), `lint`, `index`.

**`mf article new <TITLE>`**
Create an article. `<TITLE>` is the sole positional argument; the article type is derived from `--template`.
`-t`, `--template <S>` — Built-in schema name (`blank` / `arch` / `prd` / `blog`) or path under project root (default: `blank`). Built-in names win on exact case-sensitive match; use a subdirectory prefix (e.g. `./arch`) to force path resolution.
`--file` — Write a single file `docs/{slug}.md` instead of a directory (alias `--single-file`).
`--tag <TAG>` — Tag (repeatable).
`--draft` — Mark as draft.

JSON envelope `details`: `template`, `shape` (`directory`|`file`), `path`, `files`, `typora_front_matter_injected` (bool), `typora_copy_images_to` (string|null). When the Typora plugin is enabled, each generated file starts with a YAML front-matter block containing `typora-copy-images-to` pointing to the project assets directory.

**`mf article list`** (alias `ls`) — List articles.

**`mf article show <PATH>`** — Show article details. `<PATH>` accepts a path (e.g. `docs/weekly.md`) or a title.

**`mf article rename <OLD_PATH> <NEW_PATH>`** — Rename an article.

**`mf article remove <PATH>`** (alias `rm`) — Remove an article. Interactive TTY confirmation unless `--yes` or `--force` is set.

**`mf article lint`** — Lint articles.

**`mf article index`** — Index articles (mf extension). `-n` is short for `--dry-run`.

### `mf source` — Manage content sources

Subcommands: `list` (alias `ls`), `add`, `show`, `update`, `rename`, `remove` (alias `rm`), `index`, `clean`.

**`mf source add <INPUT>`**
`-n`, `--name <NAME>` — Source name
`--file-kind <auto|pdf|file|rss|web>` — File kind (mf primary)
`--source-kind <yuque|meeting|misc>` — Source channel type (mind primary)
`-t`, `--type <KIND>` — Deprecated: use `--file-kind` or `--source-kind` instead
`--link` — Symlink instead of copy (local files)

**`mf source list`** (alias `ls`)
`--filter <PATTERN>` — Filter by name
`-t`, `--type <auto|pdf|file|rss|web>` — Filter by file kind

**`mf source show <NAME>`** — Show source details.

**`mf source update <NAME>`**
`--url <URL>` — Update URL
`--rename <NAME>` — Rename source (legacy; prefer `mf source rename`)

**`mf source rename <OLD_PATH> <NEW_PATH>`** — Rename a source by path or name.

**`mf source remove <NAME_OR_PATH>`** (alias `rm`)
`--keep-file` — Remove from index only, keep file on disk

**`mf source index`** — Index sources (mf extension).

**`mf source clean`** — Clean stale index entries.

### `mf asset` — Manage project assets

Subcommands: `list` (alias `ls`), `add`, `show`, `update`, `rename`, `remove` (alias `rm`), `index`, `clean`.

**`mf asset add <PATH>`**
`--name <NAME>` — Asset name
`--tag <TAG>` — Tag (repeatable)
`--copy` — Copy file (mutually exclusive with `--link`)
`--link` — Symlink file (mutually exclusive with `--copy`)

**`mf asset list`** (alias `ls`)
`--filter <PATTERN>` — Filter by name
`--type <image|video|audio|other>` — Filter by kind

**`mf asset show <NAME>`** — Show asset details.

**`mf asset update [PATH]`**
`--set-url <URL>` — Set publish URL
`--channel <CHANNEL>` — Set publish channel
`--all` — Update all assets (mutually exclusive with `PATH`)

**`mf asset rename <OLD_PATH> <NEW_PATH>`** — Rename an asset.

**`mf asset remove <FILE>`** (alias `rm`) — Remove an asset.

**`mf asset index`** — Index assets (mf extension). `--refresh-metadata` recomputes size/hash.

**`mf asset clean`** — Clean stale index entries.

### `mf term` (alias `mf terms`) — Manage terminology

Subcommands: `list` (alias `ls`), `new`, `show`, `add`, `update`, `rename`, `remove` (alias `rm`), `lint`.

**`mf term new <TERM>`**
Create a term (mf extension).
`--definition <TEXT>` — Term definition
`--description <TEXT>` — Long description
`--confidence <N>` — Confidence score
`--alias <TEXT>` — Alias (repeatable)
`--tag <TAG>` — Tag (repeatable)
`--misrecognition <TEXT>` — Common misrecognition variant

**`mf term list`** (alias `ls`)
`--filter <PATTERN>` — Filter by name
`--term <NAME>` — Look up a single term (deprecated: use `term show <NAME>`)

**`mf term show <NAME>`** — Show term details (term + corrections sub-section).

**`mf term add`**
Add a term correction (canonical name; replaces the legacy `term learn`).
`--term <CANONICAL>` — Canonical term name (mind primary)
`--alias <VARIANT>` — Variant/alias for the term (mind primary)
`--original <TEXT>` — Deprecated: use `--term` instead
`--correct <TEXT>` — Deprecated: use `--alias` instead

**`mf term update <TERM>`**
Update term metadata (mf extension; replaces the legacy `term fix`).
`--definition <TEXT>` — Update definition
`--description <TEXT>` — Update description (`--clear-description` to unset)
`--confidence <N>` — Update confidence (`--clear-confidence` to unset)
`--alias <TEXT>` — Add alias (repeatable)
`--tag <TAG>` — Add tag (repeatable)

**`mf term rename <OLD_TERM> <NEW_TERM>`**
Rename a term.
`--keep-alias` — Preserve the old name as an alias on the renamed term.

**`mf term remove <TERM>`** (alias `rm`) — Remove a term. Interactive TTY confirmation unless `--yes` or `--force` is set.

**`mf term lint [PATH]`**
Lint term consistency in project docs.
`--fix` — Auto-correct term usage in docs (pair with `--dry-run` to preview).

### `mf build <ARTICLE>` — Build/assemble an article

`-o`, `--output <PATH>` — Output file path
`--dry-run` — Show build plan without rendering

`ARTICLE` may be an indexed article name/slug or a repo-relative path prefixed with `@`, such as `@projects/blog/docs/2026-03-review/`. Directory articles are built by merging Markdown files in filename order.

### `mf publish` — Publish articles and manage targets

Subcommands: `run`, `update`, `target`.

**`mf publish run <ARTICLE>`**
Publish article to a target (supported: `local`, `yuque-prompt`).
`--target <TARGET>` — Publish target

**`mf publish update <ARTICLE>`**
Update a `publish_records` entry in `mind-index.yaml`.
`--target <TARGET>` — Target name (required)
`--status <draft|published|archived>` — Set status
`--target-url <URL>` — Set target URL
`--set <KEY=VALUE>` — Set arbitrary field (repeatable)

**`mf publish target list`** — List publish targets and diagnostics.

**`mf publish target show <NAME>`** — Show publish target details.

### `mf render template` — Render templates

Subcommands: `list`, `show`.

**`mf render template list`** — List built-in and project-local render templates.

**`mf render template show <NAME>`** — Show template details, including a preview of the first lines.

### `mf config` — Manage configuration

Subcommands: `schema`, `show`, `generate`, `default`, `init` (deprecated).

**`mf config schema`** — Show config JSON schema. `--output-format <json|yaml>` (default: `json`).

**`mf config show`** — Show effective config. `--output-format <json|yaml>` (default: `yaml`). JSON envelope `data` is the canonical config object (no embedded YAML string).

**`mf config generate`** — Generate effective config file. `--output-format <json|yaml>` (default: `yaml`), `-o, --output <PATH>`.

**`mf config default`** — Show default config values. `--output-format <json|yaml>` (default: `yaml`). Generated config includes a `plugins.typora-front-matter` block with `enabled: true`.

**`mf config init`** — **Deprecated:** use `mf init`. `--output <PATH>`, `--target <project|repo>` (default: `project`), `--force`.

### `mf completion <SHELL>` — Generate shell completion scripts

Supported shells: `bash`, `zsh`, `fish`, `powershell`, `elvish`.

### `mf version` — Show version information

Text format: `mf {version} ({commit}, built {build_date}, rustc {rustc})` (with `unknown` fallbacks when build metadata is unavailable).
JSON envelope `data`: `{ version, commit, build_date, rustc, target_triple }`.

## Common Workflows

```bash
# Repo lifecycle
mf init                            # bootstrap a Mind Repo (creates minds.yaml + projects/)

# Project management (path-based identity, Unicode/emoji/dates supported)
mf project new my-project
mf project new workspaces/team/projects/2026-W21
mf project new 2026-W21                                # from inside workspaces/team/projects/
mf project list
mf project show my-project
mf project rename old-name new-name
mf project remove obsolete --yes                       # non-interactive
mf project archive completed
mf project lint --project my-project --fix
mf project import /path/to/existing --force

# Sources
mf source add https://example.com/article --name ref-a --file-kind web --project my-project
mf source add paper.pdf --file-kind pdf --project my-project
mf source list --project my-project
mf source show ref-a --project my-project
mf source rename ref-a ref-canonical --project my-project
mf source index --project my-project
mf source update ref-canonical --url https://example.com/v2 --project my-project
mf source remove sources/yuque/foo.md --keep-file --project my-project
mf source clean --dry-run --project my-project

# Assets
mf asset add image.png --project my-project
mf asset list --project my-project
mf asset show image.png --project my-project
mf asset rename old.png new.png --project my-project
mf asset update --all --project my-project
mf asset index --refresh-metadata --project my-project
mf asset remove diagram.png --project my-project
mf asset clean --project my-project

# Articles
mf article new "My First Post" --project my-project
mf article new "Auth Rewrite" --template arch --project my-project
mf article new "Quick Note" --template blog --project my-project
mf article new "Single Page" --file --project my-project
mf article list --project my-project
mf article show docs/my-first-post --project my-project
mf article rename docs/old-title docs/new-title --project my-project
mf article index --project my-project
mf article lint --fix --project my-project

# Build & publish
mf build my-first-post --project my-project
mf build @projects/my-project/docs/2026-03-review/ --output ./_build/review.md
mf build my-first-post --dry-run --project my-project
mf publish run "My First Post" --target local --project my-project
mf publish run "My First Post" --target yuque-prompt --project my-project
mf publish update "My First Post" --target local --status published --project my-project
mf publish target list
mf publish target show local

# Render templates
mf render template list
mf render template show arch

# Terms
mf term new "Zettelkasten" --definition "A note-taking method" --project my-project
mf term new "API" --alias "Application Programming Interface" --tag tech
mf term list
mf term show Zettelkasten --project my-project
mf term add --term "API" --alias "Application Programming Interface" --project my-project
mf term update "API" --definition "Updated definition" --project my-project
mf term rename "API" "Application API" --keep-alias --project my-project
mf term remove obsolete-term --yes --project my-project
mf term lint --project my-project --fix

# Config
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
- Destructive verbs (`remove`, `archive`) prompt on `/dev/tty` in interactive shells. In scripts/CI pipe the command through `--yes` (confirms) or `--force` (also bypasses safety checks).
- `term add` replaces the legacy `term learn`; `term update` replaces the legacy `term fix`. Old subcommand names are no longer accepted.
- `mf init` replaces `mf config init` for repo bootstrap; the latter still works but is deprecated.
