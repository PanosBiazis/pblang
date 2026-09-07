# Bounded session comparison — 2026-09-08

## Evidence and scope

Baseline `d4a2c11` plus this commit's extrema fix and benchmark driver, on the
existing dirty checkout. This is a local development measurement, not a clean
release build, before/after optimization comparison or universal ranking.
Linux 7.0.0-31-generic x86-64; Intel Core i7-1195G7, eight logical CPUs.
Toolchains: PBLang 0.6.0 retained-host executable; GCC 13.3.0;
Python 3.12.3; Node.js v18.19.1. PBLang C99/native compiler drivers are PBLang
source executed by that host. No Rust source changes or toolchain installs.

Twelve successful rows, each checking the exact output on every execution:
startup prints 1; integer_kernel executes the existing 1,000,000-step modulo
recurrence and prints 474518. The fixtures in `stdlib/repro_benchmark.sapp`
were reused unchanged. Integer intermediates in this fixture are exactly
representable; the C99 integer profile is not evidence of general float parity.
C/C99 use GCC -O3, -DNDEBUG and stripped output; C99 also uses -std=gnu99
and -lm. Direct codegen uses the existing compile_safe driver. Do not infer
hostile-code safety from that driver's name or successful execution.

Each row ran sequentially: first compile plus three repeat compiles (when
applicable), first execution, one unmeasured warmup, then five retained runs.
No tests or other benchmark rows ran concurrently. OS background activity,
frequency scaling, thermal state and cache state were not controlled. First
runs are not a flushed-cache cold-start experiment. Each child has the existing
10-second harness timeout; the outer row has a 60-second deadline with a
2-second kill grace, 2-GiB address-space limit, 1-MiB file limit and no core dump.
These are trusted local experiments, not an aggregate hostile-process sandbox.

## Results

| Workload | Execution path | Run median ms | Run p95 ms | Peak RSS median KiB | Artifact bytes | Compile median ms | Run CV |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: |
| startup | pblang-interpreter | 2.829 | 3.847 | 8712 | N/A | N/A | 20.6% |
| startup | pblang-c99 | 1.462 | 1.491 | 1572 | 14472 | 1235.119 | 3.0% |
| startup | pblang-codegen-x64 | 1.211 | 1.422 | 264 | 372 | 676.152 | 20.0% |
| startup | c | 1.054 | 1.412 | 1316 | 14472 | 29.881 | 15.8% |
| startup | python | 16.226 | 16.865 | 10312 | N/A | N/A | 1.9% |
| startup | javascript | 86.662 | 88.734 | 48148 | N/A | N/A | 2.2% |
| integer_kernel | pblang-interpreter | 271.230 | 273.092 | 8776 | N/A | N/A | 1.4% |
| integer_kernel | pblang-c99 | 7.122 | 7.217 | 1572 | 14472 | 1305.992 | 2.4% |
| integer_kernel | pblang-codegen-x64 | 3.236 | 3.753 | 264 | 507 | 828.408 | 7.1% |
| integer_kernel | c | 7.070 | 7.352 | 1572 | 14472 | 45.549 | 2.8% |
| integer_kernel | python | 192.302 | 200.935 | 10348 | N/A | N/A | 2.3% |
| integer_kernel | javascript | 125.776 | 129.431 | 51388 | N/A | N/A | 1.8% |

Wall time includes process creation, timeout/time wrappers and program startup;
it is not isolated in-process throughput. Startup-scale differences have a
substantial measurement floor. Five-sample p95 is interpolated and must not be
used as a production tail-latency claim. CV is retained, including unstable rows.

RSS is the peak reported by GNU time for each execution, not retained heap,
allocated bytes or the whole installed runtime. Artifact size excludes dynamic
libraries, compiler/interpreter installation, caches and packaging. The direct
ELFs and libc-linked C executables have different runtime dependencies; small
ELF size alone is not an equivalent deployment-footprint win. N/A means no
standalone binary or no compilation phase, never zero storage cost.
CPU percentages below timer granularity can read zero; they are not proof of
zero CPU use. The JSON uses the existing statistics helper's *_ms field names
inside RSS/CPU summaries: units are KiB/percent respectively, not milliseconds.

In this integer microbenchmark the interpreter is slower than Python and Node;
C99 is close to C, and direct x64 has a lower measured process duration.
Conversely, C99/direct compilation costs exceed C compilation here. These are
observations for this fixed workload, not claims about all applications.

## Reproduction and continuation

From the repository root, for each permitted language and workload:

```bash
ulimit -c 0
ulimit -v 2097152
ulimit -f 1024
PB_BENCH_LANGUAGE=pblang-c99 PB_BENCH_WORKLOAD=integer_kernel \\
  timeout --kill-after=2s 60s target/release/pblang run bench/session_compare.sapp --allow-all
```

Permitted languages: pblang-interpreter, pblang-c99, pblang-codegen-x64,
c, python, javascript. Workloads: startup, integer_kernel. Raw successful
row evidence is in `bench/results/session-2026-09-08.jsonl`; no failed rows
were excluded. The earlier single startup preflight is not part of this batch.
The driver prints explicit unavailable/failure status without publishing raw
child stderr, private paths or process output. All temporary benchmark folders
were removed by the existing harness; no benchmark worker remained.

Standing follow-up: use a small relevant comparison at each completed
implementation slice; keep exact outputs, versions, raw samples, resource
limits and uncertainty visible. Extend the finite workload matrix to data
structures, JSON, filesystem/network I/O, concurrency, GUI/web/Android,
database, GPU and mixed-language calls only with equivalent workload contracts.
Report unavailable devices/backends as unmeasured, never extrapolate Linux
measurements to Windows/macOS/iOS/Android, emulators or other architectures.
This turn did not connect, wake, install on or qualify those devices.

The broader requested all-language/category/backend and multi-fragment polyglot
scope is recorded in `BENCHMARK_COVERAGE_PROGRAM.md`; none of its pending cells
are represented as measured by these twelve rows.
