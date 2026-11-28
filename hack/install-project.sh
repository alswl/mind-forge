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

finish_install() {
    install_status=$1
    trap - EXIT
    trap '' HUP INT TERM
    restore_failed=false
    if [ "$commit_in_progress" = true ]; then
        for name in $install_names; do
            # A same-filesystem rename consumes the staged file only if it
            # reached the destination, including when interrupted.
            [ ! -e "$install_lock/next/$name" ] || continue
            saved="$install_lock/previous/$name"
            if [ -e "$saved" ] || [ -L "$saved" ]; then
                mv -f "$saved" "$install_dir/$name" || restore_failed=true
            else
                rm -f "$install_dir/$name" || restore_failed=true
            fi
        done
    fi
    if [ "$restore_failed" = true ]; then
        die "Could not restore the previous installation. Recovery files remain in $install_lock"
    fi
    rm -rf "$install_lock" || die "Cannot clean up $install_lock"
    exit "$install_status"
}

install_pair() (
    primary=$1
    companion=$2
    version=$3

    mkdir -p "$install_dir" || die "Cannot create install directory: $install_dir"
    install_lock="$install_dir/.mf-install.lock"
    waits=0
    until mkdir "$install_lock" 2>/dev/null; do
        [ -d "$install_dir" ] && [ -w "$install_dir" ] || die "Cannot lock $install_dir"
        [ "$waits" -lt 30 ] || die "Timed out waiting for $install_lock"
        waits=$((waits + 1))
        sleep 1
    done
    commit_in_progress=false
    trap 'finish_install "$?"' EXIT
    trap 'exit 1' HUP INT TERM

    mkdir "$install_lock/next" "$install_lock/previous" || die 'Cannot create installation staging directories'
    install_names=mf
    install -m 0755 "$primary" "$install_lock/next/mf" || die 'Cannot stage mf'
    primary_report=$(validate_candidate "$install_lock/next/mf" "$version") ||
        die 'Mind Forge candidate validation failed'
    if [ -n "$companion" ]; then
        install_names='mf mf-report'
        install -m 0755 "$companion" "$install_lock/next/mf-report" || die 'Cannot stage mf-report'
        primary_version=$(printf '%s\n' "$primary_report" | awk 'NR == 1 {print $2}')
        validate_report_companion "$install_lock/next/mf-report" "$primary_version" ||
            die 'mf-report does not match the mf candidate'
    fi

    for name in $install_names; do
        target_path="$install_dir/$name"
        [ ! -d "$target_path" ] || die "Install destination is a directory: $target_path"
        if [ -e "$target_path" ] || [ -L "$target_path" ]; then
            cp -pP "$target_path" "$install_lock/previous/$name" ||
                die "Cannot preserve $target_path"
        fi
    done

    commit_in_progress=true
    for name in $install_names; do
        mv -f "$install_lock/next/$name" "$install_dir/$name" || die "Cannot replace $name"
    done
    commit_in_progress=false

    log "Installed mf -> $install_dir/mf"
    printf '%s\n' "$primary_report"
    case ":$PATH:" in
        *":$install_dir:"*) ;;
        *) log "$install_dir is not on PATH; add it with: export PATH=\"$install_dir:\$PATH\"" ;;
    esac
    if [ -n "$companion" ]; then
        log "Installed mf-report -> $install_dir/mf-report"
    fi
)

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
