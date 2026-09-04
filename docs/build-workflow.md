# Build workflow

How to keep compile cost proportional to the size of a change. All numbers below
were measured on this repository after a one-line edit to `src/model/manifest.rs`,
with a warm cargo cache on a 10-core machine.

## Where the cost actually is

Dependencies are **not** rebuilt during the edit loop. `cargo build --timings`
attributes 100% of an incremental rebuild to the single `mf` unit; the other ~490
dependency crates hit cache and never appear.

The cost comes from three properties of the crate itself:

1. **Cargo's recompilation unit is the crate**, and `mf` is one crate of ~50,000
   lines. A one-line edit and a thousand-line edit cost the same.
2. **`cargo test` compiles that crate twice** — once as the binary, once as the
   `cfg(test)` unit-test target. Roughly 9s each.
3. **Each of those links a ~490MB artifact** against 66 crates of the LanceDB
   family (`liblance.rlib` alone is 211MB). Linking is single-threaded.

The high CPU figure is parallelism, not waste: each compilation splits into 256
codegen units, so ~30s of wall time consumes ~113s of CPU.

Ruled out by measurement — do not spend time on these:

- **Debug info.** Dropping `debug = "line-tables-only"` to `debug = 0` moved the
  rebuild by 0.5s and the binary from 490MB to 485MB. The size is monomorphised
  dependency code, not debug information.
- **The 112 integration test binaries.** They drive the CLI as a subprocess
  rather than linking it, are ~3.2MB each, and are not rebuilt when crate source
  changes. Consolidating them would gain nothing.
- **Linker choice.** The system linker is current (`ld-1267`).
- **sccache for the edit loop.** Incremental compilation passes `-C incremental`,
  which is not cacheable. sccache only helps cold builds.

## The three tiers

| Command | Wall | CPU | Use it for |
|---|---|---|---|
| `cargo ck` (alias for `cargo check`) | 2.0s | — | The inner loop while writing code |
| `cargo t1 <name>` (alias for `cargo test --test <name>`) | 5.2s | 6.0s | Verifying one area, e.g. `cargo t1 cli_article` |
| `cargo build` | 5.75s | ~20s | When you need the binary itself |
| `cargo check --all-targets` | 9.9s | 22.8s | Type-checking tests too, without codegen |
| `cargo test` | 30.2s | 84.9s | The pre-push gate |

Running the whole suite after every edit costs **6× the wall time and 14× the CPU**
of running the one test target that covers what you changed. `cargo t1` is faster
because it skips both the unit-test compilation and the other ~111 test binaries.

CI (`.github/workflows/ci.yml`) runs `cargo test --locked`, the e2e suite,
`cargo clippy -- -D warnings`, and `cargo fmt --check`, so the full gate is
enforced there. Locally, run the full suite before pushing — not after every edit.

The aliases are defined in `.cargo/config.toml` at the repository root.

## Installing

Use `scripts/install.sh` rather than `cargo install --path .`.

Plain `cargo install --path .` builds into a throwaway temporary directory, so
every install recompiles all ~491 dependency crates from scratch in release mode.
The script points `--target-dir` at a persistent directory
(`~/.cache/mind-forge/install-target`, override with `MF_INSTALL_TARGET_DIR`), so
dependencies are compiled once and later installs rebuild only `mf` itself. The
directory sits outside `./target` so `cargo clean` does not discard it.

The first run still pays the full cost. Subsequent ones do not.

## What remains

The two tiers above address how *often* the expensive path runs. They do not make
the expensive path itself cheaper — the full suite still compiles a 50k-line crate
twice and links 490MB twice.

Reducing that requires removing the LanceDB family from the core binary's
dependency graph, which is the subject of spec `078-split-rag-binary`.
