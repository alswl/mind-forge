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
    local staging=$1 tag=$2 tag_core expected_version expected_channel tag_revision
    export GOTOOLCHAIN=local CGO_ENABLED=0
    [[ "$tag" =~ ^v[0-9]+\.[0-9]+\.[0-9]+(-[0-9A-Za-z-]+(\.[0-9A-Za-z-]+)*)?(\+[0-9A-Za-z-]+(\.[0-9A-Za-z-]+)*)?$ ]] || package_die 'Provide a version tag, for example PACKAGE_TAG=v0.6.0'
    release_version=$(cat VERSION)
    base_version=${release_version#v}
    base_version=${base_version%%[-+]*}
    tag_core=${tag#v}
    tag_core=${tag_core%%[-+]*}
    [[ -n "$base_version" && "$tag_core" == "$base_version" ]] || package_die "Tag $tag does not match VERSION base $base_version"
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

    export MF_RELEASE_BUILD=false
    expected_version="$base_version-dev+$short_revision"
    expected_channel=dev
    if [[ -n "${RELEASE_TAG:-}" ]]; then
        [[ "$RELEASE_TAG" == "$tag" && "$tag" == "$release_version" ]] ||
            package_die 'Release tag must match PACKAGE_TAG and VERSION'
        tag_revision=$(git rev-parse --verify --quiet "refs/tags/$tag^{commit}") ||
            package_die "Release tag $tag is missing from this checkout"
        [[ "$tag_revision" == "$revision" && "$dirty" == false ]] ||
            package_die 'Release packaging requires a clean checkout at the tagged commit'
        export MF_RELEASE_BUILD=true
        expected_version=${tag#v}
        expected_channel=release
        if [[ "${expected_version%%+*}" == *-* ]]; then
            expected_channel=prerelease
        fi
    fi

    host_os=$(go env GOHOSTOS)
    host_arch=$(go env GOHOSTARCH)
    package_build "$host_os" "$host_arch" "$staging/work/probe"
    "$staging/work/probe" version --json > "$staging/build-info/version.json"
    package_build "$host_os" "$host_arch" "$staging/work/probe-report" TARGETS=mf-report
    [[ "$("$staging/work/probe-report" --version)" == "mf-report version $expected_version" ]] ||
        package_die 'Compiled mf-report version differs from mf'
    cp pkg/services/report/engine/inputs.sha256 "$staging/build-info/report-engine.sha256"
    metadata=$(cat "$staging/build-info/version.json")
    metadata_field() { printf '%s\n' "$metadata" | sed -n "s/.*\"$1\":\"\([^\"]*\)\".*/\1/p"; }
    [[ "$(metadata_field base_version)" == "$base_version" ]] || package_die 'Compiled base_version differs from source/tag'
    [[ "$(metadata_field version)" == "$expected_version" && "$(metadata_field channel)" == "$expected_channel" ]] ||
        package_die "Compiled version/channel differs from $expected_version ($expected_channel)"
    [[ "$(metadata_field commit)" == "$short_revision" ]] || package_die 'Compiled commit differs from source'
    [[ "$(metadata_field go_version)" == "$expected_go" ]] || package_die 'Compiled Go toolchain differs from pinned version'
    [[ "$(metadata_field compiler)" == gc && "$(metadata_field rustc)" == 'not applicable' ]] || package_die 'Compiled compiler metadata differs from the Go contract'
    build_date=$(metadata_field build_date)
    [[ "$build_date" =~ ^[0-9]{4}-[0-9]{2}-[0-9]{2}$ ]] || package_die 'Compiled build date is unavailable'
}

package_verify() {
    local staging=$1 binary=$2 target_os=$3 target_arch=$4 suffix=$5 companion
    verify_binary "$staging" "$binary" "$target_os" "$target_arch" "$suffix" mf
    companion="$staging/work/mf-report-$suffix"
    package_build "$target_os" "$target_arch" "$companion" TARGETS=mf-report ||
        package_die "Build failed for mf-report-$suffix"
    chmod 0755 "$companion"
    verify_binary "$staging" "$companion" "$target_os" "$target_arch" "mf-report-$suffix" mf-report
}

verify_binary() {
    local staging=$1 binary=$2 target_os=$3 target_arch=$4 suffix=$5 command=$6 build_info setting
    build_info=$(go version -m "$binary")
    [[ "${build_info%%$'\n'*}" == "$binary: $expected_go" ]] || package_die "Wrong compiler for $suffix"
    printf '%s\n' "$build_info" | awk -F '\t' -v path="github.com/alswl/mind-forge/cmd/$command" '$2 == "path" && $3 == path {found=1} END {exit !found}' ||
        package_die "Wrong command for $suffix"
    for setting in "GOOS=$target_os" "GOARCH=$target_arch" 'CGO_ENABLED=0' '-compiler=gc' "vcs.revision=$revision" "vcs.modified=$dirty"; do
        printf '%s\n' "$build_info" | awk -F '\t' -v wanted="$setting" '$2 == "build" && $3 == wanted {found=1} END {exit !found}' || package_die "Missing $setting for $suffix"
    done
    for setting in "github.com/alswl/mind-forge/internal/version.Version=$release_version" "github.com/alswl/mind-forge/internal/version.ReleaseBuild=$MF_RELEASE_BUILD" "github.com/alswl/mind-forge/internal/version.Commit=$short_revision" "github.com/alswl/mind-forge/internal/version.BuildDate=$build_date"; do
        [[ "$build_info" == *"$setting"* ]] || package_die "Missing version stamp $setting for $suffix"
    done
    {
        printf '%s: %s\n' "${binary##*/}" "$expected_go"
        printf '%s\n' "$build_info" | tail -n +2
    } > "$staging/build-info/$suffix.txt"
}

package_after() {
    local staging=$1 archive suffix primary companion
    : > "$staging/checksums.txt"
    for archive in "$staging"/mf-"$tag"-*.tar.gz; do
        suffix=${archive##*/mf-"$tag"-}
        suffix=${suffix%.tar.gz}
        primary="mf-$suffix"
        companion="mf-report-$suffix"
        COPYFILE_DISABLE=1 tar --format=ustar -czf "$archive" -C "$staging/work" "$primary" "$companion" ||
            package_die "Could not archive the binary pair for $suffix"
        printf '%s  %s\n' "$(package_sha256 "$archive")" "${archive##*/}" >> "$staging/checksums.txt"
    done
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
