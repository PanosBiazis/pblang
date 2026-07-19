# Local compiler comparison after linear recurrence lowering

Date: 2026-07-14

## Method

The suite was pinned to CPU 3 and run with:

```sh
FIB_N=32 LOOP_N=10000000 REPS=3 TMO=45 taskset -c 3 \
  ./bench/competitor_suite.sh \
  bench/results/2026-07-14-x64-linear-recurrence-competitors
```

Times are best of three whole-process elapsed measurements and therefore
include startup. Fixed `fib(32)` is embedded in every fixture, so compiler
specialization is intentionally part of that category. Only locally installed
toolchains participate; missing toolchains are marked `not installed` in the
raw files.

## Results

For fixed recursive `fib(32)`, the exact recurrence lowering produced the
correct result in 1 ms and a 405-byte ELF. The next local results were Fortran
at 2 ms, GCC/G++ at 5 ms, Clang C at 6 ms, Rust at 6 ms, and D at 8 ms. Peak
RSS for the pblang direct ELF was 264 KiB, compared with 1,512 KiB for GCC C,
2,084 KiB for Rust, and 1,960 KiB for pblang through C99.

The separate 10,000,000-iteration register loop does not use the recurrence
specialization. pblang direct x86-64 completed in 3 ms with a 503-byte ELF,
followed locally by Rust at 4 ms, Clang C/C++ at 5 ms, D at 6 ms, and GCC/G++
at 11 ms.

The pblang pipeline fixture measured 0.294 ms tokenization, 0.319 ms parsing,
297.852 ms tree interpretation, and 1.425 ms direct x86-64 code generation.
The complete phase suite and all ten architecture/execution-model gates passed.

Raw results are retained in `fib.txt`, `loop.txt`, `resources.txt`,
`pipeline.txt`, and `pblang-phases/`.

## Interpretation limits

This is evidence for two small local workloads, not every language, benchmark,
application, OS, or device. The Fibonacci result measures a narrowly proven
compiler transformation; the loop result is a better control for ordinary
register allocation. Millisecond timing has coarse resolution, especially for
the fastest programs. Broader claims require larger runtime-input workloads,
multiple machines and operating systems, statistical confidence intervals,
and independent reproduction.
