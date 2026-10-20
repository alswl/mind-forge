#!/bin/sh
# Install mf from this working tree, or install a prebuilt GitHub release.
#
#   scripts/install.sh                 # build/install this checkout with cargo
#   curl -fsSL https://raw.githubusercontent.com/alswl/mind-forge/master/scripts/install.sh | sh
#
# Optional environment variables for release installs:
#   MF_VERSION      released tag to install (for example v0.4.2; default: latest stable)
#   MF_INSTALL_DIR  destination directory (default: $XDG_BIN_HOME or ~/.local/bin)
#
# Pass arguments to cargo install only for a source install. Remote mode accepts
# no arguments so a piped invocation cannot accidentally alter the installer.
set -eu

REPO="alswl/mind-forge"
PROJECT="mf"
BASE_URL="https://github.com/${REPO}"

log() { printf '==> %s\n' "$*"; }
die() { printf 'Error: %s\n' "$*" >&2; exit 1; }

install_source() {
	repo_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
	target_dir="${MF_INSTALL_TARGET_DIR:-${XDG_CACHE_HOME:-$HOME/.cache}/mind-forge/install-target}"
	mkdir -p "$target_dir"
	log "Installing mf from $repo_root"
	log "Build directory: $target_dir"
	exec cargo install --path "$repo_root" --locked --target-dir "$target_dir" "$@"
}

# A downloaded script (including curl | sh) cannot locate this repo's Cargo
# manifest. Local invocation keeps the optimized, persistent source install.
case "$0" in
	*/install.sh|install.sh)
	script_root=$(CDPATH= cd "$(dirname "$0")/.." 2>/dev/null && pwd || true)
	if [ -n "$script_root" ] && [ -f "$script_root/Cargo.toml" ]; then
		install_source "$@"
	fi
	;;
esac
[ "$#" -eq 0 ] || die "Remote installation does not accept arguments."

if [ -n "${MF_VERSION:-}" ]; then
	VERSION="$MF_VERSION"
else
	latest_url="${BASE_URL}/releases/latest"
	VERSION=""
	if VERSION=$(curl -fsSL -o /dev/null -w '%{url_effective}' "$latest_url" 2>/dev/null); then
		case "$VERSION" in */releases/tag/v*) VERSION="${VERSION##*/}" ;; *) VERSION="" ;; esac
	else
		VERSION=""
	fi
	if [ -z "$VERSION" ]; then
		VERSION="$(curl -fsSL "https://api.github.com/repos/${REPO}/releases?per_page=1" 2>/dev/null | awk -F'"' '/"tag_name"/ {print $4; exit}')" || VERSION=""
	fi
fi
case "$VERSION" in v[0-9]*) ;; *) die "Could not determine a release version. Set MF_VERSION explicitly, for example MF_VERSION=v0.4.2." ;; esac

OS=$(uname -s)
case "$OS" in Darwin) OS=apple-darwin ;; Linux) OS=unknown-linux-gnu ;; *) die "Unsupported operating system: $(uname -s)" ;; esac
ARCH=$(uname -m)
case "$ARCH" in x86_64|amd64) ARCH=x86_64 ;; aarch64|arm64) ARCH=aarch64 ;; *) die "Unsupported architecture: $ARCH" ;; esac
TARGET="${ARCH}-${OS}"
case "$TARGET" in
	x86_64-apple-darwin|aarch64-apple-darwin|x86_64-unknown-linux-gnu|aarch64-unknown-linux-gnu) ;;
	*) die "No release binary is available for $TARGET" ;;
esac

tmpdir=$(mktemp -d)
trap 'rm -rf "$tmpdir"' EXIT HUP INT TERM
asset="mf-${VERSION}-${TARGET}.tar.gz"
url="${BASE_URL}/releases/download/${VERSION}/${asset}"
log "Downloading $asset"
curl -fsSL --retry 3 --retry-delay 2 "$url" -o "$tmpdir/$asset" || die "Failed to download $url"

checksums_url="${BASE_URL}/releases/download/${VERSION}/checksums.txt"
if curl -fsSL "$checksums_url" -o "$tmpdir/checksums.txt" 2>/dev/null; then
	expected=$(awk -v a="$asset" '$2 == a {print $1}' "$tmpdir/checksums.txt")
	[ -n "$expected" ] || die "No checksum entry for $asset"
	if command -v sha256sum >/dev/null 2>&1; then
		actual=$(sha256sum "$tmpdir/$asset" | awk '{print $1}')
	elif command -v shasum >/dev/null 2>&1; then
		actual=$(shasum -a 256 "$tmpdir/$asset" | awk '{print $1}')
	else
		die "Install sha256sum or shasum to verify the downloaded release"
	fi
	[ "$actual" = "$expected" ] || die "Checksum mismatch for $asset"
else
	die "Could not download checksums for $VERSION"
fi

tar -xzf "$tmpdir/$asset" -C "$tmpdir" "mf-${TARGET}" || die "Failed to extract $asset"
INSTALL_DIR="${MF_INSTALL_DIR:-${XDG_BIN_HOME:-$HOME/.local/bin}}"
mkdir -p "$INSTALL_DIR" || die "Cannot create install directory: $INSTALL_DIR"
[ -w "$INSTALL_DIR" ] || die "Install directory is not writable: $INSTALL_DIR (set MF_INSTALL_DIR)"
install -m 0755 "$tmpdir/mf-${TARGET}" "$INSTALL_DIR/mf"
log "Installed mf $VERSION -> $INSTALL_DIR/mf"
"$INSTALL_DIR/mf" --version
case ":${PATH}:" in *":${INSTALL_DIR}:"*) ;; *) log "$INSTALL_DIR is not on PATH; add it with: export PATH=\"${INSTALL_DIR}:\$PATH\"" ;; esac
