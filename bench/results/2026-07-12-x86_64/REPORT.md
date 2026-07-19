# pblang competitor benchmark report

Run date: 2026-07-12 UTC. Host: Intel Core i7-1195G7, 4 cores / 8 threads,
15 GiB RAM, Linux x86-64. Full versions are in `environment.txt`.

## Method

- Every row runs the same deterministic algorithm and must print the exact expected result.
- Runtime includes process startup. Fibonacci is the post-optimization best of
  five; the existing loop run is best of three.
- Fibonacci uses naive recursive `fib(32)` to stress calls and recursion.
- Its `N` is embedded in each source fixture, so compiler specialization and
  constant propagation are included in this fixed-input category.
- The loop uses 10,000,000 rolling-hash iterations to stress register allocation.
- Peak RSS is one correctness-gated run measured by `/usr/bin/time`.
- Artifact size is the directly executed file. JVM class/JAR sizes do not include the JVM; script rows do not include their runtime.
- Missing toolchains are reported as unavailable rather than estimated.

## Runtime summary

| Target | fib(32), ms | Loop, ms | Notes |
|---|---:|---:|---|
| Fortran `-O2` | 2 | 11 | Installed native compiler |
| C `gcc -O2` | 5 | 10 | Optimized native baseline |
| Rust `-O` | 6 | 5 | Optimized native baseline |
| C `clang -O2` | 7 | 4 | Optimized native baseline |
| **pblang direct x86-64** | **10** | **3** | Pure-pblang code generator, raw syscalls |
| Go | 11 | 16 | Native toolchain/runtime |
| pblang native C99 | 18 | 80 | Portable C99 route |
| Java | 28 | 41 | Cold JVM process per run |
| Node/V8 | 89 | 97 | Cold process per run |
| Python | 216 | 935 | CPython interpreter |
| pblang interpreter | 797 | 2730 | Current optimization target |

For Fibonacci, direct pblang is 2x slower than GCC `-O2`, about 1.7x slower
than Rust, slightly faster than Go at this timer resolution, about 2.7x faster
than cold Java, 9x faster than Node, 22x faster than Python, and 81x faster
than the pblang interpreter. These ratios describe this run only.

The loop result places pblang in the 3-5 ms leading cluster. Millisecond timing
is too coarse to claim a definitive win over Clang or Rust. The included assembly
program is one hand-written fixture, not proof of the best possible assembly.

## Memory and storage

| Target | Peak RSS | Artifact |
|---|---:|---:|
| **pblang direct x86-64** | **264 KB** | **425 B** |
| C `gcc -O2` | 1,512 KB | 15,992 B |
| Go | 1,372 KB | 1,893,698 B |
| pblang native C99 | 1,760 KB | 25,224 B |
| Rust `-O` | 2,184 KB | 3,954,888 B |
| pblang interpreter | 8,368 KB | runtime required |
| Python | 10,316 KB | runtime required |
| Java | 40,064 KB | 511 B class plus JVM |
| Node | 52,320 KB | runtime required |

The direct backend is about 38x smaller than the C executable and uses about
5.7x less RSS in this run. It is about 4,455x smaller than Go and 9,306x smaller
than Rust, primarily because it has no bundled language runtime or libc.

The x86-64 optimization in this run changed the recursive guard from
`mov+cmp imm32+jcc rel32` to `cmp arg,imm8+jcc rel8`, and omitted a proven
unreachable implicit-return epilogue. The Fibonacci ELF fell from 441 B to
425 B (-3.6%); best-of-five runtime measured 10 ms versus the earlier 11 ms.

## Pipeline

The updated self-hosted `fib(20)` path measured 0.239 ms tokenization, 0.255 ms
parsing, 273.320 ms tree interpretation, and 1.444 ms native code generation.
Native execution fell below the wrapper's 1 ms clock resolution. Caching
closure parameter/body metadata and replacing map return signals with one-slot
arrays reduced the three-run median tree time from 295.725 ms to 273.320 ms
(7.6%).

The pure-pblang bytecode VM's `fib(18)` five-execution average moved from a
three-run median of 147.288 ms to 142.412 ms (3.3%) after removing one temporary
array allocation from every call and array-literal construction. The main
current performance gap remains interpretation, not parsing or x86-64 emission.

## Cross-generation x86 encoding

The pure-pblang i386 backend now uses compact immediate and branch encodings and
maps ordinary and value-return ELF32 segments `R|X` (`p_flags=5`). Its small
encoding fixture reports 3 B `addi`, 6 B `ifnz`, 9 B loop, and a 112 B complete
ELF. `build_value` writes through the process stack, so it no longer needs an
`RWX` code/data segment. The x86-64 and i386 values are reproducible with
`bench/codegen_encoding_matrix.sapp`.

## Limits

- This is one laptop, one OS, one CPU state, and two microbenchmarks.
- Best-of-N favors peak performance and does not describe variance or tail latency.
- CPU frequency scaling, thermal state, and background processes were not controlled.
- JVM/V8 rows are cold-start measurements; long-running warmed-up services need a separate benchmark.
- Compile latency, energy, GC pauses, application throughput, and large working sets are not measured here.
- Language implementations are similar but not guaranteed to produce identical machine-level work after optimization.
- Fixed-input Fibonacci does not isolate runtime-input execution; a compiler may
  transform or specialize the known call more aggressively than another backend.

Raw evidence: `fib.txt`, `loop.txt`, `resources.txt`, `pipeline.txt`,
`selfhost_vm.txt`, and `encoding_matrix.txt`.
