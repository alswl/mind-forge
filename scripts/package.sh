#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/.."
export GOTOOLCHAIN=local CGO_ENABLED=0

die() { printf 'Error: %s\n' "$*" >&2; exit 1; }
usage() { printf 'Usage: scripts/package.sh --tag v<VERSION> [--out NEW_DIRECTORY]\n'; }
sha256() {
    if command -v sha256sum >/dev/null 2>&1; then
        sha256sum < "$1" | awk '{print $1}'
    else
        shasum -a 256 < "$1" | awk '{print $1}'
    fi
}

tag=""
output=""
while [[ $# -gt 0 ]]; do
    case "$1" in
        --tag|--out)
            [[ $# -ge 2 && -n "$2" ]] || die "$1 requires a value"
            if [[ "$1" == --tag ]]; then tag="$2"; else output="$2"; fi
            shift 2
            ;;
        -h|--help) usage; exit 0 ;;
        *) usage >&2; die "Unknown argument: $1" ;;
    esac
done
[[ "$tag" =~ ^v[0-9]+\.[0-9]+\.[0-9]+(-[0-9A-Za-z-]+(\.[0-9A-Za-z-]+)*)?(\+[0-9A-Za-z-]+(\.[0-9A-Za-z-]+)*)?$ ]] || die 'Provide a version tag, for example --tag v0.6.0'
base_version=$(awk '$1 == "const" && $2 == "BaseVersion" {gsub(/"/, "", $4); print $4}' internal/version/version.go)
tag_core=${tag#v}
tag_core=${tag_core%%[-+]*}
[[ -n "$base_version" && "$tag_core" == "$base_version" ]] || die "Tag $tag does not match base_version $base_version"
expected_go="go$(awk '$1 == "golang" { print $2 }' .tool-versions)"
[[ "$(go env GOVERSION)" == "$expected_go" ]] || die "Expected $expected_go; select the version in .tool-versions"
command -v sha256sum >/dev/null 2>&1 || command -v shasum >/dev/null 2>&1 || die 'Install sha256sum or shasum'

output=${output:-release/$tag}
[[ "$output" == /* ]] || output="$PWD/$output"
[[ ! -e "$output" && ! -L "$output" ]] || die "Output already exists: $output; choose a new directory"
revision=$(git rev-parse HEAD)
short_revision=$(git rev-parse --short HEAD)
mkdir -p "$(dirname "$output")"
parent=$(cd "$(dirname "$output")" && pwd)
output="$parent/$(basename "$output")"
staging=$(mktemp -d "$parent/.mf-package.XXXXXX")
trap 'if [[ -n "$staging" ]]; then rm -rf "$staging"; fi' EXIT
mkdir -p "$staging/build-info" "$staging/work"
source_manifest() {
    local source_path source_sha256
    while IFS= read -r -d '' source_path; do
        [[ "$PWD/$source_path" != "$staging/"* ]] || continue
        [[ -f "$source_path" && ! -L "$source_path" ]] || continue
        source_sha256=$(sha256 "$source_path")
        printf '%s  %s\n' "$source_sha256" "$source_path"
    done < <(git ls-files -z --cached --others --exclude-standard)
}
source_manifest > "$staging/build-info/source-files.sha256"
source_sha=$(sha256 "$staging/build-info/source-files.sha256")
dirty=false
[[ -z "$(git status --porcelain)" ]] || dirty=true

host_os=$(go env GOHOSTOS)
host_arch=$(go env GOHOSTARCH)
GOOS="$host_os" GOARCH="$host_arch" bash scripts/build.sh --out "$staging/work/probe"
"$staging/work/probe" version --json > "$staging/build-info/version.json"
metadata=$(cat "$staging/build-info/version.json")
metadata_field() { printf '%s\n' "$metadata" | sed -n "s/.*\"$1\":\"\([^\"]*\)\".*/\1/p"; }
[[ "$(metadata_field base_version)" == "$base_version" ]] || die 'Compiled base_version differs from source/tag'
[[ "$(metadata_field version)" == "$base_version-dev+$short_revision" ]] || die 'Compiled version differs from the retained development identity'
[[ "$(metadata_field commit)" == "$short_revision" ]] || die 'Compiled commit differs from source'
[[ "$(metadata_field go_version)" == "$expected_go" ]] || die 'Compiled Go toolchain differs from pinned version'
[[ "$(metadata_field compiler)" == gc && "$(metadata_field rustc)" == 'not applicable' ]] || die 'Compiled compiler metadata differs from the Go contract'
build_date=$(metadata_field build_date)
[[ "$build_date" =~ ^[0-9]{4}-[0-9]{2}-[0-9]{2}$ ]] || die 'Compiled build date is unavailable'

: > "$staging/checksums.txt"
for target in x86_64-unknown-linux-gnu aarch64-unknown-linux-gnu x86_64-apple-darwin aarch64-apple-darwin; do
    case "$target" in
        x86_64-unknown-linux-gnu) target_os=linux; target_arch=amd64 ;;
        aarch64-unknown-linux-gnu) target_os=linux; target_arch=arm64 ;;
        x86_64-apple-darwin) target_os=darwin; target_arch=amd64 ;;
        aarch64-apple-darwin) target_os=darwin; target_arch=arm64 ;;
    esac
    member="mf-$target"
    binary="$staging/work/$member"
    archive="mf-$tag-$target.tar.gz"
    printf 'Building %s (%s/%s)\n' "$target" "$target_os" "$target_arch"
    GOOS="$target_os" GOARCH="$target_arch" bash scripts/build.sh --out "$binary"
    chmod 0755 "$binary"
    build_info=$(go version -m "$binary")
    [[ "${build_info%%$'\n'*}" == "$binary: $expected_go" ]] || die "Wrong compiler for $target"
    for setting in "GOOS=$target_os" "GOARCH=$target_arch" 'CGO_ENABLED=0' '-compiler=gc' "vcs.revision=$revision" "vcs.modified=$dirty"; do
        printf '%s\n' "$build_info" | awk -F '\t' -v wanted="$setting" '$2 == "build" && $3 == wanted {found=1} END {exit !found}' || die "Missing $setting for $target"
    done
    for setting in "github.com/alswl/mind-forge/internal/version.Commit=$short_revision" "github.com/alswl/mind-forge/internal/version.BuildDate=$build_date"; do
        [[ "$build_info" == *"$setting"* ]] || die "Missing version stamp $setting for $target"
    done
    {
        printf '%s: %s\n' "$member" "$expected_go"
        printf '%s\n' "$build_info" | tail -n +2
    } > "$staging/build-info/$target.txt"
    COPYFILE_DISABLE=1 tar --no-xattrs --format=ustar -czf "$staging/$archive" -C "$staging/work" "$member"
    archive_sha=$(sha256 "$staging/$archive")
    printf '%s  %s\n' "$archive_sha" "$archive" >> "$staging/checksums.txt"
done
cat > "$staging/build-info/package.txt" <<MANIFEST
tag=$tag
base_version=$base_version
revision=$revision
dirty=$dirty
source_manifest_sha256=$source_sha
go_version=$expected_go
build_date=$build_date
CGO_ENABLED=0
MANIFEST
source_manifest > "$staging/build-info/source-files-after.sha256"
[[ "$(sha256 "$staging/build-info/source-files-after.sha256")" == "$source_sha" ]] || die 'Source inputs changed while packaging; retry from a stable tree'
rm "$staging/build-info/source-files-after.sha256"
rm -rf "$staging/work"
[[ ! -e "$output" && ! -L "$output" ]] || die "Output appeared while building: $output"
mv "$staging" "$output"
staging=""
printf 'Prepared four archives and checksums in %s\n' "$output"
