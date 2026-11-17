#!/usr/bin/env bash
# Generate the user-facing command reference from the selected command tree.
# Do not edit docs/manual/cli.md directly; run this script after changing commands.
set -euo pipefail

repo_root=$(CDPATH='' cd -- "$(dirname -- "$0")/.." && pwd)
manual_path="$repo_root/docs/manual/cli.md"
mkdir -p "$(dirname -- "$manual_path")"
if [ -z "${MF_BIN:-}" ]; then
  if [ "${MF_IMPLEMENTATION:-go}" != go ]; then
    printf 'Only the Go implementation is available.\n' >&2
    exit 2
  fi
  bash "$repo_root/scripts/build.sh"
  mf_bin="$repo_root/bin/mf"
else
  mf_bin="$MF_BIN"
  if [ ! -x "$mf_bin" ]; then
    printf 'MF_BIN is not executable: %s\n' "$mf_bin" >&2
    exit 2
  fi
fi

tmp_path=$(mktemp "${TMPDIR:-/tmp}/mf-manual.XXXXXX")
trap 'rm -f "$tmp_path"' EXIT

"$mf_bin" --generate-manual | sed 's/[[:space:]]*$//' > "$tmp_path"

mv "$tmp_path" "$manual_path"
trap - EXIT
printf 'Generated %s\n' "$manual_path"
