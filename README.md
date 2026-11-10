# mind-forge

[![CI](https://github.com/alswl/mind-forge/actions/workflows/ci.yml/badge.svg)](https://github.com/alswl/mind-forge/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/alswl/mind-forge)](https://github.com/alswl/mind-forge/releases)
![Rust 1.91+](https://img.shields.io/badge/rust-1.91%2B-orange)

**A local-first, AI-native CLI for card-based writing and personal knowledge.**

`mf` manages minds: named collections of intent, reasoning, evidence, and editable
outputs. A mind can have zero, one, or several outputs. Each output can be a single
Markdown file or ordered blocks. Build artifacts stay separate from authored
sources, and private content is excluded from builds and publication.

## Install

```sh
curl -fsSL https://raw.githubusercontent.com/alswl/mind-forge/master/scripts/install.sh | sh
export PATH="${MF_INSTALL_DIR:-${XDG_BIN_HOME:-$HOME/.local/bin}}:$PATH"
```

See the [installation guide](docs/guide/install.md) for tagged versions, supported
platforms, source installation, and completion.

## Create a project

New projects choose `category` or `bundle` layout in `mind.yaml`:

```sh
mf init
mf project new research --layout category
mf mind new brief -p research
mf output new brief design -p research
mf output new brief summary --file -p research
mf output list projects/research::brief
mf build projects/research::brief::design
mf build projects/research::brief::design --format html
```


## Documentation

- [User guides](docs/guide/README.md): create, organize, search, build, and publish.
- [Reference manual](docs/manual/README.md): commands, layouts, fields, and terms.
- [Developer documentation](docs/dev/README.md): build, test, and current architecture.

Projects without `content_layout` keep their [legacy Article workflow](docs/guide/legacy-articles.md).
Existing Articles are not automatically migrated or assigned to Minds.

## Agent skills

[`skills/`](skills/) contains Claude Code skills for the `mf` workflow:

| Skill | Role |
|---|---|
| `mf-cli` | Full command, flag and JSON-envelope reference |
| `mf-plan` | Research, evidence comparison, outline-first planning and feedback routing |
| `mf-write` | Drafting, revision, assembly, build and explicit publishing |
| `mf-source` | Safe source registration, RAG sync and search (manual invocation) |

To install one, copy or symlink its directory into your agent's skills folder
(for example `~/.claude/skills/`). [skills/README.md](skills/README.md) explains
how the skills hand work to each other.
## Contributing

Use `cargo ck` while editing and `cargo t1 <target>` for a focused check.
Run the full suite before pushing. See [build and test](docs/dev/build-workflow.md)
for the complete workflow. Commit messages follow Conventional Commits.
