# e2e test runner image used by scripts/e2e.sh.
#
# Tests are run inside a container so their side effects — publish writes under
# /tmp, the global mf config dir, ad-hoc `git init` repos — stay in the
# container and never touch the host filesystem.
#
# The repo is bind-mounted read-only at runtime and compiled into an ephemeral
# in-container target dir, so the host source tree is never modified and every
# run is a clean build.
FROM rust:slim

# ── Step 1: apt mirror → USTC (http preferred over https in CN networks) ──
# Handles both old sources.list (*.list) and new DEB822 (*.sources) formats.
RUN find /etc/apt \( -name '*.list' -o -name '*.sources' \) -exec sed -i \
        -e 's|https\?://deb\.debian\.org/debian|http://mirrors.ustc.edu.cn/debian|g' \
        -e 's|https\?://security\.debian\.org/debian-security|http://mirrors.ustc.edu.cn/debian-security|g' \
    {} + \
    && apt-get update \
    && apt-get install -y --no-install-recommends git ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# ── Step 2: cargo crates-io → USTC sparse index ──
# Written to CARGO_HOME so every `cargo` invocation inside the container uses it.
RUN printf '[source.crates-io]\nreplace-with = "ustc"\n\n[source.ustc]\nregistry = "sparse+https://mirrors.ustc.edu.cn/crates.io-index/"\n' \
    >> /usr/local/cargo/config.toml

# ── Step 3: bake the latest stable toolchain + required components ──
# `rustup update stable` pulls the current stable (e.g. 1.96.0) so the image
# always ships the same version rust-toolchain.toml requests — no components
# are re-downloaded when the container starts.
# Uses the official server (better connectivity than CN mirrors for rustup).
RUN rustup update stable \
    && rustup component add rustfmt clippy

# Allow build.rs to run `git rev-parse` on the read-only mounted repo
# (host uid ≠ container root → git "dubious ownership" warning without this).
RUN git config --global --add safe.directory '*'

WORKDIR /work
