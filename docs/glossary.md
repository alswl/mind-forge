# MindForge Glossary

This glossary defines user-facing terms used by MindForge commands and docs.
Use these terms consistently when describing the writing workflow.

## Core Objects

| Term | Meaning |
| --- | --- |
| Mind Repo | A local repository that contains `minds.yaml`. It is the root for project discovery and repo-relative paths. |
| Project | A path-backed workspace inside a Mind Repo. A project directory contains `mind.yaml`. |
| Mind | A named collection within a project, registered independently of its outputs. |
| Output | An editable deliverable source belonging to one mind; zero or more per mind, each single-file or block-based. Markdown/HTML are formats, not output identities. |
| Build artifact | A derived result under `build/<mind>/<output>.md`, separate from editable outputs. |
| Article | Legacy writing unit and internal transition adapter. It is not the new management subject and does not turn an output into a mind. |
| Source | Reference material tracked by `mf source`. Do not use this term for an article's own content path. |
| Asset | A project file tracked by `mf asset`, usually media or other article-supporting files. |
| Block | An ordered Markdown file that forms part of a block article. |

For current paths, commands, and ownership see [mind/output layout](mind-output-layout.md).
Mind identity is `<project>::<mind>`; output identity is `<project>::<mind>::<output>`.
Project identities are repository-relative. Titles are presentation fields.

## Legacy Article Identity And Paths

| Term | Meaning |
| --- | --- |
| Path Identity | The canonical identity for path-backed entities. For articles, assets, and sources, this is project-relative. For projects, this is repo-relative. |
| Article Path | The project-relative path that identifies an article, such as `docs/review` or `docs/review.md`. In human CLI output, this is shown as `PATH`. |
| Display Title | Human-readable article text. It is presentation-only and must not be used to derive paths, build inputs, publish sources, or lookup identity. |
| Selector | User input that resolves to a canonical path identity. Shorthand selectors may be accepted only when unambiguous. |
| Source Path | The project-relative file or directory path used by build and publish as article content input. For article commands, prefer `Article Path` unless discussing build/publish internals. |

## Legacy Article Content

| Term | Meaning |
| --- | --- |
| Content Shape | The way an article's content is organized on disk. In `mf article list`, this is shown as `CONTENT`. |
| Block Article | The default article shape. Its article path is a directory containing ordered block files. |
| Single File | An article shape created with `--file`. Its article path is one Markdown file. |
| Missing | A declared article whose expected article path does not exist on disk. It remains visible so the declaration can be fixed. |

## Legacy Article Discovery

| Term | Meaning |
| --- | --- |
| Declared Article | An article declared in `mind.yaml`, usually under `build.articles` or compatibility `articles`. |
| Generated Article | An article discovered from a generated template pattern. |
| Docs Article | An article discovered by scanning the project's article directory, such as `docs/`. |
| Origin | The structured discovery source for an article: `declared`, `generated`, or `docs`. This may appear in JSON output, but is not shown in the default human `mf article list` table. |
