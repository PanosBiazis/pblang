# Array windowing measurements — 2026-09-08

Baseline `bd697fb`, followed by guarded slot-copy and final guarded bulk-copy
implementations on the same dirty checkout. No Rust source edits. Algorithms
and validation are PBLang; final bulk copying uses the existing native slice
primitive with internally derived bounds. Raw array_chunk is no longer invoked.

## Identical before/intermediate/after workloads

`bench/array_windowing_stages.sapp` uses 1,024 numbers 0..1023, chunk sizes
1/16/1024, and windows of those sizes with fixed step16 (step is irrelevant for
chunk). Each sample times ten invocations, followed by untimed verification of
the final invocation's every result slot/shape and every unchanged input slot.
One first run, two warmup samples, five retained samples per case.
The first result is not flushed-cache cold evidence. All raw samples, first
values and summary statistics are retained in
`bench/results/array-windowing-2026-09-08.jsonl`, including the slower intermediate
implementation; no samples or cases were removed.

| Stage | Operation | Size | Median ms / 10 calls | p95 ms / 10 calls |
| --- | --- | ---: | ---: | ---: |
| baseline-native-chunk | chunk | 1 | 0.901 | 1.367 |
| baseline-native-chunk | chunk | 16 | 0.146 | 0.162 |
| baseline-native-chunk | chunk | 1024 | 0.076 | 0.156 |
| baseline-native-chunk | windows | 1 | 0.573 | 0.577 |
| baseline-native-chunk | windows | 16 | 2.128 | 2.672 |
| baseline-native-chunk | windows | 1024 | 1.568 | 1.957 |
| guarded-slot-copy | chunk | 1 | 9.609 | 10.333 |
| guarded-slot-copy | chunk | 16 | 2.312 | 2.877 |
| guarded-slot-copy | chunk | 1024 | 1.678 | 1.842 |
| guarded-slot-copy | windows | 1 | 0.629 | 0.930 |
| guarded-slot-copy | windows | 16 | 2.401 | 2.777 |
| guarded-slot-copy | windows | 1024 | 2.216 | 2.645 |
| guarded-bulk-copy | chunk | 1 | 5.024 | 5.375 |
| guarded-bulk-copy | chunk | 16 | 0.339 | 0.362 |
| guarded-bulk-copy | chunk | 1024 | 0.054 | 0.058 |
| guarded-bulk-copy | windows | 1 | 0.323 | 0.347 |
| guarded-bulk-copy | windows | 16 | 0.285 | 0.316 |
| guarded-bulk-copy | windows | 1024 | 0.052 | 0.054 |

Guarded bulk copying improves these windows workloads and avoids most of the
intermediate per-slot cost. Chunking into many small groups remains slower than
the original native builtin (size1 roughly 5.6x, size16 roughly 2.3x median in
these samples). This cost is not hidden: the final code trades raw delegation
for PBLang validation/iteration. No universal speedup is claimed. Tiny durations
are noisy; use the raw spread and repeat on target devices before choosing
performance-sensitive workloads.

The timer is existing now_ms/SystemTime, not monotonic. Clock adjustment, CPU
frequency, temperature, ordinary OS activity and caches are uncontrolled. Stages
run sequentially, not randomized. Five-sample p95 is not production tail latency.
No sorter/window-specific RSS or allocation counters were measured; this is not
proof of reduced aggregate memory. Workload/resource limits are trusted-process
limits, not hostile-code containment.

## Separate numeric competitor batch

Existing session_compare integer_kernel, 1,000,000 modulo-recurrence iterations
with exact checksum474518, nine sequential paths. The batch ran after the
initial guards but before bulk-copy extraction; it does not invoke these APIs
and is not evidence about the final helper's speed.
One first run, one warmup, five retained samples; compiled rows additionally one
first build and three repeat builds. Existing flags unchanged: C/C99/C++ O3/
DNDEBUG/strip (C99 gnu99/-lm, C++17), Rust opt-level3/debuginfo0/strip-symbols,
Go trimpath/ldflags=-s -w. PBLang C99/direct-x64 use existing PBLang drivers.

| Execution path | Median ms | p95 ms | Peak RSS median KiB | Artifact bytes | Repeat-build median ms |
| --- | ---: | ---: | ---: | ---: | ---: |
| pblang-interpreter | 267.855 | 273.016 | 8644.000 | N/A | N/A |
| pblang-c99 | 4.342 | 4.890 | 1572.000 | 14472 | 2468.959 |
| pblang-codegen-x64 | 3.383 | 3.546 | 264.000 | 507 | 1392.211 |
| c | 7.527 | 7.837 | 1392.000 | 14472 | 44.218 |
| cpp | 7.656 | 8.001 | 3364.000 | 14472 | 346.110 |
| go | 8.671 | 9.074 | 1500.000 | 1228952 | 37.973 |
| rust | 7.960 | 8.218 | 1968.000 | 350560 | 78.486 |
| python | 195.217 | 205.550 | 10320.000 | N/A | N/A |
| javascript | 131.536 | 134.114 | 51656.000 | N/A | N/A |

All nine exact checksums pass; raw samples, first values, CPU/RSS/build metrics:
`bench/results/session-2026-09-08-array-windowing.jsonl`.
The retained-host PBLang interpreter remains slower than Python/Node here.
Node may JIT. These rows do not time the PBLang-written VM or demonstrate
interpreter superiority from native results.

Same local Linux7.0.0-31-generic x86-64, i7-1195G7/eight logical CPUs, PBLang0.6.0,
GCC/G++13.3, Go1.22.2, Rust1.94, Python3.12.3, Node18.19.1; no tools installed.
Times include numeric child startup/wrappers; RSS is process peak, not total
allocation. Artifacts exclude runtime/library/packaging dependencies and N/A is
not zero. Repeat-build caches are uncontrolled, especially Go. Raw RSS/CPU *_ms
fields mean KiB/percent; quantization can produce zero CPU observations.

## Reproduction and qualification

From repository root, sequentially and without concurrent tests/Graphify:
```bash
ulimit -c 0
ulimit -v 2097152
ulimit -f 65536
timeout --kill-after=2s 60s target/release/pblang run bench/array_windowing_stages.sapp --allow-all
PB_BENCH_LANGUAGE=python PB_BENCH_WORKLOAD=integer_kernel timeout --kill-after=2s 60s target/release/pblang run bench/session_compare.sapp --allow-all
```
Select each numeric language id for that batch. Numeric children have ten seconds,
outer runs sixty seconds plus two-second kill grace, 2-GiB address space and
64-MiB file limits. No tests or Graphify ran during measurement.

Correctness is separately tested on retained-host and actual PBLang frontend/VM
paths; explicit nil default behavior differs and is documented. The attempted
retained CLI C99 library fixture failed to link import; no C99 library execution
claim follows from successful numeric C99 rows. Physical devices, other OS/
architectures, raw malformed native boundaries, broad compatibility, embedded
polyglot and general 1.0.0 readiness remain open.
