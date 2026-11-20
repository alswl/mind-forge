repo=alswl/mind-forge
binary_name=mf
requested_version=${MF_VERSION:-}
version_variable=MF_VERSION
install_dir=${MF_INSTALL_DIR:-}

asset_suffix() {
    case "$1" in darwin) system=apple-darwin ;; linux) system=unknown-linux-gnu ;; esac
    case "$2" in amd64) cpu=x86_64 ;; arm64) cpu=aarch64 ;; esac
    printf '%s-%s\n' "$cpu" "$system"
}

validate_candidate() {
    installed_version=$("$1" --version) || return 1
    expected=${2#v}
    expected=${expected%%[-+]*}
    reported=$(printf '%s\n' "$installed_version" | awk '$1 == "mf" {version=$2; sub(/[-+].*/, "", version); print version; exit}')
    [ -n "$reported" ] && [ "$reported" = "$expected" ] || return 1
    printf '%s\n' "$installed_version"
}

install_source() {
    repo_root=$1
    shift
    [ "${MF_IMPLEMENTATION:-go}" = go ] || die 'Only the Go implementation is available.'
    [ "$#" -eq 0 ] || die 'Go source installation does not accept arguments.'
    target_dir="${MF_INSTALL_TARGET_DIR:-${XDG_CACHE_HOME:-$HOME/.cache}/mind-forge/go-install-target}"
    case "$target_dir" in /*) ;; *) target_dir="$(pwd)/$target_dir" ;; esac
    log "Building mf from $repo_root"
    make --no-print-directory -C "$repo_root" build MF_BUILD_OUT="$target_dir/mf"
    source_version=$(cat "$repo_root/VERSION")
    install_verified "$target_dir/mf" "$source_version"
}
