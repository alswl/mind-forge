#!/usr/bin/env bash
# Bump the mf version and regenerate the CHANGELOG via git-cliff.
# Modeled on skm's hack/bump.sh.
#
# The version lives in internal/version/version.go. CHANGELOG.md is regenerated
# from conventional commit history with git-cliff.
#
# Usage: scripts/bump.sh [--stage <stage>] [--scope <scope>] [--dry-run <bool>] [--push <bool>]
#   --stage:    final, alpha, beta, candidate (default: final)
#   --scope:    major, minor, patch (default: patch)
#   --dry-run:  true|false (default: true)
#   --push:     true|false (default: false)
#
# Example: scripts/bump.sh --stage final --scope minor --dry-run false --push true

set -euo pipefail
cd "$(dirname "$0")/.."

stage="final"
scope="patch"
dry_run="true"
push="false"

while [ $# -gt 0 ]; do
  case "$1" in
    --stage|--scope|--dry-run|--push)
      [ $# -ge 2 ] || { echo "missing value for $1"; exit 1; }
      var=${1#--}; var=${var//-/_}; eval "$var=\"\$2\""; shift 2
      ;;
    *) echo "unknown option: $1"; exit 1 ;;
  esac
done

case "$stage" in final) suffix="" ;; alpha) suffix="alpha" ;; beta) suffix="beta" ;; candidate) suffix="rc" ;;
  *) echo "invalid --stage: $stage (final|alpha|beta|candidate)"; exit 1 ;;
esac
case "$scope" in major|minor|patch) ;;
  *) echo "invalid --scope: $scope (major|minor|patch)"; exit 1 ;;
esac

current=$(awk '$1 == "const" && $2 == "BaseVersion" {gsub(/"/, "", $4); print $4}' internal/version/version.go)
[ -n "$current" ] || { echo "cannot read version from internal/version/version.go"; exit 1; }

base=${current%%-*}
IFS=. read -r major minor patch <<< "$base"
case "$scope" in
  major) major=$((major + 1)); minor=0; patch=0 ;;
  minor) minor=$((minor + 1)); patch=0 ;;
  patch) patch=$((patch + 1)) ;;
esac
next_base="$major.$minor.$patch"
next="$next_base"
[ -z "$suffix" ] || next="$next-$suffix"

command -v git-cliff > /dev/null || { echo "git-cliff is required (brew install git-cliff)"; exit 1; }

if [ "$(git status --porcelain --untracked-files=no)" != "" ]; then
  echo "working tree has uncommitted changes; commit or stash first"
  exit 1
fi

if [ "$dry_run" = "true" ]; then
  echo ""
  echo "=============================="
  echo "  Version Bump Preview"
  echo "=============================="
  echo "  Current version : $current"
  echo "  Next version    : $next"
  echo "  Scope           : $scope"
  echo "  Stage           : $stage"
  echo "------------------------------"
  echo "  Actions (if DRY_RUN=false):"
  echo "    1. Write $next_base to internal/version/version.go"
  echo "    2. Regenerate CHANGELOG.md (via git-cliff)"
  echo "    3. git commit \"chore: bump version to $next\""
  echo "    4. git tag v$next"
  echo "    5. git push (with --push true)"
  echo "=============================="
  echo ""
  echo "------------------------------"
  echo "  Changelog Preview (unreleased)"
  echo "------------------------------"
  git cliff --unreleased --tag "v$next" 2>/dev/null | sed -n '/^## /,$p' || echo "  (git cliff failed, skipped)"
  echo "=============================="
  echo ""
  echo "  To execute, re-run with: --dry-run false"
  echo ""
  exit 0
fi

# 1. Update the base version.
NEXT="$next_base" perl -pi -e 's/^const BaseVersion = ".*"/const BaseVersion = "$ENV{NEXT}"/' internal/version/version.go
gofmt -w internal/version/version.go

# 2. Regenerate CHANGELOG.md from commit history
git cliff --tag "v$next" -o CHANGELOG.md

# 3-4. Commit and tag
git add internal/version/version.go CHANGELOG.md
git commit -m "chore: bump version to $next"
git tag "v$next"

echo ""
echo "=============================="
echo "  Release Complete"
echo "=============================="
echo "  Version: $next"
echo ""
echo "  Push commands:"
echo "    git push origin master v$next"
echo "=============================="

if [ "$push" = "true" ]; then
  echo ""
  echo "Pushing..."
  git push origin master "v$next"
  echo "Done."
fi
