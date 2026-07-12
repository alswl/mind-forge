---
name: mf-write
description: Draft, revise, review, build, or publish a mind-forge article using its prompt, sources, thinking ledger, current prose, and inline human feedback. Use for prose creation, editorial revision, feedback incorporation, assembly, publication readiness, or explicit publishing; use mf-plan when the primary work is research, comparison, or changing goals and evaluation criteria.
---

# Write and ship a mind-forge article

Own expression, editorial quality, assembly, and delivery. Work from the same prompt, evidence, reasoning, and feedback state as `$mf-plan`; writing is not a disconnected downstream stage.

## Resolve and reconcile context

1. Locate the Mind Repo and resolve the project and article through `mf article list/show --json`; preserve the returned canonical identity.
2. Resolve exactly one prompt by frontmatter `article`, using `prompts/<article-key>.md` only as its canonical discovery convention. If none or several match, stop and recommend `$mf-plan`; do not guess.
3. Read in order: the prompt and `prompts/constitution.md`, registered sources and terms, `<project>/thinking/<article-key>.md`, current article source, then all `<!-- mf-feedback ... -->` annotations.
4. Create the thinking file if absent. Keep it as the working ledger for planned-vs-done deviations, evidence conflicts, assumptions, feedback, decisions, blockers, follow-ups, and prompt-improvement signals.
5. Respect the article shape: write a single file in place or selected top-level Markdown blocks in a directory article. Only the first block uses H1; later blocks start at H2.

Use the four stores consistently:

- prompt controls objective, mode, constraints, criteria, deliverable contract, and durable decisions;
- sources preserve evidence and provenance;
- thinking preserves reasoning and work state;
- docs contain the current user-readable result.

Never move planning material into `docs/`, copy source bodies into the prompt, or hand-edit `outputs/`.

## Draft or revise

- Select only the requested files and scope; otherwise continue empty, stub, or explicitly targeted sections.
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

Recognize short and multiline Markdown comments beginning with `mf-feedback`. Associate each with its surrounding paragraph or section and maintain stable entries in the thinking file's Feedback ledger.

- Classify each annotation before acting. Material — content or phrasing the author supplies — folds into the prose with the author's own wording taking precedence over any rewrite. An instruction — a directive such as cut, move, merge, or emphasize — is executed and never pasted into the prose as content. A single annotation may carry both; separate them.
- Resolve local prose, structure, and style feedback directly.
- Send evidence gaps and changed comparison logic to `$mf-plan`, unless the research is small and necessary to complete the requested revision.
- After resolution, edit the article, remove the annotation, and preserve the request and resolution in thinking.
- Keep unresolved annotations in place and state what blocks them.
- Promote feedback to the prompt only when it establishes a durable goal, constraint, criterion, protocol, or writing rule.
- Never silently delete feedback.

Because current build behavior does not guarantee removal of these comments, unresolved `mf-feedback` annotations block publication.

## Review and build

For revision or publication readiness:

1. confirm prompt, sources, thinking, docs, and feedback state agree;
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
- no unresolved `mf-feedback` annotations;
- fresh lint and editorial review against current sources;
- a fresh build from verified inputs;
- every critical open loop resolved or explicitly accepted with the user's reason;
- an explicit target and confirmed destination;
- an explicit user request to publish after the destination is known.

List targets with `mf publish target list --json`. If a gate fails, do not publish or update a publish record. Otherwise run the verified target workflow and confirm the result with `mf article show <canonical-identity> --json`.

Use JSON envelopes and exit codes. Destructive fixes require dry-run and explicit confirmation; consult `$mf-cli` for exact flags.
