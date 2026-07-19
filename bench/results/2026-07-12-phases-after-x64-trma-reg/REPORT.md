# pblang x86-64 control-flow optimization report

This snapshot adds a structurally proven tail-recursion-modulo-addition (TRMA)
lowering to the pure-pblang x86-64 backend. It also adds a five-sample
`fib(36)` stress metric to the phase suite so generated-code throughput is not
hidden by process-startup noise.

## Accepted implementation

For a one-parameter function containing exactly a base-case return followed by
`return self(left) + self(right)`, the backend preserves left-to-right
evaluation while executing the right recursive branch as a loop in the current
frame. Functions with captures, additional statements, different operators, or
different call shapes are unchanged. Checked-overflow mode also keeps the
original recursion tree so trap ordering cannot change.

The generated loop also gains:

- direct constant-true loop emission, without a per-iteration `mov/test/jz`;
- removal of a proven unreachable implicit epilogue;
- register in-place accumulator updates;
- register-save analysis aligned with the temp-free emitter path.

## Results

| Metric | Before this round | Final | Change |
|---|---:|---:|---:|
| Direct x86-64 `fib(32)`, best of 5 | 10 ms | 8 ms | 20% lower, startup dominated |
| Direct x86-64 `fib(36)`, five-run median | not previously recorded | 45 ms | new stable stress metric |
| Intermediate TRMA `fib(36)` median | 93 ms | 45 ms | 51.6% lower after register refinement |
| Direct x86-64 artifact | 425 B | 442 B | +17 B for asymptotically fewer calls |
| Direct x86-64 peak RSS | 264 KB | 264 KB | unchanged |
| x86-64 source codegen | 1.515 ms | 1.606 ms | 6.0% higher |

The final `fib(36)` samples are 45, 45, 44, 46, and 45 ms; all produced
`14930352`. The fixed-input `fib(32)` competitor run measured pblang at 8 ms,
GCC C at 5 ms, Clang C at 6 ms, Rust at 6 ms, and Fortran at 2 ms. Therefore
this is a material improvement, but not evidence that pblang is universally
the fastest language.

## Verification

- `codegen_smoke`: 254/254 passed.
- semantic suite: 640/640 passed.
- phase suite: passed.
- emulator process gates: 10/10 passed.
- x86-64 source check and focused hardening suites: passed.

Raw evidence is in `phases.txt`, `phase_resources.txt`,
`direct_x64_fib36.csv`, `direct_x64_fib36_resources.txt`,
`direct_x64_resources.txt`, `c99_resources.txt`, and `emulators.csv`.
