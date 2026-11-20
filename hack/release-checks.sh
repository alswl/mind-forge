# Project validation hooks loaded by makefile-go distribution.mk.

submodule_git() (
    unset GIT_DIR GIT_WORK_TREE
    git -C "$submodule_path" "$@"
)

source_manifest() {
    local staging=$1 source_path source_sha256
    while IFS= read -r -d '' source_path; do
        [[ "$PWD/$source_path" != "$staging/"* ]] || continue
        [[ -f "$source_path" && ! -L "$source_path" ]] || continue
        source_sha256=$(package_sha256 "$source_path")
        printf '%s  %s\n' "$source_sha256" "$source_path"
    done < <(git ls-files -z --cached --others --exclude-standard)
}
package_before() {
    local staging=$1 tag=$2 tag_core
    export GOTOOLCHAIN=local CGO_ENABLED=0
    [[ "$tag" =~ ^v[0-9]+\.[0-9]+\.[0-9]+(-[0-9A-Za-z-]+(\.[0-9A-Za-z-]+)*)?(\+[0-9A-Za-z-]+(\.[0-9A-Za-z-]+)*)?$ ]] || package_die 'Provide a version tag, for example PACKAGE_TAG=v0.6.0'
    base_version=$(awk '$1 == "const" && $2 == "BaseVersion" {gsub(/"/, "", $4); print $4}' internal/version/version.go)
    tag_core=${tag#v}
    tag_core=${tag_core%%[-+]*}
    [[ -n "$base_version" && "$tag_core" == "$base_version" ]] || package_die "Tag $tag does not match base_version $base_version"
    expected_go="go$(awk '$1 == "golang" { print $2 }' .tool-versions)"
    [[ "$(go env GOVERSION)" == "$expected_go" ]] || package_die "Expected $expected_go; select the version in .tool-versions"
    revision=$(git rev-parse HEAD)
    short_revision=$(git rev-parse --short HEAD)
    mkdir "$staging/build-info"
    submodule_path=hack/.upstream-makefile-go
    submodule_revision=$(submodule_git rev-parse HEAD)
    expected_submodule=$(git ls-files -s -- "$submodule_path" | awk '$1 == "160000" {print $2}')
    [[ -n "$expected_submodule" && "$submodule_revision" == "$expected_submodule" ]] || package_die 'makefile-go submodule does not match the pinned revision'
    [[ -z "$(submodule_git status --porcelain)" ]] || package_die 'makefile-go submodule has uncommitted changes'
    printf '%s\n' "$submodule_revision" > "$staging/build-info/makefile-go-revision.txt"
    source_manifest "$staging" > "$staging/build-info/source-files.sha256"
    source_sha=$(package_sha256 "$staging/build-info/source-files.sha256")
    dirty=false
    [[ -z "$(git status --porcelain)" ]] || dirty=true

    host_os=$(go env GOHOSTOS)
    host_arch=$(go env GOHOSTARCH)
    package_build "$host_os" "$host_arch" "$staging/work/probe"
    "$staging/work/probe" version --json > "$staging/build-info/version.json"
    metadata=$(cat "$staging/build-info/version.json")
    metadata_field() { printf '%s\n' "$metadata" | sed -n "s/.*\"$1\":\"\([^\"]*\)\".*/\1/p"; }
    [[ "$(metadata_field base_version)" == "$base_version" ]] || package_die 'Compiled base_version differs from source/tag'
    [[ "$(metadata_field version)" == "$base_version-dev+$short_revision" ]] || package_die 'Compiled version differs from the retained development identity'
    [[ "$(metadata_field commit)" == "$short_revision" ]] || package_die 'Compiled commit differs from source'
    [[ "$(metadata_field go_version)" == "$expected_go" ]] || package_die 'Compiled Go toolchain differs from pinned version'
    [[ "$(metadata_field compiler)" == gc && "$(metadata_field rustc)" == 'not applicable' ]] || package_die 'Compiled compiler metadata differs from the Go contract'
    build_date=$(metadata_field build_date)
    [[ "$build_date" =~ ^[0-9]{4}-[0-9]{2}-[0-9]{2}$ ]] || package_die 'Compiled build date is unavailable'
}

package_verify() {
    local staging=$1 binary=$2 target_os=$3 target_arch=$4 suffix=$5 build_info setting
    build_info=$(go version -m "$binary")
    [[ "${build_info%%$'\n'*}" == "$binary: $expected_go" ]] || package_die "Wrong compiler for $suffix"
    for setting in "GOOS=$target_os" "GOARCH=$target_arch" 'CGO_ENABLED=0' '-compiler=gc' "vcs.revision=$revision" "vcs.modified=$dirty"; do
        printf '%s\n' "$build_info" | awk -F '\t' -v wanted="$setting" '$2 == "build" && $3 == wanted {found=1} END {exit !found}' || package_die "Missing $setting for $suffix"
    done
    for setting in "github.com/alswl/mind-forge/internal/version.Commit=$short_revision" "github.com/alswl/mind-forge/internal/version.BuildDate=$build_date"; do
        [[ "$build_info" == *"$setting"* ]] || package_die "Missing version stamp $setting for $suffix"
    done
    {
        printf '%s: %s\n' "${binary##*/}" "$expected_go"
        printf '%s\n' "$build_info" | tail -n +2
    } > "$staging/build-info/$suffix.txt"
}

package_after() {
    local staging=$1
    cat > "$staging/build-info/package.txt" <<MANIFEST
tag=$tag
base_version=$base_version
revision=$revision
dirty=$dirty
source_manifest_sha256=$source_sha
makefile_go_revision=$submodule_revision
go_version=$expected_go
build_date=$build_date
CGO_ENABLED=0
MANIFEST
    [[ "$(submodule_git rev-parse HEAD)" == "$submodule_revision" && -z "$(submodule_git status --porcelain)" ]] || package_die 'makefile-go submodule changed while packaging'
    source_manifest "$staging" > "$staging/build-info/source-files-after.sha256"
    [[ "$(package_sha256 "$staging/build-info/source-files-after.sha256")" == "$source_sha" ]] || package_die 'Source inputs changed while packaging; retry from a stable tree'
    rm "$staging/build-info/source-files-after.sha256"
}
