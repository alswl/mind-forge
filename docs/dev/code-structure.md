# Go code structure

MindForge follows the fixed layers in the adopted
[Go CLI guide](standards/go-cli-guides.md). Domain subpackages inside a layer
retain their own APIs, embedded assets and colocated component tests.

| Directory | Responsibility | Allowed project dependencies |
| --- | --- | --- |
| `cmd/mf`, `cmd/mf-report` | Command construction, flags/arguments, confirmation and presentation | services, config, common, report server, private output/version and report CLI adapter |
| `pkg/server/report` | Huma operations, HTTP validation, listener and shutdown | report service |
| `internal/reportcli` | Shared report command selection and presentation helpers | report service and server, private output |
| `pkg/services` | Use cases, selection, business orchestration and transaction boundaries | other services, managers, dal, common, config |
| `pkg/managers/lifecycle` | Entity inventory, conversion plans, journal state and recoverable transaction algorithms | managers, dal, common |
| `pkg/dal/store` | Filesystem boundaries, locks, atomic writes, YAML/index persistence | dal, common |
| `pkg/common` | Models, outcome contracts, Markdown/text algorithms, task and logging utilities | common |
| `pkg/config` | Runtime TOML/input snapshot resolution, project/root discovery and effective layout | config, dal, common |
| `internal/output` | Private terminal rendering, capabilities, confirmation and diagnostics | common contracts and platform libraries |
| `internal/version` | Executable metadata and linker injection points | standard library |

Commands never import managers or dal. Managers and persistence never depend on
services, commands, or terminal renderers. The component architecture check in
`pkg/common/architecture` enforces the production import directions, including
build-tagged source. Managers exist for lifecycle's entity/state rules; simple
use cases call dal directly without a redundant manager facade.

## Commands

`main.go` only exits with `Execute()`'s returned status. `Execute` owns signal
setup and cleanup; `Run` provides per-invocation execution and error handling.
Each public command and subcommand has its own file, allocates its own
`*cobra.Command`, and registers its factory and action with its parent in
`init()`. Parent wiring constructs a fresh tree for each call to `NewCommand`.
There are no globally shared Cobra instances or flag values.

`spec_data.json` retains the frozen public parser/help metadata. The shared
constructor applies that metadata to each command's owned instance; it does not
dispatch business commands. `root.go` validates the registration inventory and
parent relationships. Per-command actions replace both the former root dispatch
chain and the command-family dispatch switches. Shared handler files contain
presentation, flag adapters or compatibility helpers used by multiple commands.

Build selector/path resolution, project lint selection/counting, term selection
and scan orchestration, article conversion selection/execution, render preparation,
and lifecycle recovery outcomes belong to services. Lifecycle planning and
transaction state live in the manager. Shared outcomes contain pure data and
serialization rules; services and dal do not import terminal presentation.

## Context, logs and tests

The process signal context reaches HTTP acquisition and Git commands, build
preparation/rendering/pipelines, publish execution, term file scans, lifecycle
execution/recovery and inventories. Cancellation is checked at file and durable
transaction boundaries. Shell/Git subprocesses run in their own process group;
cancelling the command kills the group, including descendants, rather than just
the shell. A failed cancelled pipeline stage restores its previous output before
returning the cancellation. Lifecycle journals retain their normal recovery
semantics.

Operational logging uses a per-invocation `log/slog` logger bound to stderr.
Debug operations follow existing verbosity; normal diagnostics and JSON/errors
continue through the existing renderer. No process-global logger is changed.

Use `testing` and `testify` for new component checks. Existing tests retain their
assertions and move with their code and fixtures. Component checks include
command-tree isolation, layer dependency rules, cancellation before publication,
pipeline cancellation/cleanup and logger isolation. Complete CLI, real HTTP,
PTY/process, installer and native-platform checks belong to `my-tests`.

## Building

The root Makefile includes the pinned shared `makefile-go/build.mk`, which owns
the compilation recipe, plus its help module. Direct builds, installers and
packaging call `make build` and share the same
version injection and CGO-disabled build. Read
[shared-module provenance and adaptations](../../hack/makefile-go/README.md)
before updating it. No sibling checkout is required.

Runtime configuration lives in `pkg/config/runtime.go`: each invocation uses a fresh Viper instance and a frozen cwd/environment/CLI snapshot. `ResolvedSelection` separates original project presence from the effective value; downstream consumers read that selection without mutating flags. Runtime TOML and author YAML have separate purposes; [configuration](../guide/configuration.md) documents precedence and adoption.

Application queries own article aggregation/sort, project inspection, output flattening, source filtering and preview validation. DAL owns term document/raw-byte handling and shared index render/write/readback outcomes, including committed-with-error; strict-load policy stays in services. Configuration export calls a service serializer and DAL writer with the existing umask/mode/symlink/explicit-output semantics.

Build process IO and runner are invocation-scoped. Prepare selects the complete IO group once: unspecified zero IO inherits process streams; an explicit group retains nil streams as discard. ProcessIO.Explicit marks an intentionally all-nil group, and an explicit Execute runner takes precedence. Child diagnostics use invocation stderr; quiet discards child stdout only. `common/task` supplies an optional synchronous observer; services report stages without importing a renderer. `internal/output` renders plain stage text and cmd injects the dual-TTY/text/nonquiet/TERM policy. No timer, animation, worker queue or latency promise is added. See [build workflow](build-workflow.md) for local checks and external
candidate identity requirements.
