# pblang local competitor snapshot - 2026-07-14

This snapshot covers the toolchains installed on this x86-64 host. Raw output,
environment paths, artifacts, RSS, pipeline stages, and emulator results are in
this directory. Missing toolchains are reported as `not installed`; this is not
a benchmark of every programming language or every application category.

## Recursive calls: `fib(32)`

All targets computed `2178309`; best-of-three wall time includes process
startup. The input is embedded in each program, so constant specialization is
part of this category.

| Target | Time | Artifact |
|---|---:|---:|
| Fortran `gfortran -O2` | 2 ms | 16,240 B |
| C/C++ `gcc/g++ -O2` | 5 ms | about 16 KB |
| Clang C/C++ `-O2` | 6 ms | about 16 KB |
| Rust `rustc -O` | 7 ms | 3,954,888 B |
| D `ldc2 -O3` | 7 ms | 29,864 B |
| **pblang direct x86-64** | **8 ms** | **442 B** |
| OCaml `ocamlopt` | 10 ms | 1,062,232 B |
| Go | 11 ms | 1,893,698 B |
| Pascal/Delphi | 12 ms | 191-196 KB |
| pblang C99 route | 19 ms | 25,224 B |
| Node.js | 96 ms | runtime-managed |
| Python | 218 ms | runtime-managed |
| pblang interpreter | 857 ms | runtime-managed |

The direct pblang backend is competitive and produces the smallest measured
native artifact, but Fortran and optimized C/C++ are faster on this fixture.

## Register loop: 10,000,000 iterations

All targets computed `52032`. The direct pblang backend measured 3 ms with a
503-byte artifact, ahead of the measured Clang/Rust 4 ms and D 5 ms rows. The
iteration count is embedded, so this result must not be generalized to
runtime-input loops or whole applications without additional benchmarks.

## Resources and pipeline

For `fib(32)`, direct pblang used 264 KiB peak RSS and a 442-byte artifact.
Measured comparison rows included C at 1,512 KiB/15,992 B, native C99 pblang at
1,760 KiB/25,224 B, Rust at 2,172 KiB/3,954,888 B, Go at 1,628 KiB/1,893,698 B,
and Java at 39,752 KiB/511-byte class plus its runtime.

The self-hosted pipeline measured sub-millisecond tokenization and parsing,
about 1.8 ms direct x86-64 generation, and about 291 ms tree interpretation for
`fib(20)`. Compilation remains the preferred compute route. The phase suite and
all ten emulator checks completed successfully under `pblang-phases/`.

## Reproduction

```sh
FIB_N=32 LOOP_N=10000000 REPS=3 TMO=45 \
  ./bench/competitor_suite.sh bench/results/2026-07-14-vm-dispatch-competitors
```

Competitor source files are temporary benchmark fixtures only. Product and
ecosystem implementation changes in this round are exclusively pblang.
