# Local sorting and competitor measurements — 2026-09-08

Baseline `b7f760f` plus this additive PBLang library slice on the existing dirty
checkout. No compiler or Rust source changed. This report separates the new
sorter's measurements from an unchanged numeric competitor workload.

## Sort A/B method

`bench/functional_sort_compare.sapp` was added before implementation. Legacy mode
ran first on the baseline; after implementation, unchanged legacy mode and new
stable mode ran sequentially. Each mode used identical inputs: 256 and 1,024
numbers, ascending, descending, and duplicate-heavy `(i*37)%17` values. The
comparator is `a-b` on these bounded integers. Every result must equal the native
numeric reference sort and the original input must equal a saved copy.
Comparator counting runs separately, outside timing.

Per cell: one first run, two discarded warmup runs, five retained timing samples.
Time covers the sorter invocation (including copying/checks), not import/startup,
fixture creation, equality checks or untimed counting. It uses existing
`now_ms()` (SystemTime, not monotonic); clock corrections can distort timing.
No cache flush, frequency pinning, thermal control or isolation from ordinary OS
activity. Modes are sequential, not randomized. Five-point interpolated p95 is
not production tail latency. All retained samples, first values, comparison
counts and checksums appear in `bench/results/functional-sort-2026-09-08.jsonl`.
No timing samples were removed; warmup durations are not retained.

| Phase | API | n | Shape | Median ms | p95 ms | Comparisons |
| --- | --- | ---: | --- | ---: | ---: | ---: |
| before | legacy | 256 | sorted | 8.002 | 8.068 | 32640 |
| before | legacy | 256 | reverse | 16.539 | 17.665 | 32640 |
| before | legacy | 256 | duplicates | 11.674 | 12.776 | 32640 |
| before | legacy | 1024 | sorted | 130.140 | 148.639 | 523776 |
| before | legacy | 1024 | reverse | 260.180 | 268.163 | 523776 |
| before | legacy | 1024 | duplicates | 199.584 | 253.435 | 523776 |
| after | legacy | 256 | sorted | 7.959 | 8.562 | 32640 |
| after | legacy | 256 | reverse | 16.461 | 16.531 | 32640 |
| after | legacy | 256 | duplicates | 11.600 | 13.390 | 32640 |
| after | legacy | 1024 | sorted | 134.583 | 143.055 | 523776 |
| after | legacy | 1024 | reverse | 269.894 | 273.584 | 523776 |
| after | legacy | 1024 | duplicates | 193.164 | 193.805 | 523776 |
| after | stable | 256 | sorted | 0.742 | 0.746 | 1024 |
| after | stable | 256 | reverse | 0.962 | 0.974 | 1024 |
| after | stable | 256 | duplicates | 1.343 | 1.350 | 1721 |
| after | stable | 1024 | sorted | 4.877 | 4.987 | 5120 |
| after | stable | 1024 | reverse | 4.856 | 4.890 | 5120 |
| after | stable | 1024 | duplicates | 6.832 | 7.393 | 8876 |

The opt-in algorithm is substantially faster in these local retained-host cases.
For 1,024 items it uses 5,120–8,876 comparisons versus the legacy 523,776.
This is not a claim that all applications speed up: existing callers remain on
the unchanged API, comparator costs vary, and tiny inputs/other hosts are not
measured. Two shallow n-sized arrays versus legacy's one is an explicit space
tradeoff; no sorter-specific allocation, peak RSS or physical-device measurement
was performed. PBLang-written VM correctness is tested separately, not timed here.

## Unchanged numeric competitor batch

Verified host/tool versions: Linux 7.0.0-31-generic x86-64, Intel i7-1195G7,
eight logical CPUs; local retained PBLang 0.6.0, GCC/G++13.3.0, Go1.22.2,
Rust1.94.0, Python3.12.3, Node18.19.1. No tools installed.

`bench/session_compare.sapp`: nine sequential integer_kernel rows, 1,000,000
modulo-recurrence iterations, exact checksum 474518. One first run, one warmup,
five retained runs; compiled rows additionally one first build and three repeat
builds. Existing flags unchanged: C/C99/C++ O3/DNDEBUG/strip (C99 gnu99/-lm,
C++17), Rust opt-level=3/debuginfo=0/strip=symbols, Go trimpath/ldflags=-s -w.
PBLang C99 and direct-x64 use their existing PBLang drivers.

| Execution path | Median ms | p95 ms | Peak RSS median KiB | Artifact bytes | Repeat-build median ms |
| --- | ---: | ---: | ---: | ---: | ---: |
| pblang-interpreter | 275.826 | 331.627 | 8704.000 | N/A | N/A |
| pblang-c99 | 6.250 | 6.660 | 1572.000 | 14472 | 2600.584 |
| pblang-codegen-x64 | 3.112 | 3.618 | 264.000 | 507 | 1398.165 |
| c | 6.907 | 7.456 | 1572.000 | 14472 | 42.919 |
| cpp | 7.714 | 7.905 | 3436.000 | 14472 | 338.013 |
| go | 8.364 | 8.695 | 1500.000 | 1228952 | 38.537 |
| rust | 7.543 | 7.965 | 1968.000 | 350560 | 76.016 |
| python | 196.185 | 209.710 | 10312.000 | N/A | N/A |
| javascript | 124.648 | 127.720 | 51524.000 | N/A | N/A |

All nine checksum checks passed. Full first-run/build values, samples, CPU/RSS
and statistics: `bench/results/session-2026-09-08-stable-sort.jsonl`.
These programs do not invoke stable_sort_by. The interpreter row uses the retained
host executable, not the PBLang-written VM; here it remains slower than Python
and Node (which may JIT). Native results do not imply interpreter superiority.

Durations include numeric-process startup/wrappers. RSS is process peak, not total
allocation; artifacts exclude runtime/library/packaging dependencies; N/A is not
zero. Repeat-build caches, including Go's cache, are uncontrolled. Raw RSS/CPU
statistics use legacy *_ms field names but mean KiB/percent; zero CPU values can
reflect quantization. Direct ELF and libc-linked binaries have different dependencies.

## Reproduction and limits

Run from repository root, one benchmark process at a time:
```bash
ulimit -c 0
ulimit -v 2097152
ulimit -f 65536
PB_SORT_MODE=legacy timeout --kill-after=2s 60s target/release/pblang run bench/functional_sort_compare.sapp --allow-all
PB_SORT_MODE=stable timeout --kill-after=2s 60s target/release/pblang run bench/functional_sort_compare.sapp --allow-all
PB_BENCH_LANGUAGE=python PB_BENCH_WORKLOAD=integer_kernel timeout --kill-after=2s 60s target/release/pblang run bench/session_compare.sapp --allow-all
```

Select each numeric language id in the table for the full local competitor batch.
The numeric runner also has a ten-second child limit. No tests or Graphify ran
concurrently with measured workloads. These trusted-process limits do not prove
hostile-code containment. All-language/category, embedded polyglot, physical
devices, other OS/architectures and general 1.0.0 qualification remain open.
