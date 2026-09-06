# Test quality triage — 2026-09-06

Repository-visible inventory: 1,073 files under `tests`, including 969 semantic
`*_test.sapp` files and 38 Rust test files. This is filename inventory plus static
pattern triage, not a complete logical review or execution of every test.
Counts are this checkout's snapshot; unrelated uncommitted work was preserved.

Pattern triage found 109 semantic files mentioning temporary paths/filesystem
writes, 61 mentioning clock/sleep operations, and 370 containing `catch`. These
are review queues, not counts of defective/flaky tests. Synthetic source strings
and deliberate negative tests also match these patterns.

## Confirmed coverage/maintenance gaps

- `diagnostics_test.sapp` passed while the previous autofix rewrote literals and
  comments and suggested names from invisible scopes. The new
  `diagnostics_autofix_scope_test.sapp` first failed on the original content
  defect and now verifies exact source preservation, scope, bounds and rechecking.
- `selfhost_modules_test.sapp` writes a fixed shared temporary module path and
  does not remove it. Concurrent runs can collide and leave fixtures behind.
  It was not executed during this review. Replace with per-run owned fixtures
  and cleanup that runs on both success and failure.
- The two helpers in `selfhost_to_pbck_test.sapp` use time/source-length temporary
  names, launch the PATH-selected `pblang`, compare stdout without asserting the
  child exit status, and leave artifacts behind. A child can print expected text
  and still fail. Pin the tested executable, assert termination/status, bound
  output/time and give each run an owned fixture directory before executing it.
- `parallel_test.sapp` uses `assert(true)` after `par_for`: it checks normal
  return only, not whether each callback actually ran. Add a bounded observable
  completion check and error propagation cases, without unsynchronized shared
  mutable state or sleeps used as proof of completion.
- The font-path assertion in `graphics_helpers_test.sapp` is tautological after
  conversion to string (`empty or length > 0`). Separate a type/API smoke check
  from a conditional verified-font/fallback behavior test. Source-string checks
  elsewhere in that test are structural evidence, not live rendering evidence.

These pre-existing tests were not rewritten or run indiscriminately. Only the
diagnostic regression was implemented here; fixture ownership, child status and
parallel completion fixes remain work items.

## Required follow-through

Classify every test by purpose (unit/contract/integration/physical), effects,
owned fixtures, platform, dependencies, time/memory budget and cleanup. Distinguish
unsupported/skipped/unavailable from passed. Review broad exception catches for
exact expected errors; remove tautologies and only-print-success tests where a
behavioral assertion is required. Repeated/random-order isolated runs should
record instability rather than retry failures into a green result. Use mutation
checks to prove critical assertions detect the defect they claim to cover.

The 1.0 candidate requires the broader compatibility corpus and release-host
checks in `ROADMAP.md`. Daily local testing stays focused; successful focused
tests do not certify the whole suite or all existing applications.
