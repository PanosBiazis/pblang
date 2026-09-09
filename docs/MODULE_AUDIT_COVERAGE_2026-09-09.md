# Module audit requested-root coverage — 2026-09-09

This local continuation follows canonical counter admission. It changes PBLang
source only; existing unrelated work is preserved. The audit engine advances
from v7 to v8 because completeness and checkpoint admission semantics changed.
This supersedes the duplicate/requested-root follow-up recorded in
`MODULE_AUDIT_ADMISSION_2026-09-09.md`; its other stated boundaries remain.

## Observed problem and result

A live probe used canonical complete zero-finding reports and requested roots
`["a", "b"]`. Before this change, both module-root sequences `["a", "a"]`
and `["a", "unrequested"]` returned `ok=true`, `complete=true`, and a passing
quality gate. The same probe now returns `ok=false, complete=false`, without
a quality gate, with `duplicate module root` and `module root was not requested`
respectively. Count equality alone no longer establishes requested coverage.

The security review also identified a checkpoint read-scope gap: resume
previously verified every complete loaded module's snapshot before looking it
up among invocation roots. A modified root could therefore reach inventory
and bounded content reads even with an unchanged audit identity. This prior
read path was established by code review; no prior-version live read test is
claimed. Current rejection was tested with exact-path syscall tracing.

## Implemented contract

`stdlib/module_audit_contract.sapp` owns shared pure `roots_error(roots)` and
`coverage_error(modules, requested_roots)` helpers. They return empty strings
on admission or fixed diagnostics without supplied values. Roots are exact
nonempty strings, at most 4096 code points without NUL, in arrays of at most
256 entries. Requested and reported roots must be unique. If the requested
list is nonempty, every reported root must belong to it. Counter validation
continues separately; root admission is not a reusable authorization token.

A subset is a valid incomplete checkpoint with a blocked quality gate.
Reordering modules is allowed. Full coverage requires the same unique root
set and complete module reports. An empty requested list keeps the existing
unspecified-coverage convention, including a passing empty aggregate.

For example, after scanning two approved roots with `audit_modules`, combine
its `report.modules` with `map{requested_roots: [a, b]}`. Combining only
`[report.modules[1]]` is valid but incomplete; combining
`[report.modules[1], report.modules[0]]` is valid full coverage. Repeating
`report.modules[0]` rejects instead of filling the missing root's position.

`audit_modules` admits all input roots before inventory and no longer coerces
nonstring roots. Direct snapshot verification applies a requested list when
provided. The pure API does not canonicalize paths or perform filesystem I/O.

The CLI admits roots both before and after its existing absolute lexical
normalization, before previous-output deletion or checkpoint loading. Thus
normalized aliases such as `a` and `a/../a` reject with exit 2 while retaining
an existing output file. During resume, saved top-level requested roots must
exactly match the trusted invocation list, including order. Loaded modules
may be reordered or partial but must be unique members of that list. These
checks precede snapshot verification. Duplicate modules use `checkpoint module
reports are invalid`; foreign modules and inconsistent saved lists use
`checkpoint module coverage is invalid`. No supplied root is printed.

Unmodified v7 checkpoints have a different audit identity and require a fresh
scan. Rejection preserves their bytes; it does not perform migration.

## Validation

Twenty-two distinct selected semantic/integration suites passed through the
bounded trusted-checkout runner:

- `module_audit_coverage`, `module_audit_coverage_live`
- `module_audit_admission`, `module_audit_admission_live`
- `module_audit`, `module_audit_resume`
- `project_audit_gate_counters`, `project_audit_gate_counters_live`
- `project_audit`, `project_audit_limits`, `project_audit_gate_policy`,
  `project_audit_gate_policy_live`
- `project_text_audit`, `project_text_audit_gate_policy`,
  `project_text_audit_gate_policy_live`, `project_text_audit_finding_preflight`
- `pbaudit_cli`, `pbaudit_limits`, `pbaudit_numeric_preflight`
- `security_analysis`, `manifest_validate`, `portable_lock_current`

The new pure suite covers malformed root elements, exact length/count bounds,
duplicate and foreign roots, full/reordered/partial/incomplete coverage,
input preservation, mutation revalidation, legacy behavior, producer rejection
and direct snapshot membership. Existing overflow fixtures now use distinct
root labels so they continue to exercise arithmetic overflow independently.

The live suite creates only owned temporary fixture directories and uses
bounded child processes. It confirms genuine v8 resume, reordered and partial
checkpoints, duplicate/foreign modules, inconsistent saved lists, fixed
nonleaking diagnostics, unchanged rejected checkpoint bytes, normalized CLI
alias rejection and unchanged source files. The preceding admission live test
also confirms v7 identity rejection with byte preservation.

The opt-in trace run passed with `PB_MODULE_COVERAGE_TRACE=1` and
`PB_TEST_TIMEOUT_SECONDS=60`. Exact `strace -P` filters cover the owned foreign
root and its source file. A positive control first confirmed a real read was
visible; the rejected foreign-root checkpoint then produced zero matching
file events. Trace files are capped at 128 KiB and removed during cleanup.
The initial trace attempt could not establish its control because its 1 MiB
child file limit exceeded the runner's inherited limit; lowering it resolved
that validation setup failure. This is evidence for these exact owned paths,
not a general syscall, race or sandbox proof.

Source checks passed for all nine touched PBLang files with zero errors.
Retained CLI lint exited successfully with warnings, including existing
unused-variable/style diagnostics and misleading finite-number suggestions
for intentional NaN/infinity guards. No protective guard was removed. The
final project build passed without a stale-hash diagnostic; native `pblock`
published repository-relative dependency paths, and the portable-lock test
passed again. This build does not refresh the retained compiler executable.

All 206 initial Git status entries remain. Of 200 initially hashed ordinary
files, only nine intentionally edited files changed. Three new test/report
files make this a twelve-file continuation. Privacy-pattern checks and
`git diff --check` passed. No commit or push was performed.

Graphify query completed but returned no useful module-root context. Its final
AST update exceeded the 45-second cap (exit 124, no output), so graph freshness
is not confirmed. No task-owned test, trace, build or Graphify process remained.
Owned live fixtures and temporary comparison files were removed.

## Remaining limits

Exact labels do not prove distinct physical directories: case rules, lexical
aliases in direct callers, symlinks, hardlinks, mounts and filesystem races
remain separate concerns. This does not authenticate reports or prove that
findings were recorded faithfully. A consistently rewritten report can still
lie. Arbitrary metadata graphs, producer numeric bounds, shared whole-work
budgets, hard VM resource enforcement, typed FFI and independent physical
platform verification remain open. No performance improvement, whole-suite
success or production readiness is claimed.
