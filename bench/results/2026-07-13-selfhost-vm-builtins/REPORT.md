# pblang self-hosted VM builtin dispatch

The pure-pblang bytecode VM now resolves builtin names through a registry built
once at module load and consumes normal one-, two-, and three-argument builtin
calls through scalar fast paths. User closures, unusual arities, and raw legacy
function constants keep the generic argument-array path.

## Method

The release runtime executed seven fresh processes pinned to logical CPU 3.
Each process compiled two source programs once, warmed each chunk once, then
averaged four executions. Each execution performs 10,000 builtin calls and
checks its output before timing begins.

| Release metric | Baseline | Final | Change |
|---|---:|---:|---:|
| Common `len` calls | 215.466 ms | 207.677 ms | **3.62% lower** |
| Late-name `bit_shr` calls | 246.038 ms | 197.581 ms | **19.69% lower** |

The registry-only intermediate measured 214.793 ms for `len` and 227.596 ms
for `bit_shr`, separating the hash-membership gain from the scalar-call gain.
The independent `fib(18)` VM benchmark remains on the generic user-function
path and showed no intentional semantic or dispatch-path change.

Regression coverage verifies late-name lookup, user shadowing, first-class
builtin aliases, independent mutable descriptors, three-argument builtins,
multi-argument output order, and unknown-global behavior.

These measurements describe this self-hosted VM path on this machine. They are
not a claim that pblang is universally faster than other languages.
