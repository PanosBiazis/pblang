# Local competitor check — 2026-09-08, functional windowing

Baseline `9e2e930` plus this validation slice on the existing dirty checkout.
This unchanged numeric workload does not invoke chunk/windows. These are session
measurements, not a before/after measurement or speedup claim for the fix.

## Method and scope

Local Linux 7.0.0-31-generic x86-64, Intel i7-1195G7 (eight logical CPUs);
retained PBLang 0.6.0, GCC/G++13.3.0, Go1.22.2, Rust1.94.0, Python3.12.3,
Node18.19.1. No tool installation or Rust implementation edits.

Existing `bench/session_compare.sapp` ran nine language/path rows sequentially
with `PB_BENCH_WORKLOAD=integer_kernel`: 1,000,000 bounded modulo-recurrence
iterations, exact checksum 474518. Each row has a first run, one warmup and five
retained runs. Compiled rows have a first build and three repeat builds.
Flags unchanged: C/C99/C++ O3/DNDEBUG/strip (C99 gnu99/-lm, C++17),
Rust opt-level=3/debuginfo=0/strip=symbols, Go trimpath/ldflags=-s -w;
PBLang C99/direct-x64 use their existing PBLang drivers.

No tests or Graphify ran alongside measured workloads. OS background activity,
CPU frequency, temperatures and caches were uncontrolled; first runs are not
flushed-cache cold runs. Ten-second child limit, sixty-second outer row deadline
plus two-second kill grace, 2-GiB address space, 64-MiB file limit, no core dumps.
These limits are for trusted workloads, not aggregate hostile-code containment.

## Results

| Execution path | Median ms | p95 ms | Peak RSS median KiB | Artifact bytes | Repeat-build median ms |
| --- | ---: | ---: | ---: | ---: | ---: |
| pblang-interpreter | 271.692 | 409.860 | 8716.000 | N/A | N/A |
| pblang-c99 | 6.833 | 7.464 | 1572.000 | 14472 | 2521.255 |
| pblang-codegen-x64 | 3.902 | 3.998 | 264.000 | 507 | 1421.971 |
| c | 7.406 | 7.617 | 1572.000 | 14472 | 47.687 |
| cpp | 8.041 | 8.287 | 3488.000 | 14472 | 345.558 |
| go | 8.737 | 9.215 | 1500.000 | 1228952 | 39.748 |
| rust | 8.497 | 8.881 | 1944.000 | 350560 | 76.383 |
| python | 208.591 | 230.736 | 10368.000 | N/A | N/A |
| javascript | 129.720 | 132.535 | 51500.000 | N/A | N/A |

All nine rows passed exact checksums. No failed rows or retained samples were
discarded. Full first values, samples and CPU/RSS/build statistics:
`bench/results/session-2026-09-08-windowing.jsonl`.

The interpreter row is the retained-host executable, not the PBLang-written VM.
It remains slower than Python and Node on this kernel; Node may use JIT. Native
and C99 timings do not demonstrate an interpreter win, nor is timing variation
from earlier sessions attributable to the windowing guard.

Times include process startup/wrappers. Five-sample interpolated p95 is not
production-tail evidence. RSS is peak process memory, not allocations/installed
size. Artifacts exclude runtimes/libraries/packaging; N/A is not zero. Direct ELF
and libc-linked artifacts have different dependencies. Repeat builds can benefit
from caches, notably Go. Raw RSS/CPU *_ms field names mean KiB/percent and
quantization can produce zero CPU observations.

## Reproduce

From repository root, substitute each language id in the results table:
```bash
ulimit -c 0
ulimit -v 2097152
ulimit -f 65536
PB_BENCH_LANGUAGE=python PB_BENCH_WORKLOAD=integer_kernel timeout --kill-after=2s 60s target/release/pblang run bench/session_compare.sapp --allow-all
```

Windowing correctness is separately tested on retained-host and actual
PBLang-written frontend/VM imports. No C99 windowing, raw-builtin malformed-size,
other OS/physical-device, full-language/category or embedded polyglot benchmark
qualification is claimed. PBLang 1.0.0 readiness remains a separate open gate.
