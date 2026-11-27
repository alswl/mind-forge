# Work with legacy Articles

Use this guide for projects without `mind.yaml.content_layout`.
These projects keep the Article commands and paths shown below.
For explicit category/bundle projects, use [minds and outputs](minds-and-outputs.md).


```bash
mkdir my-repo && cd my-repo
mf init
mf project new notes
mf article new "First Note" --project notes        # creates docs/first-note/

# Capture evidence
printf '# Reference\n\nA local reference about gateways.\n' > reference.md
mf source new ./reference.md --project notes

# Assemble the article into projects/notes/outputs/first-note.md
mf build first-note --project notes
```

> [!TIP]
> `mf build` takes an article slug (`first-note`) or path (`docs/first-note`),
> and `mf publish run` takes the slug. Neither accepts the title. Run
> `mf article list` to see what is available.
>
> Commands that take an article, prompt, thinking, asset or source also accept
> a path to an existing file or directory, such as
> `mf build projects/notes/docs/first-note`, and pick the project from that
> path. `--project` still wins when given.

There is a longer offline walkthrough in [offline walkthrough](../../quickstart.md).

## Article status

Set the status of an indexed article with the command below. Add `--dry-run`
to preview the change.

```bash
mf article update docs/first-note --status completed --project notes
```

| Status | Meaning |
| --- | --- |
| `draft` | Work in progress |
| `published` | Published |
| `completed` | Finished and awaiting publication or kept unpublished |
| `abandoned` | Discontinued |

Use `mf article list --status draft` to list work in progress, or pass another
status to select that exact value. `mf article lint --status draft` applies the
same filter to the lint scan and any fixes. Discovered articles without an
index entry default to `draft`.

An abandoned article cannot be built. Set it to another status to resume
building it. This check covers the selected source and all Markdown inputs
of a directory build.

An explicit source path or a configured directory can still build when the
legacy index cannot be read. An actual build reports any auto-index failure
as a warning. A selector that requires the index returns the index error.

## Legacy project layout

A Mind Repo holds projects. Each project has four first-class knowledge stores
plus supporting assets and terms:

| Store | Responsibility |
|---|---|
| **Sources** | Evidence and provenance: what the work can rely on. |
| **Prompts** | The control plane: objective, audience, constraints, criteria and durable decisions. |
| **Thinking** | The working ledger: reasoning, conflicts, assumptions, feedback and follow-ups. |
| **Articles** | The current reader-facing synthesis or deliverable. |

Prompts and Thinking are authored Markdown, not throwaway chat context. A
Prompt binds to an Article through its `article` field, and a Thinking ledger
is matched by article key. `mf article show`, `mf prompt list` and
`mf thinking list` show these links.

```mermaid
flowchart TD
  Repo[Mind Repo: minds.yaml] --> Project[Project: mind.yaml]
  Project --> Knowledge[Sources, Prompts, Thinking, Articles]
  Project --> Support[Assets and Terms]
  Knowledge --> Work[Human or agent workflow]
  Work --> Articles[Legacy Articles]
  Articles --> Build[Build and Publish]
```

```text
my-repo/
├── minds.yaml                  # repository config
└── projects/
│   └── notes/
│       ├── mind.yaml           # project config (build, publish, ...)
│       ├── mind-index.yaml     # index projection
│       ├── docs/               # articles
│       ├── sources/            # captured evidence
│       ├── prompts/            # intent and constraints
│       ├── thinking/           # reasoning ledger
│       └── outputs/            # build products
```

`minds.yaml` describes the repository; `mind.yaml` describes a project;
`mind-index.yaml` is the project compatibility/index projection.

## Choose the article shape

Use one Markdown file for a short, cohesive article such as a weekly to-do list.
Use blocks when sections need to be maintained or rearranged independently.

```bash
mf article new "Weekly To-do" --file --project notes
```

For a document with independently maintained sections, create a block directory.

```bash
mf article new "Design Review" --template arch --project notes
mf article block new design-review risks --project notes
mf article block move design-review risks --after context --project notes
mf article block renumber design-review --project notes
```

| Template | Use |
|---|---|
| `blank` | Default; a single opening block |
| `arch` | Architecture decision: context, decision, consequence, alternatives |
| `prd` | Product requirements |
| `blog` | Blog post |
| `<path>` | Any template file under the project root |

When called without `--file`, `mf article new` creates a directory of numbered
blocks (`01-opening.md`, ...). Preserve an existing article's shape for prose
edits. Use `mf article convert --to-single-file|--to-directory` when you
intend to change its shape.

With `plugins.typora-front-matter.enabled` enabled (the default), conversion
sets `typora-copy-images-to` in the destination source file relative to the
configured assets directory. Run `mf article lint --rule missing_typora_front_matter
--project notes` to find source files and blocks that need this setting. Build
strips the Typora setting from the generated Markdown and leaves the author files intact.

`mf project lint --fix --rule missing_directory --project notes` adds
`.gitkeep` to newly created legacy article, source, and asset directories.
Commit the placeholders to carry the repair to other checkouts.
