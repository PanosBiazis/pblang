# Portable IR constant-time stack pop

The pure-pblang portable register-IR evaluator previously implemented every
`popr` by copying all remaining stack values into a new array. Draining a stack
therefore required quadratic work and repeated allocations. `popr` now checks
for underflow explicitly and uses the runtime's mutating `array_pop`, making the
operation constant-time while retaining deterministic failure on an empty
stack.

## Method

`bench/codegen_ir_stack_stages.sapp` constructs its IR before timing. Each
measured round pushes 2,000 values and pops all 2,000 values; each process runs
three rounds and correctness-gates every result. Seven fresh release-runtime
processes were pinned to logical CPU 3. Peak RSS came from `/usr/bin/time` and
includes runtime startup, imports, and the benchmark process itself.

| Metric | Baseline | Final | Change |
|---|---:|---:|---:|
| Evaluator median | 1643.439 ms | 10.910 ms | **99.34% lower** |
| Relative throughput | 1.00x | 150.63x | **150.63x** |
| Median peak RSS | 11,568 KiB | 10,780 KiB | **6.81% lower** |
| Work per `popr` | O(n) copy | O(1) mutation | no replacement array |

Semantic regressions verify LIFO order, explicit underflow failure, and a full
256-value drain with an exact checksum. `portable_ir_stack_push_pop` is also a
permanent correctness-gated phase metric. These numbers describe this isolated
portable evaluator stack path on this machine, not native generated-program
performance or a universal cross-language result.
