#!/usr/bin/env sh
# Install `mf` from this working tree, reusing a persistent build directory.
#
# `cargo install --path .` builds into a throwaway temporary directory, so every
# install recompiles all ~491 dependency crates from scratch in release mode —
# the LanceDB family (lance, datafusion, arrow) dominates that cost. Pointing
# --target-dir at a directory that survives between runs means dependencies are
# compiled once and reused; later installs rebuild only `mf` itself.
#
# The directory deliberately lives outside ./target so that `cargo clean` does
# not discard it. Override with MF_INSTALL_TARGET_DIR.
#
# Usage:
#   scripts/install.sh              # install from this tree
#   scripts/install.sh --force      # extra args are passed through to cargo

set -eu

repo_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
target_dir="${MF_INSTALL_TARGET_DIR:-${XDG_CACHE_HOME:-$HOME/.cache}/mind-forge/install-target}"

mkdir -p "$target_dir"

echo "installing from : $repo_root"
echo "build directory : $target_dir"
echo "(first run compiles all dependencies; later runs reuse them)"

exec cargo install --path "$repo_root" --locked --target-dir "$target_dir" "$@"
