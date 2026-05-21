# Changelog

All notable changes to this project will be documented in this file.
## [Unreleased]

### Features
- Default article new to directory blocks (by @alswl)
- Add Typora front-matter plugin with config and article injection (by @alswl)
- Add project layout mechanism (by @alswl)
- Add top-level `mf init` repo lifecycle command (by @alswl)
- Review and reorganize CLI command UX (by @alswl)
- Add description and confidence metadata to terms (by @alswl)

### Bug Fixes
- Strip typora front matter from build output (by @alswl)

### Documentation
- Update (by @alswl)
- Update README and SKILL for publisher→publish target rename (by @alswl)
- No migration (by @alswl)
- No changelog manual (by @alswl)

### Refactoring
- Rename article storage path fields (by @alswl)

### Miscellaneous
- Merge branch '031-layout-mechanism' (by @alswl)

## [0.1.0] - 2026-05-19

### Features
- Add configurable projects_dir to MindsManifest (#2) (by @alswl)
- Complete 014-cli-mind-parity — full CLI parity with mind tool (#3) (by @alswl)
- Add publisher channels and CLI skill, clean up codebase (by @alswl)
- Add publisher e2e tests and CI enforcement (#4) (by @alswl)
- Python mind 0.3.0 YAML compatibility (#017) (by @alswl)
- Implement build banner, article source dirs, and asset layout (#018) (by @alswl)
- Add rename subcommands for project and article (#019) (by @alswl)
- Implement render command with template system (by @alswl)
- Dual-shape terms file support — repo-format detection, read, and write (by @alswl)
- Default article new to blank directory (by @alswl)
- Add version management with draft release workflow (by @alswl)

### Bug Fixes
- Reconcile/clean 数据丢失修复及 P1 代码重构 (by @alswl)
- Upgrade GitHub Actions to use node24 runtime (by @alswl)
- Centralize defaults and honor project paths (by @alswl)
- Remove docs/images/ from project scaffold and lint rules (by @alswl)
- Harden index handling and render prompts (by @alswl)
- Complete index bug handling (by @alswl)
- Implement publish path expansion and generated-article discovery (by @alswl)
- Close BUG-3 and BUG-5 — generated-article publish identity and declared-article index gaps (by @alswl)
- Unify article resolver across index, build, and publish (by @alswl)
- Support global terms via minds-terms.yaml without project context (by @alswl)
- Converge article identity to path全名 and DOCS_DIR relative forms (by @alswl)
- Strip outputs/ prefix in article_output_stem to avoid double path (by @alswl)
- Centralize layout constants, eliminate hardcoded path strings (by @alswl)
- Support schema-tagged repository terms (by @alswl)

### Documentation
- Update SKILL.md with rename subcommands (by @alswl)
- Docs (by @alswl)
- Skill (by @alswl)
- Generate skills and readme (by @alswl)

### Miscellaneous
- Initial commit (by @alswl)
- Implement mf CLI framework (by @alswl)
- . (by @alswl)
- Fix spec compliance and code quality issues

- Fix mf source (no subcommand) exiting 0 — now returns exit code 2 per spec
- Simplify wants_json detection to use windows(2) for --format json pair
- Remove redundant copy/link bool fields from AssetAddPayload (mode suffices) (by @alswl)
- Add GitHub Actions CI workflow

Run build, test, clippy (deny warnings), and rustfmt check on push/PR to master. (by @alswl)
- Implement CLI command skeleton, model types, and CI setup

- Add multi-level command tree: source, asset, project, article, term, build, config, publish
- Add model types for all domains with serde serialization
- Add shared global flags: --config, --verbose, --quiet, --format, --no-color
- Add shell completion generation via `mf completion <shell>`
- Add integration tests with assert_cmd and insta snapshots
- Add GitHub Actions CI workflow with cargo build, test, clippy, fmt
- Set up Cargo.lock for reproducible builds (by @alswl)
- Implement Mind Repo detection and project index command

Adds Mind Repo context detection (upward search for minds.yaml with
50-level limit and symlink safety) and the `mf project index` command
that reconciles the projects list against on-disk mind.yaml entries.
Commands needing repo context now fail fast with a not-in-mind-repo
error envelope outside a Mind Repo, while config/completion/help still
work anywhere.

Refs: specs/003-mind-repo-infra (by @alswl)
- Fix clippy warning: redundant closure in ok_or_else (by @alswl)
- CI build artifacts, cross-compilation, and code review fixes

- Add GitHub Actions artifact builds for Linux x86_64, ARM64, and macOS ARM64
- Add GitHub Release workflow triggered by version tags
- Cross-compile Linux targets via cargo-zigbuild; native macOS build
- Add CommandOutcome::Success variant for proper exit 0 on implemented commands
- Replace hand-rolled iso_now date calculation with chrono crate
- Scoped #[allow(dead_code)] on placeholder-only model modules
- Misc test updates and cleanups (by @alswl)
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
      CommandOutcome::Raw, remove misleading merge _base parameters (by @alswl)
- Implement article core commands and fix code quality

- Full article lifecycle: create (mf article new), list (mf article list),
  build (mf build), index (mf article index), lint (mf article lint)
- Index reconciliation: scan docs/ and sync mind-index.yaml (add/remove)
- Lint with --fix: kebab-case filename validation and auto-rename
- Refactor compute_article_diff to single-pass iteration
- Replace entries.flatten() with explicit error propagation
- Replace String-typed severity/kind with Severity/LintKind enums (by @alswl)
- Implement article core commands: new, list, lint, index, and build

- Add ArticleStatus (Draft/Published) and LintIssue models
- Implement article CRUD: new (with --force overwrite), list, index
- Implement article lint with filename convention checks and --fix rename
- Implement build command reading markdown from docs/
- Extract shared utilities: resolve_project, to_filename, dir_name
- 11 integration tests covering all commands and edge cases
- Replace placeholders for all 5 article/build commands (by @alswl)
- Merge branch '006-article-core' into master (by @alswl)
- Implement project lifecycle commands: new, list, status, lint, and archive

- mf project new <NAME> — scaffold project skeleton, upsert to minds.yaml
- mf project list — list projects with document counts from index files
- mf project status [--project NAME] — per-project status counts
- mf project lint [--project NAME] [--fix] [--rule KIND] — 4 rule checks
- mf project archive <NAME> — placeholder, exits 64
- Add --root <PATH> global flag for repo root override
- Add canonicalize_within path boundary security check
- Add validate_project_name kebab-case validation
- 24 e2e tests covering all commands + boundary cases (by @alswl)
- Implement config-driven build command with index validation

Upgrade `mf build` skeleton to read project config (BuildConfig),
validate article existence in mind-index.yaml (exit 1 if not found),
and write output to config-driven path `{output_dir}/{article}.{format}`.
Add MfError::NotFound variant for article-not-in-index (exit 1). (by @alswl)
- Implement publish MVP: local/yuque-prompt targets and update record

- Add PublishTargetType::YuquePrompt variant for yuque-prompt target type
- Implement mf publish run for local target (atomic copy with dry-run/force)
- Implement mf publish run for yuque-prompt target (stdout prompt + JSON envelope)
- Implement mf publish update with full upsert decision tree
- Add not-implemented guard for yuque/github_pages/custom target types
- Add hint field to NotImplemented error variant for actionable messages
- Add 36 integration tests across all user stories + quickstart E2E test
- Remove publish placeholder assertions; wire real dispatch (by @alswl)
- Implement asset core commands: add, list, update, index

Replace placeholders with four real asset management commands:
- mf asset add: copy/symlink files into project assets/ with SHA-256 hashing
- mf asset list: table/JSON listing with --filter and --type support
- mf asset update: refresh size and hash for single or all assets
- mf asset index: reconcile assets directory with index

Adds sha2 and walkdir dependencies. 39 integration tests + 6 help snapshots. (by @alswl)
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
- Update tests/cli_placeholders.rs assertion tighten (by @alswl)
- Implement structural refactor: directory modules, error model, CLI hygiene

Service layer split into directory modules (term/source/asset/project),
index I/O centralized to service::index, MfError tightened with proper
kind/hint taxonomy, output render consolidated to single dispatch,
repo-context declarative via RepoRequirement, dead code and duplicate
flags removed, main.rs slimmed to 120 lines, success envelope aligned
with charter V (command field), and clig.dev cosmetic fixes applied. (by @alswl)
- Chore (by @alswl)

