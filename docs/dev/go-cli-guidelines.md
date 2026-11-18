# Go CLI requirements

MindForge adopts the [Go CLI Development Guide](standards/go-cli-guides.md)
from `alswl/guides` revision
`e34c0006b2b79aad61dfd1a6e3a881ff7372cac7`. The local snapshot is the reviewable
baseline; development does not require a sibling checkout. Compare upstream
changes before updating the snapshot and this mapping.

Apply these requirements to new Go code and review changes against them.
The table distinguishes current implementation from remaining gaps; adopting
the guide does not establish full compliance or authorize incompatible changes
to existing CLI, configuration or storage contracts.

## Requirements and current mapping

| Guide requirement | MindForge implementation and remaining work |
| --- | --- |
| Go 1.22+, modules, Cobra | Go 1.26.11 is pinned in `.tool-versions` and `go.mod`; Cobra handles parsing. |
| Thin main; exit only at the process boundary | `cmd/mf/main.go` only calls `os.Exit(Execute())`; `Execute` owns signals and returns the status. |
| One command/subcommand file in `cmd/mf`, self-registration with `init()` | Implemented with per-command Cobra constructors and parent factory registration. Each execution gets a fresh tree; no root or family business dispatch remains. Frozen parser/help metadata stays shared. |
| `pkg/{services,managers,dal,common,config}`; command handlers only adapt input/output | Fixed layers are implemented; use cases and extracted command orchestration live in services, lifecycle algorithms in managers, filesystem persistence in dal, shared contracts/text in common, and existing config resolution in config. [Structure and dependency rules](code-structure.md) are enforced by a component test. |
| Viper; flags > environment > config > defaults | Viper 1.21.0 resolves a frozen invocation snapshot: explicit CLI > selected MF_* environment > six-key runtime TOML > defaults. Explicit false/0 replace lower values. Runtime TOML is separate from author YAML; see [configuration and migration](../guide/configuration.md). |
| `log/slog`, wrapped errors, stdlib testing + testify | Per-execution slog operational logs target stderr; user diagnostics retain their renderer. New architecture/isolation/cancellation checks use testing + testify; existing assertions are retained. Shared typed errors preserve causes. |
| makefile-go and golangci-lint | The pinned shared build module owns compilation, formatting/lint/module targets; `scripts/build.sh` delegates to it. Necessary compatibility hooks and their reproducible upstream patch are [documented](../../hack/makefile-go/README.md). Linter v2.12.2 remains in local and CI/release gates. |
| No-argument help, `-h`/`--help`, examples, `--version`, version command, actionable errors | Structured prerequisites, command and Effect examples share one renderer for help and generated manual. Existing catalogue and typed diagnostic renderer own the other contracts. Keep examples copyable and hints valid. Do not change published exit/status/help behavior as an incidental cleanup. |
| Named flags, GNU short flags, conventional names, prompts skippable by flags | Preserve existing `-o/--output text\|json`, `--out` file destinations, `-q`, `-v`, `--dry-run`, `--force` and `--yes` contracts. Existing positional selectors are compatibility requirements; new commands follow the guide. |
| Human output by default, machine output, stdout/stderr separation, verbosity | `pkg/common/outcome` defines envelopes and typed errors; `internal/output` renders diagnostics; `--json`/`--output json` supply machine output. Commands must use supplied writers. Pipeline child stdout and stderr both go to invocation stderr; quiet discards ordinary child stdout and preserves child stderr. Stage failure stays best-effort with previous-output restoration and dependent-stage skipping. No global requirement to introduce a second `--plain` output mode. |
| `NO_COLOR`, no color/progress in pipes | `internal/output/capability.go` computes terminal capabilities. Preserve explicit terminal overrides and quiet behavior; avoid unconditional ANSI output in CLI renderers. |
| Destructive confirmation, `--force`/`-y`, noninteractive operation, dry runs, idempotence | Confirmation uses the controlling TTY and existing `--yes`/`--force` semantics. Lifecycle and mutation services own dry-run and recovery safety. Whether a particular command confirms or accepts `-` is part of its existing contract, not a blanket behavior change. |
| Progress and graceful Ctrl-C; context through long-running work | Synchronous stage feedback is injected for actual long operations, only with both streams on real TTYs, text output, nonquiet mode and TERM other than dumb. JSON/pipes suppress progress; NO_COLOR emits no ANSI. Signal context reaches HTTP/Git, build pipelines, publication, file scans and lifecycle execution/recovery. Pipeline/Git process groups are cancelled together. Component checks cover cancellation; actual process coverage must be reported separately with external run evidence. |
| XDG/platform directories, `filepath.Join`, portable paths and executable names | Runtime configuration follows Go 1.26.11 `os.UserConfigDir` paths and diagnostics, resolving lazily from the same frozen invocation environment; the stdlib function itself only reads live environment and has no lookup parameter. An explicitly selected config bypasses the default-directory lookup. Relevant nonempty relative XDG paths are errors. No new cache use or directory migration is introduced. Windows native support is not claimed. |
| Explicit exit codes, error chains, stderr errors | Runtime returns codes through `cmd/mf.Run`; existing semantics are 0 success, 1 runtime failure, 2 usage/refusal and 64 unimplemented. Preserve typed error mappings rather than adopting different sysexits values. |
| Pipes and `-` for stdin/stdout | Implement and test this for commands that accept streams; commands selecting repository objects are not automatically stream consumers. Validate each new stream contract. |
| Version/commit/build time through ldflags | `scripts/build.sh` stamps `internal/version`; `mf version --json` exposes compiler, target and source metadata. Use freshly built `bin/mf`, not an installed PATH binary. |
| Unit tests, isolated dependencies/mocks, command integration tests | Colocated unit tests live under cmd/pkg/internal; original assertions/fixtures are retained. Full CLI/HTTP/PTY/process/install/platform checks remain in my-tests. No duplicate product harness is added. |
| Single executable, no runtime dependency for core use, tidy and verify modules | Release builds disable CGO. Optional user-configured pipeline commands require their own tools; Git operations require Git. `go mod verify` and `go mod tidy -diff` run in the local gate without rewriting manifests. |

## Local quality checks

Install Go 1.26.11 and the pinned development-only linter:

```sh
go install github.com/golangci/golangci-lint/v2/cmd/golangci-lint@v2.12.2
make check
```

Put `$(go env GOPATH)/bin` on PATH if needed. `make help` lists build, format,
lint, vet, component test, module verification and install targets.
`bash scripts/check.sh` remains the same full-gate entry point used by CI;
it checks formatting, vet, golangci-lint, module checksums/tidy state, all
cmd/pkg/internal component tests and the stamped executable build.

The linter enables `govet`, `ineffassign`, `unused` and Staticcheck's `SA*`
correctness checks. This intentionally does not claim the default linter set
is clean: the initial default-rule audit found 20 unchecked return values
(`errcheck`) and 12 simplification suggestions (Staticcheck `S`/`QF`). Review
those individually before extending the gate, including whether a close or
diagnostic write error can affect successful output or durable storage.
Never suppress all I/O errors to obtain a green check.

## Verification boundary

A passing local gate establishes local formatting/static/module/component/build
health only. Basic `bin/mf` commands can check the freshly built executable;
they do not replace external acceptance. Keep candidate source revision,
dirty state, binary digest and version alongside any external run records.
See [build workflow](build-workflow.md) for the exact candidate/harness rules
and the remaining migration acceptance boundaries.
