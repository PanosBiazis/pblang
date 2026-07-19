# Prepared block body and scope cache

Date: 2026-07-14

## Accepted change

The pure-pblang tree interpreter now computes block scope requirements during
its existing AST preparation pass. Private `@` metadata stores immutable
statement arrays and whether each block declares a `let`, `const`, or `fn`.
Prepared `if`, `elif`, `else`, and `while` execution therefore avoids repeated
block-shape checks, statement-array extraction, and declaration scans.

Public parser fields such as `kind`, `stmts`, and expression structure remain
unchanged. A declaring block still receives a child environment, while a
non-declaring block reuses its parent. Focused tests cover lexical shadowing,
loop-local bindings, raw-AST preparation, reusable ASTs, and the private cache
contract.

## A-B-A-B method

Each phase used seven fresh release processes pinned to CPU 3. Every process
performed an untimed correctness/warmup run and reported the median of its own
samples. The source was switched baseline -> optimized -> baseline -> optimized
to expose thermal or scheduler drift. Both optimized rounds beat their adjacent
baseline round.

| Workload | Baseline 1 | Optimized 1 | Change | Baseline 2 | Optimized 2 | Change |
|---|---:|---:|---:|---:|---:|---:|
| `fib18` execution | 96.259 ms | 88.447 ms | -8.12% | 95.910 ms | 90.513 ms | -5.63% |
| `choose(15,7)` execution | 205.757 ms | 193.116 ms | -6.14% | 206.678 ms | 195.519 ms | -5.40% |
| `fib18` end to end | 97.303 ms | 90.233 ms | -7.27% | 97.388 ms | 91.575 ms | -5.97% |
| `fib18` preparation | 0.716 ms | 0.720 ms | +0.61% | 0.724 ms | 0.729 ms | +0.81% |

The small one-time preparation cost is recovered by execution, including the
end-to-end path. Median peak RSS changed from 31,396 to 31,412 KiB (+16 KiB,
+0.05%), which is within observed process noise.

Raw samples are in `baseline.txt`, `optimized.txt`, `baseline_after.txt`,
`optimized_after.txt`, `baseline_rss.txt`, and `optimized_rss.txt`. Stable
summary values are in `metrics.txt`.

## Verification

- canonical semantics: 644 passed, 0 failed, 3 explicitly filtered polyglot tests;
- native codegen smoke: 254/254 passed;
- complete phase suite and all ten emulator gates passed;
- serial polyglot cached/native tests passed; the aggregate per-language test
  still reports its pre-existing internal D and Haskell compile failures while
  returning its historical aggregate pass status.

This is a tree-interpreter optimization on the measured workloads. It does not
change direct x86-64, C99, bytecode VM, or host-interpreter execution speed.
