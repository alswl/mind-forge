# Session Retrospective — Spec 074 (fix-source-term-bugs)

**Date**: 2026-08-05
**Branch**: `074-fix-source-term-bugs`
**Spec**: `specs/074-fix-source-term-bugs/`
**Driver**: `/speckit-implement` (T001–T020, all complete)

## What shipped

- **US1 (#30, fix)** — `term lint` short-CJK false positive: a correction now fires
  only when the matched span is one exact jieba token (for the ambiguous ≤2-Han-char
  class), and short pure-CJK `word` corrections are **advisory**
  (`replacement_eligible=false`, `safety_reason="short-cjk-advisory"`), never
  auto-applied without an explicit `--term` opt-in.
- **US2 (#33, improve)** — `mf source sync --rebuild` regenerates the Lance index
  to the current storage schema, reusing the admin-rebuild sequence; schema-drift
  hints now point at `sync --rebuild` for sync/search/retrieval callers.
- **US3 (#32, improve)** — auto-derived source-name collision fails with a usage
  error naming the taken source and suggesting a concrete `-n` value.
- **#31 (wontfix)** — `source rename` continues to move the backing file; no change.
- **Polish** — `quickstart.md` walkthrough verified end to end; `mf-cli` skill,
  `docs/manual.md`, `docs/term-lint.md` synced; full gate green.

## What went well

1. **TDD per constitution.** Every behavior change shipped a test observed failing
   first (T003–T005, T009–T011, T015), then implementation.
2. **The full suite caught a plan over-reach.** The plan said "require `is_token`
   for all CJK word corrections". Running the suite showed that would have stopped
   legitimate mixed CJK+ASCII compounds (e.g. `网关api`) and longer multi-token CJK
   terms from firing. Refined the mechanism to **exact-token only for short pure-CJK**
   — the class the advisory policy already covers. This was found by running the
   whole suite, not by unit tests.
3. **Empirical segmentation checks before finalizing.** Confirmed the reported span
   is not a single jieba token, and pre-validated replacement tokens (时刻, 实现理想)
   before rewriting tests — no guess-and-pray on jieba output.
4. **Early diagnosis of the stale-binary trap** (see below).

## What went wrong / surprises

1. **Stale binary on PATH.** The quickstart run showed the reported span "still
   flagged" until it turned out `mf` on PATH was an old installed build (dated days
   earlier) and the freshly built `target/debug/mf` behaved correctly. Cost: one
   confusing result before diagnosis.
2. **Plan call-site path drift.** T008 named `src/service/term/fix.rs` as the
   auto-apply gate, but that file is the *term-definition* editor; the advisory
   opt-in actually lands in `model/term.rs::classify` + `service/term/lint.rs`.
   The plan's file inventory was inaccurate; the code was the source of truth.
3. **Sensitive names in reporter-derived fixtures.** Three test terms ported from the
   reported bug (a 2-char misrecognition pair and a homophone pair) turned out to be
   real person names. I initially reported "no person names"; the maintainer
   corrected that. Scrubbed the code fixtures and the 074 spec docs to neutral pairs;
   the historical 072/059 spec docs retain them (the specs repo is private). Lesson:
   reporter-derived test data can be personal — scrub before it reaches a public repo.
4. **Sandbox network interference.** Tests that bind loopback sockets (mock embedding
   provider, HTTP acquisition) fail under the command sandbox with
   `Operation not permitted`; the baseline run showed one such failure that looked
   real. Re-ran the suite outside the sandbox.
5. **quickstart no-op probe.** The #30 walkthrough step probed with the *correct*
   spelling, which can never produce a finding; fixed during T017 to use a term that
   genuinely surfaces as advisory.
6. **BSD grep.** `\x{4e00}` character-class syntax is unsupported by macOS grep;
   used python for CJK extraction.

## Lessons (durable guidance → CLAUDE.md)

- **Verify with the freshly built binary** (`target/debug/mf` / `cargo run`), not a
  possibly-stale PATH install.
- **Scrub real person names from reporter-derived test fixtures.** Prefer synthetic
  non-name CJK pairs. Specs are private; the code repo may not be.
- **Run socket/network tests outside the command sandbox.**
- **Trust the full suite over the plan's mechanism.** When a planned change breaks
  existing tests, refine the mechanism rather than rewriting tests to match an
  over-broad change.
- **Verify plan call-site paths against the code** before implementing.

## Metrics

- 20/20 tasks complete; 637 insertions / 75 deletions across 20 files (+1 new test file)
- Gate: `cargo fmt --check` ✓ · `cargo clippy -- -D warnings` ✓ · `cargo test` 0 failures
- Branch pushed; PR #9
