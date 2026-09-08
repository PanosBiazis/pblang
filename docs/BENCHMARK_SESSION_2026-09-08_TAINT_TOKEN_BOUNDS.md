# Bounded token production measurements — 2026-09-08

Baseline 8143c69 on the same dirty local Linux x86-64 checkout before/after the
PBLang-owned bounded-tokenizer change. No Rust edit or executable replacement.
Same installed toolchains as the preceding TAINT_SOURCE_BOUNDS session.

## Identical analyzer control

Unchanged bench/taint_admission_stages.sapp: ten analyses of the same small
interprocedural fixture per sample, one first measurement, two warmups and five
retained samples. Fixture source/sink calls are never executed. The last report
is checked outside each timed sample for one finding, rejection and complete
interprocedural coverage.

| Phase | Median ms / 10 | p95 ms / 10 |
| --- | ---: | ---: |
| Before | 5.974 | 6.636 |
| After | 7.217 | 7.846 |

Raw samples: bench/results/taint-token-bounds-2026-09-08.jsonl.
The median increased, not decreased. This small measurement includes extra token
budget validation/accounting and scheduling variation; it does not isolate their
individual costs. No samples were discarded, no performance improvement is
claimed. A separate future profile should address the added checks without
removing them. No tokenizer flood, long-token concatenation, total allocation or
peak analyzer RSS is measured by this fixture.

## Separate numeric comparison

Existing session_compare integer_kernel, 1,000,000 iterations. All nine rows
returned status=ok and checksum 474518. One first run, one warmup, five retained
runs; compiled paths additionally one first build and three repeated builds.

| Execution path | Median ms | p95 ms | Median peak RSS KiB | Artifact bytes |
| --- | ---: | ---: | ---: | ---: |
| pblang-interpreter | 269.237 | 270.970 | 8672.000 | N/A |
| pblang-c99 | 6.510 | 6.952 | 1568.000 | 14472 |
| pblang-codegen-x64 | 3.381 | 3.810 | 264.000 | 507 |
| c | 7.518 | 7.891 | 1572.000 | 14472 |
| cpp | 8.253 | 8.548 | 3364.000 | 14472 |
| go | 9.239 | 9.401 | 1500.000 | 1228952 |
| rust | 8.071 | 8.462 | 1968.000 | 350560 |
| python | 203.114 | 207.247 | 10328.000 | N/A |
| javascript | 136.576 | 141.145 | 51552.000 | N/A |

Raw samples/builds/CPU/RSS and artifact sizes:
bench/results/session-2026-09-08-taint-token-bounds.jsonl.
The numeric kernel does not exercise the modified analyzer at runtime. PBLang's
retained interpreter remains slower than Python and Node on this kernel. Native
results are not interpreter evidence, and Node may use JIT. No universal speed,
embedded-polyglot or other-device/platform claim is supported.

## Reproduce and limitations

Run sequentially from the repository root, without concurrent tests/build/Graphify:

```bash
ulimit -c 0
ulimit -v 2097152
ulimit -f 65536
timeout --kill-after=2s 25s target/release/pblang run bench/taint_admission_stages.sapp --allow-all
PB_BENCH_LANGUAGE=python PB_BENCH_WORKLOAD=integer_kernel timeout --kill-after=2s 60s target/release/pblang run bench/session_compare.sapp --allow-all
```

Select each of the nine language ids. Numeric children have ten seconds, outer
runs sixty seconds plus two-second kill grace, 2-GiB address space and 64-MiB file
limits. These trusted-process guards do not establish a hostile-code sandbox.
Startup/wrappers are included. Peak RSS is not total allocation; artifact size
excludes dependent runtimes, libraries and packaging. Missing metrics are not
zero. Historical RSS/CPU _ms fields represent KiB/percent. The now_ms/SystemTime
clock is not monotonic; frequency, temperature and cache state are uncontrolled.
First is not flushed-cache cold and five-sample p95 is not production tail latency.
