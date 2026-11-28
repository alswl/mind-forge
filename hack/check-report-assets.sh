#!/bin/sh
set -eu
manifest=pkg/services/report/engine/inputs.sha256
if command -v sha256sum >/dev/null; then
    sha256sum --quiet -c "$manifest"
else
    shasum -a 256 -s -c "$manifest"
fi
