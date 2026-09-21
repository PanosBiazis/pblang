# Kernel collection without a redundant all-live sweep

The PBLang collector now counts each newly marked owned object, including the
single-child chain path. When that count equals the ownership-table count, all
objects are reachable and the sweep/compaction pass is unnecessary. The collector
releases its temporary work table, resets the allocation counter and records the
successful collection exactly as before. It does not allocate additional heap
metadata or retain a scratch cache. The public API and object/root capacities
are unchanged. Automatic and explicit kernel collection share this path.

If some objects are unreachable, the existing sweep and stable table compaction
still run. Roots, duplicate roots, shared edges, cycles and generation wrap keep
the same meaning. Objects that become unreachable after an earlier all-live pass
are reclaimed by the next successful collection. No user opt-in is needed.

## Ownership and concurrency review

Counting occurs only on a transition to the current mark generation. Queue length
is at most the marked count, and both are at most the owned count. Admission is
also checked on the single-child path, which previously bypassed queue-based
admission. An inconsistent count discovered there returns -1 before sweep; the
advanced generation prevents retry from reusing a partial mark pass. This is an
additional consistency check, **not validation of arbitrary raw pointers**.

All of the following remain required: unique owned entries, valid same-collector
roots/edges, no direct mutation of internal marks/counts by applications, and use
of public locked APIs while objects stay rooted. A foreign graph whose node count
happens to match is not made safe by counting. Direct memory writes, invalid
addresses, NMI/fault reentry and foreign code remain outside this raw kernel API.

The same global lock covers root enumeration, marking, the count decision and
reclamation. No check/use interval was introduced in which another cooperating
CPU can mutate the graph. Scratch allocation failure still prevents any sweep;
partial-mark failure releases scratch once and preserves collection counters.
There is no new callback, reentrant effect or persisted data. IRQ state and busy
try-collect behavior are unchanged. The lock owner still masks interrupts during
collection; reduced work is not incremental collection or a hard real-time bound.

## Current validation

[Recorded checks](evidence/kernel-gc-live-sweep-validation-2026-09-21.json):

- 21 related semantic suites passed, including runtime refusal and self-hosted
  lifetime/resource cleanup contracts.
- Four kernel fixtures passed on each of qemu64 and 486: cycles/allocation failure,
  generation retry/wrap, IRQ/root lifetime, and the new live/mixed sweep fixture.
- The two-vCPU fixture passed on qemu64 and Pentium III. Both CPUs complete 2000
  rooted lifetimes; interruptible and already-masked lock waits remain checked.
- Three four-thread native runs completed 24000 rooted lifetimes in total.
- Four touched kernel files produced zero diagnostics with the committed
  diagnostics module; both existing fixture generators passed native lint.
- Isolated build and PBLang-native portable lock publication passed.

The new `examples/kernel_gc_live_sweep.sapp` checks garbage before/between/after
live entries, duplicate roots, duplicate/shared edges, a branching cycle, 1000
all-live passes, later loss of a branch, stable table order, rootless reclamation,
an empty pass and a rooted leaf. The generation fixture additionally injects an
inconsistent count inside a single-child chain and checks recovery without a
successful-collection increment. The final fixture fails on baseline `c7aed3d`
(exit 3) and passes on the new collector (exit 85).

These are generated freestanding kernels and local native test harnesses, not
installed guest operating systems. The first 486 test build used the host
compiler's default instruction selection and failed to boot; rebuilding both
variants with explicit `-march=i386` resolved it. Only the explicit-ISA runs are
counted as compatibility evidence. No physical machine kernel was booted.

## Reproducible measurements

[Final raw samples](evidence/kernel-gc-live-sweep-2026-09-21.json) compare baseline
`c7aed3d` against this collector with identical current heap/compiler/workloads.
Two warmups and five measured runs per variant alternate before/after ordering.
GNU time records QEMU user plus system CPU, wall time and peak RSS. p95 uses nearest
rank, which is the maximum of five samples; this is a small descriptive sample,
not a tail-latency guarantee.

| Workload | CPU median before / after | CPU p95 before / after | RSS median KiB before / after |
| --- | --- | --- | --- |
| 128-object live ring, 500000 collections | 1.44 / 1.05 s | 1.48 / 1.05 s | 59368 / 59256 |
| Same live ring plus 16 garbage leaves per pass, 100000 passes | 0.81 / 0.85 s | 0.82 / 0.98 s | 59328 / 59312 |
| Rootless control, 100000 passes | 0.56 / 0.55 s | 0.79 / 0.84 s | 59160 / 59144 |

The all-live workload uses `bench/kernel_gc_collect.sapp`, replacing both `50000`
literals with `500000` for both variants. The mixed workload is the unchanged
`bench/kernel_gc_mixed_sweep.sapp`. For the rootless control remove only
`gc_add_root(g,first);` and replace `gc_live(g)!=128` with `gc_live(g)!=0` in that
same mixed fixture, identically for both variants. Its initial unrooted ring is
reclaimed by the first pass. All measured runs must exit 85; timeouts/other exits
are failures, not samples.

Concatenate heap + selected collector + fixture and translate with
`compiler/ctranspile.transpile_kernel_int`. Use `kernel_linker_script()` with
`ALIGN(4096)` immediately before `.bss` **for both variants**, avoiding the known
QEMU code/data same-page artifact. Compile with:

```
cc -m32 -O2 -march=i386 -fno-toplevel-reorder -ffreestanding -nostdlib \
  -fno-pie -mgeneral-regs-only -c workload.c -o workload.o
ld -m elf_i386 -T aligned.ld -o workload.elf workload.o
qemu-system-x86_64 -cpu qemu64 -machine pc -accel tcg -m 32M \
  -kernel workload.elf -display none -serial none -monitor none -nic none \
  -no-reboot -device isa-debug-exit,iobase=0xf4,iosize=0x04
```

Use a 25-second supervisor bound per run. See [SMP reproduction](KERNEL_GC_SMP.md)
for the separate two-vCPU generator/bootstrap. Tests require no network or disk
image. Existing linker RWX warnings are not resolved by this change.

CPU decreased about 27% in the all-live process benchmark, but increased about 5%
in the mixed workload; rootless results and RSS are nearly unchanged with noise.
ELF sizes increased by 56–60 bytes in these builds. No generic speedup, memory
saving, peripheral/power improvement or maximum IRQ pause reduction is proven.
The retained [initial samples](evidence/kernel-gc-live-sweep-initial-2026-09-21.json)
include a more complicated mixed-sweep optimization that was simplified; its
rootless series briefly overlapped a diagnostic process. Use the final series
for conclusions rather than comparing different series as a controlled speedup.

For the next improvement to reclamation in fragmented heaps, see
[free-list cursor reuse](KERNEL_GC_FREE_CURSOR.md). The measurements above remain
the historical results of all-live sweep elimination.

## Remaining work

Traversal and reclamation still execute under one lock. Incremental work needs
root/mutation barriers or another proven snapshot contract before releasing that
lock mid-pass. Scratch allocation, large edge arrays and individual free-list
insertions can still cause long pauses. Fairness, per-arena concurrency, more than
two kernel CPUs, topology/scheduler/hotplug and wider physical-platform testing
remain open.

General runtime/FFI/PB-MCP/polyglot integration is also open. In particular,
`stdlib/runtime_gc.sapp` still refuses destructive retained-host collection when
nested VM roots cannot be accounted for. This kernel optimization does not supply
those missing roots and does not silently activate a host collector. No PB-MCP
installation was changed and no Specisoft server was used.

Graphify update reached its 35-second budget and its owned process group was
stopped. Graph metadata is not semantic verification of `.sapp` code.
