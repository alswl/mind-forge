---
name: mf-plan
description: Research, plan, or replan a mind-forge article from an idea, prompt, evidence, draft, or inline human feedback. Use when defining goals or constraints, collecting and comparing evidence, maintaining a research or decision article, changing structure, resolving feedback, inspecting progress, or deciding what to investigate next.
---

# Research and plan a mind-forge article

Own problem framing, evidence gathering, comparison, and judgment. Planning and writing cooperate continuously: for research work, keep the article itself current after every material turn rather than waiting for a later writing stage.

## Resolve the workspace

1. Locate the Mind Repo from `minds.yaml`; resolve projects and articles with `mf ... --json` and preserve returned canonical identities.
2. Resolve the requested project and article. If the user has requested an article deliverable, create a missing article with `mf article new ... --json` and read `data.details.path`; never guess the slug. For preliminary research with no requested deliverable, do not create an article or workflow files prematurely.
3. Derive the article key from the canonical article path's final component with one terminal `.md` removed. The canonical prompt is `<project>/prompts/<article-key>.md`, bound by its frontmatter `article` value.
4. Read `prompts/constitution.md` when present. Use [constitution.md](assets/constitution.md) only when the user asks to establish project writing rules.
5. If the prompt is absent for an existing article, use [prompt.md](assets/prompt.md). Create it only when the user has asked to plan, research, or maintain that article. If an article is absent but a bound prompt exists, report the orphan and ask before rebinding.

Treat these four stores as one workspace:

- `prompts/`: control plane for objective, mode, constraints, evaluation criteria, deliverable contract, outline, open loops, and durable decisions.
- `sources/`: evidence and provenance. Register material through `mf source new ... --json`; keep access dates and volatile-data context in the source material.
- `thinking/`: working ledger for comparisons, contradictions, assumptions, feedback, decisions, blockers, and next investigations. Use `<project>/thinking/<article-key>.md`; create it when work begins and it is absent.
- `docs/`: the current user-readable deliverable, never a deferred final dump. `outputs/` remains generated and must not be edited.

Do not copy source bodies into prompts or duplicate prompts under `docs/`.

## Read and reconcile in order

At the start of every turn, read and reconcile:

1. the bound prompt and constitution;
2. registered sources and relevant terms;
3. the thinking ledger;
4. current article source;
5. every `<!-- mf-feedback ... -->` annotation in the article.

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

- Ask first only for missing hard constraints that would invalidate the work. Continue with explicit assumptions when uncertainty is non-blocking.
- For comparisons, normalize the basis before ranking: applicable dates, people or units, variant, currency, taxes or fees, cancellation terms, access time, and other domain-specific conditions.
- Distinguish sourced facts, user-provided constraints, and agent inference. Record conflicts and volatility in thinking and expose material uncertainty in the article.
- Register durable evidence in `sources/`; record how it affects judgment in `thinking`.
- For `research` and `decision-research`, after every turn that adds a fact, constraint, exclusion, comparison, or changed judgment, update `docs/` to the best current answer. Preserve uncertainty and pending verification instead of waiting for completeness.
- For `editorial`, update `docs/` only when requested or when the planning request explicitly includes prose changes.
- Preserve substantive user content and preview broad structural mutations.

A research turn is complete only when all affected stores agree: prompt for changed control information, sources for new evidence, thinking for changed reasoning or feedback state, and docs for the current conclusion.

## Process human feedback

Recognize Markdown HTML comments beginning with `mf-feedback`, including a short form such as `<!-- mf-feedback: verify this price -->` and a multiline form. Associate the annotation with its surrounding paragraph or section. Distinguish material — content or phrasing the author supplies, folded in with the author's wording taking precedence — from an instruction — a directive to execute, never inserted as prose; a single annotation may carry both.

For each annotation:

1. add or update a stable entry in the thinking file's Feedback ledger;
2. resolve it immediately when evidence and intent are sufficient;
3. otherwise retain the annotation and record the investigation or question;
4. after resolution, update the article, remove the inline annotation, and keep the resolution in thinking;
5. promote feedback to the prompt only when it changes a durable goal, constraint, criterion, protocol, or writing rule.

Never silently delete feedback. Unresolved feedback blocks publication because current build behavior does not guarantee removal from generated Markdown.

## Hand off by concern

Use `$mf-write` for prose craft, substantial rewriting, assembly, build, and publication. Do not hand off merely because research changed the article: maintaining a research deliverable is part of this skill.

Report changes to the current conclusion, evidence, judgment, open loops, and feedback state. Use JSON envelopes and exit codes; consult `$mf-cli` for exact command behavior.
