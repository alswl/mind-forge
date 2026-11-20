# makefile-go integration

The root Makefile loads the build, version, distribution, and help modules from the
`hack/.upstream-makefile-go` submodule. After cloning, run
`git submodule update --init --recursive`.

MindForge configures the shared build to produce a single stamped binary at
`bin/mf` by default. Source installation and archive packaging use the same
build target with an explicit output path. The pinned linter and read-only
module checks remain part of the [project quality gate](../../docs/dev/build-workflow.md).

The shared version module runs in prepare mode and updates `VERSION` and
`CHANGELOG.md`. The shared build injects the version from `VERSION` into the
Go binary. Tagging is a separate step in the
[release workflow](../../docs/dev/build-workflow.md).

`make package` applies the project target matrix and `hack/release-checks.sh`
validation hooks. Use `make generate-installer` after changing
`hack/install-project.sh`. `make check` verifies the committed installer
matches the upstream template.
