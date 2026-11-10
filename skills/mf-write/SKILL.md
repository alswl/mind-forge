---
name: mf-write
metadata:
  version: 0.3.0
description: Develop a selected output of a mind into detailed prose using its framework, selected private suggestions, Word-style feedback threads, prompt, sources, and thinking ledger; resolve verified feedback with [x]. Use for drafting, editorial revision, feedback incorporation, assembly, review, build, or explicit publishing; use mf-plan when the primary work is research, comparison, user choice, or changing goals, criteria, and structure.
---

# Write and ship a mind-forge article

Own the outline-to-article pass, expression, editorial quality, assembly, and delivery. Start from the framework and private choices prepared by `$mf-plan`, then develop only the scope the author selected. Work from the same prompt, evidence, reasoning, and feedback state; writing is not a disconnected downstream stage.

## Resolve and reconcile context

1. Locate the Mind Repo (`minds.yaml`) and project with `mf project list --json`. Use `mf mind list/show --json` to resolve the mind, and `mf output list/show --json` to select the requested output. Preserve returned identities, `project_identity`, `mind_identity`, `output_identity`, paths and shape. Read `$mf-cli` for exact flags.
2. A mind owns zero to many outputs and one intent/reasoning workspace. Zero outputs is valid for research. Before editing, identify one output explicitly from the request or an unambiguous configured default; if ambiguous, show the CLI candidates and ask. Never choose the first match or derive identity from a title or basename.
3. Read the mind's `roles` returned by `mind show`: prompt, thinking, sources, assets and applicable constitution. Paths are project-relative. Read the selected output's returned path; `outputs/` contains editable sources and `build/` contains derived artifacts. Do not write generated artifacts as source or mix sibling outputs.
4. Optional roles can be absent. Do not pretend `mind show` returned a proposed role path: this CLI returns existing roles only. Continue with available explicit intent where sufficient; if a missing role must be authored, use an explicit user-provided path or report the missing-location capability. Never synthesize a prompt/thinking path from an output basename. Do not duplicate a mind prompt for each output.
5. Honor the project's `category` or `bundle` layout and the existing output shape. Do not migrate an unconfigured legacy project, change layout configuration, or convert an existing draft incidentally. Use `mf lifecycle` for authorized identity/layout changes, preview then apply, and read back. Discover extra resources with `mf additional list/show`; their presence does not establish feedback or execution status.
6. After authorized changes, use `mf mind index -p <project> --json` to reconcile projections and `mf mind index --check -p <project> --json` to verify disk and explicit bindings. Do not hand-edit indexes, implement path resolvers, or simulate lifecycle/build commands. A successful structural check does not establish editorial completion.

Read the selected output, relevant sources and terms, existing prompt/thinking, all Suggestions and Feedback, and available retrieval provenance before editing. If RAG fails, report the limitation; use verified local evidence without inventing unavailable results. Inspect status before maintenance: `source sync --offline` can activate Lance and change `minds.yaml`. Local revision authorization does not permit backend changes or corpus activation; without explicit maintenance scope, skip sync and use the supplied evidence.

## Draft or revise

- Select only the requested files and scope; otherwise continue empty, stub, or explicitly targeted sections.
- Treat the existing heading hierarchy as the writing contract. Expand it section by section; if the overall structure must change, record the reason and hand the structural decision back to `$mf-plan`.
- Read private regions titled `Suggestions` and `Feedback` as workflow input. In `Suggestions`, ensure `[x]` items are reflected in the prose and leave `[ ]` items dormant unless the user explicitly overrides the selection. A checked suggestion remains selected after drafting; on later runs, verify or refine its existing realization instead of inserting it again. Generic `mf-private` notes outside these titled regions remain context, not automatic instructions.
- Do not fill every heading from general model knowledge. Use the prompt, selected suggestions, explicit feedback, authored prose, and verified sources. When these do not establish a required judgment, leave the framework intact and ask for the missing choice instead of inventing a position.
- Edit subtractively. Delete repetition, throat-clearing, duplicated examples, stale branches, and passages that no longer serve the article's intent; prefer clean removal over rewriting material that has no job. Preserve substantive authored content, and surface broad or judgment-changing deletions before applying them.
- Preserve substantive prose. Surface thesis or factual-judgment changes instead of silently replacing them.
- Apply intent, evidence, terminology, and constitution rules without forcing prose through an obsolete outline.
- Cite registered sources for factual claims. Never invent citations.
- Honor limited-scope requests strictly. Track remaining work as open loops rather than expanding the requested deliverable.
- Write in the author's established voice, not a generic one. Prefer phrasing from the author's drafts, prior writing, and feedback over paraphrase; use external sources as evidence, paraphrase them appropriately, and cite them. Keep bridging prose in the same register as the surrounding authored text.
- Divide labor: supply structure, connective prose, and evidence placement, but leave the thesis, judgment calls, and signature lines to the author. Never manufacture the author's stance or conclusions; infer them only from the prompt, the author's prose, or explicit feedback. Where these do not establish a needed judgment, leave a marked placeholder for the author instead of adopting a source's position.
- Do not simulate conviction with unearned rhetoric. Avoid mechanical or repeated use of formulaic antithesis, rule-of-three parallelism, grand abstractions, and paragraph-closing summary uplift. Vary sentence shape and length while preserving deliberate devices already present in the author's prose.
- Match article perspective. Do not add opening pleasantries, word counts, or meta commentary to the document.

When writing uncovers new evidence, register it in sources. Record new reasoning, conflicts, deviations, and follow-ups in thinking. Update prompt open loops or decisions when needed; hand off to `$mf-plan` when the objective, hard constraints, evaluation criteria, research protocol, or overall structure must change.

For `research` and `decision-research` modes, keep the selected output consistent within the authorized scope. Each material turn must leave the article showing the current synthesis or recommendation, comparable evidence, exclusions, and material uncertainty, even when research remains incomplete.

## Incorporate human feedback

Recognize short and multiline Markdown comments beginning with `mf-feedback`. Associate each with its surrounding paragraph or section; Thinking may retain audit notes keyed to the migrated thread ID.

Treat each private `Feedback` item as a Word-style anchored comment thread:

- Treat the private thread as the authoritative current status. Thinking may record reasoning and audit history keyed by `FB-NNN`, but must not carry a separate open/resolved state.
- Preserve its stable `FB-NNN` ID, location, and immutable first-line request. Append dated `Author` and `Agent` replies; never overwrite or delete thread history.
- `[ ]` means open. Implement one open thread at a time, verify the prose satisfies it, append an `Agent` resolution reply describing the concrete change, then change it to `[x]`. A checkmark records verified resolution, not acknowledgement or effort.
- Leave partial, ambiguous, or blocked threads open and append what remains. Ask for clarification when the missing choice materially changes the result.
- When the user reopens a resolved thread, change `[x]` back to `[ ]` and append the new dated request or reason. Keep the earlier resolution visible.
- New requirements receive new stable IDs. Do not repurpose an old thread or change its original request.
- To defer feedback, require an explicit user reason. Copy the complete thread, stable ID, original request, ordered replies and source anchor to the existing prompt Open Loop, append the reason, verify that history survived, then remove the original. Deferral is not resolution. Without that authorization keep the thread open.

- Classify each annotation before acting. Material — content or phrasing the author supplies — folds into the prose with the author's own wording taking precedence over any rewrite. An instruction — a directive such as cut, move, merge, or emphasize — is executed and never pasted into the prose as content. A single annotation may carry both; separate them.
- Resolve local prose, structure, and style feedback directly.
- Send evidence gaps and changed comparison logic to `$mf-plan`, unless the research is small and necessary to complete the requested revision.
- Migrate a legacy `mf-feedback` annotation into a new open `FB-NNN` thread with its original wording before removing the annotation. Process that thread normally; check it only after implementation, verification, and an appended resolution reply.
- Keep unresolved annotations in place and state what blocks them.
- Promote feedback to the prompt only when it establishes a durable goal, constraint, criterion, protocol, or writing rule.
- Never silently delete feedback.

Feedback is development state. There is no Feedback state CLI and no universal all-feedback-closed build or publication gate. Report remaining issues for the author to judge, and honor any explicit delivery constraints. Preserve history with bounded Markdown edits after semantic verification.

## Private content: annotations and internal blocks

For content the author wants to keep in the article source but never publish — a private
note, a discussion aside, a passage written during process discussion that must not ship —
use mind-forge private content instead of `mf-feedback`. `mf build` and `mf publish`
exclude it from every generated artifact while it stays intact in the source file.
Generic private notes never need resolution. Private regions titled `Suggestions` and `Feedback`
are the exception: their stable IDs, threads, and checkboxes carry the selection and resolution
workflow described above, while the blocks themselves remain private and may stay in the source indefinitely.

- **In place, any position**: wrap it in a callout, `> [!mf-private]` (or the explicit
  `> [!mind-forge-private]`); any callout in the `mf-*` / `mind-forge-*` namespace is private
  by default.
- **A whole block file**, in a directory article: set `mind-forge-visibility: private` in
  that block's front matter.
- Both are block-level only (no inline sub-phrase hiding); the first/title block of an
  article can never be marked private front-matter (`mf build` and `mf article lint` fail
  rather than produce a titleless artifact); an unrecognized `mind-forge-visibility` value
  also fails rather than silently publishing.

## Revise old prose with new material

Map the supplied evidence against the old draft first: new facts, conflicts, affected section, original judgment, authorized edits and unresolved choices. Record that map in the same Thinking. Update only durable intent in Prompt and keep source provenance. Preserve valid author sentences and unselected sections verbatim; do not create a second prompt, invent a stance, or rewrite the whole output. When the target and edit are already authorized, perform and verify them without asking again.

## Review and build

1. Verify intent, source support, selected suggestions, author voice and feedback history within the selected output and authorized scope. Partial feedback stays open with a concrete remaining condition; never confuse an attempt with resolution.
2. Use `mf term lint <returned-output-path> --project <project> --json` where applicable. Preview bounded fixes with `--fix --dry-run`; an existing authorization for that exact fix is sufficient to apply using the documented noninteractive flags. Ask only for missing authorization or a material scope change.
3. Reconcile with `mf mind index -p <project> --json`, then `mf mind index --check -p <project> --json`. Read issues and errors; do not replace the CLI with handwritten index edits or treat check success as semantic review.
4. When build is requested or needed for authorized publication, inspect `mf build <output-identity> --dry-run --json`, then run `mf build <output-identity> --format markdown|html --json`. Use `--out` for an artifact path; global `--output json` controls the response envelope. Verify returned artifact bytes and private-content exclusion. Never overwrite source or build sibling outputs implicitly.
5. Nonzero exit requires reading the real error. Repair only authorized prerequisites or report failure and unfinished work. A failed build cannot be replaced by a handwritten artifact or a success claim.

A clean check proves mechanical consistency only. Review evidence, intent, author wording and limited scope separately. When a reliable before/after author revision exists, record its cause in Thinking and promote only durable rules into Prompt; otherwise use explicit feedback and do not invent a delta.

## Publish only with explicit authorization

Ordinary writing, readiness review or build intent does not authorize publication. Confirm the selected output and known destination from the existing user instructions; do not ask again for an already authorized action. Read targets with `mf publish target list/show --json`, build fresh from verified inputs, run the authorized `mf publish run <output-identity> --target <target> --json`, and inspect the actual destination content.

Report unresolved Feedback and evidence gaps for the human delivery decision. They do not impose a universal closure gate. Honor the user's specific release constraints. Never publish to another target or expand a local-only authorization to a remote service.

`mf publish update` records metadata; it does not publish content. Use it only when registration is requested, after actual publication succeeded, and read back the record. If build or publish fails, stop dependent publication/registration and state the failure accurately; an existing destination file does not prove this run succeeded.
