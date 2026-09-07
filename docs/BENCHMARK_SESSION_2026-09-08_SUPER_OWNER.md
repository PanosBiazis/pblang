# Expanded local competitor batch — 2026-09-08, super-owner slice

Baseline `bcf223e` plus this commit, on the existing dirty development checkout.
This is not a release benchmark, a before/after performance claim for the class
library change, or a benchmark of OOP dispatch. The numeric fixture is unchanged.

## Method

Linux 7.0.0-31-generic x86-64, Intel i7-1195G7, eight logical CPUs;
PBLang 0.6.0 retained-host executable, GCC/G++ 13.3.0, Go 1.22.2,
Rust 1.94.0, Python 3.12.3, Node 18.19.1. PBLang C99/direct-x64 compilers
are the existing PBLang drivers executed by that host. No language runtime
source in Rust was modified; the existing Rust competitor fixture was compiled.
No tools were installed and no other device was accessed.

Nine sequential rows using the existing integer_kernel fixture: 1,000,000
iterations of the same bounded modulo recurrence, exact output 474518 on every
run. These values are exactly representable in each selected numeric model;
this does not establish general float/integer backend parity.
Each compiled row has one first build plus three repeat builds. Each row has
one first run, one warmup and five retained executions. Build caches were not
flushed: especially Go's repeat-build result can include cached toolchain work.

The existing compiler flags are unchanged: C/C99/C++ -O3, -DNDEBUG, stripped
binaries (C99 -std=gnu99/-lm, C++ -std=c++17); Rust opt-level=3, debuginfo=0,
strip=symbols; Go -trimpath and -ldflags=-s -w. Direct codegen uses the existing
compile_safe driver, whose name is not proof of a hostile-code sandbox.

Each measured child has a 10-second timeout; the outer row has 60 seconds plus
a two-second kill grace, 2-GiB address space, no core dump and a 64-MiB file limit
(the earlier session used 1 MiB; this batch allows the larger Go executable).
This is a trusted local workload boundary, not aggregate process containment.
No tests/Graphify/other benchmark rows were launched concurrently. Normal OS
background work, CPU frequency and thermal state were not controlled.

## Results

| Execution path | Run median ms | Run p95 ms | Peak RSS median KiB | Artifact bytes | Repeat-build median ms | Run CV |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| pblang-interpreter | 256.613 | 261.875 | 8720 | N/A | N/A | 1.3% |
| pblang-c99 | 4.520 | 4.785 | 1508 | 14472 | 1245.255 | 7.7% |
| pblang-codegen-x64 | 2.530 | 2.886 | 264 | 507 | 691.108 | 13.3% |
| c | 4.523 | 4.965 | 1572 | 14472 | 33.090 | 7.6% |
| cpp | 5.634 | 6.089 | 3364 | 14472 | 241.715 | 10.6% |
| go | 6.181 | 6.275 | 1500 | 1228952 | 27.724 | 8.1% |
| rust | 5.607 | 6.139 | 1968 | 350560 | 60.163 | 6.6% |
| python | 200.242 | 203.313 | 10308 | N/A | N/A | 1.4% |
| javascript | 131.480 | 133.529 | 51560 | N/A | N/A | 1.5% |

All nine rows passed; no failed samples or rows were discarded. Full raw samples,
first-run/build values and summary statistics are retained in
`bench/results/session-2026-09-08-super-owner.jsonl`.

Wall time includes process startup and measurement wrappers, not isolated
in-process arithmetic. Five-sample p95 is interpolated, not a robust production
tail-latency estimate. RSS is GNU time peak RSS, not total allocated/installed
memory. Artifact bytes omit interpreters, libraries, caches and packaging:
freestanding direct ELF and libc-linked binaries have different dependencies.
N/A is not zero cost. Legacy *_ms field names inside raw RSS/CPU summaries
represent KiB/percent respectively. CPU timer quantization can yield zero.

PBLang interpreter remains slower than Python/Node for this fixture. PBLang
C99 is close to C, and direct x64 has lower process duration here, but the two
PBLang compilation paths take longer than C compilation. Differences from the
previous session are not attributed to the unrelated class-library change.
No universal-language, application-category, platform or polyglot ranking.

## Reproduce

From the repository root, execute one row at a time (select any of the nine
language ids in the table):

```bash
ulimit -c 0
ulimit -v 2097152
ulimit -f 65536
PB_BENCH_LANGUAGE=go PB_BENCH_WORKLOAD=integer_kernel \\
  timeout --kill-after=2s 60s target/release/pblang run bench/session_compare.sapp --allow-all
```

The broader program remains in `BENCHMARK_COVERAGE_PROGRAM.md`: other language
implementations/categories, physical devices, PBLang-written interpreter paths,
other native targets, and real two-or-more-language embedded polyglot combinations
remain unmeasured by this batch. A standalone Go/Rust row is not a polyglot test.
