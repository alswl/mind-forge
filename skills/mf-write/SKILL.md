---
name: mf-write
description: Explore, draft, revise, build, or publish a mind-forge article from its current prompt and repository sources. Use when writing or rewriting prose, reviewing evidence and structure, resolving feedback, assembling output, checking publication readiness, or explicitly publishing to a target.
---

# Write and ship a mind-forge article

Treat writing as an iterative conversation with the plan. Modify article sources first, feed material discoveries back into the bound prompt, and apply a hard gate only to publication.

## Resolve context

1. Locate the Mind Repo and resolve the project and article through `mf article list/show --json`; preserve the returned canonical identity.
2. Resolve the prompt by its frontmatter `article` value, using `prompts/<article-key>.md` only as the discovery convention. If none or several match, stop and recommend `$mf-plan`; do not guess.
3. Read the prompt, `prompts/constitution.md` when present, registered sources, canonical terms, and current article source before editing.
4. Respect the existing shape: write a single-file article in place or selected top-level Markdown blocks in a directory article. Do not move planning files into `docs/`.

## Draft or revise

- Select only the files and scope requested by the user; otherwise continue empty, stub, or explicitly targeted sections.
- Preserve substantive existing prose. Surface thesis or factual judgment changes instead of silently replacing them.
- Apply prompt intent, key messages, registered evidence, terminology, and constitution rules without forcing prose to follow an obsolete outline.
- Cite registered sources for factual claims. Do not invent citations or copy source bodies into the prompt.
- Never hand-edit `outputs/`; edit `docs/` and rebuild.

When writing reveals a new thesis, evidence gap, structural conflict, or dropped message, update only the prompt's Open loops or Decisions tables. Use stable IDs and the defined severity/status values. Broader changes to intent, research, or outline require explicit user direction or a handoff to `$mf-plan`.

## Review and build

For revision or publication readiness:

1. Run `mf term lint <article-path> --project <project> --json`; preview fixes with `--fix --dry-run` and require confirmation before applying them.
2. Run `mf article lint --project <project> --json` and review the prose for intent coverage, evidence, clarity, structure, and duplication. A clean lint exit is necessary but not sufficient.
3. Reconcile manual source edits with `mf article index --project <project> --json`.
4. Run `mf build "<article>" --dry-run --project <project> --json`; verify the exact ordered inputs and reject empty stubs or unintended files.
5. Run the real build only when requested or needed for publication. Treat its output as generated content.

Do not persist a synthetic Reviewed stage. Re-run these checks against current sources whenever publication is requested.

## Publish through a hard gate

Ordinary writing or build intent never implies publication. Before any external effect, require all of the following in the current run:

- the article and prompt resolve uniquely;
- lint and editorial review have just run against current source;
- a fresh build has just been produced from the verified inputs;
- every `critical` open loop is `resolved` or `accepted`; an accepted item records the user's reason in its resolution;
- the target is explicit, or exactly one valid target exists and the user accepts it;
- the user explicitly asks to publish after the target and destination are known.

List targets with `mf publish target list --json`. If any gate fails, stop without running publish or updating a publish record. Otherwise derive the kebab-case article key accepted by publish from the already verified canonical identity, run `mf publish run <article-key> --target ... --json`, perform `mf publish update <article-key> ... --set status=published --set url=<URL> --json` only when the target workflow requires it, and verify the result through `mf article show <canonical-identity> --json`.

Use `--json`, parse `{ status, command, data }`, and rely on exit codes. Destructive fixes require a dry-run and explicit confirmation. Consult `$mf-cli` for exact flags.
