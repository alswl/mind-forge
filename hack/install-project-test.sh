#!/bin/sh
# Unit-test the generated install transaction with fake candidates and rename failures.
set -eu
installer=${1:-hack/install.sh}
scratch=$(mktemp -d)
sed '/^if command -v install_source /,$d' "$installer" > "$scratch/functions.sh"
. "$scratch/functions.sh"

clean_test() {
    touch "$scratch/resume"
    for pid in ${first_pid:-} ${second_pid:-}; do
        kill "$pid" 2>/dev/null || :
        wait "$pid" 2>/dev/null || :
    done
    rm -rf "$scratch"
}
trap clean_test EXIT

install_dir="$scratch/installed"
mkdir "$install_dir"
candidate() {
    printf '#!/bin/sh\nprintf "%%s\\n" "%s"\n' "$2" > "$1"
    chmod +x "$1"
}
for identity in previous a b; do
    mkdir "$scratch/$identity"
    candidate "$scratch/$identity/mf" "mf 0.7.0-dev+$identity"
    candidate "$scratch/$identity/mf-report" "mf-report version 0.7.0-dev+$identity"
done

assert_pair() {
    cmp "$scratch/$1/mf" "$install_dir/mf"
    cmp "$scratch/$1/mf-report" "$install_dir/mf-report"
    [ ! -e "$install_dir/.mf-install.lock" ]
}

for phase in before after; do
    cp "$scratch/previous/mf-report" "$install_dir/mf-report"
    ln -s "$scratch/previous/mf" "$install_dir/mf"
    if (
        mv() {
            case "$2" in
                */next/mf-report)
                    if [ "$phase" = after ]; then command mv "$@" || return; fi
                    return 1 ;;
            esac
            command mv "$@"
        }
        install_pair "$scratch/a/mf" "$scratch/a/mf-report" v0.7.0
    ) > "$scratch/failure.log" 2>&1; then
        die "Expected companion rename to fail $phase replacement"
    fi
    grep -Fq 'Cannot replace mf-report' "$scratch/failure.log"
    assert_pair previous
    [ -L "$install_dir/mf" ]
    rm "$install_dir/mf"
done

rm "$install_dir/mf-report"
if (
    mv() {
        case "$2" in */next/mf-report) return 1 ;; esac
        command mv "$@"
    }
    install_pair "$scratch/a/mf" "$scratch/a/mf-report" v0.7.0
) > "$scratch/failure.log" 2>&1; then
    die 'Expected companion rename to fail on fresh installation'
fi
[ ! -e "$install_dir/mf" ] && [ ! -e "$install_dir/mf-report" ] && [ ! -e "$install_dir/.mf-install.lock" ] ||
    die 'A failed fresh install left an executable or lock'

wait_for() {
    attempt=0
    until [ -e "$1" ]; do
        [ "$attempt" -lt 100 ] || die "Timed out waiting for $1"
        attempt=$((attempt + 1))
        sleep 0.05
    done
}
(
    mv() {
        case "$2" in
            */next/mf-report)
                touch "$scratch/paused"
                wait_for "$scratch/resume" ;;
        esac
        command mv "$@"
    }
    install_pair "$scratch/a/mf" "$scratch/a/mf-report" v0.7.0
) > "$scratch/a.log" 2>&1 &
first_pid=$!
wait_for "$scratch/paused"
(
    mkdir() {
        if command mkdir "$@"; then return 0; fi
        if [ "$1" = "$install_dir/.mf-install.lock" ]; then
            touch "$scratch/contended"
        fi
        return 1
    }
    install_pair "$scratch/b/mf" "$scratch/b/mf-report" v0.7.0
) > "$scratch/b.log" 2>&1 &
second_pid=$!
wait_for "$scratch/contended"
cmp "$scratch/a/mf" "$install_dir/mf"
[ ! -e "$install_dir/mf-report" ]
touch "$scratch/resume"
wait "$first_pid" || { cat "$scratch/a.log" >&2; exit 1; }
first_pid=
wait "$second_pid" || { cat "$scratch/b.log" >&2; exit 1; }
second_pid=
assert_pair b

install_pair "$scratch/a/mf" "" v0.7.0 >/dev/null
cmp "$scratch/a/mf" "$install_dir/mf"
cmp "$scratch/b/mf-report" "$install_dir/mf-report"
[ ! -e "$install_dir/.mf-install.lock" ]
printf '%s\n' 'Installer unit checks passed (rollback, fresh install failure, concurrency, legacy)'
