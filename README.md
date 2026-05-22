# mind-forge

**A local-first, AI-native CLI for card-based writing.**

`mf` treats your knowledge base as a codebase. Articles are assembled from
composable Blocks, every piece of state lives in plain files on disk, and the
CLI is shaped so both humans and Agents can drive it.

## Philosophy

Three ideas guide every decision in `mf`:

### Diffusion

Knowledge is meant to spread. Capture it once as a Block, then let it
diffuse — through articles, glossary terms, builds, and downstream
publishers like Yuque or static sites. The same atomic unit can land in a
report today and a paper tomorrow, without copy-paste drift.

```mermaid
flowchart LR
  subgraph Capture
    B((Block))
    S[Source]
    T[Term]
  end
  subgraph Compose
    A1[Article: Report]
    A2[Article: Paper]
  end
  subgraph Ship
    P1[Publisher: Yuque]
    P2[Publisher: Local]
  end
  B --> A1
  B --> A2
  S --> A1
  T --> A2
  A1 --> P1
  A1 --> P2
```

### DaC — Document as Code

Your writing follows the same discipline as your infrastructure:

declarative YAML configs (`minds.yaml`, `mind.yaml`, `mind-index.yaml`),
schema validation, deterministic builds, and full git auditability.
If you can review a PR, you can review a chapter.

### AI Native CLI

`mf` is designed first for AI Agents, not for human terminal sessions.
Every command speaks a JSON envelope (`{ status, command, data }`), exits
with stable codes, and produces deterministic output contracts.
Build a pipeline with shell, Make, or an LLM — the contract is the same.

This is an independent philosophy, not a subset of DaC: AI Native CLI
rejects interactive prompts, colored output designed for human eyes, and
inconsistent exit codes. The tool is a reliable API for an LLM to call.

Local-first underpins all three: no cloud, no lock-in, plain markdown and
YAML you can edit in any editor.

## Install

Requires Rust 1.75+.

```bash
git clone https://github.com/alswl/mind-forge.git
cd mind-forge
cargo install --path .
```

Or run from source while iterating:

```bash
cargo run -- --help
```

Shell completion:

```bash
mf completion zsh   # or bash | fish | powershell | elvish
```

## Quick Start

```bash
# 1. Create a Mind Repo
mkdir my-repo && cd my-repo
mf config init                         # creates minds.yaml

# 2. Create a project (path-based identity, Unicode/emoji/dates supported)
mf project new blog
mf project new workspaces/team/projects/2026-W21

# 3. Create an article
mf article new "First Post" --project blog

# 4. Add sources, assets, and terms
mf source add https://example.com/ref --file-kind web --project blog
mf asset add diagram.png --project blog
mf term new "Zettelkasten" --definition "A note-taking method" --project blog

# 5. Index, build, and publish
mf article index --project blog
mf build "First Post" --project blog
mf publish run "First Post" --target local --project blog
```

## Core Concepts

| Concept          | What it is                                                                 |
| ---------------- | -------------------------------------------------------------------------- |
| **Mind Repo**    | A directory rooted at `minds.yaml`. The outermost unit of organization.    |
| **Project**      | A subdirectory with `mind.yaml`. Default layout: `docs/`, `sources/`, `assets/`, `outputs/`. |
| **Article**      | A document — either a single Markdown file or a directory of ordered files. |
| **Block**        | An atomic, reusable unit of content composed into articles.                |
| **Source**       | An external reference (web page, PDF, RSS feed, file) tracked per project. |
| **Asset**        | A binary or non-text resource attached to a project.                       |
| **Index**        | `mind-index.yaml` per project — the source of truth for everything above.  |
| **Publisher**    | A target (e.g. `local`, `yuque-prompt`) that ships built output somewhere. |

All on-disk YAML follows the mind 0.3.0 format (`schema: "1"`), so repos move
freely between `mf` and other mind-compatible tools.

How the pieces fit on disk:

```mermaid
flowchart TD
  Repo[Mind Repo<br/><code>minds.yaml</code>]
  Repo --> P1[Project: blog<br/><code>mind.yaml</code>]
  Repo --> P2[Project: papers<br/><code>mind.yaml</code>]
  P1 --> Docs[docs/]
  P1 --> Sources[sources/]
  P1 --> Assets[assets/]
  P1 --> Outputs[outputs/]
  P1 --> Idx[<code>mind-index.yaml</code>]
  Docs --> A1[essay.md<br/>file article]
  Docs --> A2[2026-review/<br/>directory article]
  A2 --> A2a[01-intro.md]
  A2 --> A2b[02-details.md]
```

## Workflow

A typical loop:

```mermaid
flowchart LR
  C[Capture<br/><code>mf source</code><br/><code>mf asset</code><br/><code>mf term</code>]
  D[Draft<br/><code>mf article new</code>]
  I[Index<br/><code>mf … index</code><br/><code>mf project lint --fix</code>]
  B[Build<br/><code>mf build</code>]
  P[Publish<br/><code>mf publish run</code>]
  C --> D --> I --> B --> P
  P -. new insights .-> C
```

1. **Capture** — `mf source add` and `mf asset add` pull raw material into a
   project. `mf term new` records vocabulary.
2. **Draft** — `mf article new <TYPE> <TITLE> [--template <S>] [--file]`
   scaffolds a directory article (default) or single file (`--file`) under
   `docs/`. The default template is `blank`; `--template arch|prd|blog`
   selects another built-in scaffold, and `--template <path>` reads a
   project-local Markdown template. New articles automatically get Typora
   front matter (`typora-copy-images-to`) pointing to the project assets
   directory (disable with `plugins.typora-front-matter.enabled: false` in
   `mind.yaml`). Edit in any Markdown editor.
3. **Index** — `mf source index`, `mf article index`, and
   `mf project lint --fix` reconcile `mind-index.yaml` with the filesystem.
4. **Build** — `mf build <article>` assembles output (directory articles
   merge their files in filename order) into `outputs/<article>.md`.
5. **Ship** — `mf publish run … --target <publisher>` pushes to a configured
   target.

Every step is idempotent and pipe-friendly. Pass `--json` to any command to
get a machine-readable envelope.

## Command Reference

### Flags

These flags are available on most commands:

| Flag | Description |
|------|-------------|
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

`--project` is available on project-scoped commands (`article`, `asset`,
`source`, `term`, `build`, `publish run`, etc.) and accepts repo-relative
paths or project names. When running inside a project directory,
`--project` can be omitted — the CLI auto-detects the current project.

### `mf project` — Manage projects

| Subcommand | Description |
|-----------|-------------|
| `new <PATH>` | Create a project. Accepts cwd-relative or repo-relative paths with Unicode, emoji, dates, spaces. `--template <TEMPLATE>`, `--force` |
| `list` (ls) | List projects |
| `archive <NAME_OR_PATH>` | Move a project to `_archived/` |
| `status` (info) | Show project status. Requires `-p, --project <PROJECT>` |
| `lint` | Lint project(s). `--fix`, `--rule <RULE>` (repeatable: `missing_directory`, `stale_index_entry`, `name_convention`, `missing_manifest`, `duplicate_key`). Requires `-p, --project <PROJECT>` |
| `index` | Index projects (mf extension). `--dry-run` |
| `show <PROJECT>` | Show project details |
| `import <DIRECTORY>` | Import a directory as a project. `--type <TYPE>`, `--source <DIR>`, `--assets <DIR>`, `-f`, `-y, --non-interactive` |

### `mf article` — Manage articles

| Subcommand | Description |
|-----------|-------------|
| `new <TYPE> <TITLE>` | Create an article. `-t, --template blank\|arch\|prd\|blog\|<path>`, `--file`, `--tag <TAG>`, `--draft`, `-f` |
| `list` (ls) | List articles |
| `lint` | Lint articles. `--fix` |
| `index` | Index articles (mf extension). `-n, --dry-run` |

### `mf source` — Manage content sources

| Subcommand | Description |
|-----------|-------------|
| `add <INPUT>` | Add a source. `-n, --name <NAME>`, `--file-kind auto\|pdf\|file\|rss\|web`, `--source-kind yuque\|meeting\|misc`, `--link`, `-f` |
| `list` (ls) | List sources. `--filter <PATTERN>`, `-t, --type <KIND>` |
| `update <NAME>` | Update a source (mf extension). `--url <URL>`, `--rename <NAME>` |
| `index` | Index sources (mf extension). `--dry-run` |
| `remove <NAME_OR_PATH>` (rm) | Remove a source. `--keep-file` |
| `clean` | Clean stale index entries. `--dry-run` |

### `mf asset` — Manage project assets

| Subcommand | Description |
|-----------|-------------|
| `add <PATH>` | Add an asset. `--name <NAME>`, `--tag <TAG>`, `--copy`/`--link`, `-f` |
| `list` (ls) | List assets. `--filter <PATTERN>`, `--type image\|video\|audio\|other` |
| `update [PATH]` | Update assets. `--set-url <URL>`, `--channel <CHANNEL>`, `--all` |
| `index` | Index assets (mf extension). `--dry-run`, `--refresh-metadata` |
| `clean` | Clean stale index entries. `--dry-run` |
| `remove <FILE>` (rm) | Remove an asset. `-f` |

### `mf term` (alias: `mf terms`) — Manage terminology

| Subcommand | Description |
|-----------|-------------|
| `new <TERM>` | Create a term (mf extension). `--definition <TEXT>`, `--alias <TEXT>`, `--tag <TAG>` |
| `list` (ls) | List terms. `--filter <PATTERN>`, `--term <NAME>` (deprecated: use `show`) |
| `lint [PATH]` | Lint term consistency in project docs. `--fix`, `--dry-run` |
| `learn` | Learn a term correction. `--term <CANONICAL>`, `--alias <VARIANT>` |
| `fix <TERM>` | Fix a term metadata (mf extension). `--definition <TEXT>`, `--alias <TEXT>`, `--tag <TAG>` |
| `show <NAME>` | Show term details |

Global terms (created without `--project`) are stored in `minds-terms.yaml` at
the repo root. Project-scoped terms live in each project's `mind-index.yaml`.

### `mf build <ARTICLE>` — Build articles

`-o, --output <PATH>`, `--dry-run`. `ARTICLE` may be an indexed name/slug or
a repo-relative path prefixed with `@` (e.g. `@projects/blog/docs/2026-03-review/`).
Directory articles are built by merging Markdown files in filename order.

### `mf publish` — Publish articles

| Subcommand | Description |
|-----------|-------------|
| `run <ARTICLE>` | Publish to a target (supported: `local`, `yuque-prompt`). `--target <TARGET>`, `--dry-run`, `-f` |
| `update <ARTICLE>` | Update a publish record. `--target <TARGET>` (required), `--status draft\|published\|archived`, `--target-url <URL>`, `--set <KEY=VALUE>`, `--dry-run` |

### `mf publisher` — Manage repo-wide publishers

| Subcommand | Description |
|-----------|-------------|
| `list` | List publishers and diagnostics |

### `mf config` — Manage configuration

| Subcommand | Description |
|-----------|-------------|
| `schema` | Show config JSON schema. `--output-format json\|yaml` (default: `json`) |
| `show` | Show effective config. `--output-format json\|yaml` (default: `yaml`) |
| `compile` | Alias of `show` |
| `generate` | Generate effective config file. `--output-format json\|yaml` (default: `yaml`), `-o, --output <PATH>` |
| `default` | Show default config values. `--output-format json\|yaml` (default: `yaml`) |
| `init` | Initialize config file (mf extension). `--output <PATH>`, `--target project\|repo` (default: `project`), `--force` |

### `mf completion <SHELL>` — Generate shell completion

Supported shells: `bash`, `zsh`, `fish`, `powershell`, `elvish`

### `mf version` — Show version information

Accepts `--json` for machine-readable output.

## Features

- **Project lifecycle** — `mf project new | list | archive | status | lint | index | show | import`; path-based identity supports Unicode, emoji, dates, spaces
- **Project auto-detection** — running inside a project directory auto-injects `--project`; cwd-relative paths normalized to repo-relative canonical identity
- **Article management** — `mf article new | list | lint | index`; directory articles by default, `--file` for single-file shape; `--template blank|arch|prd|blog` or custom project-local template path
- **Sources** — `mf source add | list | update | index | remove | clean`; `--file-kind auto|pdf|file|rss|web`, `--source-kind yuque|meeting|misc`
- **Assets** — `mf asset add | list | update | index | clean | remove`; `--copy`/`--link` for copy vs symlink
- **Glossary** — `mf term new | list | show | lint | learn | fix`; global terms in `minds-terms.yaml`, project-scoped terms in `mind-index.yaml`
- **Build** — config-driven assembly, directory-article merging, `--dry-run`, `--output`, and `@path/`-style article addressing
- **Publish** — `mf publish run | update` against per-target publishers (`local`, `yuque-prompt`); project-level local targets resolve relative paths from project root
- **Publishers** — `mf publisher list` for repo-wide publisher diagnostics
- **Config** — `mf config schema | show | compile | generate | default | init`; centralized defaults for `docs/`, `sources/`, `assets/`, `_archived/`, and `outputs/`
- **Plugins** — `mind.yaml` supports a `plugins` block for forward-compatible plugin configuration; the `typora-front-matter` plugin is enabled by default and injects `typora-copy-images-to` front matter into new articles
- **Compatibility** — reads and writes mind 0.3.0 YAML; tolerates older `schema_version` and list-based shapes on read
- **Shell completion** — `mf completion <SHELL>` for bash, zsh, fish, powershell, elvish
- **Version** — `mf version` outputs the current CLI version in text or JSON
- **Output contract** — `text` by default, `--json` for `{ status, command, data }` envelopes; stable exit codes

## Project Status

See [specs/](specs/) for detailed specifications and the feature evolution plan.

## License

[MIT](LICENSE)
