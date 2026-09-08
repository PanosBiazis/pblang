# Taint admission/completeness measurements — 2026-09-08

Baseline `ec42869`, followed by numeric/lexical admission and completeness
hardening on the same dirty checkout. No Rust source edits or host rebuild.

## Identical analysis-only control

`bench/taint_admission_stages.sapp` analyzes a fixed small interprocedural source
ten times per sample. It never executes that source's filesystem or environment
calls. Outside timing, the last report must contain exactly one finding, reject
the source, and report complete interprocedural coverage. One first sample,
two warmups and five retained samples for each stage.

| Stage | Median ms / 10 analyses | p95 ms / 10 analyses |
| --- | ---: | ---: |
| Before | 6.550 | 6.981 |
| After | 6.427 | 6.510 |

Raw first values, all samples and summaries:
`bench/results/taint-admission-2026-09-08.jsonl`.
The small difference is within noisy microbenchmark variation; no performance
improvement is established or used to justify the security change. No samples
were discarded. The test does not measure large linked compiler analysis,
finding floods, tokenizer amplification, allocation volume or analyzer RSS.

## Separate numeric comparison

Existing session_compare integer_kernel, 1,000,000 iterations, exact checksum
474518 on all nine paths. One first run, one warmup and five retained runs;
compiled rows additionally one first build and three repeated builds.

| Execution path | Median ms | p95 ms | Median peak RSS KiB | Artifact bytes |
| --- | ---: | ---: | ---: | ---: |
| pblang-interpreter | 265.667 | 272.247 | 8720.000 | N/A |
| pblang-c99 | 6.708 | 7.057 | 1572.000 | 14472 |
| pblang-codegen-x64 | 3.566 | 3.825 | 264.000 | 507 |
| c | 7.290 | 7.723 | 1572.000 | 14472 |
| cpp | 8.055 | 8.095 | 3360.000 | 14472 |
| go | 8.897 | 9.231 | 1500.000 | 1228952 |
| rust | 7.862 | 8.503 | 1968.000 | 350560 |
| python | 200.367 | 209.897 | 10192.000 | N/A |
| javascript | 133.268 | 137.779 | 51604.000 | N/A |

All numeric raw timings, first/repeated builds, CPU/RSS summaries and sizes:
`bench/results/session-2026-09-08-taint-admission.jsonl`.
This kernel does not exercise the changed analyzer at runtime. The retained
PBLang interpreter remains slower than Python/Node here; native timing is not
interpreter evidence, and Node may JIT. No embedded-polyglot comparison ran.

Same local Linux x86-64 host and installed tools as the preceding
`BENCHMARK_SESSION_2026-09-08_SELFHOST_LEXICAL.md` session; no installation or
compiler-flag change. No other platform/device is qualified. Child startup and
wrappers are included; peak RSS is not total allocation. Artifact sizes exclude
dependent runtimes/libraries/packaging. Missing metrics are not zero; the helper's
RSS/CPU *_ms fields represent KiB/percent, not milliseconds.

## Reproduce sequentially from repository root

```bash
ulimit -c 0
ulimit -v 2097152
ulimit -f 65536
timeout --kill-after=2s 30s target/release/pblang run bench/taint_admission_stages.sapp --allow-all
PB_BENCH_LANGUAGE=python PB_BENCH_WORKLOAD=integer_kernel timeout --kill-after=2s 60s target/release/pblang run bench/session_compare.sapp --allow-all
```

Select each numeric language id. Children have ten seconds, outer numeric runs
sixty seconds plus two-second kill grace, 2-GiB address space and 64-MiB file
limits. No tests/build/Graphify ran during retained measurements. The existing
now_ms/SystemTime clock is not monotonic; CPU frequency, temperature, caches and
OS scheduling are uncontrolled. First is not flushed-cache cold, repeated build
caches are uncontrolled, and five-sample p95 is not production tail latency.
These trusted-process limits do not establish hostile-code containment.
