# Audit finding-budget preflight and report preservation — 2026-09-08

## Reproduction and repair

Base: 1102757. PBLang-only production changes in project_audit,
project_text_audit and pbaudit. No Rust source or binary replacement.

A bounded baseline reproduction used an owned synthetic prior report and
--max-findings=0: the original CLI removed that report, then exited 1 with the
downstream limit error. No real user report was involved. The same late-admission
ordering allowed root collection before rejection, and module mode admitted
fractions until the first file was read and the direct auditor rejected them.

The new narrow public API, project_audit.require_finding_limit(value=nil),
delegates the existing finite integral ordinary-number 1..4096 check, returning
the admitted scalar or throwing the same fixed error. Missing/nil returns 500.
There is no caller-selected fallback and no reusable execution capability.
Direct text/record analyzers still independently validate their budgets.

audit_roots now calls it before collect_text_records. Missing max_findings
retains 200; explicit nil retains the existing aggregate default 500. Invalid
budgets keep the controlled exception but now precede collection/collection
errors. Success schemas are unchanged.

audit_modules validates max_findings_per_file before inventory and before its
combined ordered numeric checks. Omission stays 256; nil and all malformed values
return the exact existing {ok:false,complete:false,format:"pblang.module_audit.v1",
error:"invalid module audit bounds"} outcome. Invalid root shape retains first
precedence. This fixes the finding field, not every other module numeric option.

pbaudit parses only --max-findings with failure=>nil, then rejects malformed final
values before root normalization, prior-output removal, checkpoint loading,
snapshot verification, inventory and source reads. The fixed status-2 diagnostic
contains no caller values or paths. Default 160, integral syntax such as 1.0,
and duplicate-last-wins remain unchanged, including invalid-final rejection.
Other numeric options still use their existing parser and require separate work.
Runtime startup and trusted module imports still perform I/O.

## Checkpoint compatibility

module_audit_engine_revision is now v6. The CLI includes this value in audit_id.
An actual v5 checkpoint generated with the operator-reviewed old CLI/bridge was
rejected by the new --resume path, with the old file preserved byte-for-byte.
A fresh audit is required; it is not automatically rescanned or deleted on
rejected resume. Existing same-version resume tests pass.

This is identity/version consistency for the normal CLI workflow, not publisher
authenticity or proof against forged checkpoint contents. Direct report-combining
APIs and external report trust remain separate boundaries.

## Tests and file-effect evidence

The live test preserves an existing non-JSON output marker across ordinary,
module and resume modes for malformed strings, empty values, NaN, positive/
negative infinity, fractions, zero and over-cap values. It also verifies
reversed duplicates and valid omitted/1.0/4096/invalid-then-valid requests.
Library tests cover extended numeric types, exact module errors, defaults,
boundaries and root-shape precedence.

Linux file-syscall traces for rejected root and module library requests showed
the intended limit diagnostics and no accesses naming the audited fixture or
its source. A rejected CLI resume request likewise showed no fixture or
checkpoint accesses. Static control flow independently puts validation before
the relevant calls. Host-path-bearing raw traces are not retained.

Two test setup errors were corrected without changing product policy: the
initial valid CLI fixture lived under tmp, which inventory deliberately skips;
it was moved to a unique owned repository-fixture directory. A preliminary
trace probe outside the project failed stdlib import resolution; that run is
discarded and is not no-I/O evidence. The corrected probe used resolved trusted
imports and checked the actual limit outcomes.

## Paired warm fixture audit benchmark

bench/audit_preflight_pair.sapp performs complete root/module scans over the same
one-file repository fixture. Two warmup pairs and 12 alternating AB/BA pairs.
Result comparisons remove only measured duration fields; all other report
fields must match and coverage must be complete. No source/filter/budget was
weakened. Imports and JSON comparisons are outside timing, filesystem and
downstream analysis are inside. This is a warm filesystem-cache fixture test,
not a cold-drive or whole-project benchmark. now_ms is not monotonic.

Short 5-call samples were noisy. Both pilot series using a relocated baseline
with absolute dependency imports and series using an exact original module
within the project are retained. The final 50-call samples keep the original
stdlib import syntax for both versions and increase batch duration to reduce
timer/scheduling sensitivity. Use PB_AUDIT_PAIR_ROUNDS=5 to reproduce pilot batch
size; default is 50. PB_TEXT_AUDIT_REFERENCE must point to a reviewed prior module
inside a project context for matched import routing.

| Series | Case | Calls/sample | Before median ms | After median ms | After p95 ms | Median paired ratio |
| --- | --- | ---: | ---: | ---: | ---: | ---: |
| absolute-import-pilot-1 | roots | 5 | 9.706 | 8.739 | 11.365 | 0.8569 |
| absolute-import-pilot-1 | modules | 5 | 10.145 | 9.268 | 10.393 | 0.8250 |
| absolute-import-pilot-2 | roots | 5 | 9.588 | 9.103 | 11.199 | 0.9842 |
| absolute-import-pilot-2 | modules | 5 | 8.693 | 8.902 | 13.961 | 1.0313 |
| matched-import-pilot-1 | roots | 5 | 8.308 | 8.693 | 10.321 | 1.0555 |
| matched-import-pilot-1 | modules | 5 | 9.910 | 9.642 | 11.232 | 0.9753 |
| matched-import-pilot-2 | roots | 5 | 8.316 | 8.149 | 9.627 | 0.9511 |
| matched-import-pilot-2 | modules | 5 | 9.328 | 9.422 | 12.388 | 0.8687 |
| matched-50-first | roots | 50 | 90.282 | 89.906 | 96.488 | 0.9931 |
| matched-50-first | modules | 50 | 92.409 | 95.375 | 101.859 | 1.0020 |
| matched-50-confirmation | roots | 50 | 92.657 | 92.391 | 97.295 | 1.0246 |
| matched-50-confirmation | modules | 50 | 94.022 | 95.307 | 101.063 | 0.9990 |

For the larger matched samples, root paired ratios range from about -0.7% to
+2.5% time; module ratios are approximately parity (-0.1% to +0.2%). Variation
and pilot differences do not justify a general speedup claim. This security/
data-preservation change is retained for correctness. Timing-only measurements
do not establish lower heap allocation or RSS.
Raw: bench/results/audit-preflight-2026-09-08.jsonl.

## Separate numeric comparison

Unchanged session_compare integer_kernel, one million iterations, status=ok and
checksum 474518 for all nine paths. First run, one warmup and five retained runs;
compiled paths also first compilation plus three repeated compilations.
No test/build/benchmark worker ran concurrently during measurements.

| Path | Median ms | p95 ms | Median peak RSS KiB | Artifact bytes |
| --- | ---: | ---: | ---: | ---: |
| pblang-interpreter | 263.477 | 269.121 | 8740 | N/A |
| pblang-c99 | 4.273 | 4.792 | 1572 | 14472 |
| pblang-codegen-x64 | 2.335 | 2.836 | 264 | 507 |
| c | 5.177 | 5.375 | 1572 | 14472 |
| cpp | 7.716 | 8.090 | 3316 | 14472 |
| go | 9.086 | 9.291 | 1500 | 1228952 |
| rust | 8.080 | 8.353 | 1916 | 350560 |
| python | 197.651 | 199.792 | 10236 | N/A |
| javascript | 132.814 | 134.663 | 51612 | N/A |

Linux x86_64 only. Interpreter remains slower than Python/JavaScript for this
kernel. Numeric rows do not exercise audit, so differences from earlier sessions
are not causal patch results. C99/native rows qualify this fixture, not the whole
dynamic analyzer. No universal platform/device/polyglot claim. RSS is residence,
not allocation; artifact sizes omit runtime/dependencies; N/A is not zero.
Short-run CPU zeros reflect timer granularity. Raw complete metrics:
bench/results/session-2026-09-08-audit-preflight.jsonl.

## Validation and remaining release work

Fourteen targeted suites passed: project audit limits/live diagnostics/core;
source analysis limits/core; security analysis; manifest validation; portable
lock/current lock; project text audit; module audit/resume; library finding
preflight; CLI finding preflight. Additional observed checks cover v5 rejection/
preservation and rejected-request file effects. No full suite or physical-device
campaign ran. Independent defensive review found no blocker.

Final project build passed without a dependency-hash note. The PBLang-native
lock publisher and current portable-lock test passed; this validates the project,
not a rebuilt executable. All six touched code files linted without errors, with
warnings for unused variables/collapsible branches and misleading float/NaN
comparison advice; the intentional non-finite checks remain unchanged.
Added-line and new-file privacy scans found no matched host-path/credential
patterns. git diff --check passed. Owned synthetic fixtures, raw traces and old
reference modules were removed, and no owned test/benchmark/analyzer worker
remained. Graphify update reached the supervised 20-second limit (status 124)
without completing or leaving a process; graph freshness is not claimed.
The 193 pre-existing working-tree status entries remain outside this slice.

Remaining: all other audit CLI numeric parsers and module/snapshot/aggregate
option admission; quality/progress gate input and threshold validation;
suppressed-row and whole-input/record/work budgets; report/source path-collision
and publication review; snapshot/namespace consistency; trusted security_report
ownership; compiler/paradigm/C99 parity and linter float reasoning; self-bootstrap/
executable refresh; VM heap/time/ownership/cleanup; typed FFI/raw bypass/sandbox;
package trust/artifact handoff/revocation/rollback; broad product compatibility
and physical platforms. PBLang 1.0.0 remains NOT READY.
