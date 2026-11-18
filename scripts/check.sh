#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/.."
export GOTOOLCHAIN=local CGO_ENABLED=0

expected_go="go$(awk '$1 == "golang" { print $2 }' .tool-versions)"
if [[ "$(go env GOVERSION)" != "$expected_go" ]]; then
    printf 'Expected %s; select the version in .tool-versions.\n' "$expected_go" >&2
    exit 1
fi

unformatted=$(gofmt -l cmd internal)
if [[ -n "$unformatted" ]]; then
    printf 'Run gofmt on:\n%s\n' "$unformatted" >&2
    exit 1
fi

go vet ./...
go test -count=1 ./internal/...
bash scripts/build.sh
