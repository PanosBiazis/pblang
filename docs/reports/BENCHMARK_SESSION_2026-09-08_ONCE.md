# Local competitor check — 2026-09-08, once lifecycle

Baseline `d50f136` plus this commit on the existing dirty checkout. This unchanged
numeric fixture does not exercise once and is not a before/after measurement of
the lifecycle fix. It does not establish general interpreter superiority.

## Method

Same local host/toolchain as the preceding collection-keys batch: Linux
7.0.0-31-generic x86-64, Intel i7-1195G7 (eight logical CPUs), retained PBLang
0.6.0, GCC/G++13.3.0, Go1.22.2, Rust1.94.0, Python3.12.3, Node18.19.1.
No tools were installed and no Rust implementation source changed.

Nine sequential integer_kernel rows: 1,000,000 bounded modulo-recurrence
iterations, exact checksum 474518. Each row has a first run, one warmup and five
retained runs; compiled rows also have a first build and three repeat builds.
Existing flags: C/C99/C++ -O3/-DNDEBUG/stripped (C99 gnu99/-lm, C++17),
Rust opt-level=3/debuginfo=0/strip=symbols, Go trimpath/ldflags=-s -w.
PBLang C99/direct-x64 use their existing PBLang drivers.

No tests/Graphify/other rows were launched during measurements. OS background
activity, CPU frequency, temperature and caches were uncontrolled; repeat builds
are not clean builds. Each child has ten seconds, outer row sixty seconds plus
two seconds kill grace, 2-GiB address-space and 64-MiB file limits, no core dumps.
These are trusted-workload limits, not aggregate hostile-code containment.

## Results

| Execution path | Run median ms | Run p95 ms | Peak RSS median KiB | Artifact bytes | Repeat-build median ms |
| --- | ---: | ---: | ---: | ---: | ---: |
| pblang-interpreter | 265.236 | 284.039 | 8688 | N/A | N/A |
| pblang-c99 | 6.724 | 7.264 | 1572 | 14472 | 2734.415 |
| pblang-codegen-x64 | 3.480 | 3.951 | 264 | 507 | 1574.565 |
| c | 6.941 | 7.381 | 1572 | 14472 | 42.815 |
| cpp | 7.994 | 8.258 | 3364 | 14472 | 336.338 |
| go | 8.802 | 8.938 | 1436 | 1228952 | 36.988 |
| rust | 7.545 | 7.808 | 1968 | 350560 | 77.202 |
| python | 191.858 | 197.826 | 10220 | N/A | N/A |
| javascript | 131.787 | 136.511 | 51548 | N/A | N/A |

All nine rows passed exact checksums. No rows or samples discarded. Full first
values, raw samples, CPU/RSS/build stats: `bench/results/session-2026-09-08-once.jsonl`.

The retained PBLang interpreter still trails Python and Node for this kernel.
Node may use JIT compilation. Native/C99 results do not establish interpreter
wins, and differences from earlier batches are not attributed to once changes.

Durations include startup/wrappers. Five-sample interpolated p95 is not production
tail evidence. RSS is peak process memory, not total allocation or installed size.
Artifacts exclude runtimes/libraries/packaging; N/A is not zero. Direct ELF and
libc-linked binaries have different dependencies. Build caches (notably Go) may
affect repeat measurements. Raw RSS/CPU *_ms fields are KiB/percent; quantization
can produce zero CPU values.

## Reproduce

From repository root, select each language id in the table sequentially:
```bash
ulimit -c 0
ulimit -v 2097152
ulimit -f 65536
PB_BENCH_LANGUAGE=python PB_BENCH_WORKLOAD=integer_kernel timeout --kill-after=2s 60s target/release/pblang run bench/session_compare.sapp --allow-all
```

Actual once correctness is tested separately on retained and PBLang-written VM
paths. All-language/category, physical-device, other native target and embedded
polyglot qualification remains open under `BENCHMARK_COVERAGE_PROGRAM.md`.
