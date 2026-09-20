# Kernel collector: cycles, growth and failure handling

This change is PBLang code in `stdlib/kernel/gc.sapp`, lowered through the integer
C kernel backend. It is not a collector for ordinary retained-host VM objects,
PB-MCP, foreign runtimes, or every polyglot block. Those integrations remain open.

The old object/root tables had fixed space for 256 objects and 64 roots but no
capacity check. Registration beyond those counts could write outside the tables.
The tables now grow geometrically in the supplied free-list heap; old metadata
is freed after copying, and failed growth leaves the prior table/count intact.
`gc_new`, `gc_alloc` and failed root registration return zero on recoverable arena
allocation failure. Their callers must check results before publishing pointers
or continuing a rooted allocation sequence. Successful `gc_add_root` and
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
validate arbitrary addresses, isolate hostile code, or synchronize interrupt
handlers/other threads. A caller must keep any value needed after a possible
collection rooted before another allocation can trigger automatic collection.
No thread-wide retained-host destructive GC has been enabled.

## Measured tradeoffs

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

This is a correctness/scalability change with a measured small-graph CPU cost,
not a universal optimization. Efficient concurrent collection, generic object
lifetime, ordinary runtime integration and physical-platform benchmarks remain
open. No EnterpriseOrg server was used.

Graphify refresh was attempted and stopped at its 35-second budget. It does not
provide semantic verification for this PBLang kernel source.
