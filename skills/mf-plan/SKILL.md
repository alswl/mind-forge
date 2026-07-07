---
name: mf-plan
description: Plan or replan a mind-forge article from an idea, existing prompt, research, or draft. Use when starting an article, clarifying audience or thesis, researching claims, changing structure, resolving writing feedback, inspecting progress, or deciding what to write next.
---

# Plan a mind-forge article

Maintain a living prompt that can change before, during, and after drafting. Planning and writing are cooperating modes, not a linear stage machine.

## Resolve the article and prompt

1. Locate the Mind Repo from `minds.yaml`; use `mf project list --json` and canonical identities from JSON responses.
2. Resolve the requested project and article. If starting from an idea, create the article first with `mf article new ... --json` and read `data.details.path`; never guess the slug.
3. Derive the article key from the canonical article path's final component with one terminal `.md` removed. Store the full canonical project-relative path in prompt frontmatter.
4. Use `<project>/prompts/<article-key>.md`. `prompts/` is an existing Mind convention; create that directory only when the user has asked to create a prompt and it is missing. Do not add another workflow directory.
5. Read `prompts/constitution.md` when present. Use [constitution.md](assets/constitution.md) only when the user asks to establish project writing rules.

If the prompt is absent, use [prompt.md](assets/prompt.md). If the article is absent but a prompt exists, report the orphan and ask before creating or rebinding anything.

- `prompts/` holds only the living design prompt (Intent, Key Messages, Outline, Research, Decisions, Open Loops). Collected facts and source material go to `sources/` (registered via `mf source`) or `docs/` for draft content — never into `prompts/`.
- The canonical prompt location is `prompts/<article-key>.md`; never copy or duplicate the prompt into `docs/` blocks.

## Bind and synchronize safely

- Treat frontmatter `article` as authoritative; the filename is only for discovery.
- Reserve `constitution` as a project prompt name. If an article key collides, ask for an explicit non-conflicting prompt filename and keep the canonical `article` binding.
- `mf article rename` now preserves the article's file/directory shape and automatically renames the bound prompt file while updating its `article:` frontmatter. After rename, verify the new identity resolves and its content is not blocked. If the prompt association needs manual adjustment, report it.
- For a rename performed outside this workflow, never infer the old identity from title similarity alone. Report orphan prompts and candidate articles; mutate only after the user confirms one mapping.
- When zero prompts match, offer to create one. When multiple prompts claim an identity, stop without mutation and list them.

## Plan iteratively

Read the current prompt, article source, registered sources, constitution, and relevant terms. Then update only the concerns affected by the user's request:

- **Intent**: audience, thesis, outcome, scope, and mode (`exploratory` or `committed`).
- **Key messages**: observable claims or ideas the article should land.
- **Research**: findings linked to canonical identities from `mf source ... --json`; put unsupported claims in evidence gaps.
- **Outline**: ordered units that work with the existing article shape. Preserve substantive `docs/` content unless the user explicitly requests restructuring.
- **Open loops**: use stable IDs, severity (`critical`, `major`, `minor`), status (`open`, `resolved`, `accepted`, `superseded`), issue, and resolution.
- **Decisions**: append dated choices and rationale; do not erase contradictory history silently.

Register new evidence with `mf source new ... --json` and reconcile it with `mf source index --json`. Register canonical terminology only when useful. Reference source identities in the prompt instead of copying source bodies.

## Report tensions, not stages

Report current planning, evidence, structure, and coverage tensions. Recommend `$mf-write` when useful, while also stating what planning can continue now. Do not label the article Briefed, Researched, Outlined, or Drafted.

Before writing, preview any broad mutation and preserve user content. Use `--json`, parse `{ status, command, data }`, and rely on exit codes. Consult `$mf-cli` when exact command behavior is needed.
