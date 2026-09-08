# Local competitor check — 2026-09-08, iterate correctness slice

Baseline `123991a` plus this commit, on the existing dirty development checkout.
This does not exercise functional.iterate, so it cannot measure the speed impact
of removing its unused callback. It is a separate, unchanged numerical regression
benchmark, not a release or universal-language ranking.

## Method

Same host/toolchain as the preceding VM-calls batch: Linux 7.0.0-31-generic
x86-64, Intel i7-1195G7, eight logical CPUs; retained-host PBLang 0.6.0,
GCC/G++ 13.3.0, Go 1.22.2, Rust 1.94.0, Python 3.12.3, Node 18.19.1.
No compiler/tool installation or Rust product code changes.

The existing integer_kernel performs 1,000,000 bounded modulo-recurrence
iterations with exact checksum 474518. Nine sequential rows, each with first run,
one warmup and five retained runs. Compiled rows additionally have a first build
and three repeat builds. C/C99/C++ use -O3/-DNDEBUG and stripped artifacts
(C99 gnu99/-lm, C++17); Rust opt-level=3/debuginfo=0/strip=symbols;
Go trimpath/ldflags=-s -w. PBLang direct/C99 use existing PBLang drivers.
Caches were not cleared; repeat builds are not clean-build measurements.

A first complete batch exited successfully, but its retrieved transcript was
truncated. The entire nine-row batch was repeated, not selected rows. The table
and raw file contain the complete second batch; no second-batch samples were
dropped. First-batch numerical evidence is incomplete and not used for ranking.

No tests or Graphify were launched alongside benchmark executions. Normal OS
background work, temperature and CPU frequency were uncontrolled. Each child has
a ten-second limit; outer rows have sixty seconds, a two-second kill grace,
2-GiB address-space and 64-MiB file limits with core dumps disabled. These are
trusted-workload process limits, not aggregate hostile-code isolation.

## Results

| Execution path | Median run ms | p95 run ms | Median peak RSS KiB | Artifact bytes | Median repeat build ms |
| --- | ---: | ---: | ---: | ---: | ---: |
| pblang-interpreter | 261.241 | 279.777 | 8720 | N/A | N/A |
| pblang-c99 | 4.196 | 4.952 | 1572 | 14472 | 1257.059 |
| pblang-codegen-x64 | 3.321 | 3.801 | 264 | 507 | 706.490 |
| c | 7.506 | 7.625 | 1572 | 14472 | 46.808 |
| cpp | 7.866 | 8.203 | 3364 | 14472 | 350.064 |
| go | 9.377 | 9.537 | 1500 | 1228952 | 43.737 |
| rust | 7.876 | 8.022 | 1968 | 350560 | 81.302 |
| python | 195.068 | 200.703 | 10324 | N/A | N/A |
| javascript | 132.194 | 133.198 | 51412 | N/A | N/A |

All rows passed exact output checks. Raw first values, samples and CPU/RSS/build
summaries: `bench/results/session-2026-09-08-iterate.jsonl`.

PBLang's retained-host interpreter still trails Python/Node on this fixture.
Node can JIT compile; this is not a pure-interpreter-only competition. C99/direct
x64 results are distinct execution paths and cannot establish interpreter wins.
Differences from earlier sessions are not attributed to the iterate library fix.

Wall time includes startup/wrappers. RSS is process peak memory, not total
allocation or installed size. Artifacts exclude runtime/library dependencies;
N/A does not mean zero cost. Direct ELF and libc-linked binaries are not identical
deployment packages. Five-sample interpolated p95 is not robust tail evidence.
Raw *_ms RSS/CPU fields represent KiB/percent; timer quantization can yield zero.

## Reproduce

From repository root, choose each table id in turn:

```bash
ulimit -c 0
ulimit -v 2097152
ulimit -f 65536
PB_BENCH_LANGUAGE=python PB_BENCH_WORKLOAD=integer_kernel timeout --kill-after=2s 60s target/release/pblang run bench/session_compare.sapp --allow-all
```

The PBLang-written VM has separate functional correctness tests in this slice,
not a new performance comparison here. Other languages, application categories,
operating systems/devices and actual embedded polyglot combinations remain open
under `BENCHMARK_COVERAGE_PROGRAM.md`.
