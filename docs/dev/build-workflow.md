# Build workflow

## Current development and verification rules

The active implementation plan declares the language/toolchain and exact local
formatting, static-analysis, unit-test and build checks. The constitution fixes
no language, compiler, build command or source-directory layout.

Keep component unit tests beside the code they validate. Complete CLI,
integration/E2E, HTTP/PTY/process, recovery, installation and platform acceptance
reuse the designated external verification repository and its cases. Do not
build another black-box harness in this repository. Local checks must work
without that repository or its credentials; slice and final acceptance still
require external evidence for the exact candidate and harness revision.

The Go candidate uses Go 1.26.11, pinned in `.tool-versions` and `go.mod`.
Run `go test ./internal/<changed-package>/...` while editing, then
`bash scripts/check.sh` for the full local format/vet/unit/build gate. This gate
needs no external verification checkout. It writes the explicit candidate to
`bin/mf`. Component tests live under `internal/`; the `cmd/mf` package is the
process entry point. The candidate covers the CLI command families, including
build/publish and lifecycle recovery, while final compatibility, assertion
equivalence, native platform and cost gates remain open.

Use `bash scripts/build.sh` for a candidate stamped with the current commit and
UTC build date. The source installer and manual generator accept
`MF_IMPLEMENTATION=go` to select this candidate explicitly; their default remains
the Rust reference during migration. `MF_BIN=/absolute/path/to/mf` also selects an
already-built artifact for manual generation. Installation honors `MF_INSTALL_DIR`
and `MF_INSTALL_TARGET_DIR`; use isolated directories for external acceptance.

The Rust implementation remains the working reference until migration acceptance.
Keep its coverage until it has been mapped and reused or migrated. CI retains
the Rust jobs alongside the independent Go gate. A passing local Go gate does
not prove business compatibility or build-cost improvement.

At each business slice, use the designated external `my-tests` pass with
`VERIFY_MF_BIN` set to the freshly built candidate's absolute path. Record its
revision, dirty state, digest and version alongside the harness revision and
actual selected batch. Final acceptance is a separate external gate; partial
results cannot replace it.

## External CLI acceptance

Use the designated `my-tests` checkout/pass and its documented batch. Set
`VERIFY_MF_BIN` to the absolute path of this checkout's freshly built `bin/mf`
before running `mage verify mf` in that external checkout. Missing explicit
subject coordinates are refused; the verifier must not select a cached Rust
binary. Record both the candidate SHA256 and harness revision/dirty inputs,
along with the selected cases and actual run identity.

The full historical catalogue includes diagnostic cases. A selected batch pass
and skipped or unavailable native checks do not establish final acceptance.
Follow the pass's plan and retain its run/acceptance records; no external CLI
verification is added to `go test` or the local gate.

## Independent Go CI candidate

The Go quality job runs only this repository's format/vet/component-unit/build
gate. `scripts/check.sh` now uses the stamped build helper, so the resulting
`bin/mf` already contains the active worktree commit and UTC build date.
It needs no external verification checkout or credentials.

On a clean Linux x86_64 CI checkout, the job verifies the binary's embedded
revision and uploads `mf-go-<commit>-linux-amd64` with `mf`, `version.json`,
`build-info.txt`, `source.txt` and `checksums.txt`. The job also exposes its
artifact name and binary SHA256 as outputs. External verification consumes that
fixed artifact independently. The retained Rust reference jobs are separate
while migration coverage/cutover gates remain open. Workflow preparation and
local checks do not constitute a remote CI or native four-platform acceptance.

## Go archive preparation

Run `bash scripts/package.sh --tag v0.6.0` to prepare the four macOS/Linux
x86_64/ARM64 archives in a new `release/v0.6.0` directory. `--out` selects another
new directory; an existing destination is refused. The tag's core version must
match `internal/version.BaseVersion`. Existing development version/channel text
is retained, including its commit identity.

Each `mf-<tag>-<legacy-target>.tar.gz` contains only the executable
`mf-<legacy-target>` at mode0755. `checksums.txt` lists all four archives.
`build-info/` records the pinned compiler, CGO-disabled OS/architecture settings,
version metadata, actual Git revision/dirty state and a source-file manifest.
Source inputs are checked again before publishing the prepared directory.
Archive creation omits macOS metadata and extended attributes.

The build helper binds Git queries to the active worktree so nested worktrees
record their own revision. The packager checks every binary's embedded settings
using [`go version -m`](https://pkg.go.dev/cmd/go#hdr-Print_Go_version); cross-build
inspection is separate from the external native execution/installation gate.
The release workflow uploads a commit-named preparation artifact and creates no
GitHub release. Native four-platform, installation and checksum-failure acceptance
remain the external verification repository's responsibility.

## Controlled build measurements

The runtime source associated with artifact `a30a21e9` at dirty `518c477` was
measured against frozen Rust `3864e88` on the same Darwin ARM64 host, using
Go 1.26.11 and Rust 1.98.0. Later runtime changes require final remeasurement;
these samples keep their original source and artifact coordinates.
Each build workload has five actual samples; dependency downloads are separate.
Compiler and dependency caches are isolated, and the two implementations run
sequentially. Neither run clears global caches.

| Measurement | Rust | Go |
| --- | ---: | ---: |
| Cold development build, median | 21.87 s | 4.84 s |
| Equivalent single-file edit rebuild, median | 1.84 s | 0.56 s |
| Cold source-install primitive, median | 42.63 s | 5.31 s |
| Cache and worktree outputs after builds and component tests, allocated | 1,633.4 MiB | 216.6 MiB |

Disk totals deduplicate device/inode identities, include Go's cache and worktree
binary, and count the Rust binary inside its target directory only once.
Downloads, installed executables and toolchains are reported separately. Both
implementations use their declared unstripped flags; the Go release binary is
19,601,698 bytes versus Rust's 19,007,664 bytes.

Reproduce the sequence with `scripts/benchmark-build.sh` and
`--component-tests`. Rust uses the frozen revision. Go uses the current revision
with `--working-tree` and an explicit equivalent `--edit-file`/`--edit-from`/
`--edit-to` replacement. Each implementation's complete colocated unit gate
executes five times; their test structures differ, so those timings do not prove
equivalent coverage or a test-runtime improvement.

The raw series are `rust-cost-hnglnp6h` and `go-cost-4bkjiqln` under the declared
measurement directory. Their isolated benchmark binaries retain separate
identities from the stamped `a30a21e9` artifact used by external acceptance.
The three numerical SC bounds are met in this sample; final SC acceptance still
requires original assertion equivalence, the remaining native platforms and the
final clean candidate. Cold source installation was measured separately with
`--cold-install-only`: five fresh compiler caches and destinations per language,
prefetched offline dependencies, direct Cargo/Go install commands, and installed
binary metadata probes. Its raw series are `rust-cost-br2b83r9` and
`go-cost-bevlv2tk`; these timings include the disk sampler's overhead.

Installation disk scans requested 100 ms intervals; the largest actual gaps were
245 ms for Rust and 304 ms for Go. These are non-atomic sampled estimates, not
exact peaks or rigorous bounds. Combined compiler-cache, temporary and installed
file allocation estimates ranged from 543.8–544.8 MB for Rust and 319.9–339.5 MB
for Go. Temporary-directory-only values are a different quantity and do not
establish total footprint. Complete development/test-sequence temporary high-water
and repeated matched external-test timing remain unmeasured. The earlier
208-case local acceptance remains attached to its exact artifact and inputs.

## Historical Rust build workflow

> The measurements below predate removal of the RAG/Lance backend. Keep them as
> historical profiling evidence; remeasure the current build before using the
> timings, dependency counts, or artifact sizes for a new decision.

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

Cargo runs the ~113 test binaries in sequence and abandons the rest as soon as one
fails, so a red run reports far fewer results than a green one — a single failing
unit test truncated a 2386-test run to 856, hiding whether the CLI-level tests for
the same behaviour would also have failed. Nothing in the output says it stopped
early.

When the question is *how much* a change breaks — impact assessment, deletion
experiments, judging whether a fix is load-bearing — pass `--no-fail-fast` and read
the whole failure list.

## Installing

Use `scripts/install.sh` rather than `cargo install --path .`.

Plain `cargo install --path .` builds into a throwaway temporary directory, so
every install recompiles all ~491 dependency crates from scratch in release mode.
The script points `--target-dir` at a persistent directory
(`~/.cache/mind-forge/install-target`, override with `MF_INSTALL_TARGET_DIR`), so
dependencies are compiled once and later installs rebuild only `mf` itself. The
directory sits outside `./target` so `cargo clean` does not discard it.

The first run still pays the full cost. Subsequent ones do not.

## Compile cost

The build tiers control how often the expensive path runs.
They do not change the crate's compilation unit or linking cost.
Use targeted checks during editing and the full suite at the pre-push gate.
