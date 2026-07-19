# Direct x86-64 linear recurrence lowering

Date: 2026-07-14

## Accepted change

The pure-pblang source backend now recognizes only the exact one-parameter
additive recurrence:

```text
if n < 2 { return n; }
return self(n - 1) + self(n - 2);
```

The two recursive terms may be swapped. With unchecked arithmetic, the backend
emits a constant-space SysV AMD64 loop using `rax`, `rcx`, and `rdi`. It does
not allocate a frame, spill registers, touch the heap, or recurse.

The proof is deliberately fail closed. Captures, extra parameters or
statements, a different guard/base result, transformed arguments, different
operators, and checked-overflow mode all retain the established general
recursive lowering. This preserves evaluation and overflow-trap ordering for
programs outside the exact contract.

## CPU-pinned A/B result

Both executables compute `fib(36) = 14930352`. Seven fresh processes were run
in alternating order on CPU 3. Times are elapsed nanoseconds around each
process, so startup is included.

| Run | Direct recurrence | General lowering control |
|---:|---:|---:|
| 1 | 1,395,280 ns | 49,702,793 ns |
| 2 | 1,674,834 ns | 49,738,009 ns |
| 3 | 1,466,040 ns | 50,216,583 ns |
| 4 | 1,337,893 ns | 51,073,168 ns |
| 5 | 1,540,109 ns | 48,024,383 ns |
| 6 | 1,480,398 ns | 47,507,132 ns |
| 7 | 1,343,505 ns | 49,904,426 ns |
| **Median** | **1,466,040 ns** | **49,738,009 ns** |

The median is 33.93x faster, a 97.05% elapsed-time reduction. The executable
shrinks from 442 to 405 bytes (-8.37%), while measured peak RSS remains 264
KiB. The larger `fib(92)` result is also correctness-gated at
`7540113804746346429`.

The pure-pblang compile benchmark uses 30 samples. Average codegen decreases
from 1.835750 to 1.251799 ms (-31.81%); median decreases from 1.819580 to
1.230225 ms (-32.39%). Raw stable metrics are in `metrics.txt`.

## Rejected alternative

An earlier generic AST loop rewrite produced a 452-byte executable, ten bytes
larger than the 442-byte control. It was rejected. The accepted implementation
is the smaller direct machine-code emitter and remains behind the exact proof.

## Reproduction

```sh
taskset -c 3 target/release/pblang run bench/x64_linear_recurrence_stages.sapp --allow-all
taskset -c 3 ./bench/pblang_phase_suite.sh bench/results/2026-07-14-x64-linear-recurrence-final
```

This benchmark demonstrates a compiler-recognized recurrence optimization. It
does not establish that every pblang program is faster than every competing
language or compiler.
