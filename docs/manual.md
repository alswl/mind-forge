# mind-forge Manual

`mf` is a local-first CLI for mind 0.3.0-compatible knowledge repos. It stores
projects, articles, sources, assets, terms, builds, and publish state in plain
Markdown and YAML files so the repo can be edited by humans, scripts, and AI
agents.

## 1. Install

Requires Rust 1.91+.

```bash
git clone https://github.com/alswl/mind-forge.git
cd mind-forge
cargo install --path .
```

Run from source while developing:

```bash
cargo run -- --help
```

Generate shell completion:

```bash
mf completion zsh
```

Supported shells: `bash`, `zsh`, `fish`, `powershell`, `elvish`.

## 2. Repository Model

A Mind Repo is a directory rooted at `minds.yaml`.

```text
my-repo/
  minds.yaml
  projects/
    blog/
      mind.yaml
      mind-index.yaml
      docs/
      sources/
      assets/
      outputs/
```

Core files:

| File | Purpose |
| --- | --- |
| `minds.yaml` | Repo manifest and project list |
| `mind.yaml` | Project metadata and layout configuration |
| `mind-index.yaml` | Project index for articles, sources, assets, terms, and publish records |
| `minds-terms.yaml` | Global glossary terms |

All generated YAML uses `schema: "1"` and mind 0.3.0-compatible shapes.

## 3. First Repo

```bash
mkdir my-repo
cd my-repo
mf init

mf project new blog
mf article new "First Post" --project blog
mf source new https://example.com/ref --file-kind web --project blog
mf asset new diagram.png --project blog
mf term new "Zettelkasten" --definition "A note-taking method" --project blog

mf article index --project blog
mf build "First Post" --project blog
mf publish run "First Post" --target local --project blog
```

## 4. Shared CLI Rules

Global flags:

| Flag | Description |
| --- | --- |
| `--root <PATH>` | Mind Repo root directory |
| `--config <PATH>` | Config file path |
| `-p`, `--project <PROJECT>` | Project selector for project-scoped commands |
| `-o`, `--output <text|json>` | Output format (default: `text`) |
| `--json` | Shorthand for `--output json` |
| `-q`, `--quiet` | Suppress successful output (diagnostics and exit codes remain) |
| `--no-color` | Disable colored output |

Shared mutating flags:

| Flag | Applies to | Description |
| --- | --- | --- |
| `--dry-run` | Mutating commands | Preview without writing |
| `--force` | Create, rename, remove, archive | Proceed despite safety checks: overwrite an existing target, or remove an entity referenced by others |
| `--yes` | Remove, archive | Confirm destructive actions in scripts |

JSON output always uses:

```json
{
  "status": "ok",
  "command": "project.update",
  "data": {}
}
```

`data` is always an object. Lists expose stable `identity` values that can be
passed back to `show`, `update`, `rename`, and `remove`.

## 5. Projects

Projects are path-backed workspaces under a Mind Repo.

```bash
mf project new blog
mf project list
mf project show blog
mf project update blog --description "Writing workspace"
mf project update blog --clear-description
mf project rename blog writing/blog
mf project remove writing/blog --yes
mf project archive writing/blog --yes
mf project lint --project writing/blog --fix
mf project index
mf project import /path/to/existing --force
```

Notes:

- Project identity is repo-relative and may include Unicode, emoji, dates,
  spaces, and nested paths.
- `project update` currently updates only `project.description` in `mind.yaml`.
- `project rename` renames the directory and updates repo/project indexes.
- `project archive` moves the project to `_archived/`.
- `project index` reconciles the repo project list and each project's article
  index: stale article entries whose target file no longer exists on disk are
  pruned; entries whose files exist (including declared and template-origin
  articles outside `docs/`) are never removed. A project whose
  `mind-index.yaml` fails to load is skipped with a warning (stderr, and
  `data.warnings` in JSON) rather than silently.

## 6. Articles

Articles are Markdown writing units inside a project. The default article shape
is a directory article under `docs/`; pass `--file` for a single Markdown file.

```bash
mf article new "First Post" --project blog
mf article new "Architecture Review" --template arch --project blog
mf article new "Quick Note" --file --project blog

mf article list --project blog
mf article show docs/first-post --project blog
mf article update docs/first-post --status published --project blog
mf article update docs/first-post --title "Better Title" --project blog
mf article rename docs/first-post first-post-v2 --project blog
mf article remove docs/first-post-v2 --project blog --yes

mf article convert docs/quick-note --to-directory --project blog
mf article convert docs/quick-note --to-single-file --project blog
mf article index --project blog
mf article lint --project blog --fix
```

Notes:

- Article selectors accept a project-relative article path or an unambiguous
  title.
- `article remove` accepts the title, the `article_path`, or the index key —
  with or without a trailing `.md`; all forms resolve to (and remove) the same
  index entry.
- `article update` updates article metadata in the index:
  `--status draft|published` toggles publication status,
  `--title "New Title"` changes the display title without renaming files.
- `article rename` changes only the article slug (file/directory path); the
  title is left unchanged. Use `--title` on `article update` to change the title.
- `article list` without `--project` outside a project scans all projects and
  sorts by most recently modified.

## 7. Sources

Sources track reference material used by a project.

```bash
mf source new https://example.com/ref --name ref-a --file-kind web --project blog
mf source new paper.pdf --file-kind pdf --project blog
mf source new sources/file/existing.md --register-only --project blog
mf source new report.pdf --project blog --article docs/my-analysis.md
mf source list --project blog
mf source show ref-a --project blog
mf source update ref-a --url https://example.com/v2 --project blog
mf source rename ref-a ref-canonical --project blog
mf source remove ref-canonical --keep-file --project blog
mf source index --project blog
mf source clean --dry-run --project blog
```

Use `source rename` for identity changes. `source update --rename` remains
available for compatibility.

`--register-only` indexes a file that already lives inside the project's
`sources/` directory without copying or moving its bytes. It is idempotent —
re-registering the same path is a no-op — and cannot be combined with `--link`
or `--force`.

`--article <PATH>` records the originating article (project-relative) that
introduced this Source, persisted as authoritative `imported_by` provenance and
returned with every search hit for the Source. The path must stay within the
project; an escaping path is a usage error.

### 7.1 Source RAG (LanceDB)

`mf source` is the repository-wide LanceDB backend for Sources. Its
data lives at `.mind-forge/cache/source/advanced/`, is local and rebuildable
(gitignored via `.mind-forge/.gitignore`), and uses a five-table LanceDB store
(registrations, documents, registration_content, chunks, enrichments) with
atomic snapshot publication and retained-snapshot recovery.

#### Quick start

```bash
# From the Mind Repo root
mf source sync --offline
mf source status
mf search "your query"
```

#### Search

```bash
mf search <QUERY> [-o text|json] [--project NAME] [--source NAME] [--limit N]
                        [--revision N|DATE]
```

- Searches Sources, authored article prose, Prompts, Thinking, project goals,
  and repository terms.
- **--revision** — integer revision number, ISO date, or relative date

Default output is a text table; `-o json` for machine-readable envelopes.

Each hit carries a structured `context` on every `registrations[]` entry
(schema v2): `repository`, `project_identity`, `project_goal`, `content_kind`,
`lifecycle_status`, `relations` (internal links + prompt/thinking siblings;
dangling targets are `resolved:false`), and — for Source hits — `imported_by`
provenance (`project` plus the originating `article` when captured). Single-owner
content (article/project/term) also folds a compact context preamble into its
embedded chunk text so attribution participates in matching. Search is read-only.

#### Management

```bash
mf source status
mf source sync --rebuild            # schema-drift recovery in the sync family
mf source admin rebuild [--dry-run]
mf source admin clear --yes
mf source admin recover --snapshot ID --yes
mf source export|import|trace
```

`mf source sync` reports per-kind `coverage` and item-by-item `skipped_items`
(each with a `reason`: `empty`/`binary`/`excluded`/`error`), so indexing is
auditable and nothing is silently dropped. `--dry-run` reports without writing
or fetching.

The RAG storage schema is `v3`. Schema compatibility is read from the
`registrations` table's actual on-disk structure — never a recorded value —
so it cannot go stale or be hand-edited. A repository whose tables predate
what the build requires refuses `search`/`sync` with an actionable
diagnostic pointing at `mf source sync --rebuild`; run that once to
regenerate the index and adopt the current schema (no migration shim, and
no detour through `source admin rebuild`).

Machine-local activation state (gitignored, per-worktree) carries only this
machine's activation status — nothing else. It cannot go stale relative to
the corpus on disk: `mf source status`/`mf source sync` read the corpus
directly, so deleting the local state file self-heals with zero manual
edits — `status` reports what it finds and `sync` adopts an existing corpus
or activates from scratch, whichever applies.

**Storage authority** — who owns what, and what that implies for recovery:

| Dataset | Authority | Notes |
|---|---|---|
| Files on disk (`sources/**`, docs, prompts, thinking) | Fact | Recoverable only from version control |
| `mind-index.yaml` → `terms:` | Fact, permanently | The store never owns terms |
| `mind-index.yaml` → `sources:` | Lossless mirror of the store | Reproduces every field, including timestamps and unrecognised ones |
| Registration store (LanceDB) | Authoritative store | Holds the full source record: identity, location, tags, `added_at`/`updated_at`, and an `extras_json` passthrough for anything it does not itself interpret |
| Machine-local state | This machine's activation status only | A single field; everything else is read from the corpus on disk |
| Corpus generations, snapshots, pointer | Authoritative record on disk | The source of generation and schema facts |

Two invariants hold across every intermediate state:

- **Divergence resolves by import, never by deletion.** When the index and
  the store disagree, the store learns the index's content; an index entry
  the store doesn't (yet) know about is never dropped to force agreement.
  `mf source index` is a disk-adoption and reconcile pass on this backend:
  files present on disk but unknown to the store are imported (reported
  under `imported`, with populated paths); registrations whose file has
  vanished are reported as `missing`, never deleted — removal happens only
  through the explicit `mf source remove` / `mf source clean`.
- **A writer may only project a key it fully holds.** Source-side commands
  never touch `terms:`; article/prompt/thinking commands never touch
  `sources:`. Each writer stays confined to the subtree it owns.

#### Embedding provider

Configure an OpenAI-compatible `/v1/embeddings` endpoint in `minds.yaml`:

```yaml
source:
  backend: lance
  advanced:
    embedding_endpoint: "https://api.siliconflow.cn/v1"
    embedding_model: "BAAI/bge-m3"
    embedding_dimension: 1024
    embedding_api_key: "${SF_API_KEY}"
```

Credentials resolve from a gitignored `minds-secrets.yaml` (`SF_API_KEY: sk-...`)
or the `MF_EMBEDDING_API_KEY` env var; they are never serialized.

#### Claude enrichment

The experimental enrichment CLI is intentionally not part of the terminal
surface. Existing enrichment data remains durable and is not migrated or deleted.

## 8. Assets

Assets are project files, usually images or other media.

```bash
mf asset new diagram.png --project blog
mf asset new logo.png --copy --project blog
mf asset list --project blog
mf asset show diagram.png --project blog
mf asset update diagram.png --dry-run --project blog
mf asset update --all --project blog
mf asset update --set-url https://cdn.example.com/diagram.png --project blog
mf asset rename diagram.png diagrams/system.png --project blog
mf asset remove diagrams/system.png --project blog
mf asset index --refresh-metadata --project blog
mf asset clean --project blog
```

`asset update` recomputes each asset's size and hash from the file on disk.
Pass `<PATH>` to refresh a single asset — a path always selects this metadata
contract, even when the legacy `--set-url`/`--channel` options are also
supplied — or `--all` to refresh every registered asset. Both forms honor
`--dry-run` and report the prospective changes without writing (the `--all`
JSON output carries `kind: "asset"` and a `{ total, changed, missing }`
summary). The legacy publish-URL form (`--set-url`/`--channel`, no path) stores
a project-level publish setting instead.

## 9. Terms

Terms can be global or project-scoped. Global terms live in `minds-terms.yaml`;
project terms live in `mind-index.yaml`.

Corrections follow two paths. **Rules** are the declared glossary corrections
that `mf` applies deterministically: `mf term lint`/`fix` rewrites recurring,
closed-set domain terms in project docs under guardrails — it never edits inside
a protected term occurrence, honors declared-correction precedence, keeps edits
non-overlapping, and writes atomically after diff/confirm. **Open-domain** errors
that no fixed list can enumerate — near-homophone or context-dependent ASR
mistakes — are corrected by the agent driving `mf`; once such an error recurs,
persist it with `mf term correction add` so the rules path catches it next time.
In short, `mf` owns the deterministic guardrails; the agent owns the open-domain
judgment.

```bash
mf term new "API" --definition "Application Programming Interface"
mf term new "Zettelkasten" --definition "A note-taking method" --project blog
mf term list
mf term show API
mf term new API --alias "application programming interface"
mf term update API --definition "Interface contract" --tag architecture
mf term rename API "Application API" --keep-alias
mf term remove obsolete-term --yes
mf term lint --project blog
mf term lint --project blog --fix --dry-run
mf term fix --project blog
mf term fix --project blog --term API
mf term fix --project blog --term API:api --exclude-original apis
mf term fix --project blog --include-suggested --min-confidence 0.8
```

`term update` changes metadata and corrections. Use `--add-correction
<ORIGINAL[:CORRECT]>` to append a correction to an existing term; the optional
`:CORRECT` sets the replacement text, and a bare `ORIGINAL` uses the term's own
name as the replacement (never an empty correction). Use `--correction-match`/
`--correction-fix`/`--correction-pinyin` to set correction attributes.
`--correction-match` accepts `word`, `substring`, or `pinyin`. Substring uses
the safe `standalone` boundary by default; set `--correction-boundary
<ORIGINAL:loose>` when embedded literal matching is intentional. Switching
`--correction-match` to `pinyin` automatically clears the boundary because the
pinyin scanner does not use it. To remove a single
correction, use
`--delete-correction` or the subcommand `term correction remove`.
Use `--dry-run` to validate and preview without writing.

`term show`, `term update`, and `term remove` can inspect and mutate any
supported correction independently. Other substring corrections do not block a
targeted update or removal, so migration never requires hand-editing YAML.

`term fix` and `term lint --fix` accept a repeatable `--term <NAME>` flag to
scope corrections to one or more named terms (case-sensitive exact match on
the canonical name). Pass `--term <NAME:ORIGINAL>` to target a single correction
pair instead of the whole term. When omitted, all terms are applied (unchanged).
Naming a term that does not exist in scope exits with code 2 and lists the
unknown term(s) on stderr — no edits are made. Deleting a single correction is a
separate existing command: `mf term correction remove <TERM> <ORIGINAL>`.

To narrow rather than widen, use `--exclude-term <NAME>` (repeatable) to skip a
named term or `--exclude-original <ORIGINAL>` (repeatable) to drop one exact
original text across every term. Suggested corrections are off by default; add
`--include-suggested` to apply them, and `--min-confidence <0.0..1.0>` to apply
only suggested corrections at or above the threshold (`--min-confidence` requires
`--include-suggested`). A `--fix --dry-run` preview reports each finding with its
context, confidence, and selection state so you can see why a correction was or
was not applied.

### Corrections

Corrections are a first-class subresource of a term:

```bash
mf term correction add API api API --match word --dry-run
mf term correction list API
mf term correction show API api
mf term correction update API api --fix suggested --dry-run
mf term correction remove API api --dry-run
```

`correction add` is idempotent on an exact `(original, correct)` pair. A first
add reports `added` (JSON `created: true`); repeating the same pair reports
`already exists, skipped` (JSON `created: false`) and leaves storage untouched.
`add`, `update`, and `remove` all accept `--dry-run` to validate and preview
without writing (JSON `dry_run: true`). For boundary and pinyin matching
details, see [term-lint.md](term-lint.md).

### Moving between scopes

```bash
mf term move API --to-global --project blog
mf term mv API --to-project blog --from-global
mf term move API --to-project other --project blog --force
```

A move that would overwrite an existing term at the destination is rejected
unless `--force` is given; the source copy is preserved on rejection.

### Listing and targeted checks

```bash
mf term list --tag architecture --has-correction
mf term list --scope global
mf term lint --project blog --article weekly-note
mf term lint docs/draft.md --project blog
```

List filters use AND semantics. `--scope` accepts `project`, `global`, or `all`
(default: project terms with a global fallback). `term lint`/`term fix` can
target a whole project, a single `--article`, or one Markdown file path.

`term fix` is a first-class alias for `term lint --fix`. For boundary and
pinyin matching details, see [term-lint.md](term-lint.md).

CJK `word` corrections use jieba word segmentation. Substring corrections support
two explicit profiles: `standalone` (default) also requires CJK token alignment,
while `loose` preserves literal matching inside longer text. For ASCII,
`standalone` suppresses identifier/path-internal matches and `loose` allows them.

## 10. Build And Publish

Build assembles an article into an output Markdown file. Directory articles are
merged in filename order, and relative image/link/reference paths are
automatically rewritten to resolve from the output directory so links remain
correct in the built artifact.

```bash
mf build first-post --project blog
mf build @projects/blog/docs/first-post/ --output ./_build/first-post.md
mf build first-post --dry-run --project blog
```

Publish sends built output to a configured target.

```bash
mf publish run "First Post" --target local --project blog
mf publish run "First Post" --target yuque-prompt --project blog
mf publish update "First Post" --target local --status published --project blog
mf publish target list
mf publish target show local
```

Supported publish targets currently include `local` and `yuque-prompt`.

`publish run` resolves the effective target from an explicit `--target` flag,
or from the configured `publish.default_target` in `mind.yaml` when `--target`
is omitted. File-based publishers (`.mind-forge/publisher/<name>.yaml`) are
discovered for both paths. Local publishers respect `config.prefix` in the
publisher definition for the destination filename.

### Publish-time transforms (all target types)

Both `local` and `yuque-prompt` targets support these config-driven transforms:

| Config Key | Effect | Default |
|------------|--------|---------|
| `config.svg_to_png` | Replace `.svg` image refs with `.png` when a sibling `.png` exists next to the build artifact | `false` |
| `config.banner_markdown` | Prepend inline markdown banner at publish time (takes precedence over `banner_file`) | — |
| `config.banner_file` | Prepend banner from a project-relative file | — |

Banner injection is idempotent — re-running publish does not stack a second banner.

### yuque-prompt target

`yuque-prompt` writes a publish-ready file to `outputs/<stem>.yuque.md`
(deterministic path next to the build artifact) in addition to its usual
stdout prompt/envelope output. Override the path with `config.out_path`
(project-relative).

The JSON outcome envelope gains a `destination` field (the written file path,
or absent under `--dry-run`).

For publishing to yuque.com:

```bash
mf publish run <article> --target <yuque-prompt>
yuque update doc <ns/slug> --body-file outputs/<article>.yuque.md --upload-images
```

:::info cards (author-in-source): for highlight/callout blocks, write them in
the article or block source file. `mf build` preserves `:::` blocks verbatim,
so cards survive every rebuild without hand-patching.

### Private content (author-in-source)

Keep private notes and internal discussion in an article's source without
ever shipping them, using either of two mind-forge markers:

```markdown
> [!mf-private] discussion with a reviewer
> This stays in the source but is stripped from every build/publish artifact.

---
mind-forge-visibility: private
---
```

- A `> [!mf-private]` (or explicit `> [!mind-forge-private]`) callout hides an
  in-place span — an aside, a paragraph, a whole discussion section — anywhere
  in an article. Any callout in the `mf-*` / `mind-forge-*` namespace is
  private by default.
- In a directory (multi-file) article, a block file's `mind-forge-visibility:
  private` front-matter key excludes that entire block from the merged build.
  `public` (or no key) is the default and is unchanged.
- Both forms are stripped at `mf build`, so `mf publish` (which reads the
  build artifact) never sees them either. Source files are never modified —
  private content stays fully visible and editable on every rebuild.
- Fail-safe: an unrecognized `mind-forge-visibility` value, or marking the
  article's first/title block private, fails the build and is reported by
  `mf article lint` rather than silently publishing.
- Markers written inside a fenced code block (to document the syntax itself)
  are left as literal text.

## 11. Render Templates

```bash
mf render template list
mf render template show arch
```

Built-in article templates include `blank`, `arch`, `prd`, and `blog`. A
project-local Markdown template path may also be passed to `article new
--template`.

## 12. Config

```bash
mf config schema
mf config show
mf config generate -o minds.yaml
mf config default
mf config terminal
```

`mf init` is the repo bootstrap command. Use it to create `minds.yaml` and the
default `projects/` container in a new or existing directory.

`mf config terminal` reports terminal capability detection for colors,
hyperlinks, and relevant environment overrides.

## 13. Scripting

Prefer JSON output in scripts:

```bash
mf project update blog --description "Writing" --json
mf article update docs/first-post --status published --title "New Title" --project blog --json
```

Use `--dry-run` before mutating commands:

```bash
mf project rename blog writing/blog --dry-run --json
mf article remove docs/old-post --project blog --dry-run --json
```

Use `--yes` for destructive commands in non-interactive environments:

```bash
mf project remove old-project --yes
mf article remove docs/old-post --project blog --yes
```

## 14. Troubleshooting

Common checks:

```bash
mf project lint --project blog
mf article index --project blog --dry-run
mf source clean --project blog --dry-run
mf asset index --project blog --refresh-metadata
mf config terminal --json
mf version --json
```

If a command cannot find a project, pass `--project <PROJECT>` explicitly or
run it from inside the project directory.

If manually edited YAML is rejected, check that it uses `schema: "1"` and
dictionary sections for indexed resources.
