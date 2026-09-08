# Taint source admission measurements — 2026-09-08

Baseline 56d41d3, same dirty local Linux x86-64 checkout before/after the
PBLang-owned source-byte admission fix. No Rust source change or executable
rebuild. Same installed toolchains as the preceding TAINT_ADMISSION session.

## Identical analyzer control

Unchanged bench/taint_admission_stages.sapp: ten analyses of the same small
interprocedural source per sample, one first measurement, two warmups and five
retained samples. Source/sink calls inside the fixture string are never executed.
The last report of each sample is checked outside timing for one finding,
rejection and complete interprocedural coverage.

| Phase | Median ms / 10 | p95 ms / 10 |
| --- | ---: | ---: |
| Before | 5.758 | 6.553 |
| After | 6.174 | 6.847 |

Raw samples: bench/results/taint-source-bounds-2026-09-08.jsonl.
The higher median is a measured cost/noise observation, not a speedup. Five
samples do not isolate the implementation overhead from scheduling/frequency
variation. No samples were discarded. This small source uses the four-byte
scalar fast path; large near-limit conversion, tokenizer amplification, peak
analyzer heap and hostile parser inputs are not measured.

## Separate numeric comparison

Existing session_compare integer_kernel, 1,000,000 iterations. All nine rows
returned status=ok and checksum 474518. One first run, one warmup, five retained
runs; compiled rows additionally one first build and three repeat builds.

| Execution path | Median ms | p95 ms | Median peak RSS KiB | Artifact bytes |
| --- | ---: | ---: | ---: | ---: |
| pblang-interpreter | 262.912 | 266.994 | 8760.000 | N/A |
| pblang-c99 | 4.645 | 5.127 | 1508.000 | 14472 |
| pblang-codegen-x64 | 3.809 | 4.302 | 264.000 | 507 |
| c | 7.265 | 7.735 | 1392.000 | 14472 |
| cpp | 8.046 | 8.137 | 3432.000 | 14472 |
| go | 9.037 | 9.348 | 1500.000 | 1228952 |
| rust | 8.496 | 8.668 | 1928.000 | 350560 |
| python | 198.184 | 199.532 | 10252.000 | N/A |
| javascript | 135.542 | 136.863 | 51508.000 | N/A |

Raw timings, build samples, CPU/RSS and artifact sizes:
bench/results/session-2026-09-08-taint-source-bounds.jsonl.
This kernel does not exercise the changed analyzer at runtime. PBLang's retained
interpreter is still slower than Python and Node here. Native codegen results
do not establish interpreter performance or universal language superiority.
No embedded polyglot or other device/platform comparison ran.

## Reproduce and interpretation

Run from repository root, sequentially with no concurrent tests/build/Graphify:

```bash
ulimit -c 0
ulimit -v 2097152
ulimit -f 65536
timeout --kill-after=2s 25s target/release/pblang run bench/taint_admission_stages.sapp --allow-all
PB_BENCH_LANGUAGE=python PB_BENCH_WORKLOAD=integer_kernel timeout --kill-after=2s 60s target/release/pblang run bench/session_compare.sapp --allow-all
```

Select the nine numeric language ids shown above. Numeric children have ten
seconds; outer runs sixty seconds plus two-second kill grace, 2-GiB address
space and 64-MiB file limits. These are trusted-process guards, not proof of
hostile-code containment. Child startup/wrappers are included. Peak RSS is not
total allocation; artifact sizes omit dependent runtimes, libraries and packaging.
Missing metrics are not zero. RSS/CPU stats retain the helper's historical _ms
field names despite representing KiB/percent. now_ms/SystemTime is not monotonic;
temperature/frequency/cache state is uncontrolled. First is not flushed-cache
cold and five-sample p95 is not a production tail-latency measurement.
