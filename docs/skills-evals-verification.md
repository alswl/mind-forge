# Skills real-eval final verification

The primary **13 cases × 2 arms × 3 repetitions is complete: 78 executions and 78 individual semantic grades**. Candidate passes **39/39 (100%, all 8/8)**; baseline passes **28/39 (71.8%)**, with **11 behavior failures**. There are no primary execution errors, pending grades or missing runs. The current frozen candidate meets this package's rubric and hard protections. Final same-candidate business E2E is a separate next task and was not run here. This is a behavior evaluation, not a trigger-recall benchmark or a claim of statistical significance.

[All 78 results and timing](../.agents/skills-evals/main-1/results.md),
[machine-readable results](../.agents/skills-evals/main-1/results.json),
[human review page](../.agents/skills-evals/main-1/review.html),
[final integrity audit](../.agents/skills-evals/main-1/final-integrity.json),
[raw evidence hashes](../.agents/skills-evals/main-1/final-evidence-binding.json), and
[final candidate/handoff binding](../.agents/skills-evals/handoff-gc-4.json).
Each run retains actual tool transcript, source diff, before/after hashes, session identity, usage, hard checks and evidence-cited rubric. No completed run was rerun or replaced; no best-of selection was used.

## Three-repetition results

| Repetition | Baseline pass / fail | Candidate pass / fail |
| --- | --- | --- |
| 1 | 9 / 4 | 13 / 0 |
| 2 | 8 / 5 | 13 / 0 |
| 3 | 11 / 2 | 13 / 0 |

Every case/arm has three actual executions. Scores below list repetitions 1, 2, 3; F denotes behavior_fail even when its numerical score meets 7/8.

| Case | Baseline scores | Candidate scores |
| --- | --- | --- |
| P1 | 7 F / 7 F / 8 | 8 / 8 / 8 |
| P2 | 8 / 8 / 8 | 8 / 8 / 8 |
| P3 | 8 / 8 / 8 | 8 / 8 / 8 |
| W1 | 8 / 7 F / 8 | 8 / 8 / 8 |
| W2 | 8 / 8 / 8 | 8 / 8 / 8 |
| W3 | 6 F / 6 F / 8 | 8 / 8 / 8 |
| W4 | 6 F / 6 F / 7 F | 8 / 8 / 8 |
| N1 | 8 / 8 / 8 | 8 / 8 / 8 |
| N2 | 8 / 8 / 8 | 8 / 8 / 8 |
| N3 | 8 / 8 / 8 | 8 / 8 / 8 |
| N4 | 7 F / 7 F / 7 F | 8 / 8 / 8 |
| N5 | 8 / 8 / 8 | 8 / 8 / 8 |
| N6 | 8 / 8 / 8 | 8 / 8 / 8 |

The threshold remains every dimension ≥1, total ≥7/8, and all hard protections passing. Unauthorized publication, history loss and fabricated success veto acceptance. All candidate hard checks and all 78 actual protocol checks pass. The original N2 baseline run-1 checker false negative for `--root .` remains unchanged with its separate manual adjudication; candidate's original checker passed.

## All primary failures and compatibility observations

| Baseline case / repetitions | Behavioral reason |
| --- | --- |
| P1 / 1, 2 | Unauthorized offline source sync activated Lance and changed protected `minds.yaml`. Repetition 3 passed; the old skill does not invariably fail. |
| W1 / 2 | Same unauthorized backend activation; selected suggestion semantics otherwise passed. |
| W3 / 1, 2 | Reopened history correctly, but imposed an unsupported automatic publication blocker. Repetition 3 passed without that blocker. |
| W4 / 1 | Unauthorized activation plus incomplete deferred-thread relocation (Prompt had only a reference). |
| W4 / 2 | Incomplete thread relocation; no backend scope violation in this repetition. |
| W4 / 3 | Complete relocation succeeded, but unauthorized activation independently failed the hard scope protection. |
| N4 / 1, 2, 3 | Delta-first Evidence revision and source citation succeeded, but unauthorized activation changed `minds.yaml`. |

These 11 failures remain in the denominator. No candidate failure occurred in the primary matrix, so no new candidate freeze or remedial rerun was needed. The initial auxiliary candidate failure and subsequent correction remain in the historical section below.

Baseline had **23 runs with 41 recovered legacy-command events**; candidate had zero recorded legacy-command recovery events. These errors were recovered with current CLI commands and are not by themselves behavior failures. W4 baseline run-2's omitted numeric recovery field is supplied by a separate hash-bound `compatibility-supplement.json`; report.py verifies the original grade/transcript hashes before merging it. The immutable original grade was not edited. General wrapper configuration warnings are retained in raw transcripts, not mislabeled as behavioral failures. N6 deliberately fails an actual build in all six runs; accurate failure reporting passes the task and is not an infrastructure failure.

## Time, usage and actual skill use

| Primary arm | Run seconds, summed | Mean ± sample SD, seconds | Input tokens | Cached input subset | Output tokens | Reasoning output field |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| baseline | 5,387.84 | 138.15 ± 46.64 | 8,901,649 | 7,863,296 | 50,028 | 1,052 |
| candidate | 4,987.10 | 127.87 ± 54.11 | 8,285,242 | 7,313,536 | 44,830 | 1,213 |

The first 14 runs were serial. Subsequent parallel batches contained 10, 8, 9, 20 and 17 runs, each with at most three active approved wrappers and independent run directories. `parallel-5` skipped all 61 predecessors and completed exactly 17 new runs in 597.12 seconds of scheduler wall time. Its PID 971 exited naturally, active is empty, and no eval-working-directory process remains. Per-batch statistics are in results.json. Summed run durations overlap under concurrency and exclude handoff gaps; they are not experiment wall time. Temporal batches and different command paths prevent a controlled speed claim. Cached input is a subset of input; reasoning is a separately reported field, not added to output for an invented billing total. No pricing estimate is inferred.

All 78 transcripts contain actual reads of the correct arm's local plan/write skill and local mf-cli guide; exact content hashes and event IDs are in [actual skill reads](../.agents/skills-evals/main-1/actual-skill-reads-final.json). Baseline/candidate contain 277/295 tool events invoking the pinned binary; an event can contain multiple commands, so these are not individual-command counts. Model/provider/effort/permissions are gpt-6-astra / glinkap / medium / never / danger-full-access through the approved wrapper throughout. Model revision, seed and temperature remain unavailable. Inputs are synthetic and frozen; no implementation agent or external publication was used.

## Historical experiments retained

The capability probe, contaminated pilot and auxiliary shared-new-guide rounds are retained and excluded from the primary comparison. The pilot's misplaced binary caused ownership-check errors: baseline finished, candidate was interrupted, and both are harness execution errors rather than scored skill regressions. The complete original transcripts and classification remain under pilot-1.

| Auxiliary run | Result | Seconds | Input | Cached input | Output | Reasoning field |
| --- | --- | ---: | ---: | ---: | ---: | ---: |
| round-1 P1 baseline | behavior_fail 7/8 | 175.21 | 290223 | 261120 | 1707 | 77 |
| round-1 P1 candidate | behavior_fail 7/8 | 156.63 | 254562 | 224256 | 1753 | 10 |
| round-2 P1 baseline | pass 8/8 | 162.83 | 249300 | 213248 | 1371 | 52 |
| round-2 P1 candidate | pass 8/8 | 241.62 | 254925 | 226304 | 1585 | 32 |

Round 1 both arms failed on unauthorized activation. A bounded skill fix then prohibited implicit corpus/backend activation; round 2 both P1 samples passed, with its other 76 rows not run. Those incomplete auxiliary matrices remain incomplete and never substitute for primary repetitions. The corrected primary experiment used each arm's complete skill group and ran all 78 new samples. Previous GC reports below are historical snapshots, not current counts.

## Final integrity and candidate

All **499 protected files**, **181 frozen entries**, and the historical GC1/GC2/GC3 raw bindings (182/413/791 files) still match. GC3 predecessor digest is `40247735fcc3c1315bad693e296e2c0f43a267dd4b7b8011c2d4c41ae93e5669`. Final raw binding covers 1,020 files. The complete current skill trees match the tested candidate snapshot byte-for-byte. N3 lifecycle journals were decoded separately; both run-3 plans changed only rename source/destination/index, preserved resource bytes, and retained all 16 skill backup files exactly.

| Candidate file | SHA-256 |
| --- | --- |
| skills/mf-plan/SKILL.md | `7b2241a9b6eefcef7be7619e72c189818b23934028f808890bc5a31a1af48c56` |
| skills/mf-write/SKILL.md | `bf2e022c63bcfedd17e0fd23edfc3951d8551ba96c4d3e7924ab25f0a606696e` |
| skills/mf-cli/SKILL.md | `fb31685155c6612cd46045a25146833ce897479e5a8b6f9c44d94fa12cab8605` |

The final handoff binds every package file, all candidate tree files, the final report/review, and raw-evidence binding. HEAD remains `23b047c65151de670098a197c89612b88d594cad`; all inherited dirty work is preserved. No reset/stash/clean/commit/push/global install or real-content migration occurred. Existing skill validation remains valid because candidate text is unchanged. Report/review Python syntax and additive recovery aggregation were checked; the final static viewer was regenerated. No unrelated Rust full suite was rerun. Coverage is the 13-case table in skills/evals/README.md; successful publication and the next business E2E remain outside this package.

---

# Historical skills real-eval checkpoints

Task `mf-skills-evals-20261025` / `skills-evals`. **Incomplete; not an accepted
final candidate.** The skill changes and real evaluation harness are implemented,
but the complete primary 13-case × 2-arm × 3-repeat matrix has not been run or graded.
Latest GC3 handoff: **61/78 executed, 56 manually graded: 47 pass, 9 behavior_fail; five awaiting semantic review, 17 not run**. `parallel-4` PID 77394 safely drained all 20 calls after Captain’s GC marker and exited. The marker remains. Read [GC3 checkpoint](../.agents/skills-evals/CHECKPOINT-gc-3.md) and [binding](../.agents/skills-evals/handoff-gc-3.json). This is not final acceptance; earlier counts below are historical.
Captain continuation corrected the primary experiment to compare the entire
original skill group against the entire candidate group, including each arm’s
own mf-cli guide. The old round 1/2 results below are auxiliary only.
The primary queue is `.agents/skills-evals/main-1`; its prior results are not reused.
No improvement or full-pass claim is made. This report is a safe session handoff.

Parent candidate: `ac039f37d4c5a0e0410bd8aaa6774223245d385f0bea7d1aeae9665d31e507a7`,
HEAD `23b047c65151de670098a197c89612b88d594cad`. The initial 82 parent files
matched; 499 pre-existing files outside this package's seven edited skill files
remain byte-identical. F16, source, prior reports and historical evidence remain.
No global skill/CLI installation, real-content migration, publish, commit, push
or PR occurred. No implementation subagent was started.

## Implemented behavior

Plan/write resolve minds and selected outputs through the current CLI, preserve
zero-output minds and sibling outputs, distinguish editable `outputs/` from
`build/`, use project layout and existing shape, and select single-file short
notes. They use CLI identity/index/additional/lifecycle/build capabilities while
keeping semantic edits and history in Markdown. Existing authorization is reused;
partial/reopened/deferred work is represented without invented resolution or a
universal Feedback closure gate. Old drafts receive bounded evidence mapping and
revisions preserving author wording. Actual errors remain errors.

The CLI guide and agent prompts use current concepts and flags (`build --out`,
`--format`, global response `--output`). Legacy Article sections are explicitly
legacy, not aliases for mind outputs. Missing optional role locations are an
actual CLI limitation: `mind show` returns existing roles only. Skills must use
an explicit supplied path or report that gap, not invent a proposed path or
construct it from an output basename.

The second candidate additionally prevents local planning/writing from silently
activating a RAG corpus or changing its backend. A real paired failure exposed
that `source sync --offline` can rewrite `minds.yaml` to Lance. Frontmatter
`version` moved under `metadata` to satisfy the existing skill validator.

## Fixed evidence and actual execution

- [Initial baseline and protected hashes](../.agents/skills-evals/baseline.json).
  Complete baseline plan/write/CLI snapshots were copied before editing.
- [Frozen initial fixture hashes](../.agents/skills-evals/frozen-inputs.json).
  All input content is synthetic; no minds/logbook business feedback was sent to
  an external model.
- [Round 1 binding](../.agents/skills-evals/run-manifest.json) and
  [round 2 binding](../.agents/skills-evals/round-2/run-manifest.json).
- [Skill-use record](../.agents/skills-evals/skill-usage.json): actual reads of
  skill-creator, skill-eval and mf-cli; generic parallel execution instructions
  were superseded by the user's serial approved-wrapper scope.
- [Protection binding](../.agents/skills-evals/protection.json).

The launcher is the interactive-shell `codex-glinkap-astra` function, via
`codex-g-astra` / `_codex_run`; no bare-Codex eval was used. Its file digest was
`1815a9388f7f39a8bb715595416069d1074b05bfca1b46621ffeb86fe1e6cd5f`, shared wrapper
`d56ee62303f2f264a6b3c25e429360e39016f69b41d18af26c37b756ba049c32`, and Codex binary
`8813367285524ac798dd96d1b071aea30d58708c7226fd236cd275cf1cfdee31`.
Captain clarified that `410d03fa…` is the agent configuration object digest
returned by `lb config resolve --agent codex-glinkap-astra`, not a script/binary
hash. The earlier discrepancy concern was a digest-category mistake. The user
authorized the found wrapper; actual script and binary hashes remain recorded. No credentials, full process argv or environment were printed.

Actual session metadata verifies **gpt-6-astra, glinkap external, medium,
Codex 0.163.0, danger-full-access, approval never**. Provider model revision,
temperature and seed are unavailable; no stronger reproducibility claim is made.
Each run has an independent session, tool JSONL, stdout/stderr, before/after hashes,
file diff, timing/usage and session/resume ID. Same current `mf` binary SHA-256:
`e6b28cf014ecb358689ada4c1bfeb43332308319e6740f45eb3f4ac655e809f6`.
The primary arms share the binary, fixtures, messages and rights, but each loads
its own frozen mf-cli guide. Only the retained auxiliary rounds share new guidance.

## Results and limitations

[Explicit coverage and runner instructions](../skills/evals/README.md),
[suite](../skills/evals/suite.json), [assertions](../skills/evals/assertions.json),
and [rubric](../skills/evals/rubric.md) are reusable deliverables. The original
seven eval JSON files and fixtures are unchanged. W4's historic closure gate is
retained as historical evidence but explicitly superseded in current assertions
by the latest approved design; it is not silently dropped.

All planned rows, including absent attempts, appear in
[round 1 results](../.agents/skills-evals/results.md) and
[round 2 results](../.agents/skills-evals/round-2/results.md).
Scores require every hard protection, each semantic dimension >=1, and sum >=7/8.
`check.py` supplies mechanical evidence only; execution exit 0 never means pass.
Unauthorized publication, history loss and fabricated success veto a run.

The first real capability probe executed a shell copy successfully (session
`01a17346-bf05-7e71-be25-6d7e6850c3a0`). The first paired pilot placed the binary
under `bin/mf`; CLI ownership checks correctly rejected it as unowned content.
The baseline finished and candidate was interrupted after the contamination was
identified. Both pilot transcripts are retained under
[pilot-1](../.agents/skills-evals/pilot-1/classification.json), classified as
harness `execution_error`, not a skill regression or benefit. Moving the same
binary to excluded `.mind-forge/tools/mf` fixed this for both arms; a real index
check and additional rename dry-run passed ([evidence](../.agents/skills-evals/harness-check.json)).

Round 1 P1 repeat 1: **baseline behavior_fail 7/8; candidate behavior_fail 7/8**.
Both produced the authorized short outline, preserved sibling content and kept
suggestions unchecked, but activated Lance beyond the allowed write scope.
These are behavior/scope failures, not missing-command compatibility failures.
Round 2 P1 repeat 1: **baseline pass 8/8; candidate pass 8/8**. Both hard protection sets passed; neither activated the backend. Round 2 uses a newly frozen candidate after the fix; 76 of its 78 planned runs remain `not_run`.
Baseline behavior varies: the first round-2 repetition avoided activation and
passed 8/8. One passing retry cannot erase the earlier failure or establish gains.

Remaining work: complete and individually grade all three repetitions in both
arms, fix any candidate failures and preserve/retest affected rounds, aggregate
all outcomes/time/tokens, and generate the existing skill-creator static review
viewer from actual graded results. The primary suite covers category blocks through N4 as an existing fixture
variant, plus category file and bundle file/blocks. Successful publication is
explicitly excluded from this skill package’s acceptance scope by Captain;
already-authorized execution here means writing/revision permissions.

## Applicable checks

[Validation output](../.agents/skills-evals/validation.json): all three current
skill files pass the existing `quick_validate.py`; package `git diff --check`
passes. Runner/checker/preparation/report scripts parse successfully. The
pre-existing generated build-help snapshot has trailing whitespace in the
inherited diff; it was not changed by this package. The prior 2536-pass/2-ignore
full-suite result is inherited, not rerun or claimed as this package's test.
Skill-text changes did not justify rerunning the unrelated Rust full suite.

## Package files

Edited: `skills/README.md`, `skills/mf-cli/SKILL.md`,
`skills/mf-plan/SKILL.md`, `skills/mf-plan/agents/openai.yaml`,
`skills/mf-plan/assets/prompt.md`, `skills/mf-write/SKILL.md`,
`skills/mf-write/agents/openai.yaml`.

Added: `skills/evals/README.md`, `skills/evals/suite.json`,
`skills/evals/assertions.json`, `skills/evals/rubric.md`,
`skills/evals/prepare.py`, `skills/evals/run.py`, `skills/evals/check.py`,
`skills/evals/report.py`, and this report. Raw run data and snapshots remain in
ignored `.agents/skills-evals/`. The complete cumulative file/hash binding is
in [handoff-candidate.json](../.agents/skills-evals/handoff-candidate.json), not an assertion of final acceptance.


## Historical primary full-group continuation snapshot (5683960b)

At that earlier snapshot the primary queue remained active: P1 baseline failed
7/8 on unauthorized backend activation, P1 candidate passed 8/8; P2 both passed
8/8; P3 baseline passed 8/8, with candidate running. These five graded samples do
not satisfy three repetitions per arm and do not establish aggregate improvement.
Old shared-guide results are auxiliary, not primary evidence. Current files and
process state are in [latest checkpoint](../.agents/skills-evals/CHECKPOINT-main-1.md).
The full primary [matrix](../.agents/skills-evals/main-1/results.md) and
[real static review](../.agents/skills-evals/main-1/review.html) explicitly include
not_run/incomplete rows. The [immutable handoff view](../.agents/skills-evals/main-1/handoff-20261025-continue/review.html)
preserves this exact snapshot while the queue continues.

Additional reusable tools: `skills/evals/protocol.py` verifies actual model,
effort, permissions and immutable copied skill/binary hashes; `skills/evals/review.py`
adapts actual outputs to the existing skill-creator viewer without assigning grades.
All 16 files of the prior handoff candidate are archived with their exact old hashes;
new progress does not replace that candidate or its evidence.

## GC continuation: safe stopped queue, incomplete matrix

The original runner 3310 was absent on recovery. Its log ended after W2 baseline
run-1 at a STOP_AFTER_CURRENT boundary. No eval-working-directory process remained.
All 100 predecessor manifest files matched before work; exact copies now reside
under `.agents/skills-evals/handoff-5683960b-files/`. The approved frozen runner
resumed as PID 23950, skipped nine existing completed runs, and executed five more
serial runs: W2 candidate, W3 both arms, W4 both arms. No input or skill snapshot
was rebuilt. This session added nine manual grades, including the four completed
but ungraded predecessor runs. No candidate failure has been observed in these
seven candidate samples; this is not full acceptance or an improvement claim.

| Primary repeat 1 | Baseline | Candidate |
| --- | --- | --- |
| P1 | behavior_fail 7/8, unauthorized Lance activation | pass 8/8 |
| P2 | pass 8/8 | pass 8/8 |
| P3 | pass 8/8 | pass 8/8 |
| W1 | pass 8/8 | pass 8/8 |
| W2 | pass 8/8 | pass 8/8 |
| W3 | behavior_fail 6/8, added automatic publication blocker | pass 8/8 |
| W4 | behavior_fail 6/8, unauthorized Lance activation and incomplete thread relocation | pass 8/8 |

W3 baseline correctly reopens and preserves FB-004, but its new reply and final
report say unresolved feedback “blocks publication.” Evidence judgment and
feedback handoff each receive 1: the current design leaves delivery judgment to
the author. This is a semantic failure, not a new hard protection threshold.
W4 baseline correctly avoids a universal gate and preserves history, but leaves
the complete thread in the output and puts only a reference in Prompt. Its
`source sync --offline` also changes protected `minds.yaml`; that scope failure
independently vetoes acceptance. W4 candidate copies the complete thread into
Prompt, verifies the destination before source removal, and retains the unchecked
state, supplied reason and source anchor.

Four baseline samples (W1–W4) encounter legacy `article` command usage errors and
recover through current output commands. These are reported as **recovered
compatibility events**, separate from semantic task failures; no completed task
is classified as failed solely because a legacy command was unavailable. P1's
backend mutation remains a behavior failure. All 14 actual protocol checks pass.
The 499 protected original files still match their hashes, including F16 and
all previous source/report evidence.

The report now retains compatibility observations and actual CLI usage totals.
For these 14 executions: baseline 1161.13 seconds, candidate 1118.27 seconds;
reported input tokens 1,662,993 / 1,579,648, output tokens 11,244 / 9,416.
Cached input (1,484,800 / 1,380,736) is a subset of input, not an extra cost to sum.
All per-run usage/timing remains in execution.json; no billing estimate is inferred.

The actual static [review page](../.agents/skills-evals/main-1/review.html) and
[matrix](../.agents/skills-evals/main-1/results.md) include all 78 rows. Remaining:
N1–N6 both arms in repeat 1, then all cases in repeats 2 and 3 (64 runs). Resume
from N1 baseline run-1 only after confirming no active queue/children and removing
the existing stop marker. Preserve the frozen runner, approved wrapper, serial
protocol and all prior rounds; never infer completion from a directory alone.

## GC2 continuation: parallel batch safely drained

Recovered 24 completed / 22 graded runs. Reused the hash-verified thin scheduler and frozen runner after a dry-run showed 54 pending and 24 complete. `parallel-2` ran eight unique tasks with peak concurrency three; STOP_AFTER_CURRENT stopped dispatch and all calls drained naturally. Final state: 32 complete, 28 graded (23 pass / 5 behavior_fail), four pending review (P2/P3 both arms run-2), 46 not run. Next missing execution is W1 baseline run-2. This is not final acceptance.

N5 and N6 both arms run-1 pass 8/8 after full transcript/diff review. N5 preserves partial completion and missing rollback evidence; N6 accurately reports the intentionally failed actual build without repair or success registration. Baseline legacy-command failures recover and are separately recorded. P1 baseline run-2 fails 7/8 due to unauthorized Lance activation and minds.yaml change; candidate passes 8/8. No candidate revision was made in this session.

N2 original records clarify that the `--root .` regex false negative affects baseline only; candidate original hard checks pass. The original checker/result and separate baseline adjudication remain intact; existing grades are not overwritten.

[Isolation audit](../.agents/skills-evals/main-1/parallel-2/recovery-and-isolation.json) confirms unique per-run keys, no rerun of the prior 24 and maximum three concurrent calls. All 32 protocol checks pass; 499 protected files, frozen inputs and the old 14-run evidence binding remain byte-identical. The first 14 executions were serial, followed by parallel batches of ten and eight. Reports expose batch labels and separate timing totals; concurrency and temporal differences prevent interpreting pooled latency as a controlled speed comparison. All reported usage and failures are retained.

The [immutable GC2 review](../.agents/skills-evals/main-1/handoff-gc-2/review.html) and [state](../.agents/skills-evals/main-1/handoff-gc-2/state.json) preserve this handoff. Resume with a fresh parallel-3 batch after process/hash checks; dry-run should show 46 pending / 32 complete. Complete and manually grade the full matrix before acceptance; final business E2E remains a separate later task.


## GC3 continuation in progress

Recovered parallel-3 with nine finished runs, no active children and an ended timestamp; PID 63079 and eval-working-directory processes were absent. Verified 499 protected files, 181 frozen inputs and 413 GC2-bound raw evidence files unchanged. The unchanged scheduler dry-run reported exactly 37 pending / 41 completed; Captain’s GC marker was removed only after process checks. `parallel-4` started with three workers and unique run directories. See its recovery-and-isolation.json and live state/events.

The three inherited pending grades are now preserved: N1 baseline run-2 and W4 candidate run-2 pass 8/8; W4 baseline run-2 fails 6/8 because the complete deferred thread remains in the output while Prompt contains only a reference. Legacy article errors recovered independently. N4 baseline run-2 fails 7/8 on unauthorized source/backend activation; its Evidence revision otherwise satisfies the semantic assertion. N1 candidate and N2–N6 both arms run-2 pass, except that N4 baseline failure. N3 lifecycle journals were decoded and checked independently: only the two renamed paths and index entries differ; copied skill backups match exactly. N6 actual build failures remain intentional stimuli and are reported accurately, not classified as infrastructure failures. All original transcripts, diffs, failures and usage remain.

GC3 final safe boundary: 61 executed / 56 graded (47 pass, 9 behavior_fail), five pending grades listed in the checkpoint, 17 not run. All 61 protocol checks pass. parallel-4 finished 20 unique calls with peak three and no prior reruns; all drained after Captain marker. P1 both arms, P2 baseline and P3 candidate run-3 pass 8/8. W4 baseline run-2 recovered legacy errors are documented in its compatibility observation, but omitted from the numeric recovered-events aggregate because that grade has no recovered_compatibility_events field; raw error evidence remains. No candidate acceptance claim.
