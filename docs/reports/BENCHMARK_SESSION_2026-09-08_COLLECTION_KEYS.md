# Local competitor check — 2026-09-08, scalar collection keys

Baseline `addbb8a` plus this commit on the existing dirty checkout. The existing
numeric fixture does not exercise collection helpers. No before/after collection
performance or universal interpreter ranking is inferred from this batch.

Same local host/toolchain as the preceding iterate batch: Linux 7.0.0-31-generic
x86-64, Intel i7-1195G7, eight logical CPUs; retained PBLang 0.6.0,
GCC/G++13.3.0, Go1.22.2, Rust1.94.0, Python3.12.3, Node18.19.1.
No runtime/tool installation or Rust implementation edit.

## Method

Nine sequential unchanged integer_kernel rows: 1,000,000 bounded modulo
iterations with exact checksum 474518. Each row has a first run, one warmup and
five retained samples. Compiled rows also have a first build and three repeat
builds. Existing C/C99/C++ flags are -O3/-DNDEBUG/stripped (gnu99/-lm for C99,
C++17); Rust opt-level=3/debuginfo=0/strip=symbols; Go trimpath/ldflags=-s -w.
PBLang C99/direct-x64 use their existing PBLang drivers.

No test/Graphify/other benchmark row was launched concurrently. OS background
activity, caches, CPU frequency and temperature were uncontrolled. Each child
has ten seconds; each outer row has sixty seconds plus two seconds kill grace,
2-GiB address-space and 64-MiB file limits, no core dumps. These trusted-workload
limits are not aggregate containment. Caches were not flushed.

## Results

| Execution path | Run median ms | Run p95 ms | Peak RSS median KiB | Artifact bytes | Repeat-build median ms |
| --- | ---: | ---: | ---: | ---: | ---: |
| pblang-interpreter | 260.815 | 268.508 | 8676 | N/A | N/A |
| pblang-c99 | 4.694 | 5.082 | 1568 | 14472 | 2539.502 |
| pblang-codegen-x64 | 2.053 | 2.794 | 264 | 507 | 1359.977 |
| c | 4.825 | 5.155 | 1568 | 14472 | 32.469 |
| cpp | 8.206 | 8.971 | 3388 | 14472 | 266.291 |
| go | 9.185 | 9.599 | 1500 | 1228952 | 40.295 |
| rust | 8.371 | 8.587 | 1964 | 350560 | 79.569 |
| python | 203.516 | 213.748 | 10276 | N/A | N/A |
| javascript | 166.432 | 220.600 | 51464 | N/A | N/A |

All nine rows passed. All first values, retained samples, CPU/RSS/build statistics:
`bench/results/session-2026-09-08-collection-keys.jsonl`. No row or sample discarded.

Retained PBLang interpreter is still slower than Python/Node on this kernel.
Node may JIT compile. C99/native results are not interpreter results, and this
numeric workload does not test typed grouping, embedded NUL or structured keys.
Changes from earlier batches are not attributed to this unrelated library fix.

Durations include startup/wrappers. Five-sample interpolated p95 is not production
tail evidence. Peak RSS is not total allocation/installed footprint. Artifacts
exclude libraries/interpreters/packaging; N/A is not zero. Direct ELF and libc
artifacts have different dependencies. Repeat builds can use cached toolchain
work, especially Go. Raw RSS/CPU *_ms fields are KiB/percent; quantization may
produce zero CPU readings.

## Reproduce

Select each id in the table sequentially from repository root:
```bash
ulimit -c 0
ulimit -v 2097152
ulimit -f 65536
PB_BENCH_LANGUAGE=python PB_BENCH_WORKLOAD=integer_kernel timeout --kill-after=2s 60s target/release/pblang run bench/session_compare.sapp --allow-all
```

Actual collection correctness is tested separately on the retained interpreter
and PBLang-written frontend/VM. C99 collection parity, all languages/categories,
physical devices and real embedded polyglot combinations remain open under
`BENCHMARK_COVERAGE_PROGRAM.md`. No general performance improvement is claimed.
