# pblang phase comparison: tree-call fast path

This snapshot uses the same workloads and host as `../2026-07-12-phases`.
Every phase and all ten emulator processes passed their correctness gates.

## Accepted change

The pure-pblang tree interpreter now has scalar zero- and one-argument user-call
paths. Recursive `fib(n)` no longer allocates an argument array on every call.
Arguments are still evaluated before callee resolution, preserving observable
behavior when an argument mutates the function binding. Missing parameters are
still initialized to `nil`; the generic multi-argument path is unchanged.

| Metric | Baseline | After | Change |
|---|---:|---:|---:|
| Tree interpreter `fib(18)` | 109.461 ms | 105.263 ms | 3.8% faster |
| Whole phase process wall time | 1.25 s | 1.19 s | 4.8% lower |
| Whole phase process peak RSS | 45,324 KB | 44,676 KB | 1.4% lower |
| Direct x86-64 artifact | 425 B | 425 B | unchanged |
| Direct x86-64 peak RSS | 264 KB | 264 KB | unchanged |
| CLI C99 compile | 218 ms | 204 ms | run-to-run improvement, not attributed |
| CLI C99 artifact | 25,240 B | 25,240 B | unchanged |

The dedicated three-run medians were 109.526 ms before and 105.586 ms after,
a 3.6% improvement. This is the more conservative interpreter estimate.

## Rejected experiment

A shared mutable return sentinel was tested to remove one return allocation.
Its three-run median was 108.052 ms, slower than the accepted 105.586 ms path,
so it was reverted. Per-return one-slot arrays remain in use.

Other phase differences are treated as measurement noise unless separately
isolated. Raw metrics are in `phases.txt`, `phase_resources.txt`,
`direct_x64_resources.txt`, `c99_resources.txt`, and `emulators.csv`.
