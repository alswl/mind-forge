# Minds, outputs, and project layouts

A mind is a named collection of authored content. It can exist without an output.
An output belongs to one mind and is an editable Markdown source, either one file
or a directory of ordered blocks. File formats do not create separate outputs.
Build products are derived files, separate from those sources.

## Start a project

```sh
mf project new research --layout category
mf mind new brief -p research
mf mind show projects/research::brief
mf output new brief design -p research
mf output new brief summary --file -p research
mf output list projects/research::brief
mf build projects/research::brief::design
mf output index -p research
```

Use `--layout bundle` when creating a project to group each mind's content in its
own directory. Projects with different layouts can coexist in one repository.
`--layout` applies only to new projects; it is not a conversion command. Existing
Article content is not automatically assigned to minds. Creation refuses existing
names and file/directory or case collisions; there is no overwrite option.

## Project configuration and paths

The project root `mind.yaml` stores configuration and mind registrations:

```yaml
schema: '1'
content_layout: category
minds:
  brief:
    title: Release proposal
    default_output: summary
```

`content_layout` is `category` or `bundle`. Absence identifies a legacy project;
new mind/output operations refuse to infer ownership there. The existing `layout`
path settings continue to configure legacy Article features. New mind/output
source and build paths use the fixed mappings below; configured legacy build,
template, asset, source, and article roots are also reserved in bundle discovery.
Changing `content_layout` on a populated project is not a supported migration.

| Content | Category | Bundle |
| --- | --- | --- |
| Mind registration | `minds.brief` in `mind.yaml` | Same; ordinary non-reserved first-level directories are also minds |
| Single-file output | `outputs/brief/design.md` | `brief/outputs/design.md` |
| Block output | `outputs/brief/design/01-opening.md` | `brief/outputs/design/01-opening.md` |
| Prompt | `prompts/brief.md` | `brief/prompt.md` |
| Thinking | `thinking/brief.md` | `brief/thinking.md` |
| Sources | `sources/brief/` | `brief/sources/` |
| Assets | `assets/brief/` | `brief/assets/` |
| Markdown build | `build/brief/design.md` | `build/brief/design.md` |

Category discovery unions registrations with mind names found under the core role
roots. Bundle discovery excludes hidden directories, `build`, `outputs`, `docs`,
`sources`, `assets`, `prompts`, `thinking`, `templates`, `cache`, `target`,
`_archived`, and roots reserved by project configuration. It uses neither a
`minds/` intermediate directory nor a hidden `.mind` directory. Registration-only
minds have `path: null`; missing optional roles are not invented as existing paths.
Role directories are created when needed. The legacy project scaffolder can leave
empty `docs`, `sources`, and `assets` roots; they are not minds.

Names are literal, case-sensitive names, independent of display titles. Names
cannot contain `/`, `\`, `:`, control characters, leading dots, surrounding
whitespace, or a trailing `.md`. Case-insensitive collisions are rejected. Source
paths and build paths reject symlink components and traversal.

## Selection and machine contracts

Use the `identity` emitted by a list as the corresponding show/build/publish
selector:

- Mind: `<repo-relative-project>::<mind>`.
- Output: `<repo-relative-project>::<mind>::<output>`.
- Within an explicitly selected project, mind names, unique output names,
  `<mind>::<output>`, and exact project-relative output paths are accepted.

Full identities select their project without `-p`. A conflicting `-p` is an error.
List and index commands without a full selector require `-p` or a current project.
Output locations include `project_identity`, `mind_identity`, `output_identity`,
`path`, `shape`, `mind`, `name`, and `build_path`. Build and publish JSON include
this object as `data.location`. Create results distinguish actual `path` from
`details.proposed_path`; dry runs report `path: null` and write nothing.

Selecting a mind with zero outputs fails for build. One output can be selected
implicitly. Multiple outputs require an explicit selection or that mind's
`default_output` name. A missing configured default fails. Ambiguity returns
`error.kind: selection_ambiguous` and `error.candidates` containing exact
identities. It never chooses the first output. Single-file and block outputs
with the same name are a `shape_conflict`.

## Index, retrieval, build, and publish

`mf mind index`, `mf output index`, and `mf project index` reconcile the derived
`outputs` projection in `mind-index.yaml`. Mind/output index also reconcile the
`additional` projection; `--check` compares disk, projections and explicit bindings
without writing (see [additional resources](additional-cli.md)). They preserve other index sections,
including publish records. Source files and mind registrations remain authoritative;
list/show/build resolve from disk, including before indexing. No output is stored
as a separate mind or persisted as an `articles` entry.

Repository search discovers authored outputs and mind prompts/thinking from the
same resolver, including registered projects outside `projects/`. Metadata search
works without embeddings. The existing Lance content acquisition path uses an
internal `article` registration type for output assembly; `source_identity` is
the exact output identity, `registered_location` is the source path, and ownership
is carried in annotations. This adapter does not make the output a mind. Advanced
content indexing still requires the existing embedding/backend configuration.
Private content remains available to the author's RAG, as before. Build products
and additional files are not added to the output corpus.

Build sorts Markdown blocks by filename, rebases relative links, and applies the
existing banner, first-H1, pipeline, and private-content rules. Private callouts
and private non-title blocks are excluded from the build without editing the
source. A private title block or invalid visibility value fails. Only Markdown is
implemented in this package. `--out` for a mind output must remain under the
project's `build/`; source and symlink destinations are rejected, including in
previews. Build names are scoped by both mind and output.

```sh
mf build projects/research::brief::summary --dry-run
mf publish run projects/research::brief::summary --target local-test --dry-run
mf publish update projects/research::brief::summary \
  --target local-test --set status=published
```

Publishing locates `build/<mind>/<output>.md` and records the actual source path.
Local destination filenames retain `<mind>/<output>.md` to avoid collisions.
A local destination inside the project must be under `build/`. Yuque-prompt files
also stay under `build/`, defaulting to `<mind>/<output>.yuque.md`. Publishing does
not build automatically. No external publication is needed to test these contracts.

## Scope of this package

The initial package added creation, reading, indexing, retrieval location, Markdown
build, and publish location. Subsequent packages provide [lifecycle operations](layout-lifecycle.md)
and [additional management and consistency checks](additional-cli.md). [HTML rendering](build-html.md) is also available.
The completed [Skill behavior evaluation](skills-evals-verification.md) covers
the current planning and writing workflows. Legacy `mf article` commands remain a separate
transition surface for old content; their lifecycle operations are not mind/output
lifecycle commands; project-scoped Article commands reject new layouts. F16's directory-article publish-update resolution is retained.
