#!/usr/bin/env bash
set -euo pipefail
cd "${BUMP_PREPARE_DIR:-$(dirname "$0")/..}"
version=$(tr -d '\r\n' < VERSION)
[[ "$version" =~ ^v([0-9]+\.[0-9]+\.[0-9]+)([-+][0-9A-Za-z.+-]+)?$ ]] || { echo "Invalid VERSION: $version" >&2; exit 1; }
base_version=${BASH_REMATCH[1]}
BASE_VERSION="$base_version" perl -pi -e 's/^const BaseVersion = ".*"/const BaseVersion = "$ENV{BASE_VERSION}"/' internal/version/version.go
gofmt -w internal/version/version.go
