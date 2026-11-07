# Additional resources and deterministic checks

Additional resources belong to a known mind. They are discovered from disk;
there is no kind registry and no project-wide kind rename operation.

| Resource | category | bundle |
| --- | --- | --- |
| Review file for `brief` | `reviews/brief.md` | `brief/reviews.md` |
| Evaluation directory | `evals/brief/` | `brief/evals/` |

Names use ASCII letters or digits separated by single hyphens; lowercase is
recommended. Core roles and configured reserved paths take precedence. Hidden
entries, `.gitkeep`, build, template, cache, target, node_modules, vendor, dist,
and child project containers do not become additional kinds. A directory is one
resource; its descendants do not introduce more kinds. Unknown ownership,
file/directory collisions, case collisions and unsafe paths fail explicitly.

```sh
mf additional list -p research
mf additional list projects/research::brief
mf additional show projects/research::brief::additional::reviews
mf additional show reviews/brief.md -p research
mf additional rename reviews/brief.md assessments/brief.md -p research -n
mf additional rename reviews/brief.md assessments/brief.md -p research -y
mf additional remove assessments/brief.md -p research -n
mf additional remove assessments/brief.md -p research -y
```

For bundle layout, the corresponding rename is
`mf additional rename brief/reviews.md brief/assessments.md -p research -y`.
The destination is a concrete project-relative path with the same owner and
shape. Other minds' files in `reviews/` remain in place. The list/show result has
`kind: additional`, `identity`, `project_identity`, `mind_identity`, `mind`,
`additional_kind`, `path`, and `shape`. Exact identities work for root projects
(`::brief::additional::reviews`) and nested projects. Short kind selectors only
succeed when unique; `selection_ambiguous` includes sorted `candidates`.

Rename/remove use the existing lifecycle planner, reference parser, journal and
recovery commands. Their result has `kind: lifecycle`. Rename maintains managed
Markdown and supported HTML links by default. Inbound references block removal
with `inbound_reference`. Unsafe or unsupported affected references fail instead
of guessing. `-n --emit-plan` returns a hash-bound plan without creating any file,
lock, cache or journal; it can be passed to `mf lifecycle apply`. Adding resources
or inbound references makes the saved plan stale. The established local-filesystem
and YAML-authority constraints apply; Lance authority is still explicitly refused
by lifecycle mutation. Recovery applies to interrupted operations; a completed
transaction cannot be reversed with rollback. See [lifecycle](layout-lifecycle.md).

## Disk, projections and bindings

```sh
mf mind index --check -p research --json
mf output index --check -p research --json  # same check
mf mind index -n -p research --json         # preview projection reconciliation
mf mind index -p research                  # reconcile outputs and additional
```

`--check` is read-only: success envelope `data.kind: content_check`, `consistent`,
`project_identity`, `disk` and sorted `issues`. Exit status is 0 when consistent,
1 when differences exist. Structural errors use the normal error envelope. Each
issue has a stable `kind`, section/identity/path as applicable, and `candidates`.
Checks include missing/stale/duplicate/mismatched projections, invalid projection
shapes, missing default output, and missing/unsafe/duplicate/mismatched explicit
Source, Asset, Prompt, Thinking and publish bindings. No ownership is inferred
from titles, natural language, or `imported_by`.

`index` returns `kind: content_index`, `outputs`, `additional`, `differences` and
`dry_run`. It replaces only the two derived projection sections. It does not
repair authority bindings, change authored files, or decide whether a result is
ready for delivery. Removing an index row never removes the corresponding file.
Repeated checks against unchanged files produce the same JSON.

Additional content does not automatically participate in build, publish, RAG,
feedback state classification, or execution. List/show and index checks ignore
tool and child-project subtrees; lifecycle mutation retains the accepted complete
repository snapshot and safety checks, so an unsafe unrelated repository entry
can still prevent a transaction.
