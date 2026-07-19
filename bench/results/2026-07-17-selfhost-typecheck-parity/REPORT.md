# Self-hosted typechecker parity and hardening

Date: 2026-07-17

## Scope

This slice replaces the shallow PBLang-written typechecker with a bounded
gradual checker for core lexical and function semantics. It also replaces the
legacy `frontend_analyze` substring heuristics with the real self-hosted
tokenizer/parser. No Rust implementation was added or changed.

## Correctness and safety changes

- lexical scopes preserve inferred and declared binding types;
- function signatures are hoisted for recursion and call-before-declaration;
- scalar and nested-array annotations are resolved;
- assignments, default values, call arity/arguments, and declared returns are
  checked when both sides have concrete types;
- arrays, maps, index/member access, closures, option access, coalescing,
  try/catch, if-expressions, and match arms are traversed without placeholder
  results hiding internal failures;
- malformed ASTs, including recognized nodes with missing or invalid required
  fields, and configurable node/depth budgets return structured diagnostics;
- compiler entry points no longer use `catch []` around type checking.

## Verification

- `43/43` self-host semantic tests passed.
- The complete semantic suite passed `676/676`, with zero failed, ignored, or
  filtered tests.
- `--check --via-pblang` passed for `typeck.sapp`, `parser.sapp`,
  `codegen.sapp`, and `selfhost_frontend.sapp` after the frontend heuristic was
  removed.
- The focused parity test covers assignment, argument/arity, default, return,
  hoisting, shadowing, array indexing, malformed AST, resource exhaustion, and
  frontend diagnostic propagation.

## Measurement method

The existing `pblang_phase_stages.sapp` typecheck row was run in seven fresh
processes pinned to CPU 2 before and after the change. Each process performs 100
checks of the same parsed four-statement AST. Imports occur before timing.
Raw samples are retained in `baseline-typecheck.txt`,
`candidate-typecheck.txt`, and `final-typecheck.txt`.

| implementation | median ms/check | interpretation |
|---|---:|---|
| old shallow checker | 0.033049 | did not traverse/check lexical function bodies |
| first full candidate | 0.231338 | complete core checks in this slice |
| optimized core checker | 0.212722 | interned scalar types and cached builtin registry |
| final fail-closed checker | 0.246829 | complete required-field validation without temporary field arrays |

The first safety implementation used temporary arrays for field schemas and
measured `0.269739 ms/check`. Arity-specific validators reduced that draft by
`8.49%` to the final `0.246829 ms/check`. Compared with the old shallow pass,
the final checker adds `0.213779 ms/check` (`+646.85%`) on this tiny fixture.
That is recorded as a real correctness and safety cost, not described as a
speedup.

The dedicated PBLang-only `selfhost_typecheck_stages.sapp` fixture reported:

| workload | statements | diagnostics/check | ms/check |
|---|---:|---:|---:|
| legacy-compatible | 4 | 0 | 0.248713 |
| typed signatures | 128 | 0 | 4.519775 |
| typed diagnostics | 128 | 64 | 4.857402 |

The separately timed dedicated process used 19,744 KiB peak RSS and 0.67 s wall
time. The raw phase and resource files are retained in this directory.

## Limits and next parity work

This does not claim full parity with the legacy Rust checker. The self-hosted
parser currently exposes scalar and nested-array annotation strings; richer
option, result, generic, non-null, struct, enum, and exhaustiveness semantics
remain differential parity work. The benchmark is an internal compiler-phase
measurement and is not a cross-language performance comparison.
