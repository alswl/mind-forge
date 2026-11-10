---
name: mf-plan
metadata:
  version: 0.3.0
description: Research, plan, or replan a legacy article or a mind and its selected output as an outline-first workflow from an idea, prompt, evidence, draft, or human feedback. Use when defining goals or constraints, collecting and comparing evidence, creating or changing structure, presenting private suggestions for user selection, routing Word-style feedback threads, inspecting progress, or deciding what to investigate next; leave detailed prose development to mf-write.
---

# Research and plan a mind-forge article

Own problem framing, evidence gathering, comparison, judgment, and the article outline. Planning ends with a reviewable structure and explicit user choices; it does not silently turn research into finished prose. `$mf-write` expands the approved structure in a later pass.

## Resolve the project mode and writing target

1. Locate the Mind Repo (`minds.yaml`) and the requested project with `mf project list --json`; use the selected entry's repo-relative `path` for `mf project show <project-path> --json` and subsequent project scope. Require a successful `project show`, then read that project's actual `mind.yaml` before invoking `mf mind` or `mf output`. Route each project independently, including projects sharing one repo: absent `content_layout` uses legacy Article; explicit `category` or `bundle` uses Mind/Output. An unreadable or invalid configuration (including an invalid layout value) is an error to report, not a reason to fall back or migrate. Read the file itself for routing; `mf config show` can return defaults after a load error and is not proof that the project configuration is valid. Use `$mf-cli` for exact flags.
2. Resolve the target using the matching column below. Preserve CLI-returned identities, paths, shape and bindings; never infer ownership from a title, basename, or a directory named `outputs`. If the request is ambiguous, show the CLI candidates and ask; never select the first match. For preliminary research without a requested deliverable, create no article, output or workflow files merely to satisfy the skill.

| Operation | Legacy: no `content_layout` | Mind/Output: `category` or `bundle` |
|---|---|---|
| Locate and read | `mf article list -p <project> --json`, then `mf article show <article-selector> -p <project> --json` | `mf mind list/show --json`, then `mf output list/show --json`, scoped to the project/mind |
| Requested new deliverable | `mf article new <title> -p <project> --json`; read `data.details.path`, refresh `article list`, then use the listed identity for that created path in `article show` (directory create responses may include a trailing `/`); do not create a parallel mind | Reuse the resolved mind; only create a missing mind with `mf mind new`, then `mf output new <mind> <name> --json`; read back actual identities and paths |
| Reconcile after edits | `mf article index -p <project> --json`, then `mf article lint -p <project> --json` (Article index has no `--check`) | `mf mind index -p <project> --json`, then `mf mind index --check -p <project> --json` |
| Build / authorized publish | `mf build <article-selector> -p <project> ...`; `mf publish run <article-selector> -p <project> --target <target> ...` | `mf build <output-identity> ...`; `mf publish run <output-identity> --target <target> ...` |

3. In legacy mode, read existing `prompt` and `thinking` bindings returned by `article show`; use their returned paths and binding status. Stop on conflicting/duplicate bindings; do not repair or rebind an orphan by guessing. In Mind/Output mode, read `project_identity`, `mind_identity`, `output_identity` and the existing mind `roles` (prompt, thinking, sources, assets and applicable constitution). Role paths are project-relative; `mind show` returns existing roles only. Discover extra resources with `mf additional list/show` in Mind/Output mode; presence does not establish feedback status. A mind owns zero to many outputs and one intent/reasoning workspace. Choose one requested output or an unambiguous configured default before editing; do not duplicate a shared prompt for sibling outputs.
4. Prompt and Thinking are optional in both modes. Read and maintain existing bound files and any applicable constitution; do not create missing stores or block writing because they are absent. Use explicit user intent, authored prose, selected Suggestions, Feedback and verified evidence. Throughout the steps below, instructions to read/update Prompt or Thinking apply only when that resource exists. Keep necessary working decisions or open questions in local `mf-private` regions of the resolved source, or in conversation during preliminary research; missing stores are not completion prerequisites. Explicit requests to author a new resource require a verified location and binding, not a path synthesized from the target basename.
5. Preserve directory articles and single-file articles as found. For a new target, use `--file` when a single note is appropriate; use blocks for independently maintained sections. In legacy projects, editable source is the configured article path (often `docs/`), while `outputs/` is usually generated. In category/bundle projects, `outputs/` can contain editable sources, and `build/` contains generated artifacts. In both modes, configuration and CLI-returned source/artifact paths decide; never edit a generated draft as source or overwrite source during build. Dry-run `path: null` is not a created file.
6. Retain the selected mode and selector for indexing, build, publishing and metadata updates throughout this workflow. Do not add `content_layout`, move files, convert shapes, or migrate a project as a compatibility repair: setting the field disables the legacy Article entry without converting existing articles. For separately authorized lifecycle changes, use the mode's CLI, preview then apply, and read back identities, paths and bindings. Do not hand-edit indexes or simulate CLI operations. A structural check does not establish editorial completion.

Prompt holds durable intent; Thinking holds comparisons, conflicts and open questions; Source holds evidence and provenance. Register new sources with the real CLI and inspect the returned location before citing it. An already-owned source should be referenced rather than copied to another mind. Retrieval results from prompts and thinking provide context, not factual evidence. If RAG is unavailable, report that limitation and continue only from verified available materials. A local writing/planning request does not authorize corpus activation or backend changes.

In the rest of this skill, “selected output” also means the resolved legacy article source. Prompt/Thinking references remain conditional on existing bindings; they never require backfilling files.

## Read and reconcile in order

At the start of every turn, read and reconcile:

1. the bound prompt and constitution;
2. registered sources and relevant terms;
3. the thinking ledger;
4. current article source;
5. every `<!-- mf-feedback ... -->` annotation and every `mf-private` suggestion or feedback block in the article.

Use the resolved legacy article bindings or mind roles and output identity as the ownership context. After an authorized CLI lifecycle operation, read back identities, bindings and the mode-specific index validation. Stop on conflicting explicit bindings; never infer mappings from title similarity.

## Maintain the prompt deliberately

Select a mode; record it in existing prompt frontmatter, or retain it with the working decisions when no prompt exists:

- `editorial`: thesis, argument, narrative, and audience are primary.
- `research`: a current synthesis of gathered facts is primary.
- `decision-research`: constraints, comparable candidates, tradeoffs, and an actionable recommendation are primary.

Update the prompt when the objective, audience, scope, hard constraints, evaluation criteria, research protocol, deliverable contract, or intended structure changes. Append dated decisions and mark superseded choices; do not silently rewrite decision history. Keep detailed findings, candidate rows, and transient reasoning out of the prompt.

When the author revises a delivered article and both versions are identifiable, treat the delta as prompt signal: record the observed change and its cause in thinking, keep concrete wording in the selected output, and promote only a durable rule or decision inferred from recurring changes into the prompt as a dated decision, marking any superseded one. If no reliable baseline exists, use only changes the author explicitly identifies; do not reconstruct or guess the delta. Report prompt changes so the author can revert.

Use stable open-loop IDs with severity (`critical`, `major`, `minor`) and status (`open`, `resolved`, `accepted`, `superseded`). Unsupported claims remain evidence gaps.

## Research and materialize

Before retrieval, inspect `mf source status --json`. If the corpus is missing or unavailable, use verified local sources and report the limitation. Do not activate a corpus, switch its backend, or run a synchronization that changes repository configuration without explicit maintenance authorization. `source sync --offline` can activate Lance and rewrite `minds.yaml`; offline does not mean read-only. For an existing corpus, refresh only when necessary and within the authorized write scope:

```bash
mf source sync --offline
mf search "<research question>" --output json
```

Search is read-only; inspect result provenance and registration identity before
using a hit as evidence. Each hit's `registrations[].context` carries structured
attribution — repository, owning project and its goal, content kind, article
lifecycle status, internal `relations` (with dangling links marked
`resolved:false`), and, for source hits, `imported_by` provenance. Consume and
cite this context when placing a hit; the corpus now also covers `project` goals
and `term` definitions, so search for related work, prior decisions, terminology,
and source origins before adding new material. If the corpus is missing or
degraded, check `mf source status --output json` and report the limitation
rather than silently falling back to an incomplete source list.

- Ask first only for missing hard constraints that would invalidate the work. Continue with explicit assumptions when uncertainty is non-blocking.
- For comparisons, normalize the basis before ranking: applicable dates, people or units, variant, currency, taxes or fees, cancellation terms, access time, and other domain-specific conditions.
- Distinguish sourced facts, user-provided constraints, and agent inference. Record conflicts and volatility in thinking; reflect material uncertainty in the outline or private planning regions only when it changes the structure or the user's choice.
- Register durable evidence in `sources/`; record how it affects judgment in `thinking`.
- Keep detailed evidence, comparisons, and provisional conclusions in `sources/` and `thinking/`. Update the prompt's outline and open loops when they change.
- In every mode, materialize only the article framework in the public selected output during planning: title and section hierarchy, plus a minimal placeholder only when a heading alone cannot communicate the intended role. Do not draft body paragraphs, examples, transitions, or detailed conclusions at this stage.
- If details would help the author choose a direction, keep them beside the relevant heading as private planning content. Split them into separate, local regions rather than one article-wide dump:

  ```markdown
  > [!mf-private] Suggestions
  > - [ ] SG-001 — Candidate direction A
  > - [ ] SG-002 — Candidate direction B

  > [!mf-private] Feedback
  > - [ ] FB-001 — Original user request, preserved verbatim
  >   - Author (2026-09-08): Clarification or reply
  ```

  Give each item an article-local stable ID. In `Suggestions`, `[x]` means the user selected that option for `$mf-write`; `[ ]` means it remains unselected. Leave new suggestions unchecked unless the user has already selected one explicitly. Selection persists after drafting.
- Treat each `Feedback` item like a Word comment thread anchored beside the section it governs. The first line is the immutable request. Append dated `Author` or `Agent` replies beneath it; never rewrite earlier messages. `[ ]` means open, `[x]` means resolved by `$mf-write` after implementation and verification. Reopening changes `[x]` back to `[ ]` and appends a dated reason while preserving the earlier resolution.
- The private thread is authoritative for current feedback status. Thinking may reference `FB-NNN` for reasoning and audit history, but must not maintain a competing open/resolved flag.
- Do not invent feedback or resolve private Feedback in this skill. If the user explicitly defers an item, move the complete thread, including its ID, original request and ordered replies, to the existing prompt Open Loop with the explicit reason and source anchor; verify the destination before removing the original; every remaining open Feedback thread is current work. If no prompt exists, keep the complete thread at its source anchor and append the deferral reason; do not create a prompt or mark it resolved.
- Use as many suggestion/feedback pairs as the decisions require, normally adjacent to the section they govern. Keep factual detail and citations in the registered sources or thinking ledger; private blocks should contain concise choices, rationale, and user direction rather than hidden draft prose.
- Prune the plan actively: remove redundant or obsolete headings, duplicated choices, and branches that no longer serve the objective. Preserve substantive authored material and decision history in the appropriate store; preview broad structural deletions before applying them.
- Finish the planning pass with a coherent outline and clearly report which choices still need the user's selection. Do not proceed to detailed prose merely because enough evidence exists.

A research turn is complete when the affected existing resources agree: any bound prompt for changed control information, registered sources for new evidence, any bound thinking for changed reasoning, and the selected source for the current outline and private choices. Preserve feedback status in its authoritative private thread. Missing Prompt/Thinking stores need no backfill; retain necessary decisions privately in the source or, for preliminary research, in conversation. A detailed conclusion remains a `$mf-write` deliverable.

## Process human feedback

Classify legacy Markdown comments beginning with `mf-feedback` before acting:

- Resolve research, comparison, goal, choice, and structure comments here. Record the request and resolution in existing thinking or an anchored private Feedback thread, update the outline, verify the history is preserved, then remove the comment.
- Migrate prose, expression, and local-edit comments into an open private `Feedback` thread with a stable ID and the original wording, then remove the legacy comment. `$mf-write` owns resolution.
- If one comment contains both kinds, resolve the planning part and create an open thread for the writing part.
- Promote feedback to the prompt only when it establishes a durable goal, constraint, criterion, protocol, or writing rule.

Never silently delete feedback. Feedback is development state, not a universal build or publication gate. Report unresolved issues for the author to judge; ordinary planning does not authorize publication. There is no Feedback state CLI: preserve and edit Markdown history directly after semantic review.

For a persistent private aside — a process-discussion note or exploratory passage that must stay in the source but never ship — use mind-forge private content rather than `mf-feedback`: wrap it in a `> [!mf-private]` callout, or set `mind-forge-visibility: private` on a whole block file. `mf build`/`mf publish` exclude it while it stays intact in the source, and it remains available for the author's own RAG retrieval. A generic private aside has no workflow state; only private regions titled `Suggestions` or `Feedback` use the checkbox lifecycle defined above. See `$mf-write` / `$mf-cli` for the exact rules (block-level only; the title block cannot be private).

## Hand off by concern

Use `$mf-write` after the outline and user choices are ready for prose craft, substantial rewriting, assembly, build, and publication. Further research may refine the outline and private choices, but it does not cause `$mf-plan` to draft the article body.

Report changes to the current conclusion, evidence, judgment, open loops, and feedback state. Use JSON envelopes and exit codes; consult `$mf-cli` for exact command behavior.

## Revise an existing mind with new material

Before changing an old outline, map the new facts and conflicts, affected sections, original author judgment, authorized scope and unresolved choices in the same Thinking. Keep valid author sentences and prior decisions. Update only durable constraints in Prompt and link evidence from Source; do not create a second prompt or rewrite the entire output. Hand prose changes to mf-write with the selected output identity and bounded scope.

Read the existing authorization before asking: when the target and scope are already explicit, execute and verify. Nonzero CLI results require inspecting the error and either repairing an authorized prerequisite or reporting the unfinished action. Never call a failed lookup, rename, build, or publication successful.
