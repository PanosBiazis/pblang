# Benchmark quality and shared IR constant-copy report

Date: 2026-07-15

Host:

- Linux 6.17.0-40-generic x86_64
- Intel Core i7-1195G7, 8 logical CPUs
- pblang 0.5.7 release binary

## Acceptance gates

- `pblang test semantics`: 660 passed, 0 failed, 0 ignored.
- Focused benchmark, language benchmark, performance budget, IR, x86, and
  ARM64 semantic tests: all passed.
- pblang checker: 15 changed/new `.sapp` files, 0 errors, 0 warnings.
- Language category matrix: 90 rows, 54 passed, 36 unavailable, 0 failed.
- Benchmark quality, IR optimizer, phase, polyglot, platform-emulator, and
  self-hosted C99 suites: all passed their correctness markers.

## Shared IR artifact change

The fixture copies a known constant through `movr`, crosses a bounded loop,
then returns the copied value. Baseline and optimized forms both evaluate to
42.

| Output | Before | After | Reduction |
|---|---:|---:|---:|
| Shared IR instructions | 7 | 3 | 57.1% |
| x86-64 ELF | 167 B | 150 B | 10.2% |
| i386 ELF | 121 B | 110 B | 9.1% |
| ARM64 ELF | 168 B | 148 B | 11.9% |
| Portable C99 source | 210 B | 154 B | 26.7% |

The optimizer fixture measured constant-copy optimization at 0.065 ms per
iteration over 1,000 same-process iterations. Optimized backend generation was
0.118 ms for x86-64, 0.100 ms for i386, and 0.123 ms for ARM64 over 500
iterations each. These are fixture costs, not application runtime results.

## Benchmark evidence quality

`benchmark.sapp` now reports interpolated p90/p95/p99, standard deviation,
MAD, IQR, CV, outliers, and stability. On the deterministic noisy fixture the
median was 10 ms, p95 21.9 ms, IQR 0.75 ms, CV 49.0%, two IQR outliers, and the
classification was `unstable`. This demonstrates why a median without
dispersion is insufficient.

Same-process library overhead in that fixture:

| Operation | Iterations | Total | Per operation |
|---|---:|---:|---:|
| Robust summary | 2,000 | 33.75 ms | 0.0169 ms |
| Four-metric regression budget | 5,000 | 140.17 ms | 0.0280 ms |

## Runtime category position

The six-category subprocess matrix had nine available runtimes. pblang median
times were startup 2.23 ms, recursion 9.31 ms, integer loop 30.79 ms, array
13.37 ms, string 3.23 ms, and hash map 6.24 ms. It beat Python, Node, and Ruby
on startup, recursion, array, string, and hash-map fixtures, but Python won the
integer-loop fixture. Lua, Perl, or AWK led every category on this host. The
interpreter's integer and collection paths therefore remain priority work.

## Compiled fib(32) position

The existing fixed-input best-of-three shootout includes process startup and
allows compiler specialization. Selected installed results:

| Target | Best time | Artifact |
|---|---:|---:|
| pblang direct x86-64 | 1 ms | 405 B |
| Fortran `-O2` | 1 ms | 16,240 B |
| pblang self-host C99 integer profile | 4 ms | 15,992 B |
| GCC C `-O2` | 4 ms | 15,992 B |
| pblang self-host C99 f64 profile | 5 ms | 15,992 B |
| C++ `-O2` | 5 ms | 16,000 B |
| Rust `-O` | 6 ms | 3,954,888 B |
| pblang interpreter | 873 ms | n/a |

The 1 ms direct/Fortran result is at timer and process-startup granularity; it
is a tie in this fixture, not evidence that either language is universally
faster. Runtime-input, larger-workload, memory, energy, latency-tail, and
application benchmarks remain separate required categories.

## Phase and platform evidence

Selected pblang-only phase averages were tokenize 0.625 ms, parse 0.512 ms,
semantic analysis 0.107 ms, typecheck 0.034 ms, hosted C99 transpile 0.623 ms,
x86-64 source codegen 1.274 ms, x86-64 IR codegen 0.133 ms, i386 IR codegen
0.107 ms, ARM64 IR codegen 0.142 ms, and WASM source codegen 0.923 ms.

The platform model passed profile resolution, path normalization, bounded
memory, and portable-syscall workloads. The self-hosted C99 suite generated a
14,464-byte size-profile artifact and verified five fib(32) executions. All
polyglot registry, alias, guard, policy, and route stages passed.

## Reproduction

```sh
target/release/pblang run bench/benchmark_quality_stages.sapp --allow-all
target/release/pblang run bench/codegen_ir_optimizer_stages.sapp --allow-all
target/release/pblang run bench/language_category_matrix.sapp --allow-all
target/release/pblang run bench/polyglot_control_plane_stages.sapp --allow-all
target/release/pblang run bench/pblang_phase_stages.sapp --allow-all
target/release/pblang run bench/platform_emulator_matrix_stages.sapp --allow-all
target/release/pblang run bench/selfhost_c99_stages.sapp --allow-exec
./bench/lang_shootout.sh 32
```
