# Shared makefile-go modules

Source: <https://github.com/alswl/makefile-go>, revision
`79641c287e33c87873c839145763f74d262dd5e1`.

The root Makefile **includes `build.mk`** and uses its actual build, download,
format, lint and clean targets. `general.mk` is copied unchanged for help.
`scripts/build.sh` delegates compilation to the shared build target rather than
maintaining a second Go build recipe. Source installers and four-target archive
packaging reuse that entry point.

`build.mk` carries a small, explicit compatibility patch against the pinned
upstream file. `mind-forge-build.patch` reproduces every change. The added hooks
select source directories, lint command, verbosity, ldflags, executable path and
whether to create extra platform/version copies. MindForge selects:

- cmd/pkg/internal formatting and the pinned linter;
- a non-mutating tidy check during lint;
- unstripped `bin/mf` (or `scripts/build.sh --out PATH`), without duplicate outputs;
- existing `internal/version.Commit` and `BuildDate` symbols;
- CGO-disabled compilation with the exact compiler selected in `.tool-versions`;
- Git queries bound to the active worktree, preserving nested-worktree identity.

The adapted recipe also stops on compiler/copy errors. Its integration keeps
existing Go APIs, version JSON and candidate provenance checks. `make fmt` uses
upstream's `goimports`; install it separately when using that formatting target.
`make check` does not require goimports.

To update, fetch the declared upstream revision, compare its modules with this
copy, reapply/review the compatibility patch and run the complete local and
external build/install/package checks. Do not overwrite the module with a newer
sibling checkout or drop version/provenance validation as an incidental update.

The recorded patch uses zero context to avoid treating Make recipe indentation
as whitespace errors inside a patch file. From a directory containing the
pristine pinned `build.mk`, apply it with
`git apply --unidiff-zero /absolute/path/to/mind-forge-build.patch`, then compare
the result with this vendored module before updating it.
