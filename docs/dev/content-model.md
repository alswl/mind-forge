# Content model and schema names

Mind registrations and authored files define ownership. Output and Additional
index sections are derived projections. Reconciliation replaces those sections
without rewriting the other index sections or authored content.
List/show/build discover content from the configured layout rather than requiring
an earlier indexing step.

| Name | Current responsibility |
| --- | --- |
| Mind | Shared intent, reasoning, sources, assets, and zero or more Outputs |
| Output | Editable Markdown source with a single-file or block shape |
| Prompt | Intent, audience, constraints, criteria, and durable decisions |
| Thinking | Reasoning, comparisons, assumptions, feedback, and follow-ups |
| Source | Tracked evidence and provenance |
| Asset | Tracked supporting files |
| Article | Legacy writing unit and internal compatibility adapter |

`Prompt`, `Thinking`, and `duplicate_binding` remain the public names.
Directory names do not need to match entity names. Legacy `article:` bindings do
not override explicit Mind/Output ownership. Proposed noun renames are not part
of the current schema.

The Lance acquisition adapter uses an internal `article` registration for Output
assembly. This does not register a new Article or Mind. `source_identity` carries
the Output identity. `registered_location` carries the editable source location.
Ownership is carried in annotations.

Source storage and lifecycle mutation have different constraints. Source/RAG can
use Lance authority. Lifecycle refuses existing or configured Lance authority
instead of falling back to YAML. See [lifecycle implementation](content-lifecycle.md).

The [layout reference](../manual/content-layout.md) defines paths, identities, and
projection behavior. The [glossary](../manual/glossary.md) defines public terms.
