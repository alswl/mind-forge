# Install mind-forge

> [!NOTE]
> Prebuilt binaries for tagged versions are attached to
> [GitHub Releases](https://github.com/alswl/mind-forge/releases). Building from
> source requires Rust 1.91+.

```bash
curl -fsSL https://raw.githubusercontent.com/alswl/mind-forge/master/scripts/install.sh | sh
export PATH="${MF_INSTALL_DIR:-${XDG_BIN_HOME:-$HOME/.local/bin}}:$PATH"
```

The installer downloads the latest stable tagged release for macOS or Linux on
x86_64 or ARM64 and verifies its SHA-256 checksum. Set `MF_VERSION=vX.Y.Z` to
install a specific tag, or `MF_INSTALL_DIR` to choose the install directory.
To build from a checkout instead, run `scripts/install.sh`; it keeps its build
directory between runs, so later installs rebuild only `mf` and skip the ~490
dependency crates.

Shell completion: `mf completion <shell>`.
