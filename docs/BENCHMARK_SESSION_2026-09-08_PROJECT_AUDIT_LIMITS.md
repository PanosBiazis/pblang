# Direct project-audit finding limits — 2026-09-08

## Scope and evidence

Base commit: 04b1d51. Only production file changed: stdlib/project_audit.sapp.
No Rust source, executable replacement, broad disk scan or default-build gate
change. Existing caller modules and their contracts were reviewed before editing;
an independent defensive review found no blocking issue in the bounded fix.

The new fractional regression failed on the original implementation: a limit
of 1.5 did not reject. The original ordered comparisons also lacked ordinary
number, finiteness and upper-bound admission; infinity removed the numeric
retention ceiling and NaN could throw a generic comparison error on this VM.
No NaN clean-report bypass on this VM is claimed.

A private helper now validates both scan_text_report and scan_records at entry,
before source conversion/splitting or record traversal. scan_text delegates,
and audit_project/release_readiness propagate the rejection. Invalid inputs
throw the fixed message:
"project audit max_findings must be a finite integer in 1..4096".

Returning an error-shaped map with empty findings would not be safe here:
the established successful reports have no shared ok field, and downstream
quality_gate/IDE consumers may interpret empty/missing counts as a clean audit.
The controlled exception preserves successful schemas without introducing
a new success-shaped failure. Callers must not catch it and substitute a clean
report. Uncaught VM diagnostics may include stack/source locations; the fixed
exception payload itself does not include input values, source content or paths.

## Compatibility

Only ordinary finite integral numbers in 1..4096 are admitted. Booleans, strings,
arrays/maps/functions, bigint/rational/decimal, NaN/infinity, fractions, zero,
negative values and values over 4096 reject. Explicit nil continues to select
the existing default: 200 for text scanning, 500 for records. Omission behaves
identically. This differs intentionally from source_analysis's explicit-nil
rejection; these are separate pre-existing public conventions.

The upper bound agrees with project_text_audit's existing per-file maximum of
4096; source_analysis's tighter maximum remains 1024. Cross-module summary
indexes can still exceed 4096 because they combine separate bounded scans.
Former silent fallback for values below one and undocumented larger direct-call
budgets are intentionally tightened. Valid counts, ordering, suppressions,
truncation fields and all successful report schemas remain unchanged.

Tests lock actual default counts using 501 harmless synthetic marker lines,
valid boundaries, malformed/extended numeric types, empty-record rejection,
input nonmutation, and propagation through quality_gate/readiness. A Linux
subprocess test checks status 1, the fixed diagnostic and absence of synthetic
private input markers for malformed strings, NaN, infinity, fractions and
over-cap values. No source fixture payload is executed as program code.

## Paired complete-audit benchmark

bench/project_audit_limits_pair.sapp imports an operator-reviewed prior module
with the same current secscan dependency alongside the modified module. Identical
input and parameters, 100 calls per sample, two warmup pairs then 12 alternating
AB/BA pairs. Every final report is compared in full as JSON. Cases include clean
text, marker text, cap-one text and two-record aggregation. Two complete runs
were retained; one preliminary run preceded persistent output capture setup
and is not used for conclusions.

| Series | Case | Before median ms/100 | After median ms/100 | After p95 ms/100 | Median paired ratio |
| --- | --- | ---: | ---: | ---: | ---: |
| first | text-clean | 10.993 | 11.137 | 11.494 | 0.9946 |
| first | text-markers | 35.400 | 34.976 | 38.181 | 0.9941 |
| first | text-cap-one | 13.285 | 12.959 | 15.999 | 0.9761 |
| first | records | 51.549 | 50.908 | 58.496 | 1.0087 |
| confirmation | text-clean | 10.802 | 10.605 | 11.561 | 1.0044 |
| confirmation | text-markers | 35.626 | 35.047 | 35.930 | 0.9934 |
| confirmation | text-cap-one | 12.339 | 12.575 | 15.450 | 1.0215 |
| confirmation | records | 50.369 | 50.543 | 56.799 | 1.0060 |

Aggregation is about 0.6–0.9% slower in paired medians. Text cases range roughly
from -2.4% to +2.1% between runs; this supports near-parity, not a robust general
speedup. Inputs/reports and workload counts were not weakened for timing.
Imports and JSON comparisons are outside timed regions; scanning/sorting is
inside. now_ms is not monotonic; these short timing-only measurements establish
neither total allocation nor RSS reductions. Raw samples/distributions:
bench/results/project-audit-limits-2026-09-08.jsonl.

## Separate numeric benchmark

Unchanged bench/session_compare.sapp integer_kernel, one million iterations.
All nine paths returned status=ok and checksum 474518. First run, one warmup,
five retained runs; compiled paths first compile plus three repeated compiles.
No concurrent test/build/benchmark worker during measurements.

| Path | Median ms | p95 ms | Median peak RSS KiB | Artifact bytes |
| --- | ---: | ---: | ---: | ---: |
| pblang-interpreter | 265.423 | 332.365 | 8660 | N/A |
| pblang-c99 | 4.723 | 5.053 | 1572 | 14472 |
| pblang-codegen-x64 | 2.402 | 2.564 | 264 | 507 |
| c | 4.916 | 5.331 | 1572 | 14472 |
| cpp | 8.046 | 8.278 | 3380 | 14472 |
| go | 9.111 | 9.312 | 1500 | 1228952 |
| rust | 8.118 | 8.157 | 1948 | 350560 |
| python | 197.147 | 199.309 | 10216 | N/A |
| javascript | 133.752 | 134.599 | 51628 | N/A |

Linux x86_64 only. The retained interpreter remains slower than Python and
JavaScript for this workload. This numeric kernel does not exercise project
audit; changes from other sessions are not causal patch measurements. C99/native
rows qualify this fixture, not the complete dynamic analyzer or all backends.
RSS is residence, not total allocation; artifact size excludes runtime and
dependencies; N/A is not zero. CPU zeros in short runs reflect timer granularity.
Raw compilation, first/warm timings, CPU, RSS and artifacts:
bench/results/session-2026-09-08-project-audit-limits.jsonl.

## Validation and remaining release work

Fourteen targeted suites passed: project audit limits, live limit diagnostics,
project audit, source analysis limits, source analysis, category selection,
catalog isolation, security analysis, manifest validation, portable lock,
current portable lock, project text audit, module audit and pbanalyze limits.
The 501-line default-cap regression also passed after its addition. No full
suite, live device campaign or broad disk scan was run.

Final build/native pblock and current-lock checks passed without the initial
changed-dependency hash note. This validates the project/lock, not a rebuilt
self-hosted executable. Lint completed without errors, with existing-style
unused-binding/collapsible-if warnings and false eq_op advice for the intentional
NaN/infinity guard. Those float checks must not be removed by applying that
advice. Independent security review found no blocking issue; its suggested
fixed-message regression was added and passed.

Owned added-line privacy checks and git diff --check passed. Temporary benchmark
references and test/benchmark fixtures were cleaned up. Graphify update reached
its 20-second deadline and terminated without completing; graph freshness is
not claimed. No owned test/benchmark/analyzer worker remains.

The new test initially miscounted the pre-existing text report fields as nine
instead of eight; the assertion was corrected after inspecting the unchanged
schema. This was a test-authoring error, not a product schema change.

Remaining: suppressed-row arrays and whole-source/record/work budgets;
project_text_audit pre-I/O limit admission and complete module-option validation;
audit CLI numeric-parser fallback; deadlines and snapshot consistency;
caller-supplied security_report trust/row ownership; quality/progress gate
validation of externally supplied reports/options and truncation semantics;
compiler/paradigm/C99 parity and linter float reasoning; self-bootstrap/
executable refresh; VM heap/time/ownership/cleanup, typed FFI/raw bypass/sandbox;
package trust/artifact handoff/revocation/rollback; broad product regression
and physical-platform evidence. PBLang 1.0.0 remains NOT READY.
