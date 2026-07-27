# mind-forge

**A local-first, AI-native CLI for a personal knowledge base.**

`mf` manages Markdown articles, Sources, assets, terms, builds, publishing,
and a repository-wide local RAG corpus. Authored files stay plain and
Git-reviewable; the RAG index is rebuildable derived state.

## Install

Requires Rust 1.91+.

```bash
git clone https://github.com/alswl/mind-forge.git
cd mind-forge
cargo install --path .
```

## Quick start

```bash
mkdir my-repo && cd my-repo
mf init
mf project new notes
mf article new "First Note" --project notes
mf source new ./reference.pdf --file-kind pdf --project notes

# Build and query the repository-wide RAG corpus
mf source sync --offline
mf search "what I am investigating" --output json

mf article index --project notes
mf build "First Note" --project notes
```

See [docs/manual.md](docs/manual.md) for the full manual.

## Core workflow

```text
capture → sync RAG → search evidence → write article → build → publish
```

- `mf source new` records a Source.
- `mf source sync --offline` initializes or refreshes the local corpus.
- `mf search <QUERY>` searches Sources and article context together.
- `mf article`, `mf build`, and `mf publish` manage the writing pipeline.

## Source RAG

### Canonical search

```bash
mf source sync --offline
mf search "topic or claim" --output json --limit 20
mf search "topic" --project notes --source reference
mf source status --output json
```

`mf search` is the canonical global retrieval command. It has no `--mode`
flag and searches registered Sources, article prose, prompts, and thinking.
Results include provenance; verify the returned project, Source identity, and
location before citing a result.

`mf source search --mode ...` is retained only for old scripts. New workflows
should use `mf search`.

### Source dual-write

When RAG is active, `mf source new` performs a deliberate dual write:

1. Lance receives the authoritative Source registration first.
2. The project's `mind-index.yaml` receives a compatibility projection.

A projection warning does not mean the primary Source was lost. Run
`mf source sync` to reconcile it. Repositories without RAG remain legacy-only
until their first successful sync.

Sync is local-first and non-destructive. It reads saved local Source files and
discovers article prose, `prompts/`, and `thinking/` by default. Unchanged
content is synchronized idempotently.

### Maintenance and bundles

```bash
mf source admin rebuild --offline
mf source admin clear --dry-run
mf source admin recover --snapshot <ID> --dry-run
mf source export --output-dir ./backup.mfbundle
mf source import ./backup.mfbundle --dry-run
mf source trace
```

The old `source advanced` command tree and terminal enrichment workflow are
removed. Existing enrichment data is not deleted by sync or maintenance.

Optional semantic embeddings use an OpenAI-compatible `/v1/embeddings`
provider. Credentials belong in environment variables or the gitignored
`minds-secrets.yaml`, never in committed configuration.

## Repository layout

```text
my-repo/
├── minds.yaml
├── projects/
│   └── notes/
│       ├── mind.yaml
│       ├── mind-index.yaml
│       ├── docs/
│       ├── sources/
│       ├── prompts/
│       ├── thinking/
│       └── outputs/
└── .mind-forge/cache/source/advanced/  # rebuildable RAG state
```

`minds.yaml` describes the repository; `mind.yaml` describes a project;
`mind-index.yaml` is the project compatibility/index projection.

## Command groups

```text
mf init
mf project new|list|show|update|rename|remove|archive|lint|index|import
mf article new|list|show|update|rename|remove|lint|convert|index
mf source new|list|show|update|rename|remove|index|clean
mf source sync|status|export|import|trace
mf source admin rebuild|clear|recover
mf search <QUERY>
mf asset ...     mf term ...     mf build ...
mf publish ...   mf render ...   mf config ...
```

Use `mf <command> --help` for current flags. Most commands support
`--project`, `--output text|json`, `--json`, and `--dry-run` where applicable.

## Output and safety

JSON commands use `{ "status", "command", "data" }` envelopes. Exit codes are:

- `0` success;
- `1` runtime/storage failure;
- `2` invalid input or rejected operation.

Read-only retrieval does not modify authored files. Destructive operations
require explicit confirmation; use `--dry-run` before changing unfamiliar data.

## Development

```bash
cargo fmt --check
cargo clippy -- -D warnings
cargo test
```

Feature specifications live under `specs/`. Commits use conventional commit
messages.

## License

[MIT](LICENSE)
