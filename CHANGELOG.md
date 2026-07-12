# Changelog

All notable changes to this project will be documented in this file.
## [Unreleased]

### Changes
- **Breaking**: Remove correction-mutation flags from `term update`; correction edits now go solely through `term correction update`/`remove` (spec 052)
- `term correction add` now reports `created: true|false` in JSON and "added" vs "already exists, skipped" in text
- Clarify `--filter`/`--alias`/`--tag` help text for `term list`

### Features
- Add `mf article block rm` to remove a single block from a directory article (refuses to remove the last remaining block); add `mf article convert --to-single-file --merge` to collapse multi-block directory articles into a single file, re-depthing asset references and rebinding any bound prompt (spec 064)
- `mf publish run` on `yuque-prompt` targets substitutes relative `.svg` image references with a sibling `.png` when one exists, reporting the result in an additive `transforms` field; the build artifact on disk is never modified (spec 064)
- Complete term CLI lifecycle (spec 051)
- Boundary-aware linting with standalone mode (spec 044)
- Add model update commands
- Safer defaults, single-step authoring, unified `new` verb (spec 046)
- Show mtime in article list, sort by recency
- CLI UX consistency audit & refactor
- Simplify writing workflow
- Split rename (slug-only) and update --title
- Add block rename subcommand
- Align CLI internals with rust-cli.md guide (spec 049)

### Bug Fixes
- `mf build` no longer emits malformed image/link paths (mixing a relative prefix with an absolute path) when a canonicalized `@`-path is combined with a relative `--out`, including from git worktrees or symlinked checkouts; also now rewrites HTML `<img src>` references, not just Markdown; an unresolvable reference is kept as-is and reported as a warning instead of written malformed (spec 064)
- `article rm` resolves the target by title, `article_path`, or index key (with or without `.md`) and persists the index removal for every form; no more false success leaving a dangling entry (spec 062)
- `project index` also reconciles each project's article index, pruning stale entries whose target file is absent on disk; declared/template-origin articles with existing files are never removed; per-project reconcile failures surface as warnings instead of silent skips (spec 062)
- Strip ./ prefix from project paths for consistency
- Fall back to filesystem mtime when created_at is empty
- Persist created_at fallback to minds.yaml
- Update test for TITLE column, remove write-back from read-only list_projects
- Zero scheme leading byte to stop URL exemption leak

### Documentation
- Update command examples for new verbs
- Align manual, mf-cli skill, and README with --output/-o flag

### Refactoring
- Replace CONTENT column with TITLE in list view

### Miscellaneous
- Align project ls with article ls format

## [0.2.1] - 2026-06-18

### Features
- Default article new to directory blocks
- Add Typora front-matter plugin with config and article injection
- Add project layout mechanism
- Add top-level `mf init` repo lifecycle command
- Review and reorganize CLI command UX
- Add description and confidence metadata to terms
- Add unified remove and rename lifecycle operations for all primary objects
- Elevate --project to global CLI parameter, fix term subcommand scope
- Path-based entity identity for writing workflow parity
- Path-centered article list output with content kind labels
- Unified output infrastructure & flag conventions (spec 039)
- Article shape conversion (spec 040)
- Terminal capability detection (spec 041)
- Wire OSC 8 hyperlink rendering into list/show/verb outputs
- Tmux
- Mf article ls auto-matches all projects when run outside a project dir
- Mispronunciation-aware lint + first-class `term fix` verb (spec 042)
- Add --delete-* flags, correction attr updates, and project-scoped --misrecognition

### Bug Fixes
- Strip typora front matter from build output
- Discover source-kind files during source index
- Cover article new missing index creation
- Resolve relative paths to absolute file:// URIs in render_path_link
- Broaden terminal hyperlink detection and encode file URIs
- Prevent --fix panic when overlapping corrections match the same text

### Documentation
- Update
- Update README and SKILL for publisher→publish target rename
- No migration
- No changelog manual
- Add CHANGELOG.md and git-cliff configuration
- Regenerate README and SKILL from latest CLI manual
- Regenerate README and SKILL from latest CLI manual
- Regenerate README and SKILL from latest CLI manual
- Regenerate README and SKILL from latest CLI manual
- Refresh SKILL.md and README after spec 040-042 features
- Update README and SKILL.md with new term update flags

### Refactoring
- Rename article storage path fields
- Remove repo-format, collapse to schema-version only
- Clean up term lint path handling

### Testing
- Derive expected --version from CARGO_PKG_VERSION

### Miscellaneous
- Merge branch '031-layout-mechanism'
- Add concurrency control, rust-cache, and pin macOS toolchain
- Bump version to 0.2.0
- Bump version to 0.2.1 and sync Cargo.lock

## [0.1.0] - 2026-05-19

### Features
- Add configurable projects_dir to MindsManifest (#2)
- Complete 014-cli-mind-parity — full CLI parity with mind tool (#3)
- Add publisher channels and CLI skill, clean up codebase
- Add publisher e2e tests and CI enforcement (#4)
- Python mind 0.3.0 YAML compatibility (#017)
- Implement build banner, article source dirs, and asset layout (#018)
- Add rename subcommands for project and article (#019)
- Implement render command with template system
- Dual-shape terms file support — repo-format detection, read, and write
- Default article new to blank directory
- Add version management with draft release workflow

### Bug Fixes
- Reconcile/clean 数据丢失修复及 P1 代码重构
- Upgrade GitHub Actions to use node24 runtime
- Centralize defaults and honor project paths
- Remove docs/images/ from project scaffold and lint rules
- Harden index handling and render prompts
- Complete index bug handling
- Implement publish path expansion and generated-article discovery
- Close BUG-3 and BUG-5 — generated-article publish identity and declared-article index gaps
- Unify article resolver across index, build, and publish
- Support global terms via minds-terms.yaml without project context
- Converge article identity to path全名 and DOCS_DIR relative forms
- Strip outputs/ prefix in article_output_stem to avoid double path
- Centralize layout constants, eliminate hardcoded path strings
- Support schema-tagged repository terms

### Documentation
- Update SKILL.md with rename subcommands
- Docs
- Skill
- Generate skills and readme

### Miscellaneous
- Initial commit
- Implement mf CLI framework
- .
- Fix spec compliance and code quality issues

- Fix mf source (no subcommand) exiting 0 — now returns exit code 2 per spec
- Simplify wants_json detection to use windows(2) for --format json pair
- Remove redundant copy/link bool fields from AssetAddPayload (mode suffices)
- Add GitHub Actions CI workflow

Run build, test, clippy (deny warnings), and rustfmt check on push/PR to master.
- Implement CLI command skeleton, model types, and CI setup

- Add multi-level command tree: source, asset, project, article, term, build, config, publish
- Add model types for all domains with serde serialization
- Add shared global flags: --config, --verbose, --quiet, --format, --no-color
- Add shell completion generation via `mf completion <shell>`
- Add integration tests with assert_cmd and insta snapshots
- Add GitHub Actions CI workflow with cargo build, test, clippy, fmt
- Set up Cargo.lock for reproducible builds
- Implement Mind Repo detection and project index command

Adds Mind Repo context detection (upward search for minds.yaml with
50-level limit and symlink safety) and the `mf project index` command
that reconciles the projects list against on-disk mind.yaml entries.
Commands needing repo context now fail fast with a not-in-mind-repo
error envelope outside a Mind Repo, while config/completion/help still
work anywhere.

Refs: specs/003-mind-repo-infra
- Fix clippy warning: redundant closure in ok_or_else
- CI build artifacts, cross-compilation, and code review fixes

- Add GitHub Actions artifact builds for Linux x86_64, ARM64, and macOS ARM64
- Add GitHub Release workflow triggered by version tags
- Cross-compile Linux targets via cargo-zigbuild; native macOS build
- Add CommandOutcome::Success variant for proper exit 0 on implemented commands
- Replace hand-rolled iso_now date calculation with chrono crate
- Scoped #[allow(dead_code)] on placeholder-only model modules
- Misc test updates and cleanups
- Squashed commit of the following:

commit 447c30d4bfcef60ea45b3093f304c1804daae203
Author: Jingchao <alswlx@gmail.com>
Date:   Wed Apr 29 23:23:22 2026 +0800

    Implement mf config business logic, 003 migration, and code review fixes

    - Add service layer (src/service/) with config merge/load/schema/init and
      repo manifest management, strictly separated from cli/ and model/
    - Implement `mf config {schema,show,init}` CLI commands with full error
      envelopes, replacing placeholder stubs
    - Migrate project index business logic from src/runtime/repo.rs to
      src/service/repo.rs (003 tech debt cleanup)
    - Add 14 integration tests in tests/cli_config.rs and 13 E2E tests
    - Add schemars dependency for JSON Schema (Draft-07) derivation
    - Code review: extract shared util module (atomic_write, validate_schema_version),
      remove dead_code allowances, fix --format json double-encoding via
      CommandOutcome::Raw, remove misleading merge _base parameters
- Implement article core commands and fix code quality

- Full article lifecycle: create (mf article new), list (mf article list),
  build (mf build), index (mf article index), lint (mf article lint)
- Index reconciliation: scan docs/ and sync mind-index.yaml (add/remove)
- Lint with --fix: kebab-case filename validation and auto-rename
- Refactor compute_article_diff to single-pass iteration
- Replace entries.flatten() with explicit error propagation
- Replace String-typed severity/kind with Severity/LintKind enums
- Implement article core commands: new, list, lint, index, and build

- Add ArticleStatus (Draft/Published) and LintIssue models
- Implement article CRUD: new (with --force overwrite), list, index
- Implement article lint with filename convention checks and --fix rename
- Implement build command reading markdown from docs/
- Extract shared utilities: resolve_project, to_filename, dir_name
- 11 integration tests covering all commands and edge cases
- Replace placeholders for all 5 article/build commands
- Merge branch '006-article-core' into master
- Implement project lifecycle commands: new, list, status, lint, and archive

- mf project new <NAME> — scaffold project skeleton, upsert to minds.yaml
- mf project list — list projects with document counts from index files
- mf project status [--project NAME] — per-project status counts
- mf project lint [--project NAME] [--fix] [--rule KIND] — 4 rule checks
- mf project archive <NAME> — placeholder, exits 64
- Add --root <PATH> global flag for repo root override
- Add canonicalize_within path boundary security check
- Add validate_project_name kebab-case validation
- 24 e2e tests covering all commands + boundary cases
- Implement config-driven build command with index validation

Upgrade `mf build` skeleton to read project config (BuildConfig),
validate article existence in mind-index.yaml (exit 1 if not found),
and write output to config-driven path `{output_dir}/{article}.{format}`.
Add MfError::NotFound variant for article-not-in-index (exit 1).
- Implement publish MVP: local/yuque-prompt targets and update record

- Add PublishTargetType::YuquePrompt variant for yuque-prompt target type
- Implement mf publish run for local target (atomic copy with dry-run/force)
- Implement mf publish run for yuque-prompt target (stdout prompt + JSON envelope)
- Implement mf publish update with full upsert decision tree
- Add not-implemented guard for yuque/github_pages/custom target types
- Add hint field to NotImplemented error variant for actionable messages
- Add 36 integration tests across all user stories + quickstart E2E test
- Remove publish placeholder assertions; wire real dispatch
- Implement asset core commands: add, list, update, index

Replace placeholders with four real asset management commands:
- mf asset add: copy/symlink files into project assets/ with SHA-256 hashing
- mf asset list: table/JSON listing with --filter and --type support
- mf asset update: refresh size and hash for single or all assets
- mf asset index: reconcile assets directory with index

Adds sha2 and walkdir dependencies. 39 integration tests + 6 help snapshots.
- Implement term core commands and cleanup dead code

Features (012-term-core):
- mf term new — create term entries with aliases, tags, definitions
- mf term list — list terms with optional substring filter
- mf term lint — scan docs/ for term consistency with file/block exemptions
- mf term learn — register original↔correct correction pairs
- mf term fix — modify existing terms (alias, tag, definition)

Bad-smell fixes:
- Collapse nested if in find_front_matter_close (clippy)
- Replace match with unwrap_or_default in strip_exempt_regions (clippy)
- Remove unread InternalFinding.term field
- Remove entire Placeholder dead-code path (variant, fn, module)
- Remove unused color module and is-terminal dependency
- Update tests/cli_placeholders.rs assertion tighten
- Implement structural refactor: directory modules, error model, CLI hygiene

Service layer split into directory modules (term/source/asset/project),
index I/O centralized to service::index, MfError tightened with proper
kind/hint taxonomy, output render consolidated to single dispatch,
repo-context declarative via RepoRequirement, dead code and duplicate
flags removed, main.rs slimmed to 120 lines, success envelope aligned
with charter V (command field), and clig.dev cosmetic fixes applied.
- Chore

