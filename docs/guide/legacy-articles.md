# Work with legacy Articles

Use this guide for projects without `mind.yaml.content_layout`.
These projects keep the Article commands and paths shown below.
For explicit category/bundle projects, use [minds and outputs](minds-and-outputs.md).


```bash
mkdir my-repo && cd my-repo
mf init
mf project new notes
mf article new "First Note" --file --project notes # creates docs/first-note.md

# Capture evidence
printf '# Reference\n\nA local reference about gateways.\n' > reference.md
mf source new ./reference.md --project notes

# Assemble the article into projects/notes/outputs/first-note.md
mf build first-note --project notes
```

> [!TIP]
> `mf build` takes an article slug (`first-note`) or path (`docs/first-note.md`),
> and `mf publish run` takes the slug. Neither accepts the title. Run
> `mf article list` to see what is available.
>
> Commands that take an article, prompt, thinking, asset or source also accept
> a path to an existing file or directory, such as
> `mf build projects/notes/docs/first-note.md`, and pick the project from that
> path. `--project` still wins when given.

There is a longer offline walkthrough in [offline walkthrough](../../quickstart.md).

## Article status

Set the status of an indexed article with the command below. Add `--dry-run`
to preview the change.

```bash
mf article update docs/first-note.md --status completed --project notes
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
edits. To change one article's shape, preview with its selector.

```bash
mf article convert --article weekly-to-do --to-directory --project notes --dry-run
```

Remove `--dry-run` to apply the previewed conversion to that article. Use
`--to-single-file` for the reverse direction when the article is eligible.

With `plugins.typora-front-matter.enabled` enabled (the default), conversion
sets `typora-copy-images-to` in the destination source file relative to the
configured assets directory. Run `mf article lint --rule missing_typora_front_matter
--project notes` to find source files and blocks that need this setting. Build
strips the Typora setting from the generated Markdown and leaves the author files intact.

`mf project lint --fix --rule missing_directory --project notes` adds
`.gitkeep` to newly created legacy article, source, and asset directories.
Commit the placeholders to carry the repair to other checkouts.


## Check an article before sharing

Run a final lint scan to find internal identifiers, production notes and
provenance wording that may need editing before publication.

```sh
mf article lint --mode final -p notes --status completed
mf article lint --mode final -p notes --json --max-warnings 0
```

Findings include the source file, line and a suggested revision. The scan
covers the public body of single-file and split articles. It skips opening
YAML, private blocks and private callouts using the same privacy boundaries
as build. Every `final_leak` finding is a warning. Use `--max-warnings 0` to
exit 1 on a finding. Final mode is read-only and rejects `--fix`.

Add project-specific phrases or Go regular expressions in the project's
`mind.yaml`. Each rule takes one matcher and a suggestion.

```yaml
article_lint:
  final:
    rules:
      - literal: INTERNAL-REVIEW
        suggestion: Remove the review marker before publication.
      - pattern: 'DEMO-[0-9]+'
        suggestion: Refer to the public feature name.
```

The built-in rules look for week identifiers such as `W37`, Align IDs,
common Git branch prefixes, and selected Chinese production and provenance
phrases. To use only project rules, set `disable_defaults: true` under
`article_lint.final`. Plain `mf article lint` still runs the structural checks.
