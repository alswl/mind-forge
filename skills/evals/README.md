# Serial mind/output skill evaluation

This is a real-tool behavioral suite, not a trigger benchmark. The seven original
cases remain unchanged under each skill's `evals/`; `suite.json` records their
original prompts/expectations and the explicit current-contract adaptation for
W4. Expanded cases cover zero/multiple outputs, both layouts, short single-file
and block sources, additional rename, bounded revisions with new evidence,
partial feedback, existing authorization and a real failing build.

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

Each case has three independent baseline and candidate runs. The runner uses only
`zsh -ic 'codex-glinkap-astra exec ...'`; it does not call bare Codex, synthesize
agent replies, change model, or use another implementation agent. The approved
wrapper sets the actual provider/model/permissions; verify these from its own
session metadata before the full suite. Record revisions and unavailable seed or
temperature explicitly. Do not print credentials or complete process argv/env.

The task workspace is `.agents/skills-evals/main-1` (ignored; primary experiment). Before editing skills,
freeze complete baseline skill trees, the current CLI binary, historical inputs,
source/parent hashes and protected files. `prepare.py --workspace <path>` builds
synthetic fixtures from those baseline snapshots using the actual pinned CLI.
It refuses existing case directories. The checked-in suite describes exact
messages; preserve earlier suite and fixture hashes before any revision.

Freeze `candidate-snapshot/{mf-plan,mf-write}`, arm-specific `mf-cli` in each snapshot,
`bin/mf`, `suite.json`, `rubric.md` and fixtures in `run-manifest.json.frozen`.
Both arms use the same binary, tools, fixture, messages and permissions. Each arm loads its own
complete plan/write/mf-cli snapshot, including the original CLI guide for baseline. No source documents from human repositories enter a run.
The isolated content root includes only synthetic data, local skill copies and
`.mind-forge/tools/mf`; placing the binary in a discovered root such as `bin/`
invalidates ownership checks. The pilot that exposed this is retained separately.

```sh
python3 .agents/skills-evals/main-1/runner.py --workspace .agents/skills-evals/main-1
python3 .agents/skills-evals/main-1/checker.py --workspace .agents/skills-evals/main-1
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

The former round 1/2 shared-new-guide runs are retained as auxiliary experiments.
Primary results are not reused from them. Successful publication is outside this
skill evaluation scope; the existing authorized-writing cases test non-redundant
permission handling. Category blocks are covered by N4 without adding a case.
