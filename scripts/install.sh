#!/bin/sh
# Install mf from this working tree, or install a prebuilt GitHub release.
#
#   scripts/install.sh                 # build/install this checkout with cargo
#   curl -fsSL https://raw.githubusercontent.com/alswl/mind-forge/master/scripts/install.sh | sh
#
# Optional environment variables for release installs:
#   MF_VERSION      released tag to install (for example v0.4.2; default: latest stable)
#   MF_INSTALL_DIR  destination directory (default: $XDG_BIN_HOME or ~/.local/bin)
#   MF_IMPLEMENTATION=go selects the migrating Go source candidate locally.
#
# Pass arguments to cargo install only for a source install. Remote mode accepts
# no arguments so a piped invocation cannot accidentally alter the installer.
set -eu

REPO="alswl/mind-forge"
BASE_URL="https://github.com/${REPO}"

log() { printf '==> %s\n' "$*"; }
die() { printf 'Error: %s\n' "$*" >&2; exit 1; }

download_dir=""
staged_install=""
cleanup() {
	if [ -n "$staged_install" ]; then rm -f "$staged_install"; fi
	if [ -n "$download_dir" ]; then rm -rf "$download_dir"; fi
}
trap cleanup EXIT
trap 'exit 1' HUP INT TERM

install_verified() {
	candidate="$1"
	expected_base="$2"
	install_dir="${MF_INSTALL_DIR:-${XDG_BIN_HOME:-$HOME/.local/bin}}"
	[ ! -d "$install_dir/mf" ] || die "Install destination is a directory: $install_dir/mf"
	mkdir -p "$install_dir" || die "Cannot create install directory: $install_dir"
	[ -w "$install_dir" ] || die "Install directory is not writable: $install_dir (set MF_INSTALL_DIR)"
	staged_install=$(mktemp "$install_dir/.mf-install.XXXXXX") || die "Cannot stage installation in $install_dir"
	install -m 0755 "$candidate" "$staged_install" || die "Cannot copy candidate into $install_dir"
	installed_version=$("$staged_install" --version) || die "Candidate cannot run on this platform; existing mf was preserved"
	reported_base=$(printf '%s\n' "$installed_version" | awk '$1 == "mf" {version=$2; sub(/[-+].*/, "", version); print version; exit}')
	[ -n "$reported_base" ] && [ "$reported_base" = "$expected_base" ] || die "Candidate version does not match $expected_base; existing mf was preserved"
	mv -f "$staged_install" "$install_dir/mf" || die "Cannot replace $install_dir/mf"
	staged_install=""
	log "Installed mf -> $install_dir/mf"
	printf '%s\n' "$installed_version"
	case ":${PATH}:" in *":${install_dir}:"*) ;; *) log "$install_dir is not on PATH; add it with: export PATH=\"${install_dir}:\$PATH\"" ;; esac
}

install_source() {
	repo_root=$(CDPATH='' cd -- "$(dirname -- "$0")/.." && pwd)
	if [ "${MF_IMPLEMENTATION:-rust}" = go ]; then
		[ "$#" -eq 0 ] || die "Go source installation does not accept arguments."
		target_dir="${MF_INSTALL_TARGET_DIR:-${XDG_CACHE_HOME:-$HOME/.cache}/mind-forge/go-install-target}"
		case "$target_dir" in /*) ;; *) target_dir="$(pwd)/$target_dir" ;; esac
		log "Building Go candidate from $repo_root"
		bash "$repo_root/scripts/build.sh" --out "$target_dir/mf"
		base_version=$(awk '$1 == "const" && $2 == "BaseVersion" {gsub(/"/, "", $4); print $4}' "$repo_root/internal/version/version.go")
		[ -n "$base_version" ] || die "Cannot determine source base_version"
		install_verified "$target_dir/mf" "$base_version"
		exit 0
	fi
	[ "${MF_IMPLEMENTATION:-rust}" = rust ] || die "MF_IMPLEMENTATION must be rust or go."
	target_dir="${MF_INSTALL_TARGET_DIR:-${XDG_CACHE_HOME:-$HOME/.cache}/mind-forge/install-target}"
	mkdir -p "$target_dir"
	log "Installing mf from $repo_root"
	log "Build directory: $target_dir"
	exec cargo install --path "$repo_root" --locked --target-dir "$target_dir" "$@"
}

# A downloaded script (including curl | sh) has no adjacent source manifest.
# Go source detection also works after the legacy Cargo manifest is removed.
case "$0" in
	*/install.sh|install.sh)
	script_root=$(CDPATH='' cd "$(dirname "$0")/.." 2>/dev/null && pwd || true)
	if [ -n "$script_root" ]; then
		if [ -f "$script_root/go.mod" ] || [ -f "$script_root/Cargo.toml" ]; then
		    install_source "$@"
		fi
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
		latest_response=$(curl -fsSL "https://api.github.com/repos/${REPO}/releases/latest" 2>/dev/null) || latest_response=""
		VERSION=$(printf '%s\n' "$latest_response" | awk '{json=json $0 "\n"} END {if (match(json, /"tag_name"[[:space:]]*:[[:space:]]*"[^"]+"/)) {value=substr(json,RSTART,RLENGTH); sub(/^[^:]*:[[:space:]]*"/,"",value); sub(/"$/,"",value); print value}}')
	fi
fi
printf '%s\n' "$VERSION" | grep -Eq '^v[0-9]+\.[0-9]+\.[0-9]+(-[0-9A-Za-z-]+(\.[0-9A-Za-z-]+)*)?(\+[0-9A-Za-z-]+(\.[0-9A-Za-z-]+)*)?$' || die "Could not determine a valid release version. Set MF_VERSION explicitly, for example MF_VERSION=v0.4.2."
expected_base=${VERSION#v}
expected_base=${expected_base%%[-+]*}

OS=$(uname -s)
case "$OS" in Darwin) OS=apple-darwin ;; Linux) OS=unknown-linux-gnu ;; *) die "Unsupported operating system: $(uname -s)" ;; esac
ARCH=$(uname -m)
case "$ARCH" in x86_64|amd64) ARCH=x86_64 ;; aarch64|arm64) ARCH=aarch64 ;; *) die "Unsupported architecture: $ARCH" ;; esac
TARGET="${ARCH}-${OS}"
case "$TARGET" in
	x86_64-apple-darwin|aarch64-apple-darwin|x86_64-unknown-linux-gnu|aarch64-unknown-linux-gnu) ;;
	*) die "No release binary is available for $TARGET" ;;
esac

download_dir=$(mktemp -d)
asset="mf-${VERSION}-${TARGET}.tar.gz"
url="${BASE_URL}/releases/download/${VERSION}/${asset}"
log "Downloading $asset"
curl -fsSL --retry 3 --retry-delay 2 "$url" -o "$download_dir/$asset" || die "Failed to download $url"

checksums_url="${BASE_URL}/releases/download/${VERSION}/checksums.txt"
if curl -fsSL "$checksums_url" -o "$download_dir/checksums.txt" 2>/dev/null; then
	expected=$(awk -v a="$asset" '$2 == a {print $1}' "$download_dir/checksums.txt")
	[ -n "$expected" ] || die "No checksum entry for $asset"
	if command -v sha256sum >/dev/null 2>&1; then
		actual=$(sha256sum < "$download_dir/$asset" | awk '{print $1}')
	elif command -v shasum >/dev/null 2>&1; then
		actual=$(shasum -a 256 < "$download_dir/$asset" | awk '{print $1}')
	else
		die "Install sha256sum or shasum to verify the downloaded release"
	fi
	[ "$actual" = "$expected" ] || die "Checksum mismatch for $asset"
else
	die "Could not download checksums for $VERSION"
fi

tar -xzf "$download_dir/$asset" -C "$download_dir" "mf-${TARGET}" || die "Failed to extract $asset"
[ -f "$download_dir/mf-${TARGET}" ] && [ ! -L "$download_dir/mf-${TARGET}" ] || die "Archive member is not a regular executable"
install_verified "$download_dir/mf-${TARGET}" "$expected_base"
