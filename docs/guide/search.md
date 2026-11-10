# Search and maintain sources

## Canonical search

```bash
mf source sync --offline                       # build or refresh the corpus
mf search "topic or claim" --output json --limit 20
mf search "topic" --project notes --source reference
mf source status --output json
```

`mf search` is the canonical global retrieval command. It has no `--mode`
flag and searches registered Sources, authored Outputs and Articles, Prompts, Thinking, project
goals, and repository terms.

Every hit carries a structured `context` on each `registrations[]` entry —
repository and owning project, content kind, article status, internal
`relations`, and `imported_by` provenance for Source hits. Use it to attribute
and cite a result instead of re-deriving it. Search is read-only.

`mf source search --mode ...` is retained only for old scripts.

## Source dual-write

When RAG is active, `mf source new` performs a deliberate dual write:

1. Lance receives the authoritative Source registration first.
2. The project's `mind-index.yaml` receives a compatibility projection.

A projection warning does not mean the primary Source was lost. Run
`mf source sync` to reconcile it. Repositories without RAG remain legacy-only
until their first successful sync. Pass `--no-index` to register a Source
without indexing it.

Sync is local-first and non-destructive. It reads saved local Source files and
discovers authored legacy Article prose, mind Outputs, Prompts, Thinking,
project goals (`mind.yaml`), and repository terms by default. Legacy projects
store Article sources in `docs/` and build artifacts in `outputs/`. Explicit
category/bundle layouts use [their fixed source paths](../manual/content-layout.md)
and put build artifacts under `build/`. Build artifacts and Additional resources
do not enter the authored Output corpus.

Unchanged content is synchronized idempotently. The sync
report includes per-kind `coverage` and item-by-item `skipped_items` (each with
a `reason`) so nothing is silently dropped.

`mf source new` accepts `--article <PATH>` to record the originating article as
authoritative import provenance on the Source binding.

> [!WARNING]
> `mf source rename <old> <new>` treats `<new>` as a path
> (cwd-relative or absolute). To rename in place, give the full path including
> directory and extension, e.g. `mf source rename srcfile sources/pdf/renamed.pdf`.

## Maintenance and bundles

```bash
mf source admin rebuild --offline
mf source admin clear --dry-run
mf source admin recover --snapshot <ID> --dry-run
mf source export --output-dir ./backup.mfbundle
mf source import ./backup.mfbundle --dry-run
mf source trace
```

The RAG storage schema is `v3`, and compatibility is read from the
`registrations` table's on-disk structure rather than a recorded version, so it
cannot go stale. A repository predating the current schema refuses
`search`/`sync` with a diagnostic; run `mf source sync --rebuild` once. No
migration shim is provided.

Lance is authoritative for Sources and `mind-index.yaml`'s `sources:` section is
a lossless projection of it; terms remain authoritative in `mind-index.yaml`.
Reconcile operations import missing registrations and never delete them
implicitly — the corpus on disk is the source of truth for recovery.

Optional semantic embeddings use an OpenAI-compatible `/v1/embeddings`
provider. Credentials belong in environment variables or the gitignored
`minds-secrets.yaml`, never in committed configuration.
