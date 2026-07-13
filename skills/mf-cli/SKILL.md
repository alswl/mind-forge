---
name: mf-cli
description: Use the mf Rust CLI to manage mind 0.3.0-compatible local knowledge repositories, including projects, articles, sources, assets, glossary terms, builds, publishing, render templates, configuration, JSON contracts, and exact command flags. Use for CLI operations, command lookup, automation safety, or structured output reference; use mf-plan or mf-write for article workflows.
---

# mf-cli — Knowledge Repo CLI

## Overview

`mf` manages local mind-format knowledge repos: content sources, assets, articles (file or directory), glossary terms, builds, publishing, publish targets, render templates, and configuration.

**Key concepts:**
- **Mind Repo**: A directory rooted at `minds.yaml`. Most commands require running inside one.
- **Project**: A subdirectory with `docs/`, `sources/`, `assets/`, and `mind.yaml`. Some repositories also use `prompts/` for writing intent. Project identity is a repo-relative path.
- **Index**: `mind-index.yaml` per project — source of truth for articles, sources, assets, terms, and publish records.
- **Output**: Text by default, JSON with `--json` or `--output json` (`-o json`).

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
- **Create family envelope:** `{ kind, identity, created_at, path?, dry_run, details }` plus text `✓ created {kind}: {identity}`.
- **Modify family envelope:** Per-verb `{ kind, identity, ... }`; rename adds `old_identity`/`new_identity`; remove adds `removed: bool`; update adds `changes: { field: {from, to} }`; index adds `{ added, removed, kept_count, scanned_count }`.
- **Lint envelope:** `{ kind, issues, summary: { errors, warnings, info }, dry_run }`.
- **TTY adaptation:** Pipes drop ANSI and headers, preserve row shape; `NO_COLOR` env disables color.
- **Confirmation protocol:** `remove` and `archive` open `/dev/tty` for a `[y/N]` prompt on stderr. Non-TTY without `--yes` exits 1 with a confirmation hint. `--force` bypasses safety checks but does not replace confirmation.

## Flags

Most commands accept these flags:

| Flag | Description |
|---|---|
| `--root <PATH>` | Mind Repo root directory |
| `--config <PATH>` | Config file path |
| `-p`, `--project <PROJECT>` | Project selector for project-scoped commands |
| `-v`, `--verbose...` | Verbose output (repeatable) |
| `-q`, `--quiet` | Silence non-error output on success |
| `-o`, `--output <text\|json>` | Output format (default: `text`) |
| `--json` | Shorthand for `--output json` |
| `--no-color` | Disable colored output |
| `-h`, `--help` | Show help |
| `-V`, `--version` | Show version |

Shared flag families (uniform across all commands they apply to):

| Flag family | Applies to | Description |
|---|---|---|
| `--dry-run` | every mutating command (`new`, `rename`, `remove`, `archive`, `update`, `index`, lint `--fix`) | Preview without writing; JSON envelope sets `dry_run: true` |
| `-f`, `--force` | every `new` / `rename` / `remove` / `archive` | Overwrite a target or bypass safety checks; it does not confirm remove/archive |
| `-y`, `--yes` | every `remove` and `archive` | Confirm destructive action non-interactively |
| `--no-headers`, `--no-trunc` | every `list` | Suppress table header / disable column truncation |
| `--fix`, `--rule <RULE>`, `--severity <LEVEL>`, `--max-warnings <N>` | every `lint` | Auto-fix; restrict to one rule kind; filter at-or-above severity (`error`/`warning`/`info`); exit 1 when warnings exceed N |

`--project` accepts repo-relative paths or project names. When running inside a project directory, `--project` can be omitted — the CLI auto-detects the current project and normalizes it to its repo-relative canonical identity.

## Commands

### `mf init [PATH]` — Initialize a Mind Repo

Bootstrap a directory as a Mind Repo: creates `minds.yaml` and the default `projects/` container. Defaults to the current directory.

### `mf project` — Manage projects

Subcommands: `new`, `list` (alias `ls`), `show`, `update`, `rename`, `remove` (alias `rm`), `archive`, `lint`, `index`, `import`.

**`mf project new <PATH>`**
Create a project. Accepts cwd-relative or repo-relative paths with Unicode, emoji, dates, spaces, and underscores. No `--name`/`--id`/`--title` required.
`--template <TEMPLATE>` — Project template.

**`mf project list`** (alias `ls`) — List all projects.

**`mf project show <PATH>`** — Show project details (key-value block + JSON envelope `{ kind: "project", identity, ... }`).

**`mf project update <PATH>`**
Update project metadata in `mind.yaml`.
`--description <TEXT>` — Set `project.description`
`--clear-description` — Clear `project.description`

**`mf project rename <OLD_PATH> <NEW_PATH>`** — Rename a project.

**`mf project remove <PATH>`** (alias `rm`) — Remove a project. Interactive TTY confirmation unless `--yes` is set.

**`mf project archive <NAME_OR_PATH>`** — Move project to `_archived/`. Interactive TTY confirmation unless `--yes` or `--force` is set.

**`mf project lint`**
Lint project(s). Requires `-p, --project <PROJECT>`.
Rule kinds: `missing_directory`, `stale_index_entry`, `name_convention`, `missing_manifest`, `duplicate_key`, `orphan_prompt`, `duplicate_binding`, `missing_thinking`.
`orphan_prompt` (error) — a prompt's `article:` binding resolves to no indexed article. `duplicate_binding` (error) — two or more prompts resolve to the same article. `missing_thinking` (warning) — an article has a bound prompt but no matching `thinking/<key>.md`. `missing_directory` intentionally does NOT cover `prompts/`/`thinking/`: unlike `docs/`/`sources/`/`assets/` (guaranteed by `mf project new`), those two are optional — their absence just means the writing workflow hasn't started yet, not a lint violation.

**`mf project index`** — Index projects (mf extension). Also reconciles each project's article index: prunes stale entries whose target file no longer exists on disk. Entries whose files exist — including declared and template-origin articles outside `docs/` — are never removed. A project whose `mind-index.yaml` fails to load is skipped with a warning (stderr; `data.warnings` in JSON) rather than silently. Use this to clear `stale_index_entry` lint warnings without hand-editing `mind-index.yaml`.

**`mf project import <DIRECTORY>`**
Import a directory as a project.
`--type <TYPE>` — Project type
`--source <DIR>` — Source directory override
`--assets <DIR>` — Assets directory override
`-y`, `--non-interactive` — Skip prompts

### `mf article` — Manage articles

Subcommands: `new`, `list` (alias `ls`), `show`, `update`, `rename`, `remove` (alias `rm`), `block`, `convert`, `lint`, `index`.

**`mf article new <TITLE>`**
Create an article. `<TITLE>` is the sole positional argument; the article type is derived from `--template`.
`-t`, `--template <S>` — Built-in schema name (`blank` / `arch` / `prd` / `blog`) or path under project root (default: `blank`). Built-in names win on exact case-sensitive match; use a subdirectory prefix (e.g. `./arch`) to force path resolution.
`--file` — Write a single file `docs/{slug}.md` instead of a directory (alias `--single-file`).
`--tag <TAG>` — Tag (repeatable).
`--draft` — Mark as draft.

Template `## ` headings inside fenced code blocks are treated as body text, not block boundaries — templates can safely include prompt examples with code fences. `--dry-run` runs the same parse + block-slug validation as the real run, so both always agree on success or failure.

JSON envelope `details`: `template`, `shape` (`directory`|`file`), `path`, `files`, `typora_front_matter_injected` (bool), `typora_copy_images_to` (string|null). When the Typora plugin is enabled, each generated file starts with a YAML front-matter block containing `typora-copy-images-to` pointing to the project assets directory.

**`mf article list`** (alias `ls`) — List articles. When run outside a project dir without `--project`, auto-matches all projects and sorts by most recently modified. In single-project mode each row also carries a `PROMPT` column (the bound prompt's mode, `duplicate` if two prompts conflict, `-` if none) and a `THINKING` column (`yes`/`-`); JSON rows expose nullable `prompt`/`thinking` objects. These are derived-projection reads — see "Prompt/thinking binding" below.

**`mf article show <PATH>`** — Show article details. `<PATH>` accepts a path (e.g. `docs/weekly.md`) or a title. Also shows the article's bound prompt (path, mode, binding status, updated) and thinking ledger (path, updated) when present — `-`/`null` when absent. When binding status is `duplicate`, every conflicting prompt path is listed.

**`mf article update <PATH>`**
Update article metadata in `mind-index.yaml`.
`--status <draft|published>` — Set article status
`--title <TITLE>` — Change display title (metadata only, does not rename files)

**`mf article rename <OLD_PATH> <NEW_SLUG>`** — Rename an article (slug only). Changes the file/directory path on disk; the title is left unchanged. Use `_title` on `article update` to change the title. Handles both single-file and directory articles. Automatically renames the associated prompt file and updates its `article:` frontmatter binding.

**`mf article remove <PATH>`** (alias `rm`) — Remove an article. Interactive TTY confirmation unless `--yes` or `--force` is set. `<PATH>` accepts the title, the `article_path`, or the index key — with or without a trailing `.md`; all forms resolve to (and remove) the same index entry. No match exits non-zero with `article not found`; never reports success for a no-op.

**`mf article lint`** — Lint articles.

**`mf article convert`**
Convert article shape between directory and single-file.
`--to-single-file` — Collapse eligible single-section directory articles into single-file articles.
`--to-directory` — Expand single-file articles into directory mode with an opening section.
Without a direction flag in a TTY, the CLI infers the unique reasonable direction and prompts for confirmation; non-TTY fails with a usage error if both directions are plausible.
`--dry-run` reports the plan without mutating the filesystem or index.

**`mf article block rename <ARTICLE> <OLD_BLOCK> <NEW_SLUG>`** — Rename a block file within a directory article. `<ARTICLE>` accepts a path (e.g. `docs/my-article`) or a title. `<OLD_BLOCK>` is the current block filename (e.g. `02-notes.md`), filename without extension (e.g. `02-notes`), or just the slug (e.g. `notes`). `<NEW_SLUG>` is the new slug — the number prefix is preserved (e.g. `thoughts` produces `02-thoughts.md`). The H2 heading and file content are NOT changed. Use `--dry-run` to preview. Use `--force` to overwrite an existing target block file. Only works on directory articles; single-file articles should use `mf article rename` instead.

**`mf article index`** — Index articles (mf extension). `-n` is short for `--dry-run`. In the same run, also reconciles the `prompts:` and `thinking:` projections against `prompts/*.md`/`thinking/*.md` on disk (add present files, prune stale entries, keep survivors) — no separate command is needed. The JSON envelope carries additive `prompts`/`thinking` objects (`added`/`removed`/`kept_count`/`scanned_count`), alongside the existing article-level fields. `--dry-run` covers all three stores and writes nothing.

#### Prompt/thinking binding

`prompts/` (control plane: objective, mode, constraints, decisions) and `thinking/` (working ledger) are derived-projection schema, reconciled by `mf article index` and surfaced through `mf article list`/`show` above — there is no standalone `mf prompt`/`mf thinking` command group. `prompts/<key>.md` is the source of truth (frontmatter `article:` and optional `mode:` of `editorial`/`research`/`decision-research`); `thinking/<key>.md` associates with an article purely by key alignment (its own filename stem matching the article's), carries no frontmatter, and has no `duplicate` state. Binding status (`bound`/`orphan`/`duplicate`) is computed at query time against the current `articles` set and is never persisted. Cross-article anomalies (an orphaned prompt, two prompts bound to one article) have no single article row to attach to — they surface only through `mf project lint`'s `orphan_prompt`/`duplicate_binding`/`missing_thinking` rules (see below).

### `mf source` — Manage content sources

Subcommands: `list` (alias `ls`), `new`, `show`, `update`, `rename`, `remove` (alias `rm`), `index`, `clean`.

**`mf source new <INPUT>`**
`-n`, `--name <NAME>` — Source name
`--file-kind <auto|pdf|file|rss|web>` — File kind (mf primary)
`--source-kind <yuque|meeting|misc>` — Source channel type (mind primary)
`-t`, `--type <KIND>` — Deprecated: use `--file-kind` or `--source-kind` instead
`--link` — Symlink instead of copy (local files)
`--register-only` — Index a file that already lives inside the project's `sources/` directory without copying its bytes. Idempotent (re-registering the same path is a no-op). Rejects paths outside `sources/`, URLs, and combination with `--link` or `--force`.

**`mf source list`** (alias `ls`)
`--filter <PATTERN>` — Filter by name
`-t`, `--type <auto|pdf|file|rss|web>` — Filter by file kind

**`mf source show <PATH>`** — Show source details.

**`mf source update <PATH>`**
`--url <URL>` — Update URL
`--rename <NAME>` — Rename source (legacy; prefer `mf source rename`)

**`mf source rename <OLD_PATH> <NEW_PATH>`** — Rename a source by path or name.

**`mf source remove <NAME_OR_PATH>`** (alias `rm`)
`--keep-file` — Remove from index only, keep file on disk

**`mf source index`** — Index sources (mf extension).

**`mf source clean`** — Clean stale index entries.

### `mf asset` — Manage project assets

Subcommands: `list` (alias `ls`), `new`, `show`, `update`, `rename`, `remove` (alias `rm`), `index`, `clean`.

**`mf asset new <PATH>`**
`--name <NAME>` — Asset name
`--tag <TAG>` — Tag (repeatable)
`--copy` — Copy file (mutually exclusive with `--link`)
`--link` — Symlink file (mutually exclusive with `--copy`)

**`mf asset list`** (alias `ls`)
`--filter <PATTERN>` — Filter by name
`--type <image|video|audio|other>` — Filter by kind

**`mf asset show <PATH>`** — Show asset details.

**`mf asset update [PATH]`**
`--set-url <URL>` — Set publish URL
`--channel <CHANNEL>` — Set publish channel
`--all` — Update all assets (mutually exclusive with `PATH`)

**`mf asset rename <OLD_PATH> <NEW_PATH>`** — Rename an asset.

**`mf asset remove <PATH>`** (alias `rm`) — Remove an asset.

**`mf asset index`** — Index assets (mf extension). `--refresh-metadata` recomputes size/hash.

**`mf asset clean`** — Clean stale index entries.

### `mf term` (alias `mf terms`) — Manage terminology

Subcommands: `list` (alias `ls`), `new`, `show`, `update`, `rename`, `remove` (alias `rm`), `correction`, `move` (alias `mv`), `lint`, `fix`.

**Correction model.** `mf term lint`/`fix` deterministically applies *declared* glossary corrections (closed-set, recurring domain terms) under guardrails: it never edits inside a protected term occurrence, honors declared-correction precedence, keeps edits non-overlapping, and writes atomically after diff/confirm. Open-domain ASR errors that no fixed list can enumerate (near-homophone, context-dependent) are yours to correct directly; when one recurs, persist it with `mf term correction add` so the rules path handles it next time. `mf` is the guardrail; you are the open-domain corrector.

**`mf term new <TERM>`**
Create a term (mf extension).
`--definition <TEXT>` — Term definition
`--description <TEXT>` — Long description
`--confidence <N>` — Confidence score
`--alias <TEXT>` — Alias (repeatable)
`--tag <TAG>` — Tag (repeatable)
`--misrecognition <TEXT>` — Common misrecognition variant (repeatable, global and project-scoped)

**`mf term list`** (alias `ls`)
`--filter <PATTERN>` — Match the canonical term name (substring)
`--tag <TAG>` — Match the term's tag field (repeatable; AND semantics)
`--alias <ALIAS>` — Match the term's alias field; does not match the term name (repeatable; AND semantics)
`--has-correction` — Filter to terms that have at least one correction
`--scope project|global|all` — Restrict to a scope; default merges project + global fallback

**`mf term show <TERM>`** — Show term details (term + corrections sub-section).

**`mf term update <TERM>`**
Update term metadata and corrections.
`--definition <TEXT>` — Update definition
`--description <TEXT>` — Update description (`--clear-description` to unset)
`--confidence <N>` — Update confidence 0.0–1.0 (`--clear-confidence` to unset)
`--alias <TEXT>` — Add alias (repeatable)
`--tag <TAG>` — Add tag (repeatable)
`--delete-alias <TEXT>` — Remove an alias (repeatable)
`--delete-tag <TAG>` — Remove a tag (repeatable)
`--add-correction <ORIGINAL[:CORRECT]>` — Append a correction (defaults to word/required; repeatable). Optional `:CORRECT` sets the replacement; a bare `ORIGINAL` uses the term name as the replacement (never an empty `correct`).
`--correction-match <ORIGINAL:KIND>` — Set match kind to `word`, `substring`, or `pinyin` (repeatable). Substring defaults to `standalone`; use `loose` for intentional embedded literal matching. Switching to `pinyin` clears the unused boundary.
`--correction-fix <ORIGINAL:KIND>` — Set fix kind of a correction (repeatable)
`--correction-pinyin <ORIGINAL:PINYIN>` — Set pinyin of a correction (repeatable)
`--delete-correction <ORIGINAL>` — Delete a correction by original (repeatable)
`--dry-run` — Preview planned changes without writing

**`mf term correction <SUBCOMMAND>`** — Manage corrections as a first-class subresource. Targeted operations remain usable while other substring corrections exist.
`add <TERM> <ORIGINAL> <CORRECT>` — Add a correction (idempotent on exact pair match; JSON `data.created` is `true` when newly added, `false` when the pair already existed and storage was left untouched)
  `--match word|substring|pinyin` — Match kind (default: word)
  `--fix required|suggested` — Fix kind (default: required)
  `--boundary loose|standalone` — Boundary mode (default: standalone)
  `--pinyin <TEXT>` — Pinyin string
`list <TERM>` — List all corrections for a term
`show <TERM> <ORIGINAL>` — Show one correction
`update <TERM> <ORIGINAL>` — Update correction attributes (same flags as `add` minus positionals)
`remove <TERM> <ORIGINAL>` — Remove a correction (`--dry-run` to preview)

**`mf term move <TERM>`** (alias `mv`) — Move a term between project and global scopes.
`--to-global` — Move to the global term pool
`--to-project <PROJECT>` — Move to a named project
`--from-global` — Source is the global pool (default: project scope via `-p`)
`--force` — Overwrite if the term already exists at the destination
`--dry-run` — Preview without writing

**`mf term rename <OLD_TERM> <NEW_TERM>`**
Rename a term.
`--keep-alias` — Preserve the old name as an alias on the renamed term.

**`mf term remove <TERM>`** (alias `rm`) — Remove a term. Interactive TTY confirmation unless `--yes` or `--force` is set.

**`mf term lint [PATH]`**
Lint term consistency using `word`, `substring`, and `pinyin`. `substring + loose` performs embedded literal matching; `substring + standalone` suppresses ASCII identifier/path internals and requires CJK jieba token alignment. Pinyin findings are always `fix: suggested` (trailing `?` marker).
`--fix` — Auto-correct term usage in docs (pair with `--dry-run` to preview). Non-TTY exits 2 unless `-y`/`--force` is passed. A `--fix --dry-run` preview lists each finding with its context, confidence, and selection state (`selected`, `excluded_*`, `below_confidence`, `suggested_disabled`, `ambiguous`, …).
`--term <NAME>` or `--term <NAME:ORIGINAL>` — Repeatable; scope to one or more named terms (case-sensitive exact canonical name match) or, with the `NAME:ORIGINAL` form, one specific correction pair. When omitted, all terms are scanned. Unknown name/pair exits 2 with no edits.
`--exclude-term <NAME>` — Repeatable; skip corrections for the named term(s).
`--exclude-original <ORIGINAL>` — Repeatable; drop one exact original text across every term.
`--article <SLUG>` — Set `target_type: "article"` in JSON output; scope hint for downstream tooling.
`--include-suggested` — Apply suggested fixes (pinyin matches) in addition to required corrections.
`--min-confidence <0.0..1.0>` — Apply only suggested corrections at or above the threshold. Requires `--include-suggested`; out-of-range or standalone use exits 2.
`--rule <RULE>` — Restrict to one rule kind.
`--severity <LEVEL>` — Filter at-or-above severity (`error`/`warning`/`info`).
`--max-warnings <N>` — Exit 1 when warnings exceed N.

**`mf term fix [PATH]`**
First-class alias for `term lint --fix`. Same flags as `term lint`, including `--include-suggested`/`--min-confidence` for suggested corrections and `--exclude-term`/`--exclude-original` to narrow scope.
Accepts a repeatable `--term <NAME>` (canonical name, case-sensitive exact match) or `--term <NAME:ORIGINAL>` (a specific correction pair) to scope corrections. When omitted, all terms are applied (unchanged). Naming a term or pair that does not exist in scope exits with code 2 and lists the unknown term(s) on stderr — no edits are made. Deleting a single correction uses a separate existing command: `mf term correction remove <TERM> <ORIGINAL>`.

### `mf build <ARTICLE>` — Build/assemble an article

`-o`, `--output <PATH>` — Output file path
`--dry-run` — Show build plan without rendering

`ARTICLE` may be an indexed article name/slug or a repo-relative path prefixed with `@`, such as `@projects/blog/docs/2026-03-review/`. Directory articles are built by merging Markdown files in filename order. Relative image/link/reference paths are automatically rewritten to resolve from the output directory; paths inside fenced code blocks, absolute paths, and URLs are left unchanged.

### `mf publish` — Publish articles and manage targets

Subcommands: `run`, `update`, `target`.

**`mf publish run <ARTICLE>`**
Publish article to a target (supported: `local`, `yuque-prompt`).
`--target <TARGET>` — Publish target (optional when `publish.default_target` is configured)

Without `--target`, `mf` resolves the configured `publish.default_target` from `mind.yaml`. File-based publishers (`.mind-forge/publisher/<name>.yaml`) are discovered for both explicit and default targets. Local publishers honor `config.prefix` for the destination filename.

**`mf publish update <ARTICLE>`**
Update a `publish_records` entry in `mind-index.yaml`.
`--target <TARGET>` — Target name (required)
`--set <KEY=VALUE>` — Set fields such as `status=published` or `url=<URL>` (repeatable, preferred)
`--status <draft|published|archived>` / `--target-url <URL>` — Accepted compatibility flags; prefer `--set`

**`mf publish target list`** — List publish targets and diagnostics.

**`mf publish target show <NAME>`** — Show publish target details.

### `mf render [ARTICLE]` — Generate render prompts

With an article selector, emit a render prompt without writing output files. The `template` subcommand inspects templates.

**`mf render template list`** — List built-in and project-local render templates.

**`mf render template show <NAME>`** — Show template details, including a preview of the first lines.

### `mf config` — Manage configuration

Subcommands: `schema`, `show`, `generate`, `default`, `terminal`, `init` (deprecated).

**`mf config schema`** — Show config JSON schema. `--output-format <json|yaml>` (default: `json`).

**`mf config show`** — Show effective config. `--output-format <json|yaml>` (default: `yaml`). JSON envelope `data` is the canonical config object (no embedded YAML string).

**`mf config generate`** — Generate effective config file. `--output-format <json|yaml>` (default: `yaml`), `-o, --output <PATH>`.

**`mf config default`** — Show default config values. `--output-format <json|yaml>` (default: `yaml`). Generated config includes a `plugins.typora-front-matter` block with `enabled: true`.

**`mf config terminal`** — Show terminal capability diagnostics (hyperlink support, color depth, terminfo probing). Environment-driven detection respects `TERM`, `COLORTERM`, `TERM_PROGRAM`, `NO_COLOR`, `MF_FORCE_HYPERLINKS`, and `MF_NO_HYPERLINKS`.

**`mf config init`** — Deprecated compatibility command; use `mf init`. Supports `--output`, `--target <project|repo>`, and `--force`.

### `mf completion <SHELL>` — Generate shell completion scripts

Supported shells: `bash`, `zsh`, `fish`, `powershell`, `elvish`.

### `mf version` — Show version information

Text format: `mf {base}-dev+{short_commit} (built {build_date}, rustc {rustc})`.
JSON envelope `data`: `{ version, base_version, channel, commit, build_date, rustc, target_triple }`.

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
mf project update my-project --description "Writing workspace"
mf project update my-project --clear-description
mf project rename old-name new-name
mf project remove obsolete --yes                       # non-interactive
mf project archive completed
mf project lint --project my-project --fix
mf project import /path/to/existing --force

# Sources
mf source new https://example.com/article --name ref-a --file-kind web --project my-project
mf source new paper.pdf --file-kind pdf --project my-project
mf source new sources/file/existing.md --register-only --project my-project  # index in place, no copy
mf source list --project my-project
mf source show ref-a --project my-project
mf source rename ref-a ref-canonical --project my-project
mf source index --project my-project
mf source update ref-canonical --url https://example.com/v2 --project my-project
mf source remove sources/yuque/foo.md --keep-file --project my-project
mf source clean --dry-run --project my-project

# Assets
mf asset new image.png --project my-project
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
mf article update docs/my-first-post --status published --project my-project
mf article update docs/my-first-post --title "Better Title" --project my-project
mf article rename docs/old-title new-slug --project my-project
mf article index --project my-project
mf article lint --fix --project my-project
mf article convert --to-single-file --project my-project
mf article convert --to-directory --project my-project
mf article convert --dry-run --project my-project

# Build & publish
mf build my-first-post --project my-project
mf build @projects/my-project/docs/2026-03-review/ --output ./_build/review.md
mf build my-first-post --dry-run --project my-project
mf publish run "My First Post" --target local --project my-project
mf publish run "My First Post"                         # uses publish.default_target
mf publish run "My First Post" --target yuque-prompt --project my-project
mf publish update "My First Post" --target local --set status=published --project my-project
mf publish target list
mf publish target show local

# Render templates
mf render template list
mf render template show arch

# Prompts & thinking (writing-workflow bindings) — surfaced via `mf article`
mf article index --project my-project                  # reconciles articles + prompts: + thinking: in one run
mf article list --project my-project                   # PROMPT/THINKING columns per article
mf article show my-first-post --project my-project      # bound/orphan/duplicate status + thinking presence
mf project lint --project my-project                    # surfaces orphan_prompt/duplicate_binding/missing_thinking

# Terms
mf term new "Zettelkasten" --definition "A note-taking method" --project my-project
mf term new "API" --alias "Application Programming Interface" --tag tech
mf term list
mf term list --tag tech --has-correction                     # AND-filter: tech tag + has correction
mf term list --scope project                                 # project-only (no global fallback)
mf term list --scope global                                  # global pool only
mf term show Zettelkasten --project my-project
mf term update "API" --definition "Updated definition" --project my-project
mf term update "API" --tag tech --delete-alias "old-alias" --dry-run  # preview update
mf term update "API" --add-correction "api:API" --project my-project   # add correction inline (ORIGINAL:CORRECT)
mf term update "API" --correction-match "api:pinyin"                   # set match kind (auto-clears standalone boundary)
mf term update "API" --correction-fix "api:suggested"                  # set fix kind
mf term update "API" --delete-correction "api"                         # remove correction
mf term correction add "API" "api" "API"                     # add correction (subcommand)
mf term correction list "API"                                # list corrections
mf term correction update "API" "api" --fix suggested        # update correction attribute
mf term correction remove "API" "api"                        # remove correction (subcommand)
mf term move "API" --to-global                               # project → global
mf term mv "API" --from-global --to-project my-project       # global → project (alias)
mf term rename "API" "Application API" --keep-alias --project my-project
mf term remove obsolete-term --yes --project my-project
mf term lint --project my-project
mf term lint --project my-project --fix --dry-run            # preview corrections
mf term lint --project my-project --fix --include-suggested  # apply suggested pinyin fixes too
mf term lint docs/my-article.md --project my-project        # scan a specific file only
mf term lint --article weekly-note --json --project my-project  # article target, JSON output
mf term fix --project my-project                             # alias for term lint --fix
mf term fix --project my-project --include-suggested         # apply suggested corrections
mf term fix --project my-project --term RAG                  # scope to one term only
mf term fix --project my-project --term RAG --term LLM       # scope to multiple terms
mf term fix --project my-project --term RAG:rag              # scope to one correction pair
mf term fix --project my-project --exclude-term RAG          # apply everything except RAG
mf term fix --project my-project --exclude-original apis     # skip one original across terms
mf term fix --project my-project --include-suggested --min-confidence 0.8  # suggested ≥ 0.8

# Config
mf config show
mf config default
mf config generate -o minds.yaml
mf config terminal
mf config terminal --json

# Shell & diagnostics
mf completion zsh
mf version --json
```

## Notes

- Commands that modify project state require a Mind Repo context. `config`, `completion`, `version`, and help can run outside repos. `project index` can also run outside repos (scans from cwd). `article list` without `--project` outside a project dir auto-matches all projects and sorts by most recently modified.
- `mf article convert` supports bidirectional shape conversion (`--to-single-file` / `--to-directory`). Without a direction flag in a TTY, it infers the unique reasonable direction; non-TTY requires an explicit flag.
- `--project` accepts repo-relative paths or project names. When running inside a project directory, it can be omitted — the CLI auto-detects the current project.
- Project identity is path-based: `mf project new` accepts cwd-relative or repo-relative paths with Unicode, emoji, dates, spaces, and underscores.
- `mf project update` currently updates `project.description`; `mf article update` updates indexed article `status` and/or `title`.
- Index subcommands reconcile `mind-index.yaml` with the filesystem; run them after manual file changes.
- Prefer `schema` over `schema_version` in docs, examples, and generated YAML.
- Global terms (created without `--project`) are stored in `minds-terms.yaml` at the repo root. Project-scoped terms are stored in each project's `mind-index.yaml`.
- `term lint` requires a project context — it scans project docs for term usage.
- Destructive verbs (`remove`, `archive`) prompt on `/dev/tty` in interactive shells. In scripts/CI pass `--yes` to confirm; add `--force` separately only when bypassing overwrite or referential-integrity checks is intended.
- `term lint --fix` and `term fix` treat `--force` as an alias for `--yes`; do not generalize that exception to entity removal.
- `term fix` and `term lint --fix` accept `--term <NAME>` or `--term <NAME:ORIGINAL>` (repeatable) to scope corrections to named terms or a specific correction pair, plus `--exclude-term`/`--exclude-original` to narrow, and `--include-suggested`/`--min-confidence <0.0..1.0>` for suggested corrections (`--min-confidence` requires `--include-suggested`). Matching is case-sensitive exact on canonical name. Unknown term names exit 2 with no edits. Run `mf term correction remove <TERM> <ORIGINAL>` to delete a single correction.
- `mf source new --register-only` indexes a file already inside the project's `sources/` directory without copying its bytes. It is idempotent and cannot combine with `--link` or `--force`; paths outside `sources/` or URL inputs exit 2.
- `term update` manages corrections through `--add-correction`, `--correction-match`, `--correction-fix`, `--correction-pinyin`, and `--delete-correction`. Match kinds are `word`, `substring`, and `pinyin`; substring defaults to `standalone` and supports explicit `loose` matching.
- `term show`, `term update`, and `term remove` operate on the selected correction without requiring sibling substring entries to be migrated first.
- `article convert` evaluates eligible articles project-wide; it does not take an article selector.
- `mf init` is the preferred bootstrap command; `mf config init` remains deprecated compatibility.
- `mind-index.yaml`'s `prompts:`/`thinking:` sections are reconciled caches, never the source of truth — that's always the Markdown files under `prompts/`/`thinking/`. There is no standalone `mf prompt`/`mf thinking` command group; binding info is surfaced through `mf article list`/`show` and reconciled by `mf article index`, and binding status is computed at query time and never persisted. `article rename`/`article convert` keep both files and both projections consistent automatically; a manual `mf article index` is only needed after hand-edited files.
