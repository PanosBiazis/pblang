# Profiler measurements — 2026-09-08

Baseline 8c1e53e; same Linux x86-64 checkout and retained executable. No Rust
source changes, runtime rebuild, new profiling library or full test suite.
Existing profiler, resources, benchmark and phase tools were inspected first.

## Correctness evidence

Baseline zero benchmark returned avg_ms=NaN (JSON null), min_ms=999999999 and
zero iterations. Disabled trace executed its callback once, then threw on the
missing trace_start mark. Nested traces left a shared mark. Missing count threw.
These baseline controls remain in bench/results/profiler-paired-2026-09-08.jsonl.
The new tests initially failed against the baseline, then passed after fixes.

The earlier claim of a missing profiler clock dependency was incorrect: the
private helper already wrapped now_ms. The direct global benchmark call was
invalid. Prior reports were corrected rather than adding an unnecessary builtin.
Actual clock type, zero/count/lifecycle/trace and memory fixes are in PROFILER.md.

## Paired profiler overhead

Two warmup pairs, twelve retained pairs, alternating reference/candidate order.
The exact baseline profiler is an operator-reviewed temporary Git copy. Importing
it executes code, so this harness is not suitable for untrusted reference paths.
Calls use the same constant-return callback and check count/last-result/schema
or completed trace count outside timing. Module reset is outside each timed span.
A first exploratory run used 2000 benchmark calls / 200 traces; those short
samples remain recorded. Expanded and confirmation runs use 20000 / 2000 to
reduce clock-resolution noise. No samples/outliers are removed.

| Series | Benchmark paired median ratio | Trace paired median ratio |
| --- | ---: | ---: |
| Exploratory small | 0.7686 | 0.7955 |
| Expanded | 0.7203 | 0.7440 |
| Confirmation | 0.7194 | 0.7847 |
| Final after memory correction | 0.7397 | 0.7790 |

Below one means less total elapsed time in this diagnostic. Final component
medians: benchmark 14.215 -> 10.576 ms per 20000 callbacks; trace 5.140 -> 4.071 ms
per 2000 callbacks. About 26%/22% improvement by paired median ratios on the final
run. These measure profiler overhead around trivial callbacks, not application,
compiler or taint-analyzer speedups. The previous analyzer regression stays OPEN.
Removing the timing array reduces retained sample storage; it does not prove
VM-wide constant peak memory or hard heap enforcement.

All raw pairs and whole-harness wall/RSS measurements are retained in
bench/results/profiler-paired-2026-09-08.jsonl. Process RSS includes both modules,
imports, warmups and output, so it is not before/after allocation evidence.
now_ms is not monotonic; scheduling, clocks, thermal state and GC remain sources
of variation. Ratio stats are dimensionless despite existing *_ms field names.

## Unchanged numeric comparison

Existing session_compare integer_kernel, one million iterations, all paths
status=ok and checksum 474518. One first run, one warmup, five retained runs;
compiled paths one first build plus three repeated builds.

| Path | Median ms | p95 ms | Median peak RSS KiB | Artifact bytes |
| --- | ---: | ---: | ---: | ---: |
| pblang-interpreter | 271.799 | 276.296 | 8740 | N/A |
| pblang-c99 | 4.576 | 5.023 | 1572 | 14472 |
| pblang-codegen-x64 | 3.823 | 3.959 | 264 | 507 |
| c | 8.552 | 9.062 | 1572 | 14472 |
| cpp | 8.090 | 8.320 | 3348 | 14472 |
| go | 9.386 | 9.609 | 1628 | 1228952 |
| rust | 8.262 | 8.474 | 1948 | 350560 |
| python | 201.879 | 205.833 | 10308 | N/A |
| javascript | 136.450 | 140.175 | 51472 | N/A |

Raw: bench/results/session-2026-09-08-profiler.jsonl.
The numeric workload does not execute this profiler. PBLang's retained
interpreter remains slower than Python and JavaScript on it. No universal
language/category, polyglot combination or other-device/platform claim. Artifact
size excludes dependencies; RSS is not total allocation and unavailable is not zero.

## Validation and reproduction

Eleven focused suites pass: profiler lifecycle, admission (including inclusive
one-million benchmark and warmup), memory evidence, benchmark_stdlib,
benchmark_enrich, resources, phase_benchmark_contract, language_benchmark,
repro_benchmark, manifest_validate, portable_lock_current. The inclusive boundary
test was also rerun after extension. Independent defensive review found no blocker.

Run bench/profiler_pair.sapp with PB_PROFILER_REFERENCE selecting an exact reviewed
baseline copy; keep current host/workloads identical. Supervisors used 2 GiB
address-space limits, no core dumps, 30-second paired deadlines and 2-second kill
grace; numeric rows use 60 seconds, 10-second child deadlines and 64-MiB file caps.
The first local probe failed because stdlib: lookup outside a project needs its
manifest; using the exact operator reference resolved that diagnostic. An initial
test draft used unsupported error-binding syntax and the wrong callable tag;
both were corrected using existing repository contracts and live probes.

Final lint has warnings for nonfinite comparisons, boolean style and bindings;
the preexisting mem_diff bool-arithmetic lint errors are fixed. No Rust source
change or native executable refresh was performed. See the continuation report
for the remaining release blockers.

Final project build and PBLang-native portable-lock publication passed without
the initial stale-content-hash note. Privacy pattern and diff checks passed.
Graphify update reached its 20-second deadline without completing; no graph
freshness claim. Temporary reference/probe files were removed; baseline source
remains recoverable from Git. Unrelated working-tree changes were preserved.
