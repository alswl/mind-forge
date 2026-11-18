# Build workflow

## Current development and verification rules

The adopted [Go CLI requirements](go-cli-guidelines.md) map the pinned
`alswl/guides` baseline to this implementation and list outstanding differences.
Install development-only `golangci-lint` v2.12.2 with
`go install github.com/golangci/golangci-lint/v2/cmd/golangci-lint@v2.12.2`.
`make check` wraps the same full gate as `bash scripts/check.sh`; the gate now
also runs pinned correctness lint, `go mod verify` and `go mod tidy -diff`.
`make help` lists the other local targets. Build and core runtime still require
the pinned Go compiler and Make, not the linter.

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
Run `go test ./pkg/<layer>/<changed-package>/...` while editing, then
`bash scripts/check.sh` for the full local format/vet/unit/build gate. This gate
needs no external verification checkout. It writes the explicit candidate to
`bin/mf`. Component tests live beside their code under `cmd/`, `pkg/` and `internal/`; the `cmd/mf` package is the
process entry point. The executable covers the CLI command families, including
build/publish and lifecycle recovery, while matched Rust/Go acceptance, native
platform and final cost gates remain open.

The [code structure](code-structure.md) describes command factories, application
layers and the retained configuration/output contracts. `make build` uses the
pinned shared makefile-go build module. `bash scripts/build.sh` delegates to
that target and builds `bin/mf` with the current commit and UTC build
date. `scripts/install.sh` and `scripts/generate-manual.sh` build Go by default.
`MF_BIN=/absolute/path/to/mf` selects an existing executable for manual generation.
Installation honors `MF_INSTALL_DIR` and `MF_INSTALL_TARGET_DIR`; use isolated
directories for external acceptance.

The Rust implementation, Cargo manifests, local CLI/E2E tests and containers
were removed at the owner's request. Original tests remain traceable to frozen
Git revision `3864e883bdb338139c9f09b5b0f622db5a61f332`; independent acceptance uses
the separately frozen reference executable. Removal does not close the remaining
matched-run, native platform, final clean-candidate or cost verification gates.
The four preparation checkpoints and partial journal fault have an optional
`mf_testfaults` CLI build tag for external process acceptance. The default build
does not read its environment variable or contain its injection code.

The current clean functional candidate is commit
`91dc935b8be3de184ad79ade6dc8abd1950e3329`, binary SHA256
`251fbad9f85c0943c55656f86004c25b3c46d29531965d66b9b9385e4da769d1`, version
`0.6.0-dev+91dc935`, built with Go 1.26.11 for Darwin ARM64. On the C258-specific
378-file input manifest, the external fixed non-distribution selection passed
508/508 cases with no failures or skips. A separate same-candidate selection
passed source install, prebuilt HTTP installer using a local one-member archive
fixture, published-manual consistency and manual generation outside the repo
(4/4, no failures or skips). Both selections remain partial: the first left 18
distribution cases unselected; the second left 522 catalogue cases unselected.
The archive fixture is not official release evidence. Harness, case-input and
raw artifact hashes are recorded in `specs/085-go-rewrite/migration/candidate-c258-clean-functional.json`
and `my-tests/specs/094-mf-go-content/verification/history/acceptance-c258-clean-functional.md`.
Official release/archive, shell completion, native cross-platform and final
complete acceptance remain open; release, shell and cross-platform work are
deferred by user priority. The repeated C258 fixed-selection cost slice is now
complete, while the full T072 cost gate remains open.

The local development/test cost gate was rerun after the user authorized
removing regenerable benchmark caches. On current clean HEAD
`13853e7d08216330b475f55bd7943ad41721aa15` versus frozen Rust
`3864e883bdb338139c9f09b5b0f622db5a61f332`, five samples per workload passed
the SC-001–003 numerical bounds: allocated compiler/test cache plus worktree
outputs 1,070,759,936 B Rust / 228,569,088 B Go (21.35%); cold build median
22.615 s / 5.256 s (23.24%); equivalent edit median 2.146 s / 0.612 s
(28.53%). Each side's complete colocated unit gate ran five times. The Go
runtime source is unchanged from the externally tested `91dc935`; the only
intervening path is this build-workflow documentation. Raw series and sampling
caveats are in `specs/085-go-rewrite/migration/candidate-c258-clean-development-test-cost.json`.
The same clean C258 candidate also completed five cost-sampled runs of the
fixed 508-case non-distribution external selection: every run passed 508/508
with 0 failures or skips, and selected execution median was 339.568 s. Eighteen
distribution cases remained unselected, so the harness result is partial.
Per-run identities and history hashes are in
`specs/085-go-rewrite/migration/candidate-c258-external-functional-cost-508.json`.
T072 remains open for release workload and final SC acceptance; these local and
fixed-selection measurements do not close the full cost gate.

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

The C257 functional checkpoint is a dirty review candidate: product head
`fd9b483e286666bc1fc265006e8c4a62c348673c`, patch SHA-256
`fe2ae87c470ed797b3aba35f8612cd5310c2fc7b52320223ee6d78f06237c39b`, and
binary SHA-256
`28458677c75bc41468163f22f6678cb6b5ae5905123797c7ac110f8371025eb3`. Its
fixed 508-case non-distribution selection passed five times with 0 failures and
0 skips; 18 distribution cases were not selected. The 506 non-diagnostic cases
have five cost samples each, with median selected execution time 340.124 s.
Harness and case-input identities, run IDs and artifact hashes are recorded in
`my-tests/specs/094-mf-go-content/verification/history/acceptance-c257-external-cost-508.md`
and `specs/085-go-rewrite/migration/candidate-c257-external-functional-cost-508.json`.
This checkpoint does not establish a clean final candidate, release/archive,
shell-completion or native-platform acceptance.

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

Run `bash scripts/package.sh --tag v0.5.3` to prepare the four macOS/Linux
x86_64/ARM64 archives in a new `release/v0.5.3` directory. `--out` selects another
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
The release workflow uploads a commit-named preparation artifact and creates a
draft GitHub release. Inspect the assets and verification results before publishing
the draft. Native four-platform, installation and checksum-failure acceptance
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

## Version tags

The first Go release is tagged `v0.5.3`, following the requested patch version
after `v0.5.2`; the earlier `0.6.0` migration candidates remain historical
verification inputs. The executable retains its existing `0.5.3-dev+<commit>`
identity and `dev` channel while the published tag and base version are `0.5.3`.

`scripts/bump.sh` updates `internal/version/version.go`, regenerates the changelog,
and creates a commit and tag. It previews by default. `--stage candidate` adds
`-rc` to the tag; `--stage alpha` and `--stage beta` select those suffixes.
`base_version` always contains the core `MAJOR.MINOR.PATCH`, including for
prerelease tags, so the same archive builder accepts the resulting tag.
