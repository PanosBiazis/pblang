# Taint limit validation measurements — 2026-09-08

Baseline: 9232ea3, same Linux x86-64 checkout and retained executable. Only
PBLang-owned limit validation changed. No Rust changes or executable refresh.

## Existing infrastructure checked before implementation

Existing pipeline_stages/pipeline_bench, pblang_phase_stages/pblang_phase_suite,
execution_pipeline_stages, x64_compile_pipeline_stages, taint_admission_stages
and tokenizer_lexeme_pair were inspected. Reused benchmark.stats and the existing
paired comparison methodology, rather than introducing a new profiling library.
The existing broad phase suite remains available; it was not rerun for this
limited change. Its timings do not automatically cover internals of taint.analyze.

The old taint limits implementation rebuilt one specification array and five maps
per invocation. Fixed direct checks remove those temporary specification
containers, preserving all five bounds, defaults, exact diagnostics and order.
No cache/flow precedence, source admission or tokenizer behavior changed.

## Paired measurement

bench/taint_limits_pair.sapp imports an exact operator-reviewed temporary copy of
security_taint.sapp from 9232ea3 alongside the candidate. Other imports resolve to
the same current checkout. Never supply an untrusted module: importing executes
its initialization. The reference path is not emitted in reports.

Two warmup pairs and twelve retained pairs, alternating AB/BA. Limits: 2000 calls
per sample, separately default and explicit options. Full analysis: 100 calls on
the unchanged taint_admission_stages fixture. Exact full JSON equality checked
outside timing. Independent control requires one finding and complete analysis.
All retained samples, including unfavorable observations/outliers, are retained.

| Operation | Initial paired median candidate/reference | Confirmation |
| --- | ---: | ---: |
| limits-default | 0.4981 | 0.4749 |
| limits-explicit | 0.4869 | 0.4570 |
| analyze-full | 1.0061 | 0.9913 |

This establishes about 50–54% less isolated validation time on this host, not
an analyzer-wide speedup. Full-analysis ratios cross one. The previous
5.97 → 7.22 ms observation remains OPEN; these measurements do not demonstrate
recovery and should not be directly compared across harnesses/allocation history.

## Stage diagnostics and end-to-end boundaries

Confirmation medians, normalized to one invocation:

| Scope | ms |
| --- | ---: |
| Limit validation, default (candidate) | 0.005271 |
| Bounded tokenization, isolated | 0.266560 |
| Parsing already-created tokens, isolated | 0.191738 |
| Complete analyzer, reference | 0.739048 |
| Complete analyzer, candidate | 0.735582 |

The first three are isolated diagnostic workloads, not an instrumented trace of
one invocation. They must NOT be summed or subtracted to invent a flow-walking
time: configuration, source admission, caches and allocation history differ.
Full analyze includes configuration, limits, lexical work, parsing and flow
analysis, but excludes initial imports and the caller's source loading.

Separately, the complete paired benchmark process took 4.32 s in each series,
peak RSS 131184/131236 KiB, user CPU 4.01/4.00 s, system CPU 0.30/0.32 s.
These process measurements include both versions, imports, warmups, checks and
reporting. They are NOT single-analysis latency or before/after memory evidence.

Raw data: bench/results/taint-limits-paired-2026-09-08.jsonl.
The initial default row retains raw arrays (statistics can be recomputed);
remaining rows additionally retain benchmark.stats. Ratio statistics have
dimensionless units despite the existing *_ms statistic field names.

## Clock compatibility found during preflight

Correction after reading the complete profiler module: time_monotonic_ms was a
private wrapper around now_ms, not a builtin or an unavailable profiler dependency.
The benchmark's direct global call was invalid. This failed preflight produced
no retained samples and remains recorded. The measured harness uses now_ms,
consistent with previous pairs; it is SystemTime-based, not monotonic. Nonpositive
pairs reject; positive clock jumps, scheduler, GC and CPU frequency effects are
not eliminated. No profiler/runtime change was made in that slice. See PROFILER.md
for the subsequent lifecycle fixes and honest clock capability disclosure.

## Separate numeric comparison

Unchanged session_compare integer_kernel, one million iterations, all nine
paths status=ok and checksum 474518. One first run, one warmup, five retained
runs; compiled paths one first build and three repeated builds.

| Execution path | Median ms | p95 ms | Median peak RSS KiB | Artifact bytes |
| --- | ---: | ---: | ---: | ---: |
| pblang-interpreter | 278.312 | 311.509 | 8560 | N/A |
| pblang-c99 | 4.322 | 5.179 | 1572 | 14472 |
| pblang-codegen-x64 | 2.676 | 3.124 | 264 | 507 |
| c | 7.649 | 7.852 | 1572 | 14472 |
| cpp | 8.564 | 8.926 | 3364 | 14472 |
| go | 9.313 | 9.808 | 1500 | 1228952 |
| rust | 8.550 | 8.832 | 1920 | 350560 |
| python | 206.330 | 207.357 | 10384 | N/A |
| javascript | 136.194 | 155.266 | 51424 | N/A |

Raw: bench/results/session-2026-09-08-taint-limits.jsonl.
This workload does not execute the changed analyzer. The retained PBLang
interpreter is slower than Python and JavaScript here. These are process-level
comparisons, not proof of language-wide superiority. No other-device/platform
or embedded-polyglot combinations were tested. Artifact bytes exclude runtimes,
libraries and packaging; RSS is not total allocation, and N/A is not zero.

## Validation and reproduction

Fourteen targeted suites passed: limits contract, admission, completeness,
dependency bounds, source bounds, token bounds, dependency analysis, general
security analysis, dynamic eval, tokenizer bounds, parser depth, manifest,
portable lock and security CLI. Independent defensive review found no blocker.

Run bench/taint_limits_pair.sapp with PB_TAINT_REFERENCE pointing to an exact
reviewed baseline copy. Use the same current tokenizer/parser for both modules.
Use a 2-GiB address-space cap, no core dumps, 30-second outer deadline and
2-second kill grace. Numeric rows use 60-second outer deadlines, 10-second
child deadlines and a 64-MiB file cap. Do not overlap benchmarks with other work.
No full suite or native compiler bootstrap was run.

Final project build and PBLang-native portable-lock publication passed. The
initial pre-refresh build noted a stale dependency hash; the final build did not.
Touched-file lint completed with warnings for exported constants/test iteration,
the deliberate nonfinite checks and boolean style. Privacy pattern scan and
git diff --check passed. Graphify update reached its 20-second limit without
completion; graph freshness is not claimed. The local reviewed reference copy
was removed (recoverable from baseline Git). Unrelated dirty work was preserved.
