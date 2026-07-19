# pblang tree-interpreter reusable AST and scalar builtins

The pure-pblang tree interpreter now separates source preparation from
execution with `compile_program` and `eval_ast`. Common one- and two-argument
builtins dispatch from scalar values instead of allocating a temporary args
array for every call.

## Method

The release runtime executed seven fresh processes pinned to logical CPU 3.
Each process prepared the same AST, ran one warmup, and averaged three fresh
environment executions. The target program performs 5,000 interpreted
`len(array)` calls and asserts the final result.

| Release metric | Baseline | Final | Change |
|---|---:|---:|---:|
| Builtin loop median | 64.110 ms | 61.641 ms | **3.85% lower** |

All seven baseline samples were between 63.43 and 65.61 ms. Final samples were
between 61.02 and 63.40 ms. Exact matching-arities avoid allocation; mismatched
arities still enter the generic dispatcher and preserve existing error behavior.

## Rejected experiments

A lazy block-declaration cache improved execution-only medians by 0.2-0.7%,
which did not clear the noise gate. A compile-time block annotation candidate
showed mixed A-B-A results: one workload improved while another matched or
regressed. Both candidates were reverted completely.

This result applies to the self-hosted tree interpreter builtin-call path. It
does not imply a universal language or native-code performance ranking.
