# Skill behavior evaluation tools

This suite evaluates planning, writing and report work with real tools. It does not measure
skill triggering. `suite.json` defines 16 synthetic cases, including three report cases.
Each case uses baseline and candidate arms with three repetitions.
A matrix definition does not imply that its runs have been executed or passed.

| Cases | Coverage |
| --- | --- |
| P1 | new outline, short single-file, multiple outputs, category |
| P2 | replan/prune, blocks, bundle |
| P3 | legacy comment routing, category |
| W1 | selected suggestion idempotence, category |
| W2 | verified feedback resolution, blocks, bundle |
| W3 | reopen with prior history, category |
| W4 | explicit deferral and human delivery decision, bundle |
| N1 | zero output mind, research without creating output, bundle |
| N2 | ambiguous output name across minds, no writes, category |
| N3 | explicitly selected sibling output, additional rename, prior authorization, bundle |
| N4 | old draft plus new evidence, same thinking, scoped edit, category blocks |
| N5 | partially satisfiable feedback remains open, bundle |
| N6 | actual CLI build failure, no fabricated success or publication, category |
| R1 | built-in trend, dual output and offline verification, category |
| R2 | registered component only, custom renderer reuse and catalog sample, bundle |
| R3 | ordinary Markdown revision with no report files, category |

The runner schedules three independent runs per case and arm. It uses only
`zsh -ic 'codex-glinkap-astra exec ...'`; it does not call bare Codex, synthesize
agent replies, change model, or use another implementation agent. The approved
wrapper sets the actual provider/model/permissions; verify these from its own
session metadata before the full suite. Record revisions and unavailable seed or
temperature explicitly. Do not print credentials or complete process argv/env.

Use a separate ignored workspace, such as `.agents/skills-evals/<run-id>`. Before editing skills,
freeze complete baseline skill trees, the current CLI binary, historical inputs,
source/parent hashes and protected files. Copy the selected freshly built CLI
into `<workspace>/bin/mf` and record its SHA-256 as `frozen["bin/mf"]` in
`run-manifest.json` before preparing fixtures. The prepare command below
builds synthetic fixtures with that exact binary and refuses a missing or
mismatched digest. To include R1–R3, supply `--report-store PATH` pointing
to a preloaded pnpm store. Preparation uses Node 24, pinned pnpm and the
starter lock offline, and writes the prepared suite to the run workspace.
Snapshot `mf-report` for the candidate. The historical baseline uses its own
`mf-cli` when the report skill is absent. Preparation does not select PATH binaries, retired Rust target paths,
or a hard-coded historical candidate. A different build requires a new frozen
workspace.
The repository does not include that binary, frozen skill snapshots, or run results.
It refuses existing case directories. The checked-in suite describes exact
messages; preserve earlier suite and fixture hashes before any revision.

Freeze `candidate-snapshot/{mf-plan,mf-write}`, arm-specific `mf-cli` in each snapshot,
`bin/mf`, `suite.json`, `rubric.md` and fixtures in `run-manifest.json.frozen`.
Both arms use the same binary, tools, fixture, messages and permissions. Each arm loads its own
complete plan/write/mf-cli snapshot, including the original CLI guide for baseline. No source documents from human repositories enter a run.
The isolated content root includes only synthetic data, local skill copies and
`.mind-forge/tools/mf`; placing the binary in a discovered root such as `bin/`
can change ownership discovery. Keep the binary in the hidden runtime directory.

Create the workspace with `run-manifest.json`, frozen snapshots, and the
matching binary before running prepare. Replace `<run-id>` with that workspace
name and run from the repository root.

```sh
export UV_CACHE_DIR="$PWD/bin/uv-cache"
eval_workspace=".agents/skills-evals/<run-id>"
uv run --no-project python -B skills/evals/prepare.py --workspace "$eval_workspace"
```

Record the prepared suite and fixture hashes in `run-manifest.json.frozen`
before launching runs. Continue in the same shell.

```sh
uv run --no-project python -B skills/evals/run.py --workspace "$eval_workspace"
uv run --no-project python -B skills/evals/check.py --workspace "$eval_workspace"
```

Optional `--case P1 --arm baseline --repeat 1` selects a single independent run.
The runner never overwrites an existing run. Keep failed attempts and use a new
iteration workspace for retries. An unexpected process error/timeout stops the
queue for diagnosis. `process.json` identifies only the owned subprocess;
`execution.json` records timing, usage, actual thread ID and resume entry.

Read each complete transcript, changed file and diff against the [rubric](rubric.md).
`check.py` extracts hard evidence but cannot grade semantics or infer a pass.
Write `grading.json` with `status`, four rubric scores with quoted evidence, and
`expectations: [{text, passed, evidence}]`. Include compatibility failures
separately from behavioral failures; quote errors and actual command outcomes.
`ungraded` is an execution-internal marker, never a reported behavioral pass.
Report such rows as incomplete pending review, and absent rows as not_run.

Retain all attempts, including pilot infrastructure failures, and aggregate every
repetition. Use the existing skill-creator grader protocol and static eval viewer
where available, without treating that viewer or lint as proof of behavior.

Successful external publication is outside this suite's scope. Existing
writing-authorization cases check that a skill does not ask for the same
permission repeatedly. Keep transcripts, grades, raw diffs, generated reports,
and failed attempts in the ignored workspace or external evidence storage.
Do not commit run output alongside these tools.
