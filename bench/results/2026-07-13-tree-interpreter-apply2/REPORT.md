# pblang tree-interpreter two-parameter call optimization

The pure-pblang tree interpreter now evaluates two user-call arguments into
scalar locals and binds exact two-parameter closures directly. This removes the
temporary argument array and generic binding loop from matching calls.

## Method

The release runtime executed seven fresh processes pinned to logical CPU 3.
Each process averaged three `choose(15,7)` evaluations and then three unchanged
`fib(18)` evaluations. Both programs include parsing and correctness assertions.
The ratio between the two-argument workload and the one-argument control is the
primary metric because this mobile CPU changed frequency during the run.

| Release metric | Baseline | Final | Change |
|---|---:|---:|---:|
| `choose(15,7)` median | 276.72 ms | 227.36 ms | 17.8% lower |
| `fib(18)` control median | 115.51 ms | 109.74 ms | 5.0% lower |
| `choose / fib` median ratio | 2.3546 | 2.0842 | **11.5% lower** |

`interp_runtime_test.sapp` covers exact two-parameter recursion, a two-argument
call into a three-parameter function, and a two-argument call into a
one-parameter function. Existing tests retain argument-before-callee ordering
and builtin shadowing behavior.

This is evidence for one self-hosted interpreter call path, not a claim that
pblang is universally faster than every language or workload.
