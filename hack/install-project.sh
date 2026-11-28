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
    log "Building mf and mf-report from $repo_root"
    make --no-print-directory -C "$repo_root" build build-report MF_BUILD_OUT="$target_dir/mf" MF_REPORT_BUILD_OUT="$target_dir/mf-report"
    source_version=$(cat "$repo_root/VERSION")
    install_pair "$target_dir/mf" "$target_dir/mf-report" "$source_version"
}

validate_report_companion() {
    report_version=$("$1" --version) || return 1
    [ "$report_version" = "mf-report version $2" ]
}

install_pair() {
    primary=$1
    companion=$2
    version=$3
    primary_version=$(validate_candidate "$primary" "$version") ||
        die 'Mind Forge candidate validation failed'
    primary_version=$(printf '%s\n' "$primary_version" | awk 'NR == 1 {print $2}')
    [ -z "$companion" ] || validate_report_companion "$companion" "$primary_version" ||
        die 'mf-report does not match the mf candidate'
    [ -z "$companion" ] || [ ! -d "$install_dir/mf-report" ] ||
        die "Install destination is a directory: $install_dir/mf-report"

    # Shared installer validates and replaces the legacy command as before.
    # Stage the verified companion on the same filesystem before touching mf.
    if [ -n "$companion" ]; then
        mkdir -p "$install_dir"
        companion_stage=$(mktemp "$install_dir/.mf-report-install.XXXXXX")
        if ! install -m 0755 "$companion" "$companion_stage"; then
            rm -f "$companion_stage"
            die 'Cannot stage mf-report'
        fi
        if ( install_verified "$primary" "$version" ); then
            if ! mv -f "$companion_stage" "$install_dir/mf-report"; then
                rm -f "$companion_stage"
                die 'Cannot replace mf-report'
            fi
        else
            rm -f "$companion_stage"
            die 'Could not install the verified mf candidate'
        fi
        log "Installed mf-report -> $install_dir/mf-report"
    else
        install_verified "$primary" "$version"
    fi
}

install_release_pair() {
    primary_candidate=$1
    release_tag=$2
    release_suffix=$3
    companion_member="mf-report-$release_suffix"
    companion_candidate=
    if tar -tzf "$download_dir/$asset" | grep -Fxq "$companion_member"; then
        companion_listing=$(LC_ALL=C tar -tvzf "$download_dir/$asset" "$companion_member") ||
            die "Failed to inspect $companion_member"
        printf '%s\n' "$companion_listing" | awk 'NR != 1 || substr($0,1,1) != "-" {bad=1} END {exit bad || NR != 1}' ||
            die 'mf-report archive member is not a unique regular file'
        companion_candidate="$download_dir/report-candidate"
        tar -xOzf "$download_dir/$asset" "$companion_member" > "$companion_candidate" ||
            die 'Failed to extract mf-report'
        chmod 0755 "$companion_candidate"
    fi
    # The main member is streamed by the shared installer and needs execute
    # permission for verifying the pair before either destination is written.
    chmod 0755 "$primary_candidate"
    install_pair "$primary_candidate" "$companion_candidate" "$release_tag"
}
