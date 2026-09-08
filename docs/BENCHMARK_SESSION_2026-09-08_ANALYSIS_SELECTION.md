# Analysis selection and catalog isolation — 2026-09-08

## Reproduced failures and repair

Base commit: 5b2037f. Only production change: stdlib/source_analysis.sapp.
No Rust change, executable rebuild, default-build gate change or universal
runtime-security claim.

Two focused regressions failed on the original code:
- Mixed-case ALL,performance selected only performance in the library, despite
  the CLI recognizing it as all. A security result could therefore be ignored.
- Mutating a previous all report's analyses array changed the shared exported
  AVAILABLE array and could remove security from subsequent all analyses.

The CLI discrepancy was also reproduced with an operator-reviewed prior library
and CLI reference: the high-severity danger_accept_invalid_certs fixture returned
status 0, analyses=[performance], security=null with ALL,performance. The patched
CLI returns status 1 with a retained security report for all, ALL,performance and
AlL in both entry-only and dependency modes. The fixture is analyzed as text;
its function call is never executed.

The canonical catalog is now private and never mutated by the implementation.
Public AVAILABLE is a detached compatibility copy, and success/error report
available arrays plus all selections are fresh copies. Case normalization occurs
before recognizing all. This retains canonical all ordering, first-requested
order without duplicates for subsets, default security for empty/non-array
library selections, mixed unknown filtering and unknown-only failure.
No names are silently trimmed or added as new categories.

Tests also mutate report.available, an invalid report's available and public
AVAILABLE, then verify later security analysis, canonical results and independence.
The private array is not runtime-frozen. The claim is ownership/isolation in this
module, not deep immutability, an atomic snapshot or a memory-safety change.

## Compatibility boundaries

Public AVAILABLE remains readable/mutable informational data, not a registration
API. Mutating it intentionally no longer configures analysis admission.
The CLI still permits explicit performance-only selection; this patch does not
make security mandatory for all tools or bypass intentional scope controls.

The caller-supplied security_report remains a trusted embedding input and its
finding maps may be annotated by source_analysis. That trust/ownership boundary,
max_findings and general input-size admission, plus concurrent request mutation
remain separate work. The standard build security gate is separate from this
selectable-analysis module.

The initial live-test fixture used medium-severity eval and incorrectly expected
the CLI's protect threshold to reject it. It was corrected to an existing high
rule, without changing the product threshold or weakening assertions. The unit
tests' default info threshold and CLI's protect/high threshold remain distinct.

## Paired complete-analysis benchmark

bench/source_selection_pair.sapp imports an operator-reviewed 5b2037f source
analysis module alongside the new module, with identical current dependencies.
Two warmup pairs and 12 retained AB/BA pairs; 100 full analyze calls per sample,
same source and options. Every valid report is compared in full as JSON.
Default, explicit performance and lowercase all are equivalent workloads;
previously incorrect uppercase/mutated states are regression tests, not
misrepresented as equivalent performance comparisons.

| Series | Case | Before median ms/100 | After median ms/100 | Median paired ratio |
| --- | --- | ---: | ---: | ---: |
| initial | default | 23.845 | 24.343 | 1.0116 |
| initial | performance | 12.542 | 12.609 | 1.0073 |
| initial | all | 24.514 | 24.865 | 1.0248 |
| confirmation | default | 24.157 | 24.525 | 1.0158 |
| confirmation | performance | 13.120 | 12.990 | 0.9922 |
| confirmation | all | 25.012 | 25.358 | 0.9989 |

Default paired ratios indicate about 1.2–1.6% additional time. Other cases vary
around parity through roughly 2.5% more time; no speedup claim is justified.
Copies add allocations; no allocation/RSS reduction is established by this
timing-only benchmark. Imports/setup and JSON assertions are outside timed calls;
source analysis includes downstream pattern/audit work but not CLI startup,
entry-file I/O or dependency traversal. now_ms is not monotonic.
Raw samples/p95/variation: bench/results/analysis-selection-2026-09-08.jsonl.
Initial measurements overlapped a brief lint check; the confirmation series
ran after that check completed. No heavy test/benchmark workers ran concurrently.

## Separate numeric benchmark

Unchanged session_compare integer_kernel, one million iterations, status=ok and
checksum 474518 for all nine paths. First execution, warmup, five retained runs;
compiled paths first compile and three repeated compilations.

| Path | Median ms | p95 ms | Median peak RSS KiB | Artifact bytes |
| --- | ---: | ---: | ---: | ---: |
| pblang-interpreter | 273.766 | 337.762 | 8584 | N/A |
| pblang-c99 | 4.660 | 4.786 | 1508 | 14472 |
| pblang-codegen-x64 | 3.338 | 3.715 | 264 | 507 |
| c | 7.562 | 7.816 | 1572 | 14472 |
| cpp | 8.239 | 8.604 | 3380 | 14472 |
| go | 8.710 | 8.990 | 1500 | 1228952 |
| rust | 7.540 | 8.022 | 1932 | 350560 |
| python | 203.685 | 209.897 | 10300 | N/A |
| javascript | 141.922 | 143.738 | 51372 | N/A |

Linux x86_64 only; no universal language/device, embedded-polyglot or full-C99
analyzer qualification. The retained PBLang interpreter remains slower than
Python and JavaScript on this kernel. This workload does not exercise category
selection; changes from earlier sessions are not causal patch measurements.
RSS is residence, not cumulative allocation; artifact sizes exclude runtime and
dependencies. N/A is not zero. Short-run CPU zeros reflect measurement granularity.
Raw: bench/results/session-2026-09-08-analysis-selection.jsonl.

## Validation and remaining release work

Twelve targeted suites passed: selection normalization, catalog isolation,
source analysis, live CLI selection, general security, dependency analysis,
policy preflight, dynamic eval, manifest, portable lock, bounded-read CLI and
pbanalyze policy-admission CLI. Live tests run with per-child deadlines and under
a supervised memory limit; no whole-suite or physical-platform campaign.

Initial changed dependency hash was refreshed through normal build/native pblock;
final build and portable lock passed without the stale-hash note. Touched-code
lint completed with unused-binding/style warnings and no errors. Independent
security review found no blocking defect in the bounded production change.

Owned-file privacy pattern scan and git diff --check passed. Graphify reached
its 20-second deadline (124), without completion or freshness evidence.
No owned test, benchmark, build or Graphify process remained. Temporary reviewed
reference modules and fixtures were removed. Pre-existing dirty work and the
refreshed local lock are excluded from this commit.

Remaining: trusted external security reports and row ownership, input/limit
admission, source/config/concurrent namespace snapshots, general performance,
compiler/C99/full-string/paradigm compatibility, self-bootstrap/executable refresh,
runtime hard heap/time/ownership/cleanup, typed FFI/raw bypass/sandbox, publisher
trust/artifact handoff/revocation/rollback, whole-product compatibility and
physical-platform qualification. PBLang 1.0.0 remains NOT READY.
