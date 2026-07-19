# pblang tree-interpreter single-parameter call optimization

The pure-pblang tree interpreter now binds the dominant one-argument,
one-parameter call directly into its new scope map. Calls whose function has
zero or multiple declared parameters retain the generic loop, including the
existing nil-fill behavior for missing parameters.

## Method

`bench/pblang_phase_stages.sapp` was executed with the release runtime and CPU
affinity pinned to logical CPU 3. Each side used seven fresh processes and the
median `tree_interpreter_fib18` average. Later samples on both sides showed
frequency throttling, so CPU affinity and equal sequence shape matter more than
an unpinned single run.

| Release metric | Baseline | Final | Change |
|---|---:|---:|---:|
| Tree interpreter `fib(18)` | 117.98 ms | 109.63 ms | **7.1% lower** |

The existing `interp_runtime_test.sapp` covers recursion, exact one-parameter
calls, closures, zero-argument calls, and a one-argument call into a
two-parameter function. The optimization therefore has regression coverage for
both its direct path and generic fallback.

## Rejected experiment

A self-hosted bytecode-VM `OP_CALL` specialization for zero, one, and two
arguments was measured and reverted. `fib(18)` changed only 0.1%, while the
shorter `fib(12)` phase regressed about 0.6%. The generic VM argument builder
remains until a dispatch-reducing design demonstrates a material gain.

This result is one measured interpreter improvement, not evidence that pblang
is universally faster than every language or workload.
