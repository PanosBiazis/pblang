# Kernel collector: cycles, growth and failure handling

Optional kernel recovery APIs are documented in [low-memory collection](KERNEL_GC_LOW_MEMORY.md).
They are separate from the default collector described below.

Current synchronization contract: [shared kernel lock and admission](KERNEL_GC_PARALLEL.md).
The dated IRQ-only section below describes the preceding implementation.

This change is PBLang code in `stdlib/kernel/gc.sapp`, lowered through the integer
C kernel backend. It is not a collector for ordinary retained-host VM objects,
PB-MCP, foreign runtimes, or every polyglot block. Those integrations remain open.

The old object/root tables had fixed space for 256 objects and 64 roots but no
capacity check. Registration beyond those counts could write outside the tables.
The tables now grow geometrically in the supplied free-list heap; old metadata
is freed after copying, and failed growth leaves the prior table/count intact.
`gc_new`, `gc_alloc` and failed root registration return zero on recoverable arena
allocation failure. Their callers must check results before publishing pointers
or continuing an unrooted allocation sequence. Use `gc_alloc_rooted` for atomic
allocation and root registration when interrupt-side collection is possible. Successful `gc_add_root` and
`gc_set_child` now explicitly return 1. Invalid child indexes return 0.

Reachability traversal is iterative, so native call-stack depth no longer grows
with object graph depth. Single-edge chains advance directly without queueing
every node; branches use a temporary work table. Objects are marked before
queueing/advancing, terminating self-cycles, mutual cycles and shared descendants.
`gc_remove_root(g,obj)` removes all occurrences of a root without collecting.
Unreachable cycles are swept on the next successful manual/automatic collection.

The work table reserves four bytes per registered object when roots exist. If
allocation fails, collection returns -1 without sweeping or advancing/resetting
collection counters. With no roots, collection needs no work table and can
reclaim objects even in a full arena. Collection success remains return value 0.
The automatic path still collects before the new allocation; it never sweeps
the object whose allocation triggered it. Automatic thresholds and manual mode
remain available. Arithmetic checks reflect the signed 32-bit allocator ABI,
not new global program-size limits. Tables retain their capacity after a sweep;
there is no automatic shrinking policy in this change.

## Reproduce

Use an isolated package with the complete literal imports of the integer
transpiler, heap, collector and the kernel fixture. Build and publish its lock
using the PBLang `tools/pblock.sapp` helper. Compile the concatenation of
`stdlib/kernel/heap.sapp`, `stdlib/kernel/gc.sapp` and
`examples/kernel_gc_cycles.sapp` with `transpile_kernel_int`; obtain the linker
script from `kernel_linker_script`. Compile emitted C with:

```
cc -m32 -O2 -march=i386 -fno-toplevel-reorder -ffreestanding -nostdlib \
  -fno-pie -mgeneral-regs-only -fstack-usage -c kernel.c -o kernel.o
ld -m elf_i386 -T kernel.ld -o kernel.elf kernel.o
```

`-fno-toplevel-reorder` retains the preamble's stack symbol referenced by basic
assembly. Without it this retained preamble can fail optimized linking. Its
linker script also emits an RWX segment warning; this change does not harden
kernel page permissions. No emitted C is added as product source.

Run only the owned kernel in QEMU TCG with 32 MiB, no NIC/display/serial/monitor,
`-no-reboot -device isa-debug-exit,iobase=0xf4,iosize=0x04`. The fixture returns
exit status 85 on success and 3 on an assertion failure. Give each run a bounded
supervisor timeout. The current fixture passed `qemu-system-x86_64 -cpu qemu64`
and `qemu-system-i386 -cpu 486`. These are virtual CPU models, not physical CPUs
or guest operating-system acceptance.

The fixture checks a 4096-node ring, 300 duplicate roots, branching/shared/back
edges, removing roots and reclaiming cycles, automatic collect-before-allocate,
invalid indexes/sizes, exhausted object/root tables and failed scratch allocation.
It also confirms rootless collection still succeeds under arena exhaustion.

Twenty-one suites passed: twelve kernel code-generation suites, resource scopes,
latest updates (including their self-hosted variants), scalar owned callbacks,
retained GC diagnostics and the three self-hosted lifetime suites. These source
checks are distinct from QEMU behavior. The PBLang diagnostics module accepts
the explicit kernel target/extern annotations. Retained native lint still
reports unresolved kernel intrinsics on raw kernel files; ordinary test-file
lint passes. This limitation was not hidden by inventing host implementations.

## Security scope and remaining work

See [the focused automatic-lifetime review](AUTOMATIC_LIFETIME_REVIEW.md).
The collector is precise only when the kernel supplies valid owned pointers,
complete roots and serialized access. It does not scan the machine stack,
validate arbitrary addresses or isolate hostile code. Public collector/heap
operations now share an atomic memory lock while preserving local interrupt
state; see `KERNEL_GC_PARALLEL.md` for current coverage and limits. Arbitrary
native writes are not covered. A caller must keep any value needed after a possible
collection rooted before another allocation can trigger automatic collection.
No thread-wide retained-host destructive GC has been enabled.

## Historical measurements before IRQ serialization

Benchmark source: `bench/kernel_gc_collect.sapp`. Baseline is
`4d35860:stdlib/kernel/gc.sapp`; both variants use the same 128-object rooted ring
and 50000 manual collections. This stays within the old capacities. Compile both
with the same options above, except omit `-march=i386` for this comparison (the
compiler's default 32-bit target was used). Run QEMU x86_64/TCG and collect GNU
time `%U %S %e %M`. Alternate before/after order, discard two warmups per variant,
then retain five samples each. Success requires exit 85 and matching live/count
checks, not just process termination.

[Final raw samples](evidence/kernel-gc-2026-09-20.json) give median CPU 8.24 s
before / 10.41 s after, about +26.3%; p95 9.18 / 11.07 s. Median QEMU peak RSS
was 59152 / 58884 KiB, with overlapping ranges; no general RAM saving is claimed.
These include QEMU boot/translation and measure the emulator, not native hardware
execution, guest live heap, GPU/NPU usage, power or total device I/O. Machine load
and sample variation limit conclusions. The two ELF files were 7784 / 8860 bytes
(+1076 bytes); no storage reduction is claimed.

The compiler's stack-usage output reports a 48-byte collector frame in the new
benchmark versus a 64-byte collection frame plus recursive 16-byte mark frames
in the old one. These are compiler frame estimates, not a measured whole-stack
maximum. New graph traversal itself does not recurse; its work storage moves to
the arena. The earlier generic queue implementation measured +47.4% median CPU
in a separate run; [its samples](evidence/kernel-gc-initial-2026-09-20.json) are
retained. Run-to-run baselines changed, so the two runs are not a controlled
speed comparison with one another.

These are historical process measurements, not isolated algorithm costs. The
follow-up layout control below demonstrates that QEMU code/data placement
substantially affects this fixture. The earlier +26.3% result must not be treated
as a general GC overhead estimate. No external corporate server was used.

Graphify refresh was attempted and stopped at its 35-second budget. It does not
provide semantic verification for this PBLang kernel source.

## Mark generations and interrupt serialization, 2026-09-20 (historical)

Ordinary collections now use a generation at control offset 44 instead of clearing
all object marks first. Header/control sizes are unchanged. Generation advances
before marking, including a pass that fails after partial marking. A scratch
allocation failure leaves it unchanged. At signed-32-bit generation wrap, all
owned marks are cleared before generation 1 is reused. New objects start at 0.
The wrap traversal is rare, but its cost has not disappeared.

Public collector and free-list heap operations preserve the incoming x86 IF bit,
disable maskable interrupts and restore the saved state on return. Nested calls
and failure returns preserve IF=0. The existing PBLang C backend emits two small
CPU primitives with compiler memory barriers; product changes remain PBLang
source. This is a **single-active-CPU kernel contract**, not an SMP mutex or a
solution for NMI, DMA, foreign writes or malicious raw pointers. The underscore
helpers are implementation internals and must not be called independently.

`gc_alloc_rooted(g, child_count)` returns an allocated, registered root, or 0.
Root-table growth failure rolls back the new object while still in the same
critical section. The cumulative allocation-work counter can still increase on
this failed attempt. Root/table capacity is not shrunk. Remove the root only
when no caller will subsequently use the object. Existing `gc_alloc` still
returns an **unrooted** object: callers must protect the entire publication
sequence or use the new API. Two independently guarded calls do not make such
a sequence atomic. Complete roots, valid pointers and ownership remain required.

Collection masks interrupts for its complete duration, including traversal and
sweeping. This delays interrupt delivery; pause latency is not bounded to a
real-time deadline or measured by the throughput benchmark. Arbitrary `kmalloc`
users, direct `poke32` mutations and multiple CPUs need their own discipline.
No automatic collection has been activated for PB-MCP or foreign runtimes.

The free-list allocator also refuses negative payload sizes and values above
2147483632 before list access, avoiding signed-32-bit header/alignment overflow.
This is an ABI arithmetic bound, not a new source/project size limit. It still
requires valid initialized arenas and valid, owned, not-already-freed pointers.
General invalid-pointer/double-free validation is outside this change.

### Current verification

Three kernel fixtures passed on both qemu64 and 486 (six positive cases): the
existing cycle/growth fixture; `examples/kernel_gc_generations.sapp` for partial
mark failure/retry, stale marks, wrap and repeated collection; and
`examples/kernel_gc_interrupts.sapp` for actual PIT IRQ0 collection interleaved
with main-code use, nested IF preservation, failure paths, atomic root-publication
rollback and allocator overflow refusal without altering a nonempty free list.
The interrupt fixture deliberately waits for timer collection between publishing
an object and reading it again. It prepends heap, collector, interrupts and timer.
These tests use one virtual CPU and do not prove SMP safety or physical-device
compatibility. Twenty-one related suites, build, two native lint checks and five
kernel diagnostics checks passed. Native compiler lint retains its pre-existing
unused `valid_timeout` warning. No new persistent background service is required.

### Controlled performance follow-up

The unchanged 128-object/50000-collection workload compared `fc508ac` heap+GC
against the new heap+GC, including the new guards. Both use the same current
transpiler and optimization flags from the earlier benchmark. Two warmups and
five measured samples per variant alternate order. The default-linker run gave
8.03 / 0.24 seconds median QEMU CPU, but **this is not a 97% GC algorithm speedup**.
The old heap begins at 0x100e20 in the page containing collector code; the new
heap starts at 0x1015a0. This is consistent with QEMU translation invalidation
from code/data sharing a page, rather than comparable native CPU execution.

A second control applies exactly `ALIGN(4096)` before `.bss` in the linker script
for **both** variants, leaving the workload unchanged. Median CPU is 0.16 / 0.16
seconds. This linker adjustment is only a benchmark control; it is not silently
applied to product builds. To reduce startup and timing-resolution noise, a
third control replaces both occurrences of 50000 with 500000 in the same fixture
for both variants (loop and success assertion). It measures median CPU
1.48 / 1.35 seconds (about 8.8% lower), nearest-rank p95 1.86 / 1.36 seconds.
Samples vary substantially, so this is workload-specific evidence, not a general
speed guarantee. Median QEMU RSS in that control is 59168 / 59380 KiB (about
0.36% higher); general RAM savings are not demonstrated. ELF size is
8860 / 11156 bytes (+2296). Collector frame estimates are 48 bytes before,
52 bytes for the new helper plus a 12-byte wrapper; these are not peak stack
measurements. Graph traversal remains nonrecursive.

Raw samples and build settings are retained for all three runs:
[default layout](evidence/kernel-gc-irq-default-layout-2026-09-20.json),
[aligned layout](evidence/kernel-gc-irq-aligned-layout-2026-09-20.json),
[aligned longer workload](evidence/kernel-gc-irq-aligned-extended-2026-09-20.json).
No GPU/NPU, power, physical storage I/O or installed PB-MCP improvement is claimed.
