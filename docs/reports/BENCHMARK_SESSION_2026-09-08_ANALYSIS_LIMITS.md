# Selectable-analysis finding limits — 2026-09-08

## Reproduced defect and correction

Base commit: 2f299cc. Production changes are PBLang-only:
stdlib/source_analysis.sapp and tools/pbanalyze.sapp. No Rust source change,
binary replacement or default-build security policy change.

The regression test failed on the original fractional limit. A separate probe
using an operator-reviewed original module reproduced max_findings=1.5 retaining
two findings and NaN causing an unstructured exception. This is not evidence of
a NaN finding-suppression bypass on the retained VM.

source_analysis.limits(options=nil) now returns a fresh exact outcome:
- omitted/nil options: {ok:true,max_findings:256};
- map with omitted key: the same valid default;
- explicit finite integral number in 1..1024: {ok:true,max_findings:value};
- explicit nil, non-number, NaN, infinity, fraction or out-of-range value:
  {ok:false,error:"max_findings must be a finite integer in 1..1024"};
- non-map options: {ok:false,error:"source analysis limits require map options"}.

The known constant default takes a fast path; every explicit value is validated.
Types and non-finite values are checked before ordered numeric comparisons.
The direct analyzer preserves source/options-type then category-selection error
precedence and validates limits before invoking either downstream analyzer.
Invalid outcomes retain ok=false, passed=false, format and constant error only.

The CLI rejects malformed final --max-findings values with status 2 before
config/source I/O. Omitted=256, last-duplicate-wins and integral numeric syntax
such as 1.0 remain supported. This intentionally tightens the old silent fallback
for malformed/out-of-range limits and fractional handling. Invalid limit now
wins over an invalid config; valid policy/read errors retain their ordering.
Normal analysis revalidates; the outcome grants no reusable capability.

A Linux strace check with NaN and absent source/config returned the constant
status-2 error and recorded no file syscall targeting either supplied path
(excluding their occurrence in initial exec arguments). Interpreter startup and
module imports still perform I/O. Raw host traces are not retained in the repo.

## Paired analysis timing

Unchanged bench/source_selection_pair.sapp, operator-reviewed 2f299cc module
with its two imports redirected to the same current dependencies. Identical
source/options, two warmup pairs, 12 retained alternating AB/BA pairs, 100 complete
calls per sample. Full JSON reports must match. No benchmark workload was
weakened or replaced. All three cases omit max_findings; this measures the
default path, not overhead for explicitly supplied limits.

| Series | Case | Before median ms/100 | After median ms/100 | After p95 ms/100 | Median paired ratio |
| --- | --- | ---: | ---: | ---: | ---: |
| initial-full-validation | default | 38.323 | 39.501 | 42.216 | 1.0177 |
| initial-full-validation | performance | 19.479 | 20.210 | 20.676 | 1.0264 |
| initial-full-validation | all | 37.587 | 38.086 | 42.233 | 1.0157 |
| default-fast-path | default | 24.544 | 24.723 | 26.545 | 1.0045 |
| default-fast-path | performance | 13.039 | 12.980 | 13.548 | 0.9975 |
| default-fast-path | all | 25.269 | 25.028 | 29.104 | 0.9918 |
| confirmation | default | 23.879 | 24.579 | 26.611 | 1.0041 |
| confirmation | performance | 12.846 | 13.121 | 13.510 | 1.0156 |
| confirmation | all | 26.668 | 26.303 | 30.166 | 0.9971 |

Initial code validated the constant default and showed roughly 1.6–2.6% extra
time. The final default fast path shows about +0.4% for the default case in both
series; performance-only ranges from near parity to +1.6%, and all is near parity.
These are small timing differences, not established general speedups. Absolute
times differ substantially between series; use paired within-series ratios,
not cross-series wall-time ratios, to assess the patch. Imports/setup and JSON
checks are outside timing; downstream analysis is inside. No CLI startup/read
or dependency traversal is timed. now_ms is not monotonic. This timing-only
harness establishes no allocation or peak-RSS reduction.

Raw samples, distributions and before/after percentiles:
bench/results/analysis-limits-2026-09-08.jsonl.

## Separate numeric comparison

Unchanged session_compare integer_kernel, one million iterations, checksum
474518 and status=ok for all nine paths. First run, one warmup, five retained
runs; compiled paths have a first compile and three repeated compile samples.
No other test/build/benchmark worker ran during the comparison.

| Path | Median ms | p95 ms | Median peak RSS KiB | Artifact bytes |
| --- | ---: | ---: | ---: | ---: |
| pblang-interpreter | 260.603 | 263.038 | 8564 | N/A |
| pblang-c99 | 4.444 | 4.770 | 1572 | 14472 |
| pblang-codegen-x64 | 3.068 | 3.488 | 264 | 507 |
| c | 7.154 | 7.353 | 1392 | 14472 |
| cpp | 8.163 | 8.459 | 3380 | 14472 |
| go | 8.873 | 9.135 | 1500 | 1228952 |
| rust | 7.873 | 8.131 | 1936 | 350560 |
| python | 195.258 | 203.535 | 10244 | N/A |
| javascript | 130.090 | 131.777 | 51596 | N/A |

Linux x86_64 only. Interpreter remains slower than Python and JavaScript on this
kernel. This workload does not exercise source analysis; changes from earlier
sessions are not causal patch measurements. C99/native timings qualify this
numeric fixture, not the complete analyzer. No full polyglot, physical-device
or universal-platform/language result. RSS is resident memory, not cumulative
allocation. Artifact bytes exclude runtime/dependencies; N/A is not zero.
Short-run CPU zeros reflect measurement granularity.

Raw compilation, first/warm timings, CPU, RSS and artifact evidence:
bench/results/session-2026-09-08-analysis-limits.jsonl.

## Validation and remaining work

New library/CLI tests cover exact errors, default/result independence, boundaries,
NaN/infinity/extended numeric types, fractions, privacy, preflight precedence,
retained finding counts, duplicates and blocking security at a cap of one.
Fixtures are analyzed as text, never executed.

Fifteen targeted suites passed: source-analysis limits, source analysis,
selection normalization, catalog isolation, security analysis, dependency
analysis, policy preflight, dynamic eval, manifest validation, portable lock,
current portable lock, CLI limits, CLI selection, bounded CLI reads and pbanalyze
policy admission. No full suite or physical-platform campaign was run.

Final build and native pblock/current-lock validation passed without the initial
changed-dependency hash note. This validates the project/lock, not a rebuilt
self-hosted executable. Touched-code lint returned no errors; warnings include
export/used-variable accounting, intentional strict boolean comparisons and
incorrect eq_op advice for NaN/infinity guards. The latter must not be applied:
NaN self-inequality and infinity subtraction are intentional, covered checks.
Linter floating-point reasoning remains a separate improvement.

Independent defensive review found no blocking defect in the guarded change.
Owned-file privacy scan and git diff --check passed. Graphify update reached its
20-second deadline and terminated; graph freshness is not claimed. Test,
benchmark and reference/trace fixtures were removed; no owned worker remains.

One initial bounded-read regression run failed during the development window
around a source/lock revision; it did not retain child diagnostics sufficient
to assign the cause. The unchanged test passed after build/lock stabilization.
This failed attempt is not counted as successful evidence. An initial test-list
typo named a nonexistent manifest_test; manifest_validate_test was then run.

Still open: malformed-limit admission for direct project_audit callers; trusted
external security_report/row ownership; whole-input/whole-work budgets;
category-filter/truncation semantics; source/config/namespace snapshot consistency;
compiler/paradigm and C99 full-string parity; self-bootstrap/executable refresh;
VM-wide heap/time/ownership/cleanup; typed FFI/raw-bypass and hostile-code sandbox;
package trust/artifact handoff/revocation/rollback; broad product regression and
physical-platform verification. PBLang 1.0.0 remains NOT READY.
