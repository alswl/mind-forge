#!/usr/bin/env bash
# Run the mind-forge e2e suite (tests/e2e) inside a Docker container by default.
#
# Why containerize: e2e tests exercise `publish run` (writes under /tmp),
# `mf config` (the global mf config dir) and ad-hoc `git init` repos. Running
# them in a throwaway container keeps every side effect off the host. The repo
# is bind-mounted read-only and built into an in-container target dir, so the
# host source tree is never modified and each run is a clean build.
#
# Use --host to fall back to running directly on the host (no isolation).
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
IMAGE="mf-e2e:local"
DOCKERFILE="$REPO_ROOT/docker/e2e.Dockerfile"
# Named volume keeps /build/target across runs so cargo can do incremental
# builds. First run still pays the ~2 min clean compile cost; subsequent runs
# typically finish in ~10-30s.
CACHE_VOLUME="mf-e2e-cache"

MODE="docker"
REBUILD=0
CLEAN_CACHE=0
CARGO_ARGS=()

usage() {
    cat <<'EOF'
Usage: scripts/e2e.sh [options] [-- <extra cargo test args>]

Run the e2e suite (cargo test --locked --test e2e) in an isolated container.

Options:
  --host         Run on the host instead of Docker (no isolation).
  --rebuild      Force-rebuild the Docker image before running.
  --clean-cache  Drop the build cache volume (mf-e2e-cache) before running.
  -h, --help     Show this help.

Args after `--` are forwarded to `cargo test --test e2e`, e.g.:
  scripts/e2e.sh -- repo_lifecycle       # run one e2e module
  scripts/e2e.sh -- -- --nocapture       # pass args through to the harness
EOF
}

while [[ $# -gt 0 ]]; do
    case "$1" in
        --host) MODE="host"; shift ;;
        --rebuild) REBUILD=1; shift ;;
        --clean-cache) CLEAN_CACHE=1; shift ;;
        -h|--help) usage; exit 0 ;;
        --) shift; CARGO_ARGS+=("$@"); break ;;
        *) CARGO_ARGS+=("$1"); shift ;;
    esac
done

# Note: `${CARGO_ARGS[@]+"${CARGO_ARGS[@]}"}` expands a possibly-empty array
# safely under `set -u` (macOS ships bash 3.2, which errors on a bare "${a[@]}").

if [[ "$MODE" == "host" ]]; then
    echo ">> Running e2e on host (NO isolation): cargo test --locked --test e2e"
    cd "$REPO_ROOT"
    exec cargo test --locked --test e2e ${CARGO_ARGS[@]+"${CARGO_ARGS[@]}"}
fi

if ! command -v docker >/dev/null 2>&1; then
    echo "error: docker not found on PATH." >&2
    echo "       Install Docker, or run on the host with: scripts/e2e.sh --host" >&2
    exit 1
fi
if ! docker info >/dev/null 2>&1; then
    echo "error: docker is installed but the daemon is not reachable." >&2
    echo "       Start Docker, or run on the host with: scripts/e2e.sh --host" >&2
    exit 1
fi

# Build the runner image. Docker layer-caches this; only the cargo build inside
# the container below runs clean each time. Context is docker/ so the large
# target/ dir is never uploaded.
# DOCKER_BUILDKIT=1 enables BuildKit for parallel step execution.
if [[ "$REBUILD" == "1" ]] || ! docker image inspect "$IMAGE" >/dev/null 2>&1; then
    echo ">> Building e2e runner image ($IMAGE)..."
    BUILD_FLAGS=()
    [[ "$REBUILD" == "1" ]] && BUILD_FLAGS+=(--no-cache)
    # Use BuildKit when buildx is available; fall back to legacy builder otherwise.
    if docker buildx version >/dev/null 2>&1; then
        DOCKER_BUILDKIT=1 docker build "${BUILD_FLAGS[@]}" \
            -f "$DOCKERFILE" -t "$IMAGE" "$REPO_ROOT/docker"
    else
        docker build "${BUILD_FLAGS[@]}" \
            -f "$DOCKERFILE" -t "$IMAGE" "$REPO_ROOT/docker"
    fi
fi

TTY_FLAG=()
[[ -t 1 ]] && TTY_FLAG=(-t)

if [[ "$CLEAN_CACHE" == "1" ]]; then
    docker volume rm "$CACHE_VOLUME" >/dev/null 2>&1 || true
    echo ">> Dropped cache volume $CACHE_VOLUME"
fi

echo ">> Running e2e in container (host-protected, cache volume: $CACHE_VOLUME)"
# - Named volume on /build/target: incremental cargo builds across runs.
# - RUSTUP_TOOLCHAIN pins the channel directly so cargo skips the
#   rust-toolchain.toml lookup that otherwise triggers a ~30s rustup sync.
exec docker run --rm "${TTY_FLAG[@]}" \
    -v "$REPO_ROOT:/work:ro" \
    -v "$CACHE_VOLUME:/build/target" \
    -e CARGO_TARGET_DIR=/build/target \
    -e RUSTUP_TOOLCHAIN=stable \
    -e RUSTUP_DIST_SERVER= \
    -e RUSTUP_UPDATE_ROOT= \
    "$IMAGE" \
    cargo test --locked --test e2e ${CARGO_ARGS[@]+"${CARGO_ARGS[@]}"}
