# Install mind-forge

> [!NOTE]
> Prebuilt binaries for tagged versions are attached to
> [GitHub Releases](https://github.com/alswl/mind-forge/releases). Building from
> source requires Go `1.26.11` and GNU Make.

```bash
curl -fsSL https://raw.githubusercontent.com/alswl/mind-forge/master/hack/install.sh | sh
export PATH="${MF_INSTALL_DIR:-${XDG_BIN_HOME:-$HOME/.local/bin}}:$PATH"
```

The installer downloads the latest stable tagged release for macOS or Linux on
x86_64 or ARM64 and verifies its SHA-256 checksum. Set `MF_VERSION=vX.Y.Z` to
install a specific tag, or `MF_INSTALL_DIR` to choose the install directory.
Current archives install both `mf` and `mf-report` at the same version.
Older archives can still install `mf` by itself. The report engine is embedded.
For report exports, provide Node 24 and install the report project's locked
dependencies. See the [report commands](../dev/report-core.md).
For a source install, run the installer from a checkout. Source builds use the
pinned Go toolchain and its normal module/compiler caches; `MF_INSTALL_TARGET_DIR`
chooses the binary output directory.

Shell completion supports `bash`, `zsh`, `fish`, `powershell`, and `elvish` via
`mf completion <shell>`. Load the generated program in that shell; save a
PowerShell program as a `.ps1` script. Elvish completion belongs in its
interactive editor configuration.

## Check for updates

Run `mf version --check` to compare the running executable with the latest
stable release from the [GitHub Latest Release API](https://api.github.com/repos/alswl/mind-forge/releases/latest).
The check requires internet access and accepts only a valid stable SemVer tag
from a release with both `draft` and `prerelease` set to `false`.

A stable release supersedes an installed prerelease or dev build with the same
numeric version. An installed build with a higher numeric version stays ahead,
even if it is a prerelease. Build metadata does not affect the comparison.
Run `mf version --check --json` for structured output.
The check leaves the installation untouched. Use the install command above
when you want to upgrade. Plain `mf version` and `mf --version` work offline.

## Build from source

Install Go `1.26.11` and GNU Make, then run:

```bash
hack/install.sh
```

Source builds disable CGO. `make build build-report` writes `bin/mf` and
`bin/mf-report`. Use these explicit artifact paths for verification. The installer accepts no additional
arguments. `MF_IMPLEMENTATION=go` is accepted for existing callers; other values
are refused. `MF_INSTALL_TARGET_DIR` chooses the reusable build output directory;
relative paths resolve from the caller's working directory. `MF_INSTALL_DIR`
overrides `XDG_BIN_HOME`, then `~/.local/bin`.

Downloaded installs keep the four existing archive names and the matching
`checksums.txt` entry. The installer verifies the checksum and validates the
staged executables and versions before replacing either command. Updates to the
same destination are serialized. If either replacement fails, the installer
restores the previous installation. Older single-binary archives update only `mf`.

Latest-version fallback uses GitHub's [latest release endpoint](https://docs.github.com/en/rest/releases/releases#get-the-latest-release),
which selects a published stable release. Explicit `MF_VERSION` accepts a version
tag such as `v0.5.3` or `v0.5.3-rc.1`; its core version must match the executable.
Formal source/prebuilt installation and four-platform execution are accepted in
the external verification repository, separately from package construction.
