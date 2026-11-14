# Build workflow

Use the Cargo aliases in `.cargo/config.toml` to keep verification proportional
to the change. Historical timings and artifact sizes measured with the removed
RAG backend no longer describe the current dependency graph.

| Command | Use |
| --- | --- |
| `cargo ck` | Fast type-check while editing |
| `cargo t1 <name>` | Verify one integration-test area |
| `cargo build` | Produce a fresh binary |
| `cargo check --all-targets` | Type-check production and test code |
| `cargo test --no-fail-fast` | Run the full suite and collect every failure |

Run the full suite before pushing, plus `cargo clippy -- -D warnings` and
`cargo fmt --check`. Verify behavior with the freshly built binary; the installed
`mf` can be stale. After changing Clap commands, regenerate the CLI manual with
`scripts/generate-manual.sh` and review help snapshots.

CI (`.github/workflows/ci.yml`) runs `cargo test --locked` (including the e2e suite),
`cargo clippy -- -D warnings`, and `cargo fmt --check`, so the full gate is
enforced there. Locally, run the full suite before pushing — not after every edit.

CI runs for branch pushes and PRs targeting master. When the same branch SHA has
an open PR to master, only the PR event runs the heavy jobs; a failed PR lookup
falls back to full checks. Only PR updates cancel older runs. Other pushes use
independent concurrency groups. README and Markdown-only changes under docs skip
CI; executable docs and skills still run it. There are currently no required
checks configured; revisit these filters before adding branch protection.

Each job disables setup-rust-toolchain's built-in cache and uses one explicit
Swatinem/rust-cache, with a separate shared key per build target. Keeping the
existing explicit keys avoids the different environment hash computed inside the
setup composite action. Release builds use the same keys; cache reuse still depends
on GitHub ref visibility, toolchain, flags and lockfile compatibility. Tag pushes
run Release rather than duplicating the CI matrix. CI artifacts and temporary
release uploads expire after seven days; published release assets are unaffected.

The aliases are defined in `.cargo/config.toml` at the repository root.

Every one of these must run with the command sandbox disabled: sccache cannot
write its cache under the sandbox, so even `cargo ck` fails with
`sccache: error: Operation not permitted` before compiling anything.

## Reading a failing run

Cargo runs test binaries in sequence and normally stops after the first failed
target. A failed run can therefore hide failures in later test targets.

When the question is *how much* a change breaks — impact assessment, deletion
experiments, judging whether a fix is load-bearing — pass `--no-fail-fast` and read
the whole failure list.

## Installing

Use `scripts/install.sh` rather than `cargo install --path .`.

Plain `cargo install --path .` builds into a throwaway temporary directory, so
every install recompiles dependency crates from scratch in release mode.
The script points `--target-dir` at a persistent directory
(`~/.cache/mind-forge/install-target`, override with `MF_INSTALL_TARGET_DIR`), so
dependencies are compiled once and later installs rebuild only `mf` itself. The
directory sits outside `./target` so `cargo clean` does not discard it.

The first run still pays the full cost. Subsequent ones do not.
