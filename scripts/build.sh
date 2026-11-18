#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/.."
export GOTOOLCHAIN=local CGO_ENABLED=0
expected_go="go$(awk '$1 == "golang" { print $2 }' .tool-versions)"
if [[ "$(go env GOVERSION)" != "$expected_go" ]]; then
    printf 'Expected %s; select the version in .tool-versions.\n' "$expected_go" >&2
    exit 1
fi
artifact="./bin/mf"
if [[ $# -gt 0 ]]; then
    if [[ $# -ne 2 || $1 != --out ]]; then
        printf 'Usage: scripts/build.sh [--out PATH]\n' >&2
        exit 2
    fi
    artifact="$2"
fi
revision=$(git rev-parse --short HEAD)
build_date=$(date -u +%Y-%m-%d)
mkdir -p "$(dirname "$artifact")"
# Go can discover the enclosing checkout instead of a nested worktree. Bind its
# Git subprocesses to the same worktree used for the explicit version stamp.
source_git_dir=$(git rev-parse --absolute-git-dir)
source_worktree=$(git rev-parse --show-toplevel)
GIT_DIR="$source_git_dir" GIT_WORK_TREE="$source_worktree" go build -ldflags "-X github.com/alswl/mind-forge/internal/version.Commit=$revision -X github.com/alswl/mind-forge/internal/version.BuildDate=$build_date" -o "$artifact" ./cmd/mf
