#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
version=$(tr -d '\r\n' < VERSION)
if [[ ! "$version" =~ ^v([0-9]+\.[0-9]+\.[0-9]+)([-+][0-9A-Za-z.+-]+)?$ ]]; then
    printf 'Invalid VERSION: %s\n' "$version" >&2
    exit 1
fi
base_version=${BASH_REMATCH[1]}
source_version=$(awk '$1 == "const" && $2 == "BaseVersion" {gsub(/"/, "", $4); print $4}' internal/version/version.go)
[[ "$base_version" == "$source_version" ]] || { printf 'VERSION and internal/version/version.go disagree.\n' >&2; exit 1; }
