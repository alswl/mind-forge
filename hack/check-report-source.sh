#!/bin/sh
set -eu
repo=$(pwd -P)
mkdir -p "$repo/bin"
stage=$(mktemp -d "$repo/bin/report-source.XXXXXX")
trap 'rm -rf "$stage"' EXIT
trap 'exit 1' HUP INT TERM
cp report/package.json report/pnpm-lock.yaml report/tsconfig.json report/generate.mjs "$stage"
cp -R report/src "$stage/src"
export REPORT_REPO="$repo"
export COREPACK_HOME="${COREPACK_HOME:-$repo/bin/report-corepack}"
export npm_config_cache="${npm_config_cache:-$repo/bin/report-npm-cache}"
store="${REPORT_PNPM_STORE:-$repo/bin/report-pnpm-store}"
cd "$stage"
set -- --frozen-lockfile --fetch-retries 0 --store-dir "$store"
if [ "${REPORT_PNPM_OFFLINE:-0}" = 1 ]; then
    set -- "$@" --offline
fi
corepack pnpm install "$@"
corepack pnpm exec tsc
corepack pnpm exec vitest run
node generate.mjs --check
