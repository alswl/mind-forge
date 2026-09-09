---
name: mf-write
version: 0.2.0
description: Develop an outline-first mind-forge article into detailed prose using its framework, selected private suggestions, Word-style feedback threads, prompt, sources, and thinking ledger; resolve verified feedback with [x]. Use for drafting, editorial revision, feedback incorporation, assembly, review, build, or explicit publishing; use mf-plan when the primary work is research, comparison, user choice, or changing goals, criteria, and structure.
---

# Write and ship a mind-forge article

Own the outline-to-article pass, expression, editorial quality, assembly, and delivery. Start from the framework and private choices prepared by `$mf-plan`, then develop only the scope the author selected. Work from the same prompt, evidence, reasoning, and feedback state; writing is not a disconnected downstream stage.

## Resolve and reconcile context

1. Locate the Mind Repo and resolve the project and article through `mf article list/show --json`; preserve the returned canonical identity.
2. Resolve exactly one prompt by frontmatter `article`, using `prompts/<article-key>.md` only as its canonical discovery convention. If none or several match, stop and recommend `$mf-plan`; do not guess.
3. Read in order: the prompt and `prompts/constitution.md`, relevant RAG hits
   from `mf search`, registered sources and terms, `<project>/thinking/<article-key>.md`,
   current article source, then all `mf-private` suggestion/feedback regions and `<!-- mf-feedback ... -->` annotations.
4. Create the thinking file if absent. Keep it as the working ledger for planned-vs-done deviations, evidence conflicts, assumptions, feedback, decisions, blockers, follow-ups, and prompt-improvement signals.
5. Respect the article shape: write a single file in place or selected top-level Markdown blocks in a directory article. Only the first block uses H1; later blocks start at H2.

Use the four first-class authored stores consistently:

- Prompts control objective, mode, constraints, criteria, deliverable contract, and durable decisions.
- Sources preserve evidence and provenance.
- Thinking preserves reasoning and work state.
- Articles under `docs/` contain the current user-readable result.

Keep planning material out of public prose except for the explicit private `Suggestions` and `Feedback` regions used by the outline-first workflow. Never copy source bodies into the prompt or hand-edit `outputs/`.

When a Source is added or changed, remember that an active RAG repository
dual-writes: Lance is the primary registration store and `mind-index.yaml` is a
compatibility projection. Run `mf source sync --offline` before drafting, then
use `mf search "<claim or topic>" --output json` to search Sources, Prompts,
Thinking, Articles, project goals, and terms before drafting or reusing existing
material. Each hit's `registrations[].context` gives repository/project
attribution, project goal, content kind, article lifecycle, `relations`, and —
for source hits — `imported_by` provenance; consume it to place and cite the hit
correctly. Prompt and Thinking hits provide intent or reasoning context, not
evidence by themselves. Verify Source provenance before citing a factual claim.
If the corpus is empty or unavailable, continue without retrieval and report the
degradation rather than stalling.

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

For `research` and `decision-research` modes, never leave `docs/` behind the known state. Each material turn must leave the article showing the current synthesis or recommendation, comparable evidence, exclusions, and material uncertainty, even when research remains incomplete.

## Incorporate human feedback

Recognize short and multiline Markdown comments beginning with `mf-feedback`. Associate each with its surrounding paragraph or section; Thinking may retain audit notes keyed to the migrated thread ID.

Treat each private `Feedback` item as a Word-style anchored comment thread:

- Treat the private thread as the authoritative current status. Thinking may record reasoning and audit history keyed by `FB-NNN`, but must not carry a separate open/resolved state.
- Preserve its stable `FB-NNN` ID, location, and immutable first-line request. Append dated `Author` and `Agent` replies; never overwrite or delete thread history.
- `[ ]` means open. Implement one open thread at a time, verify the prose satisfies it, append an `Agent` resolution reply describing the concrete change, then change it to `[x]`. A checkmark records verified resolution, not acknowledgement or effort.
- Leave partial, ambiguous, or blocked threads open and append what remains. Ask for clarification when the missing choice materially changes the result.
- When the user reopens a resolved thread, change `[x]` back to `[ ]` and append the new dated request or reason. Keep the earlier resolution visible.
- New requirements receive new stable IDs. Do not repurpose an old thread or change its original request.
- To defer feedback, move it to a prompt Open Loop with the user's reason and remove it from the current Feedback region. Every unchecked Feedback thread therefore blocks publication.

- Classify each annotation before acting. Material — content or phrasing the author supplies — folds into the prose with the author's own wording taking precedence over any rewrite. An instruction — a directive such as cut, move, merge, or emphasize — is executed and never pasted into the prose as content. A single annotation may carry both; separate them.
- Resolve local prose, structure, and style feedback directly.
- Send evidence gaps and changed comparison logic to `$mf-plan`, unless the research is small and necessary to complete the requested revision.
- Migrate a legacy `mf-feedback` annotation into a new open `FB-NNN` thread with its original wording before removing the annotation. Process that thread normally; check it only after implementation, verification, and an appended resolution reply.
- Keep unresolved annotations in place and state what blocks them.
- Promote feedback to the prompt only when it establishes a durable goal, constraint, criterion, protocol, or writing rule.
- Never silently delete feedback.

Because current build behavior does not guarantee removal of these comments, unresolved `mf-feedback` annotations block publication. Every unchecked item in a private `Feedback` region also blocks publication.

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

## Review and build

For revision or publication readiness:

1. confirm prompt, sources, thinking, docs, selected suggestions, and feedback state agree; verify every retained or developed section serves the intent and traces to authored material, a selected suggestion, explicit feedback, or evidence;
2. run `mf term lint <article-path> --project <project> --json`; preview fixes with `--fix --dry-run` and require confirmation before applying them;
3. run `mf article lint --project <project> --json` and review intent coverage, evidence, clarity, structure, duplication, and unresolved annotations;
4. reconcile manual source edits with `mf article index --project <project> --json`;
5. run `mf build "<article>" --dry-run --project <project> --json` and verify ordered inputs;
6. run the real build only when requested or needed for publication.

A clean lint exit is necessary but not sufficient. Reject empty stubs, unintended files, stale research conclusions, and unresolved feedback. When the author revises a delivered article and both versions are identifiable, record the observed delta and its cause in thinking, keep concrete wording in docs, and promote only a durable rule or decision inferred from recurring changes into the prompt so the next draft starts closer. If no reliable baseline exists, use only changes the author explicitly identifies; do not reconstruct or guess the delta. Report what changed in the prompt so the author can revert.

## Publish through a hard gate

Ordinary writing or build intent never implies publication. In the current run require:

- unique article and prompt resolution;
- reconciled prompt, sources, thinking, and docs;
- no unresolved `mf-feedback` annotations or unaccepted pending items in private `Feedback` regions;
- fresh lint and editorial review against current sources;
- a fresh build from verified inputs;
- every critical open loop resolved or explicitly accepted with the user's reason;
- an explicit target and confirmed destination;
- an explicit user request to publish after the destination is known.

List targets with `mf publish target list --json`. If a gate fails, do not publish or update a publish record. Otherwise run the verified target workflow and confirm the result with `mf article show <canonical-identity> --json`.

Use JSON envelopes and exit codes. Destructive fixes require dry-run and explicit confirmation; consult `$mf-cli` for exact flags.
