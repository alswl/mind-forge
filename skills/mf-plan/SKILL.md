---
name: mf-plan
version: 0.2.0
description: Research, plan, or replan a mind-forge article as an outline-first workflow from an idea, prompt, evidence, draft, or human feedback. Use when defining goals or constraints, collecting and comparing evidence, creating or changing structure, presenting private suggestions for user selection, routing Word-style feedback threads, inspecting progress, or deciding what to investigate next; leave detailed prose development to mf-write.
---

# Research and plan a mind-forge article

Own problem framing, evidence gathering, comparison, judgment, and the article outline. Planning ends with a reviewable structure and explicit user choices; it does not silently turn research into finished prose. `$mf-write` expands the approved structure in a later pass.

## Resolve the workspace

1. Locate the Mind Repo from `minds.yaml`; resolve projects and articles with `mf ... --json` and preserve returned canonical identities.
2. Resolve the requested project and article. If the user has requested an article deliverable, create a missing article with `mf article new ... --json` and read `data.details.path`; never guess the slug. For preliminary research with no requested deliverable, do not create an article or workflow files prematurely.
3. Derive the article key from the canonical article path's final component with one terminal `.md` removed. The canonical prompt is `<project>/prompts/<article-key>.md`, bound by its frontmatter `article` value.
4. Read `prompts/constitution.md` when present. Use [constitution.md](assets/constitution.md) only when the user asks to establish project writing rules.
5. If the prompt is absent for an existing article, use [prompt.md](assets/prompt.md). Create it only when the user has asked to plan, research, or maintain that article. If an article is absent but a bound prompt exists, report the orphan and ask before rebinding.

Treat these four first-class authored stores as one workspace:

- `prompts/`: control plane for objective, mode, constraints, evaluation criteria, deliverable contract, outline, open loops, and durable decisions.
- `sources/`: evidence and provenance. Register material through `mf source new ... --json`; keep access dates and volatile-data context in the source material.
- Source registrations are dual-written when the repository RAG corpus is
  active: Lance is primary and `mind-index.yaml` is the compatibility
  projection. After adding or changing sources, run `mf source sync --offline`
  and use `mf search "..." --output json` to retrieve knowledge context across
  Sources, Prompts, Thinking, and Articles. Prompt and Thinking hits provide
  intent or reasoning context, not factual evidence by themselves. Do not
  treat a successful YAML-only write as proof that the RAG corpus is current.
- `thinking/`: working ledger for comparisons, contradictions, assumptions, feedback, decisions, blockers, and next investigations. Use `<project>/thinking/<article-key>.md`; create it when work begins and it is absent.
- `docs/`: the current user-readable artifact. During planning it is an outline with private choices and feedback; after `$mf-write` it becomes the developed article. `outputs/` remains generated and must not be edited.

Do not copy source bodies into prompts or duplicate prompts under `docs/`.

## Read and reconcile in order

At the start of every turn, read and reconcile:

1. the bound prompt and constitution;
2. registered sources and relevant terms;
3. the thinking ledger;
4. current article source;
5. every `<!-- mf-feedback ... -->` annotation and every `mf-private` suggestion or feedback block in the article.

Treat prompt frontmatter `article` as authoritative. After rename, verify the new identity, prompt filename, frontmatter binding, and thinking filename. For external renames, never infer mappings from title similarity. Stop on duplicate prompt bindings.

## Maintain the prompt deliberately

Select a mode and record it in prompt frontmatter:

- `editorial`: thesis, argument, narrative, and audience are primary.
- `research`: a current synthesis of gathered facts is primary.
- `decision-research`: constraints, comparable candidates, tradeoffs, and an actionable recommendation are primary.

Update the prompt when the objective, audience, scope, hard constraints, evaluation criteria, research protocol, deliverable contract, or intended structure changes. Append dated decisions and mark superseded choices; do not silently rewrite decision history. Keep detailed findings, candidate rows, and transient reasoning out of the prompt.

When the author revises a delivered article and both versions are identifiable, treat the delta as prompt signal: record the observed change and its cause in thinking, keep concrete wording in docs, and promote only a durable rule or decision inferred from recurring changes into the prompt as a dated decision, marking any superseded one. If no reliable baseline exists, use only changes the author explicitly identifies; do not reconstruct or guess the delta. Report prompt changes so the author can revert.

Use stable open-loop IDs with severity (`critical`, `major`, `minor`) and status (`open`, `resolved`, `accepted`, `superseded`). Unsupported claims remain evidence gaps.

## Research and materialize

Before comparing evidence, refresh the repository-wide retrieval view when
needed:

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
- In every mode, materialize only the article framework in public `docs/` during planning: title and section hierarchy, plus a minimal placeholder only when a heading alone cannot communicate the intended role. Do not draft body paragraphs, examples, transitions, or detailed conclusions at this stage.
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
- Do not invent feedback or resolve private Feedback in this skill. If the user explicitly defers an item, move it from Feedback to a prompt Open Loop with the reason; every remaining open Feedback thread is current work.
- Use as many suggestion/feedback pairs as the decisions require, normally adjacent to the section they govern. Keep factual detail and citations in the registered sources or thinking ledger; private blocks should contain concise choices, rationale, and user direction rather than hidden draft prose.
- Prune the plan actively: remove redundant or obsolete headings, duplicated choices, and branches that no longer serve the objective. Preserve substantive authored material and decision history in the appropriate store; preview broad structural deletions before applying them.
- Finish the planning pass with a coherent outline and clearly report which choices still need the user's selection. Do not proceed to detailed prose merely because enough evidence exists.

A research turn is complete only when all affected stores agree: prompt for changed control information, sources for new evidence, thinking for changed reasoning or feedback state, and `docs/` for the current outline and private choices. A detailed conclusion remains a `$mf-write` deliverable.

## Process human feedback

Classify legacy Markdown comments beginning with `mf-feedback` before acting:

- Resolve research, comparison, goal, choice, and structure comments here. Record the request and resolution in the thinking Feedback ledger, update the outline, then remove the comment.
- Migrate prose, expression, and local-edit comments into an open private `Feedback` thread with a stable ID and the original wording, then remove the legacy comment. `$mf-write` owns resolution.
- If one comment contains both kinds, resolve the planning part and create an open thread for the writing part.
- Promote feedback to the prompt only when it establishes a durable goal, constraint, criterion, protocol, or writing rule.

Never silently delete feedback. Unresolved feedback blocks publication because current build behavior does not guarantee removal from generated Markdown.

For a persistent private aside — a process-discussion note or exploratory passage that must stay in the source but never ship — use mind-forge private content rather than `mf-feedback`: wrap it in a `> [!mf-private]` callout, or set `mind-forge-visibility: private` on a whole block file. `mf build`/`mf publish` exclude it while it stays intact in the source, and it remains available for the author's own RAG retrieval. A generic private aside has no workflow state; only private regions titled `Suggestions` or `Feedback` use the checkbox lifecycle defined above. See `$mf-write` / `$mf-cli` for the exact rules (block-level only; the title block cannot be private).

## Hand off by concern

Use `$mf-write` after the outline and user choices are ready for prose craft, substantial rewriting, assembly, build, and publication. Further research may refine the outline and private choices, but it does not cause `$mf-plan` to draft the article body.

Report changes to the current conclusion, evidence, judgment, open loops, and feedback state. Use JSON envelopes and exit codes; consult `$mf-cli` for exact command behavior.
