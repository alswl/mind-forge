# Configure a CLI invocation

Runtime TOML controls how `mf` is invoked. Author YAML (`minds.yaml`, project
`mind.yaml` and related content metadata) remains business data. `mf config show`
continues to show that business view; `config generate` exports business YAML,
not runtime TOML.

## Choose runtime settings

Precedence is explicit CLI flags > selected environment values > runtime TOML >
defaults. Each invocation freezes cwd and environment once. The TOML accepts only
these six top-level keys, with strict types and no unknown/nested tables:

```toml
root = "/absolute/path/to/knowledge"
project = "research"
output = "text"
quiet = false
verbose = 0
no-color = false
```

| Setting | Environment | Default |
| --- | --- | --- |
| `--root` | `MF_ROOT` | discover from cwd |
| `--project` | `MF_PROJECT` | infer where supported |
| `--output` | `MF_OUTPUT` | text (text/json only) |
| `--quiet` | `MF_QUIET` | false |
| `--verbose` | `MF_VERBOSE` | 0 |
| `--no-color` | `MF_NO_COLOR` | false |

`--config` overrides `MF_CONFIG` to select a TOML file; `config` is not a TOML key.
Otherwise the default follows Go 1.26.11 standard user-directory rules and diagnostics,
resolved lazily from the same captured invocation environment without temporary
environment mutation. It is `mf/config.toml` under the standard user configuration
directory: macOS `~/Library/Application Support`, Linux `$XDG_CONFIG_HOME` or
`~/.config`. Missing default files are normal. Explicit missing files, directories,
unreadable files, invalid TOML/types and unknown keys fail with usage exit 2 before
locking or mutation. An explicit config path bypasses default-directory lookup.
No new cache directory or automatic migration is created.

CLI/environment paths resolve against invocation cwd. TOML root resolves against
the TOML parent directory. Project selectors retain existing names/path forms.
Paths are not shell-expanded by mf: expand `~` in your shell or use absolute paths;
`${HOME}` inside TOML is literal. Effective nonempty project from any layer blocks
position-based inference; explicit empty root/project restores auto discovery.
Other explicit empty values are errors.

Explicit `--quiet=false`, `--no-color=false` and `--verbose=0` replace lower-layer
values. `-v` counts from zero rather than adding lower verbosity. Boolean env values
accept true/false/1/0; verbose is a nonnegative integer. Only selected env values
are parsed, so an explicit CLI value can override a malformed lower env value.
Quiet and verbose greater than zero conflict. Existing `--json` priority remains.

## Adopt 0.6.0

Three changes require attention:

1. **BREAKING CHANGE: configuration routing.** `--config`/`MF_CONFIG` now load
   runtime TOML and no longer select a repository. Replace repo routing through
   config with `--root /path/to/repo`, `MF_ROOT`, or running inside the repository.
   Keep author YAML as author YAML; do not pass it as runtime TOML.
2. **BREAKING CHANGE: relative XDG paths.** For actual standard-directory lookup,
   relevant nonempty relative XDG paths are errors. Unset them or use absolute
   directories. An explicit `--config /path/to/runtime.toml` bypasses the default
   config-directory lookup.
3. **BREAKING CHANGE: child log routing.** Pipeline child stdout/stderr go to
   invocation stderr. Consume stdout as the result; redirect stderr separately:

```sh
mf --root /path/to/repo --json build <selector> >result.json 2>build.log
```

Quiet drops child ordinary stdout but preserves child stderr diagnostics. Pipeline
stage failures remain best-effort: restore prior active output and skip dependent
stages, without changing to fail-fast. Configuration export still uses 0666 plus
umask for a new file, preserves existing mode and symlinks, and accepts arbitrary
explicit `--out` destinations. No new atomic-export guarantee is introduced.

## Help and terminal feedback

Help and the [manual](../manual/cli.md) share structured prerequisites, commands
and expected Effects. `--help`, version, completion and manual generation bypass
bad runtime configuration so you can recover.

Long operations may emit plain stage text to stderr only when stdout and stderr
are both real TTYs, output is text, quiet is off and TERM is not dumb. JSON and
pipes show no interactive progress. NO_COLOR produces no ANSI; no animation,
timer or time guarantee is provided. Ctrl-C keeps normal cancellation and
recovery behavior. No new remote publishing or automatic publication build occurs.
