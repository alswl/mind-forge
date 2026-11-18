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
# The shared makefile-go build module owns compilation and version stamping.
exec make --no-print-directory build MF_BUILD_OUT="$artifact"
