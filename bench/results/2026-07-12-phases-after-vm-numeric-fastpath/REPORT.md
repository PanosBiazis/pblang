# pblang phase comparison: VM primitive opcode fast path

This snapshot uses the same workloads and host as
`../2026-07-12-phases-after-call-fastpath`. Every measured phase, the full
640-test semantic suite, and all ten emulator process gates passed.

## Accepted change

The pure-pblang bytecode VM now executes primitive arithmetic and comparison
opcodes directly in its dispatch loop. Map and class values retain the existing
dunder overload path, including reflected operators. This removes one pblang
function call per ordinary `+`, `-`, `*`, `/`, modulo, equality, or ordered
comparison without changing observable operator behavior.

| Metric | Before | After | Change |
|---|---:|---:|---:|
| Dedicated VM `fib(12)` median, 3 runs | 8.1267 ms | 8.0735 ms | 0.65% faster |
| Whole phase process wall time | 1.19 s | 1.12 s | 5.9% lower, not attributed |
| Whole phase process peak RSS | 44,676 KB | 44,796 KB | 0.3% higher |
| Direct x86-64 artifact | 425 B | 425 B | unchanged |
| Direct x86-64 peak RSS | 264 KB | 264 KB | unchanged |
| CLI C99 compile | 204 ms | 170 ms | run-to-run improvement, not attributed |
| CLI C99 artifact | 25,240 B | 25,240 B | unchanged |

The before samples were 8.1267, 8.1203, and 8.4720 ms. The after samples were
8.0253, 8.0895, and 8.0735 ms. The full-suite snapshot measured 8.1209 ms,
which confirms that this is a small optimization near normal measurement noise,
not a major VM speedup. Larger VM gains require reducing call-frame and dynamic
value overhead rather than adding more dispatch special cases.

## Phase snapshot

| Phase | Average |
|---|---:|
| Tokenize | 0.5681 ms |
| Parse | 0.5659 ms |
| Semantic analysis | 0.0967 ms |
| Typecheck | 0.0307 ms |
| Bytecode codegen | 0.1756 ms |
| Safe bytecode optimization | 0.1051 ms |
| Self-hosted VM `fib(12)` | 8.1209 ms |
| Tree interpreter `fib(18)` | 99.4264 ms |
| C99 transpile | 0.6683 ms |
| x86-64 source codegen | 1.5149 ms |
| x86-64 IR codegen | 0.1231 ms |
| i386 IR codegen | 0.0983 ms |
| ARM64 IR codegen | 0.1345 ms |
| ARM64 source codegen | 1.1471 ms |
| WASM source codegen | 0.8830 ms |

Raw, machine-readable evidence is in `phases.txt`, `phase_resources.txt`,
`direct_x64_resources.txt`, `c99_resources.txt`, and `emulators.csv`. A
single recursive Fibonacci result is not evidence that a language wins every
workload; these phase metrics are intended to identify the next bottleneck and
prevent benchmark-specific changes from being mistaken for general progress.
