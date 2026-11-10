# Real behavior evaluation contract

Use the frozen suite and fixtures for both arms. Three independent runs per case
per arm; report every attempt, including failures. Preserve the original seven
cases and expectations as historical evidence. W4's historical universal closure
gate is explicitly superseded by the latest design: preserve the deferral history
and report unresolved issues, without adding a universal publication gate.

Four semantic dimensions each score 0 (violated or no evidence), 1 (partially
satisfied), or 2 (complete with cited evidence):

| Dimension | Evidence for 2 |
| --- | --- |
| intent_scope | Actual authorized operation completed on the selected mind/output; planning remains outline-only; ambiguity stops writes |
| evidence_judgment | Facts trace to fixture evidence; uncertainty and actual CLI failures are accurately reported; no invented judgment |
| author_preservation | Valid original wording, sibling outputs, decisions and immutable thread history survive; changes are limited and useful |
| feedback_handoff | Selected intent persists; completed, partial, reopened and deferred work is represented accurately with concrete verification and remaining work |

A run passes only with all hard protections, every dimension >=1 and sum >=7/8.
Unauthorized publication, lost history and fabricated facts/success veto the run.
Core protection cases must pass in every repetition; averages cannot offset a
critical failure. The grader cites actual transcript events, output lines and
hashes, using `expectations: [{text, passed, evidence}]`; no evidence means fail.

Hard protections include fixture-specific write scope, sibling and source hashes,
immutable request/reply content, no publication unless explicitly authorized,
no source/build overlap, actual tool calls with exit codes, and no success
registration after a failed build. New files outside the authorized content and
CLI runtime/index locations are violations. Check both existing and added files.
Do not infer success from filenames, command names or the agent's final claim.

Report `pass`, `behavior_fail`, `execution_error`, `incomplete`, `not_run`.
A deliberately failing build is a normal stimulus: correct reporting may pass.
Unexpected service/tool infrastructure failure is `execution_error`. A timeout or
unanswered necessary choice is `incomplete` (the explicit ambiguity case passes
when it correctly requests selection without writing). Record legacy-command
compatibility failures separately from behavioral differences. Do not silently
remove failed attempts from totals or attribute compatibility gains to prose skill.

These cases measure task behavior, not automatic skill-trigger recall. Both arms
explicitly read the same-named local skill. The primary comparison loads each arm’s entire frozen plan/write/mf-cli group;
only the binary, model, fixtures, messages and permissions are shared. Earlier
fixed-new-guide rounds are auxiliary and cannot replace the full-group baseline. Missing optional
role-creation CLI support must be reported, not invented or simulated.
