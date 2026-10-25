# Direct module audit numeric bounds — 2026-09-09

This continuation completes numeric preflight for the direct module producer
and snapshot verifier. It follows the counter, canonical-module and requested
root fixes in the three preceding audit reports. The previously open producer
numeric-admission item is now addressed; shared work and runtime enforcement
remain separate. The engine stays v8 because valid detector, report and CLI
checkpoint semantics are unchanged.

## Before and after

An owned live probe called `audit_modules` on a one-file fixture with
`max_files_per_module: 1.5`. Before the change it returned `ok=true`,
`complete=true`, and one scanned file. A NaN value threw a generic
`comparison expects numbers` error; it is not described as a NaN bypass.
After the change both inputs return the fixed structured result
`ok=false, complete=false, error="invalid module audit bounds"` without
partial modules or a quality gate.

## Implementation and compatibility

The pure `module_audit_contract.bounds_error(options=nil, snapshot=false)`
checks a fixed six-field I/O table, plus four scan-only fields for the producer.
The complete table of defaults and inclusive endpoints is in
`Security_Analysis.md`, under Direct module numeric preflight. Values must be
ordinary finite exact integers. Missing values retain their previous defaults;
present nil values reject. Whole options nil/omission is supported, and
unrelated option fields remain allowed. Caller-sized key maps are not traversed.

The producer calls admission after the outer root-array check and before gate
policy, root identity and inventory. Snapshot verification admits its six
consumed I/O fields before inventory and returns its existing fixed
`invalid module snapshot` result on failure. It intentionally ignores scan-only
finding/index fields, even malformed ones, to preserve the consumer contract.
No new numeric result schema, checkpoint migration or engine bump is needed.

## Evidence and publishing scope

The new `module_audit_bounds` test covers all ten fields with malformed values
including nil, booleans, strings, arrays, maps, functions, numeric variants,
NaN/infinities, fractions and out-of-range integers. It admits inclusive
endpoints without executing maximum workloads and verifies input preservation,
mutation revalidation, defaults, error precedence, exact diagnostics and valid
low-bound producer/snapshot behavior.

The Linux `module_audit_bounds_live` test uses only owned temporary source
fixtures and bounded child processes. With `PB_MODULE_BOUNDS_TRACE=1`, an
exact-path syscall positive control first confirms a real source read is
visible. Rejected fractional producer and snapshot bounds then produce zero
matching file events. Source bytes stay unchanged and fixtures/logs are cleaned.
Trace files have a 128 KiB ceiling. This proves only the covered owned-path
cases; it is not a general no-I/O, sandbox or filesystem-race guarantee.

The publication contains only the current conversation's audit changes,
focused tests, canonical documentation, evidence reports and portable lock.
Unrelated dirty product/UI/compiler/inventory work is excluded. An isolated
checkout based on the current main commit is used to test the selected files
against committed dependencies and generate its own portable dependency hash;
a hash of the unrelated dirty stdlib is not published.

## Final selected-snapshot validation

All 24 selected suites passed in the isolated publish checkout: the 22 named
in `MODULE_AUDIT_COVERAGE_2026-09-09.md`, plus `module_audit_bounds` and
`module_audit_bounds_live`. The checkout includes only this conversation's
selected changes on top of main's committed dependencies. Its final build
passed without a stale-hash warning, and `pblock` produced a portable lock
whose hash covers exactly that selected stdlib. The retained compiler binary
was reused; no compiler executable refresh is claimed.

An initial checkout under a temporary-directory path correctly triggered the
audit's temporary-path exclusion. The final validation uses an ordinary project
path. One older policy test was updated to expect the producer's fixed numeric
preflight error for nonmap options; the other API policy errors still propagate.

Source checks for all 14 selected PBLang files returned zero errors; retained
CLI lint exited successfully with warnings, including unused-variable/style
diagnostics and misleading suggestions on intentional non-finite guards. Both
opt-in bounds and coverage trace suites also passed in the publish checkout.

Added-line/new-file privacy-pattern checks, whitespace checks and preservation
checks passed. Of 203 initially hashed ordinary files, five were intentionally
changed by this numeric slice; the previous audit changes are included in the
publication, while unrelated dirty files remain intact. The publish set is 20
files, including the lock generated from the selected checkout.

Graphify query completed. The final AST update exceeded its 45-second cap
(exit 124, no output), so graph freshness is not confirmed.

## Limits

Numeric admission does not add hard wall-time, heap, concurrency or whole-work
enforcement. Canonical counts and requested-root identity do not authenticate
checkpoints or prove faithful detection. Direct root labels are not physical
filesystem identities. The CLI already admitted these numeric options; this
change brings direct library calls into the same numeric domains. No speedup,
full-suite pass, physical-platform coverage or production readiness is claimed.
