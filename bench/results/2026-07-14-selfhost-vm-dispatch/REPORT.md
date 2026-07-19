# Pure-pblang VM dispatch ordering

The bytecode VM in `stdlib/compiler/vm.sapp` now tests its most frequent loop
and function opcodes immediately after `OP_CONSTANT`: local loads/stores and
conditional/unconditional jumps. This changes only dispatch order. Opcode
values, bytecode format, stack effects, exception behavior, and public APIs are
unchanged.

## Method

`bench/selfhost_vm_dispatch_stages.sapp` compiles four correctness-gated
programs once and measures repeated VM execution. Baseline and final states each
used seven fresh release-runtime processes pinned to logical CPU 3. Every row
below is the median of the seven per-process averages. Workload order and
iteration counts were identical on both sides.

| Workload | Baseline | Final | Change |
|---|---:|---:|---:|
| Recursive `fib(14)` | 21.609 ms | 20.506 ms | **5.11% lower** |
| Arithmetic loop, 10,000 iterations | 145.081 ms | 94.486 ms | **34.87% lower** |
| Direct `len`, 10,000 calls | 205.940 ms | 157.559 ms | **23.49% lower** |
| Direct `bit_shr`, 10,000 calls | 195.995 ms | 148.842 ms | **24.06% lower** |

The loop gains are large enough to clear the observed CPU-frequency drift. The
recursive workload improves less because calls and fresh VM frames still
dominate it. Dispatch ordering does not change VM allocation shapes, so no RSS
or artifact-size improvement is claimed.

## Rejected interpreter experiments

Three pure-pblang tree-interpreter candidates were measured and completely
reverted before accepting the VM change:

- array-backed closure metadata: `fib(18)` improved 0.47%, while
  `choose(15,7)` regressed 0.42%;
- two-level unrolled environment lookup: improvements of 0.49% and 1.14%,
  below the noise gate;
- one reusable return cell per evaluation: regressions of 0.36% and 0.83%.

Keeping these results prevents repeating changes that add complexity without a
defensible benefit.

## Verification

- VM/import/operator/PBCK focused tests: 12/12 passed.
- Semantic files: 644 passed in the parallel-safe run and three polyglot files
  completed serially.
- The full phase suite passed, including C99, x86-64, i386, ARM64, WASM, and ten
  pblang emulator models.

`polyglot_cdylib_per_lang_test.sapp` still prints D and Haskell adapter failures
while returning success. Those adapters are not claimed as passing here.
