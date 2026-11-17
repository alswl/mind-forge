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

The CLI uses Go 1.26.11, pinned in `.tool-versions` and `go.mod`.
Run `go test ./internal/<changed-package>/...` while editing, then
`bash scripts/check.sh` for the full local format/vet/unit/build gate. This gate
needs no external verification checkout. It writes the explicit candidate to
`bin/mf`. Component tests live under `internal/`; the `cmd/mf` package is the
process entry point. The executable covers the CLI command families, including
build/publish and lifecycle recovery, while final compatibility, assertion
equivalence, native platform and cost gates remain open.

`bash scripts/build.sh` builds `bin/mf` with the current commit and UTC build
date. `scripts/install.sh` and `scripts/generate-manual.sh` build Go by default.
`MF_BIN=/absolute/path/to/mf` selects an existing executable for manual generation.
Installation honors `MF_INSTALL_DIR` and `MF_INSTALL_TARGET_DIR`; use isolated
directories for external acceptance.

The Rust implementation, Cargo manifests, local CLI/E2E tests and containers
were removed at the owner's request. Original tests remain traceable to frozen
Git revision `3864e883bdb338139c9f09b5b0f622db5a61f332`; independent acceptance uses
the separately frozen reference executable. Removal does not close the remaining
original assertion, native platform, final candidate or cost verification gates.

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
fixed artifact independently. Workflow preparation and
local checks do not constitute a remote CI or native four-platform acceptance.

## Go archive preparation

The manual **Prepare Go candidate archives** workflow uses
`.github/workflows/go-package.yml` to build review artifacts. The tag-release
workflow uses the same Go packager and uploads a draft release. Manual archive
preparation creates review artifacts without publishing a release.

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

Earlier Rust build measurements and commands belong to the frozen revision
`3864e883bdb338139c9f09b5b0f622db5a61f332`. The current tree has no Cargo build,
Rust CI, local black-box suite or Rust source-install path. For controlled
migration comparisons, `scripts/benchmark-build.sh rust <frozen-revision>` checks
out that historical revision in an isolated directory; it is separate from all
daily build, install and acceptance commands.
