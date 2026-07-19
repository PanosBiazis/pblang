# Tree-interpreter prepared-AST optimization - 2026-07-14

This round optimizes the self-hosted tree interpreter in
`stdlib/compiler/interp.sapp`. The implementation and benchmark harness are
pure pblang.

## Change

`compile_program` now prepares immutable interpreter metadata once:

- numeric expression, statement, and binary-operator tags;
- parsed numeric literal values;
- call arity and immutable builtin-candidate metadata;
- closure arity alongside normalized parameter names and body statements.

The metadata uses private `@` map keys, which cannot collide with pblang source
identifiers. Public parser fields such as `kind` and `op` remain unchanged.
`eval_ast` lazily prepares raw parser ASTs, so its existing API remains
compatible. Runtime environment lookup is still performed after argument
evaluation, preserving builtin shadowing and argument-before-callee behavior.

## Pinned A/B result

Seven fresh processes were pinned to CPU 3 in each configuration. The table
compares the median of the per-process averages; lower is better.

| Workload | Baseline | Prepared AST | Change |
|---|---:|---:|---:|
| execute `fib(18)` | 98.7398 ms | 94.1771 ms | **-4.62%** |
| execute `choose(15,7)` | 210.7920 ms | 199.4117 ms | **-5.40%** |
| closure capture | 0.031584 ms | 0.031193 ms | **-1.24%** |
| compile `fib(18)` | 0.646699 ms | 0.719780 ms | +11.30% |
| end-to-end `fib(18)` | 100.8495 ms | 96.1217 ms | **-4.69%** |

Preparation adds about 0.073 ms to this small compile fixture, but removes more
than 4.7 ms from its complete compile-and-run path. The change therefore passes
the end-to-end acceptance gate rather than optimizing execution by hiding a
larger total cost.

## Correctness and scope

- `interp_runtime_test.sapp` covers reusable prepared ASTs, raw parser ASTs,
  public AST compatibility, recursion, closures, arities, builtin shadowing,
  and evaluation order.
- The canonical suite passed 644/644 tests with three polyglot tests run
  separately.
- The phase suite and all ten emulator checks passed.
- This result does not claim universal language superiority. It is a scoped,
  reproducible improvement to the pblang-written tree interpreter.

Raw comparison artifacts are in the sibling
`2026-07-14-tree-interpreter-tags-competitors/` directory.
