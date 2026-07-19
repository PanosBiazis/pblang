# Hosted C99 module globals and language-category baseline

Date: 2026-07-16

Host: Linux 6.17.0-40-generic x86-64, Intel Core i7-1195G7 (4 cores / 8
threads). pblang 0.5.7; GCC-compatible `cc` 13.3.0.

## Correctness gates

- 42/42 pblang-native self-host suites passed.
- 16/16 hosted/freestanding/kernel C99 suites passed.
- 7/7 source-linker, execution-pipeline, PBIDE, and vision-ledger suites passed.
- 4/4 shared pbAI, pbOS, and pb-mcp execution/health boundary tests passed.
- The linked module-global fixture matched self-host VM and native C output
  (`41`, `42`) and passed the locally available ASan/UBSan build/run path.

## Hosted C99 stages

Command:

```sh
./target/release/pblang bench/selfhost_c99_stages.sapp --allow-all
```

The project fixture has two dependency-ordered modules, module constants,
mutable module state, and a global closure handle. Results are one local run:

| Metric | Result |
|---|---:|
| bounded link, 100 iterations | 239.675 ms total |
| bounded link average | 2.397 ms |
| linked C99 lowering average | 1.228 ms |
| linked input / flattened source | 258 / 235 bytes |
| C compilation | 70.431 ms |
| linked executable | 16,752 bytes |
| verified process runtime average, 5 runs | 0.367 ms |

## Language category matrix

Command:

```sh
./target/release/pblang bench/language_category_matrix.sapp --allow-all
```

Each row uses one warm-up and three measured subprocess samples. Wall time
includes process startup. Nine runtimes were available; PHP, R, Julia, Raku,
Dart, and Swift were recorded as unavailable rather than assigned synthetic
results. All 54 available rows produced their expected checksum.

| Category | pblang median | pblang rank | Fastest available median |
|---|---:|---:|---:|
| startup | 1.713 ms | 3/9 | Lua 1.107 ms |
| recursion (`fib(22)`) | 10.787 ms | 4/9 | Lua 2.826 ms |
| integer loop/modulo | 31.996 ms | 5/9 | Lua 3.482 ms |
| array construction/traversal | 15.596 ms | 4/9 | Lua 2.080 ms |
| repeated string growth | 4.029 ms | 4/9 | Lua 1.985 ms |
| hash-map insertion/lookup | 7.925 ms | 4/9 | Lua 3.509 ms |

The pblang string row was classified noisy (`CV 17.15%`); all other pblang
category rows were stable or excellent. These results identify interpreter
integer-loop and collection dispatch as high-value optimization targets. They do
not establish a universal language ranking: sample count is small, installed
runtimes are incomplete, and this matrix does not replace compiled-native,
memory, artifact-size, compile-time, parallel, polyglot, GUI, web, or application
benchmarks.
