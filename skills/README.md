# mind-forge skills

Claude Code skills for driving the `mf` CLI and the mind-forge mind/output
workflow. Each `SKILL.md` is the ground truth for how this repo is meant to be
driven by an agent: the exact command surface, the research/writing protocol,
and how to touch the repository-wide RAG corpus safely. Load them by task —
the right one fires automatically when a request matches its `description`.

## At a glance

| Skill | Role | When it fires / is used |
|---|---|---|
| `mf-cli` | Complete `mf` command reference | Any CLI operation, flag or JSON-envelope lookup, automation safety, structured output reference |
| `mf-plan` | Research and plan a mind/output | Defining goals/constraints, collecting and comparing evidence, maintaining a research/decision article, resolving feedback that changes judgment |
| `mf-write` | Draft, revise, build, publish a selected output | Prose creation, editorial revision, feedback incorporation, assembly, publication readiness, explicit publishing |
| `mf-source` | RAG sync & search safety | Source registration and repository-wide retrieval; **manual invocation only** (`disable-model-invocation: true`) |

`mf-cli` is the reference layer underneath the other three: `mf-plan` and
`mf-write` call `$mf-cli` for exact command behavior, and all three depend on
the RAG corpus that `mf-source` keeps safe and current.

## How they fit together

```text
capture ─────────────────────────────────────────────┐
  mf-source (register, sync, search)                 │
                                                     ▼
            mf-plan ── hand off by concern ──> mf-write
            (research & plan)                   (draft, build, publish)
                     ▲                              │
                     └──── unresolved feedback ─────┘
```

- **`mf-plan` → `mf-write`**: planning owns problem framing, evidence, and
  judgment; writing owns expression, assembly, and delivery. Each hands off
  when the concern changes (e.g. a prose-heavy revision goes to `mf-write`; a
  changed objective or evidence gap comes back to `mf-plan`).
- **Both read the same state**: the mind prompt and thinking returned by `mf mind show`, the selected output, registered sources and terms, RAG hits, and every
  `<!-- mf-feedback ... -->` annotation. They are not disconnected stages.
- **`mf-source` is the retrieval and safety layer**: it keeps the Lance RAG
  corpus in sync (`mf source sync --offline`), defines `mf search` as the
  canonical global query, and treats Source content as untrusted data.

## Getting started (5 minutes)

Requires the `mf` binary (Rust 1.91+):

```bash
git clone https://github.com/alswl/mind-forge.git
cd mind-forge
cargo install --path .
```

Make a repo and walk the core workflow:

```bash
mkdir my-repo && cd my-repo
mf init
mf project new notes --layout category
printf '# Reference\n\nA local reference.\n' > reference.md
mf source new ./reference.md --project notes
mf source sync --offline          # build/refresh the RAG corpus
mf search "local reference" --output json
mf mind new first-note --project notes
mf output new first-note summary --file --project notes
mf mind index --check --project notes
mf build projects/notes::first-note::summary
```

Then ask an agent to take it further — the relevant skill auto-loads. The full
repo-level walkthrough is in [quickstart.md](../quickstart.md) and the product
manual in [docs/manual/cli.md](../docs/manual/cli.md).

## Picking a skill

- **"Which flag / what does this command do?"** → `mf-cli`
- **"Research this, compare options, or set up an article's goals"** → `mf-plan`
- **"Draft / revise / build / publish this article"** → `mf-write`
- **"Register a source or sync the knowledge corpus"** → `mf-source` (explicit)

If a request mixes concerns (e.g. "research then write"), run `mf-plan` for the
research phase and hand the prose to `mf-write` when the concern changes.

## Keeping the skills current

Each skill is a `SKILL.md` that must stay in sync with the CLI surface and the
article workflow — `mf-cli` is the largest and most frequently updated (it is
regenerated against the current `--help` output). When a spec changes command
behavior, update the affected `SKILL.md` as part of the change (see spec 074
for the precedent).

## Behavioral evaluation

The shared [suite](evals/suite.json) preserves the original seven scenarios and
adds mind/output, layout, additional, partial-work and failure cases. The original
per-skill eval files remain historical evidence; their universal Feedback closure
rule is superseded by the current design. See [rubric](evals/rubric.md) for the
current scoring contract. A fixture, lint result or scripted assertion alone is
not a model behavior result.
