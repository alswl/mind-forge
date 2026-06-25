# mind-forge Manual

`mf` is a local-first CLI for mind 0.3.0-compatible knowledge repos. It stores
projects, articles, sources, assets, terms, builds, and publish state in plain
Markdown and YAML files so the repo can be edited by humans, scripts, and AI
agents.

## 1. Install

Requires Rust 1.75+.

```bash
git clone https://github.com/alswl/mind-forge.git
cd mind-forge
cargo install --path .
```

Run from source while developing:

```bash
cargo run -- --help
```

Generate shell completion:

```bash
mf completion zsh
```

Supported shells: `bash`, `zsh`, `fish`, `powershell`, `elvish`.

## 2. Repository Model

A Mind Repo is a directory rooted at `minds.yaml`.

```text
my-repo/
  minds.yaml
  projects/
    blog/
      mind.yaml
      mind-index.yaml
      docs/
      sources/
      assets/
      outputs/
```

Core files:

| File | Purpose |
| --- | --- |
| `minds.yaml` | Repo manifest and project list |
| `mind.yaml` | Project metadata and layout configuration |
| `mind-index.yaml` | Project index for articles, sources, assets, terms, and publish records |
| `minds-terms.yaml` | Global glossary terms |

All generated YAML uses `schema: "1"` and mind 0.3.0-compatible shapes.

## 3. First Repo

```bash
mkdir my-repo
cd my-repo
mf init

mf project new blog
mf article new "First Post" --project blog
mf source new https://example.com/ref --file-kind web --project blog
mf asset new diagram.png --project blog
mf term new "Zettelkasten" --definition "A note-taking method" --project blog

mf article index --project blog
mf build "First Post" --project blog
mf publish run "First Post" --target local --project blog
```

## 4. Shared CLI Rules

Global flags:

| Flag | Description |
| --- | --- |
| `--root <PATH>` | Mind Repo root directory |
| `--config <PATH>` | Config file path |
| `-p`, `--project <PROJECT>` | Project selector for project-scoped commands |
| `-o`, `--output <text|json>` | Output format (default: `text`) |
| `--json` | Shorthand for `--output json` |
| `-q`, `--quiet` | Suppress successful non-list output |
| `--no-color` | Disable colored output |

Shared mutating flags:

| Flag | Applies to | Description |
| --- | --- | --- |
| `--dry-run` | Mutating commands | Preview without writing |
| `--force` | Create, rename, remove, archive | Proceed despite safety checks: overwrite an existing target, or remove an entity referenced by others |
| `--yes` | Remove, archive | Confirm destructive actions in scripts |

JSON output always uses:

```json
{
  "status": "ok",
  "command": "project.update",
  "data": {}
}
```

`data` is always an object. Lists expose stable `identity` values that can be
passed back to `show`, `update`, `rename`, and `remove`.

## 5. Projects

Projects are path-backed workspaces under a Mind Repo.

```bash
mf project new blog
mf project list
mf project show blog
mf project update blog --description "Writing workspace"
mf project update blog --clear-description
mf project rename blog writing/blog
mf project remove writing/blog --yes
mf project archive writing/blog --yes
mf project lint --project writing/blog --fix
mf project index
mf project import /path/to/existing --force
```

Notes:

- Project identity is repo-relative and may include Unicode, emoji, dates,
  spaces, and nested paths.
- `project update` currently updates only `project.description` in `mind.yaml`.
- `project rename` renames the directory and updates repo/project indexes.
- `project archive` moves the project to `_archived/`.

## 6. Articles

Articles are Markdown writing units inside a project. The default article shape
is a directory article under `docs/`; pass `--file` for a single Markdown file.

```bash
mf article new "First Post" --project blog
mf article new "Architecture Review" --template arch --project blog
mf article new "Quick Note" --file --project blog

mf article list --project blog
mf article show docs/first-post --project blog
mf article update docs/first-post --status published --project blog
mf article update docs/first-post --title "Better Title" --project blog
mf article rename docs/first-post first-post-v2 --project blog
mf article remove docs/first-post-v2 --project blog --yes

mf article convert docs/quick-note --to-directory --project blog
mf article convert docs/quick-note --to-single-file --project blog
mf article index --project blog
mf article lint --project blog --fix
```

Notes:

- Article selectors accept a project-relative article path or an unambiguous
  title.
- `article update` updates article metadata in the index:
  `--status draft|published` toggles publication status,
  `--title "New Title"` changes the display title without renaming files.
- `article rename` changes only the article slug (file/directory path); the
  title is left unchanged. Use `--title` on `article update` to change the title.
- `article list` without `--project` outside a project scans all projects and
  sorts by most recently modified.

## 7. Sources

Sources track reference material used by a project.

```bash
mf source new https://example.com/ref --name ref-a --file-kind web --project blog
mf source new paper.pdf --file-kind pdf --project blog
mf source list --project blog
mf source show ref-a --project blog
mf source update ref-a --url https://example.com/v2 --project blog
mf source rename ref-a ref-canonical --project blog
mf source remove ref-canonical --keep-file --project blog
mf source index --project blog
mf source clean --dry-run --project blog
```

Use `source rename` for identity changes. `source update --rename` remains
available for compatibility.

## 8. Assets

Assets are project files, usually images or other media.

```bash
mf asset new diagram.png --project blog
mf asset new logo.png --copy --project blog
mf asset list --project blog
mf asset show diagram.png --project blog
mf asset update diagram.png --set-url https://cdn.example.com/diagram.png --project blog
mf asset rename diagram.png diagrams/system.png --project blog
mf asset remove diagrams/system.png --project blog
mf asset index --refresh-metadata --project blog
mf asset clean --project blog
```

## 9. Terms

Terms can be global or project-scoped. Global terms live in `minds-terms.yaml`;
project terms live in `mind-index.yaml`.

```bash
mf term new "API" --definition "Application Programming Interface"
mf term new "Zettelkasten" --definition "A note-taking method" --project blog
mf term list
mf term show API
mf term new API --alias "application programming interface"
mf term update API --definition "Interface contract" --tag architecture
mf term rename API "Application API" --keep-alias
mf term remove obsolete-term --yes
mf term lint --project blog
mf term lint --project blog --fix --dry-run
mf term fix --project blog
```

`term fix` is a first-class alias for `term lint --fix`. For boundary and
pinyin matching details, see [term-lint.md](term-lint.md).

## 10. Build And Publish

Build assembles an article into an output Markdown file. Directory articles are
merged in filename order.

```bash
mf build first-post --project blog
mf build @projects/blog/docs/first-post/ --output ./_build/first-post.md
mf build first-post --dry-run --project blog
```

Publish sends built output to a configured target.

```bash
mf publish run "First Post" --target local --project blog
mf publish run "First Post" --target yuque-prompt --project blog
mf publish update "First Post" --target local --status published --project blog
mf publish target list
mf publish target show local
```

Supported publish targets currently include `local` and `yuque-prompt`.

## 11. Render Templates

```bash
mf render template list
mf render template show arch
```

Built-in article templates include `blank`, `arch`, `prd`, and `blog`. A
project-local Markdown template path may also be passed to `article new
--template`.

## 12. Config

```bash
mf config schema
mf config show
mf config generate -o minds.yaml
mf config default
mf config terminal
```

`mf init` is the repo bootstrap command. Use it to create `minds.yaml` and the
default `projects/` container in a new or existing directory.

`mf config terminal` reports terminal capability detection for colors,
hyperlinks, and relevant environment overrides.

## 13. Scripting

Prefer JSON output in scripts:

```bash
mf project update blog --description "Writing" --json
mf article update docs/first-post --status published --title "New Title" --project blog --json
```

Use `--dry-run` before mutating commands:

```bash
mf project rename blog writing/blog --dry-run --json
mf article remove docs/old-post --project blog --dry-run --json
```

Use `--yes` for destructive commands in non-interactive environments:

```bash
mf project remove old-project --yes
mf article remove docs/old-post --project blog --yes
```

## 14. Troubleshooting

Common checks:

```bash
mf project lint --project blog
mf article index --project blog --dry-run
mf source clean --project blog --dry-run
mf asset index --project blog --refresh-metadata
mf config terminal --json
mf version --json
```

If a command cannot find a project, pass `--project <PROJECT>` explicitly or
run it from inside the project directory.

If manually edited YAML is rejected, check that it uses `schema: "1"` and
dictionary sections for indexed resources.
