# Layout lifecycle

`mf lifecycle` explicitly converts project layouts and changes mind/output path
identities. Titles are preserved. This package does not import legacy articles,
assign unowned resources, or migrate real repositories.
[Additional-resource commands](additional-cli.md) reuse this planner, reference
maintenance and recovery engine for concrete file/directory operations.

```sh
mf lifecycle convert --to bundle -p project --dry-run
mf lifecycle convert --to bundle -p project --yes
mf lifecycle mind-rename projects/project::brief renamed --yes
mf lifecycle mind-move projects/project::renamed --to-project other --yes
mf lifecycle output-rename projects/other::renamed::summary short --yes
mf lifecycle output-move projects/other::renamed::short --to-project project --to-mind reader --yes
mf lifecycle output-remove projects/project::reader::short --dry-run
mf lifecycle mind-remove projects/other::renamed --yes
```

Conversion includes every output shape, prompt, thinking, Source, Asset and
structurally owned additional root. Source/Asset authority remains in YAML only
for repositories without Lance. `imported_by` does not establish ownership.
Unknown ownership, legacy `docs` content, collisions, symlinks, hard links and
cross-device paths are refused. Configurations without explicit `content_layout`
are not converted. Nonstandard configured legacy core roots containing files
require explicit ownership mapping and are refused. These are legacy Article
settings, not new mind/output core-role overrides (see [the first-package
contract](mind-output-layout.md)). Config-only or empty custom roots do not block
conversion; the new model retains its fixed core paths.

Root projects are supported: select `-p .` from the repository root and use
identities such as `::brief` and `::brief::summary`. Conversion and lifecycle
operations preserve descendant project ownership; moves between root and child
projects update references and authority records. Child-project containers are
reserved from root bundle discovery.

Category role directories and user placeholders are retained. Conversion records
these roots in `reserved_roots`, a project configuration list excluded from
bundle mind discovery. Zero-output minds retain their registrations. Outputs
remain editable sources; derived build files are not renamed as new source
identities.

Paths in Source/Asset registrations, output projections and publish records move
with their files. Affected project indexes carry `rag_stale: true`; run the
appropriate Source synchronization after reviewing the change. Existing or
configured Lance authority is explicitly refused with
`source_backend_unsupported`. No YAML-only operation claims to restore Lance.

Markdown links, images, reference definitions and HTML `src`/`href` are rebased
by default. Query strings, fragments, link titles, code and unrelated bytes are
preserved. Deletion with external inbound references returns `inbound_reference`
and names the referring files. References entirely within a removed mind do not
block removal. Dynamic destinations, wiki links, CSS/srcset and ambiguous
site-root or container syntax are refused where no safe interpretation exists.
References outside the repository are not verified.

## Preview and saved plans

`--dry-run` reads only: it creates no directory, lock file, journal or cache. Its
write set includes before/after hashes. `--emit-plan` additionally includes a
root-bound full plan containing backed-up bytes and permissions. Treat that
output as private. Save the `data.plan` object outside the watched repository;
a new plan file inside the repository would itself invalidate the inventory.

```sh
mf lifecycle apply /outside/reviewed-plan.json --dry-run
mf lifecycle apply /outside/reviewed-plan.json --yes
mf lifecycle status
mf lifecycle resume
mf lifecycle rollback
```

The inventory watches repository files, including new references and additional
resources, while excluding root `.git` and `.mind-forge` runtime state. Apply
revalidates it under an exclusive advisory lock on the repository root directory
inode. All CLI write command families cooperate with this lock; dry-run does not
acquire it. External editors do not cooperate, so this is a local recovery
protocol, not multi-file ACID or protection against concurrent manual editing.

Backups and desired contents are fsynced before authored changes. Files and new
directories are staged with their final permissions before being renamed into
place. Recovery accepts only before/after states and refuses subsequent edits.
Completed resume/rollback retries do not rewrite files; reversing a completed
transaction requires a new plan. Ordinary commands refuse unfinished journals.
Only known local APFS/HFS/ext/XFS/Btrfs/tmpfs filesystems are supported. Watched
directories must retain owner read/write/execute permissions for recovery.

Initial preparation uses a private `.mind-forge/lifecycle-preparing` directory.
Only after the complete backup is fsynced is the directory atomically published
as the active journal. No authored changes occur before publication. The default
`convert --yes` path therefore needs no external saved plan for recovery:

- Before a preparation directory exists, rerun the original command.
- For empty or truncated preparation, `mf lifecycle resume` or `rollback`
  archives the partial evidence, reports `preparation_aborted`, and unlocks ordinary
  commands. Rerun the original command to create a fresh plan. Later edits remain
  intact. Repeating recovery is harmless.
- With a complete prepared backup, `resume` continues the transaction and refuses
  conflicting later edits. `rollback` before publication aborts preparation without
  touching authored files. After publication, normal journal recovery applies.

Partial evidence remains under `.mind-forge/lifecycle-preparation-evidence-*`.
Unknown preparation entries are refused. The `--plan /outside/original-plan.json`
option remains available for legacy incomplete active journals created before
this preparation protocol; ordinary new operations do not require it.
