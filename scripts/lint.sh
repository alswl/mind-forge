#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/.."
export GOTOOLCHAIN=local CGO_ENABLED=0

expected_version=2.12.2
if ! command -v golangci-lint >/dev/null 2>&1; then
    printf 'Install golangci-lint v%s to run the local gate.\n' "$expected_version" >&2
    exit 1
fi
actual_version=$(golangci-lint version --short)
if [[ "$actual_version" != "$expected_version" ]]; then
    printf 'Expected golangci-lint %s; found %s.\n' "$expected_version" "$actual_version" >&2
    exit 1
fi
golangci-lint run ./...
