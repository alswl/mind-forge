# makefile-go integration

The root Makefile loads the build, version, and help modules from the
`hack/.upstream-makefile-go` submodule. After cloning, run
`git submodule update --init --recursive`.

MindForge configures the shared build to produce a single stamped binary at
`bin/mf` by default. Source installation and archive packaging use the same
build target with an explicit output path. The pinned linter and read-only
module checks remain part of the [project quality gate](../../docs/dev/build-workflow.md).

The shared version module runs in prepare mode. `hack/bump-post-actions.sh`
updates the Go base version in the upstream staging directory. Tagging happens
after merge, as described in the [release workflow](../../docs/dev/build-workflow.md).
