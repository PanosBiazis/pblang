# Reusing the free-list position during kernel sweep

Kernel GC reclamation now remembers the surviving free-list position after each
freed object, within the same locked sweep. For an increasing sequence of object
addresses it continues searching from that position. For a lower address it
restarts from the head. The original address-ordered insertion and adjacent-block
coalescing are preserved, including returning the predecessor when a backward
merge absorbs the newly freed block.

This is PBLang implementation code in `stdlib/kernel/heap.sapp` and
`stdlib/kernel/gc.sapp`. It activates automatically in explicit and automatic
kernel collection. It adds no persistent heap metadata or scratch allocation.
The cursor is a local scalar and expires before leaving the collector's lock.
Public heap/GC APIs, return values, allocation limits and memory layouts are
unchanged. The existing all-live shortcut still skips sweep altogether.

## Correctness and security boundaries

`_guarded_heap_free_from` is a private free-only sequence helper. The first cursor
is zero; subsequent cursors must be the returned surviving block from the last
call, in the same arena under uninterrupted ownership of the memory lock. A saved
cursor must never cross an allocation, lock release, unrelated mutation or arena
reset. The ordinary `_guarded_heap_free` wrapper supplies zero and preserves its
previous return behavior. GC only performs frees and writes to its separate live
ownership table while reusing the cursor. It never exposes the cursor publicly.

If the next object address descends, the helper ignores the prior position before
following links. If forward coalescing absorbs a previous cursor during such a
restart, the returned replacement becomes the next cursor. If backward coalescing
absorbs the current block, the predecessor is returned instead. Continuing from
an absorbed header would be incorrect; the test exercises both directions.

The global lock still covers checking the position, traversing links and updating
the free list. No check/use interval or callback was added. Other cooperating CPUs
cannot allocate into or rewrite the cursor's block during that interval. There
is no new double-free path: GC still frees exactly the entries not marked in the
current generation. Scratch and allocation-failure cleanup use the ordinary
head-based wrapper, outside the cursor sequence.

Raw pointers and caller-owned arena metadata remain trusted kernel inputs.
Invalid pointers, double-free by applications, forged cursors, direct memory
writes, NMI/fault reentry and unsynchronized FFI/DMA are not validated or isolated
by this helper. The private helper is not a new safe API for external input.
Local IRQs still remain masked while the lock owner collects. Waiting behavior,
try-collect busy behavior and lack of a real-time guarantee are unchanged.

## Validation

[Recorded results](evidence/kernel-gc-cursor-validation-2026-09-21.json):

- 21 kernel, resource/lifetime, retained-runtime refusal and self-hosted suites.
- Five behavioral kernel fixtures on both qemu64 and 486: cycles/OOM,
  generation retry/wrap, IRQ/root ownership, live/mixed sweeping and free cursors.
- Two-vCPU execution on qemu64 and Pentium III, including interruptible and masked
  lock waiting and 2000 rooted lifetimes on each CPU.
- Three native four-thread runs, totaling 24000 rooted lifetimes.
- Four touched kernel sources with zero diagnostics using the committed diagnostics
  module, probe-generator lint, isolated build and portable lock publication.

The new `examples/kernel_heap_free_cursor.sapp` allocates 96 blocks with distinct
payload sentinels and frees them in ascending, descending and permuted order.
Every not-yet-freed sentinel remains intact; each sequence returns the entire
3072-byte arena to one free block, which can be allocated again as a single block.
It also permutes a valid GC ownership table containing rooted and unrooted
self-cycles, checks survivors and full reclamation, and verifies a later 24000-byte
allocation succeeds. Direct metadata writes are fixture-only failure/order
simulation, not a supported application operation.

The separate PBLang tool `tests/tooling/kernel_gc_sweep_probe_generate.sapp`
instruments the free-list search in **generated test source only**. It reserves
low RAM 0x7000 in the single-CPU fixture for a counter, resets it immediately before
collection and emits the final count through QEMU debug port 0xe9. Both variants
run the same 1024-live/1024-dead layout and verify the same live count. On both
qemu64 and 486, baseline makes **480187** free-list visits and the new collector
makes **1984**, about 99.6% fewer visits. These are operations, not CPU percentages.

## Timing and memory measurements

[Final samples](evidence/kernel-gc-cursor-benchmark-2026-09-21.json) compare baseline
`e2715cd` and the new heap+GC with identical source workloads and compiler options.
Each variant has two warmups and five measured samples, alternating order.
Measurements are QEMU process user+system CPU, wall time and peak RSS. Nearest-rank
p95 of five samples is the maximum; it does not establish a tail-latency bound.

| Workload | CPU median before / after | CPU p95 before / after | Peak RSS median KiB before / after |
| --- | --- | --- | --- |
| Fragmented, 80 passes of 1024 live and 1024 dead objects | 0.48 / 0.18 s | 0.48 / 0.18 s | 53680 / 56052 |
| All-live 128-object ring, 500000 collections | 1.09 / 1.09 s | 1.13 / 1.11 s | 54588 / 54440 |
| 128 live objects plus 16 garbage leaves, 100000 passes | 0.89 / 0.89 s | 0.91 / 0.90 s | 54456 / 54528 |

The fragmented benchmark's CPU total decreases about 62.5%; ordinary controls
have unchanged medians. QEMU peak RSS rises about 4.4% in the fragmented workload;
there is no demonstrated general RAM saving. These process totals include setup,
compiler-layout effects and emulator memory, and are not guest heap, physical IRQ
pause, power or GPU/NPU measurements. ELF files grow by 140–144 bytes here.

The [initial series](evidence/kernel-gc-cursor-benchmark-initial-2026-09-21.json)
is retained. It showed a small mixed-workload regression and included an avoidable
head-pointer read, which was removed before final validation. Do not compare
absolute times between separate series as a controlled speedup.

### Reproduce

Use the current compiler to concatenate the selected heap + GC + fixture and call
`compiler/ctranspile.transpile_kernel_int`. Use `kernel_linker_script()` with
`ALIGN(4096)` before `.bss` in both variants, preserving the control for QEMU's
code/data same-page artifact. Compile with `-m32 -O2 -march=i386
-fno-toplevel-reorder -ffreestanding -nostdlib -fno-pie -mgeneral-regs-only` and link
with `ld -m elf_i386` and that aligned script. See
[the preceding sweep report](KERNEL_GC_LIVE_SWEEP.md) for the complete command.

Use `bench/kernel_gc_fragmented_sweep.sapp` unchanged. It resets its dedicated
arena between ownership epochs; it is not a general collector-destruction API.
Use the existing mixed fixture unchanged. For the all-live fixture replace both
50000 literals by 500000 in both variants. Run single-vCPU qemu64/TCG with 32 MiB,
no disk/network/display, the ISA debug-exit device and a 25-second supervisor bound.
Exit 85 is success; all other exits/timeouts fail. The instrumented probe is run
separately and is never included in the timing samples:

```
pblang run tests/tooling/kernel_gc_sweep_probe_generate.sapp -- heap.sapp gc.sapp probe.c
```

Compile/link its generated C with the same options, then add `-debugcon stdio` to
the single-vCPU QEMU command to read the count. Repeat with `-cpu 486` using the
32-bit QEMU executable. Kernel tests remain QEMU-only, not host executables or
installed guest operating systems; existing linker RWX warnings remain unresolved.

For the subsequent mark-phase optimization, see
[leaf queue elimination](KERNEL_GC_LEAF_MARKING.md). The measurements above
remain the historical results for free-list cursor reuse.

## Remaining work

Monotone free sequences avoid repeatedly visiting earlier free-list nodes;
arbitrarily ordered addresses can still restart and retain quadratic worst-case
behavior. Mark traversal, work-table allocation and large child arrays remain
inside the same critical section. Incremental collection requires a proven
root/mutation barrier or snapshot contract before dropping the lock mid-pass.
Per-arena concurrency, fairness, more SMP configurations and physical-platform
acceptance are still open.

General runtime/FFI/PB-MCP/polyglot integration remains open. The retained-host
`runtime_gc` still refuses destructive collection because nested VM roots are
incomplete; this optimization neither supplies them nor bypasses that refusal.
No PB-MCP installation or EnterpriseCore server was used for these kernel tests.

Graphify update exceeded its 35-second budget; its owned process group was
stopped. Its graph is not semantic coverage of these PBLang modules.
