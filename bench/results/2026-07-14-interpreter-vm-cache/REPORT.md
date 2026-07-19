# Pure-pblang interpreter and VM call optimization

Date: 2026-07-14

## Implemented

- Tree-interpreter identifier reads and assignments cache their immutable
  lexical depth in private prepared-AST metadata. Depths 0 and 1 are direct;
  deeper captures use the cached bounded parent walk.
- The self-hosted bytecode VM uses a fixed two-element argument array for exact
  two-argument closures, avoiding generic fill and reverse-pop loops.
- Both implementations are `.sapp`; no Rust source was modified.
- New long-batch, correctness-gated benchmarks cover lexical lookup, recursive
  execution, captured mutation, and VM call arities.

## Tree-interpreter measurements

Three fresh release processes were pinned to CPU 3. Each full workload uses a
warmup and long in-process batches. Values below are medians of process averages.

| Workload | Baseline | Final | Change |
|---|---:|---:|---:|
| `fib18` | 162.496 ms | 147.710 ms | -9.10% |
| `choose(15,7)` | 330.333 ms | 326.558 ms | -1.14% |
| captured assignment, 200 calls | 6.343 ms | 5.816 ms | -8.31% |

The same final processes also ran baseline and direct lookup functions in the
same process, reducing frequency/startup bias:

| Lexical depth | Baseline | Cached direct | Speedup |
|---|---:|---:|---:|
| 0 | 138.505 ms | 64.338 ms | 2.15x |
| 1 | 235.843 ms | 81.382 ms | 2.90x |
| 2 | 330.142 ms | 93.743 ms | 3.52x |

## VM call measurements

The long-batch fixture measured recursive unary, zero-argument, and repeated
two-argument calls. Unary and zero-argument candidates were rejected and left
unchanged. The accepted two-argument median moved from **84.556 ms** to
**73.914 ms**, **12.59% lower**.

## Full verification

- complete semantics corpus: **651 passed, 0 failed, 0 filtered**;
- self-host focus: **41 passed, 0 failed**;
- native codegen smoke: **254/254 passed**;
- phase suite: every tokenizer/parser/semantic/typecheck/bytecode/interpreter,
  C99, x86-64, i386, ARM64, WASM, graph, IR, and emulator gate passed;
- polyglot control-plane benchmark passed;
- competitor/resource suite passed with raw evidence in `competitors/`.

## Local competitor snapshot

These numbers include process startup and use only installed local toolchains.
The fixed input is embedded in source, so compile-time specialization is part of
this benchmark category.

| Workload | pblang direct x86-64 | Next local result | pblang artifact |
|---|---:|---:|---:|
| recursive `fib(32)` | 2 ms | Fortran 4 ms | 405 B |
| 10M register loop | 5 ms | Clang C 5 ms; Rust 7 ms | 503 B |

For `fib(32)`, measured peak RSS was 264 KiB for the direct pblang artifact,
versus 1,512 KiB for GCC C and 2,084 KiB for Rust. The pblang C99 path and host
interpreter remain substantially slower than direct x86-64 on these workloads;
the raw tables preserve those results rather than hiding them.

This report supports only these workloads on this host. It does not establish
that one language wins every application, platform, or benchmark category.
