# pblang local competitor snapshot - 2026-07-14

This snapshot covers the toolchains installed on this x86-64 host. Raw outputs,
environment data, artifact sizes, peak RSS, pipeline stages, phase metrics, and
emulator results are stored beside this report. Missing toolchains are marked
`not installed`; this is not a comparison against every language or application
category.

## Recursive calls: `fib(32)`

All available targets computed `2178309`. Best-of-three wall time includes
process startup, and the input is embedded in each fixture.

| Target | Time | Artifact |
|---|---:|---:|
| Fortran `gfortran -O2` | 2 ms | 16,240 B |
| C/C++ `gcc/g++ -O2` | 5 ms | about 16 KB |
| Rust `rustc -O` | 6 ms | 3,954,888 B |
| Clang C/C++ `-O2` | 7 ms | about 16 KB |
| D `ldc2 -O3` | 7 ms | 29,864 B |
| **pblang direct x86-64** | **8 ms** | **442 B** |
| OCaml `ocamlopt` | 9 ms | 1,062,232 B |
| Go | 11 ms | 1,893,706 B |
| Pascal/Delphi/Crystal | 12 ms | 191 KB / 196 KB / 1.22 MB |
| pblang C99 route | 18 ms | 25,224 B |
| Node.js | 90 ms | runtime-managed |
| Python | 214 ms | runtime-managed |
| pblang main interpreter | 859 ms | runtime-managed |

The direct backend remains competitive and produced the smallest native
artifact measured here, but Fortran and optimized C/C++ were faster on this
fixed-input fixture. The main interpreter row is a separate runtime path; the
prepared-AST optimization is measured by the dedicated pure-pblang report.

## Register loop and resources

For 10,000,000 iterations, direct pblang measured 3 ms with a 503-byte artifact;
Clang C/C++, Rust, and D measured 4 ms. This embedded-input microbenchmark is
useful for register-allocation regression detection, not a whole-application
ranking.

On `fib(32)`, direct pblang used 264 KiB peak RSS and a 442-byte artifact.
Comparison rows included C at 1,512 KiB/15,992 B, pblang C99 at
1,868 KiB/25,224 B, Rust at 1,992 KiB/3,954,888 B, Go at
1,628 KiB/1,893,698 B, and Java at 39,544 KiB plus a 511-byte class.

## Pipeline and execution models

The self-hosted pipeline measured about 0.28 ms tokenization, 0.30 ms parsing,
289.84 ms tree interpretation, 1.86 ms direct x86-64 generation, and 1 ms native
execution for `fib(20)`. Sub-millisecond values remain sensitive to process and
frequency noise.

The full phase suite ended with `PHASE_SUITE|ok`. All ten execution-model gates
passed: 6502, AVR, Z80, MIPS, MSP430, PowerPC, RISC-V, SPARC, WASM, and quantum.

## Reproduction

```sh
FIB_N=32 LOOP_N=10000000 REPS=3 TMO=45 \
  ./bench/competitor_suite.sh \
  bench/results/2026-07-14-tree-interpreter-tags-competitors
```

Competitor-language files are temporary benchmark fixtures only. Product and
ecosystem implementation changes in this round are exclusively pblang.
