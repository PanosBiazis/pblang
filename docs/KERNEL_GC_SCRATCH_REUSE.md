# Borrow spare object-table space for collection work

The PBLang kernel collector now uses existing unused ownership-table slots for its
work queue when the entire queue fits without overlapping active entries. With
`n` owned entries and table capacity `capacity`, borrowing requires
`n <= capacity / 2`. The borrowed range is the **last n slots**, starting at
`objects + (capacity - n) * 4`; the active range remains `[0,n)`.

This is a dynamic storage choice, not a limit on programs or object counts.
Tables continue to grow as before. If spare capacity is insufficient, the collector
uses its existing separate `n * 4` allocation. Rootless/empty collection needs no
work buffer. No extra persistent metadata, scratch cache or larger table is added.
The choice applies automatically to both explicit and automatic kernel collection.

Borrowing can complete collection even when no free heap block remains. It avoids
both an allocation and its matching free on that path. It does not shrink the
ownership table, reduce every application's memory usage or make collection
incremental. Slots beyond the active count are internal storage, not an application
API or additional roots.

## Ownership and failure handling

A separate local `allocated_work` pointer records only a separately allocated work
block. Success and each partial-mark failure release that pointer when nonzero;
they never free the borrowed interior pointer. This distinction prevents metadata
corruption and double-free. Scratch OOM on the separate-allocation path still
returns -1 before changing generation or marks. Partial-mark failure still skips
sweep and the successful-collection increment; advancing generation prevents a
later retry from accepting incomplete marks.

The same shared memory lock spans admission, marking and sweep. Cooperating
allocation, root mutation and table growth cannot run while the borrowed range is
in use. Traversal completes before compaction; compaction writes at most the old
active prefix and therefore cannot overlap the borrowed range. Later allocations
write each new active slot before increasing the count. Table growth copies only
active entries, after any formerly borrowed slots in that prefix have been
replaced by actual object registrations.

The existing marked-count bound still limits queue writes to at most n slots.
The checked condition makes `capacity - n >= n`, including the exact half-full
boundary. Capacity and address arithmetic rely on the existing valid 32-bit arena
and table-growth contract. This is not a validator for arbitrary pointers, forged
capacities or arbitrary metadata corruption. Direct writes, unsynchronized native
code, DMA and NMI/fault reentry remain outside the raw kernel API. Failure-injection
fixtures exercise specified damaged-count cases, not all possible corrupt states.

## Validation and memory-pressure regression

[Recorded checks](evidence/kernel-gc-scratch-validation-2026-09-21.json):

- 21 related semantic suites, including retained-runtime refusal, resource scopes
  and self-hosted exception/lifetime behavior.
- Seven kernel fixtures on both qemu64 and 486: cycles/OOM, generation retry/wrap,
  IRQ ownership, live sweep, cursor reclamation, leaf admission and scratch reuse.
- Two-vCPU tests on qemu64 and Pentium III, including interruptible/masked lock
  waiting and 2000 rooted lifetimes per CPU.
- Three native four-thread runs totaling 24000 rooted lifetimes.
- Three touched kernel files with zero diagnostics against committed diagnostics,
  new generator lint, isolated build and PBLang-native portable lock publication.

The new `examples/kernel_gc_scratch_reuse.sapp` exercises:

- 128 rooted self-cycles in a 256-slot ownership table, filling the entire upper
  half with queue entries while preserving every active object/root entry.
- Exhausting the fixture's free heap, then 100 successful borrowed collections and
  generation wrap with the free-list head still zero.
- Automatic allocation with a full heap: a removed self-cycle is collected and
  its space reused for a newly rooted replacement.
- A borrowed-buffer partial-mark failure, with no interior-pointer free or success
  increment, followed by a successful retry.
- Moving from 128 to 129 owned objects, which requires separate scratch, then
  reclaiming the extra object and returning to borrowing.
- Ownership-table growth to 512 slots, stale-slot overwrite, later root publication
  and final reclamation.
- A separate-buffer partial-mark failure with unchanged total free bytes, followed
  by a successful retry.

The final pressure fixture fails with baseline `09d897d` (exit 3) and passes with
the new collector (exit 85). Its arena is 16 KiB: an initial 8 KiB test setup could
not hold both metadata tables during the growth test. The real scratch-OOM test
remains separate in `kernel_gc_cycles.sapp`: 129 entries in a 256-slot table require
separate scratch; exhausting that 4 KiB arena must still return -1 without sweep.
This keeps explicit OOM coverage rather than treating every full-heap case as a
successful borrowed collection.

The new PBLang test generator `tests/tooling/kernel_gc_scratch_probe_generate.sapp`
counts allocator calls and requested payload bytes during one collection. Its
counters are test-only low RAM at 0x7000..0x7007, read through QEMU debug port 0xe9.
Both qemu64 and 486 report:

| Owned objects / capacity | Baseline calls / requested bytes | New calls / requested bytes |
| --- | --- | --- |
| 128 / 256 | 1 / 512 | 0 / 0 |
| 129 / 256 | 1 / 516 | 1 / 516 |

These are requests during collection, excluding setup and allocator headers.
They prove that the eligible path avoids additional heap allocation; they are not
whole-process peak-memory measurements.

## Timing and reproduction

[Raw timing samples](evidence/kernel-gc-scratch-benchmark-2026-09-21.json) use the
same current heap/compiler and identical workloads for baseline and new GC. Each
variant has two warmups and five measured runs in alternating order. GNU time
records QEMU user+system CPU, wall time and peak RSS. Nearest-rank p95 of five
samples is their maximum, not a scheduling or tail-latency guarantee.

| Workload | CPU median before / after | CPU p95 before / after | Peak RSS median KiB before / after |
| --- | --- | --- | --- |
| 128-object ring, borrowing eligible, 500000 passes | 1.09 / 1.01 s | 1.11 / 1.03 s | 54596 / 53952 |
| 129-object ring, separate scratch, 500000 passes | 1.07 / 1.10 s | 1.08 / 1.13 s | 55292 / 55208 |
| 128 live objects plus 16 garbage leaves, 100000 passes | 0.87 / 0.87 s | 0.89 / 0.87 s | 55308 / 55368 |

The eligible ring uses about 7% less CPU; the separate-buffer ring uses about 3%
more; the mixed median is unchanged. There is no universal speed or RAM saving.
ELF sizes increase by 100–104 bytes. These process totals include setup and emulator
behavior, not physical-device IRQ pauses, power or GPU/NPU behavior.

For both variants use `bench/kernel_gc_collect.sapp` with both 50000 literals
replaced by 500000. The fallback ring also changes `i < 128` to `i < 129` and
`gc_live(g) != 128` to `gc_live(g) != 129`, identically for both variants. Use the
existing mixed benchmark unchanged. Concatenate current heap + selected GC +
fixture, call `compiler/ctranspile.transpile_kernel_int`, and compile with
`-m32 -O2 -march=i386 -fno-toplevel-reorder -ffreestanding -nostdlib -fno-pie
-mgeneral-regs-only`. Link with `ld -m elf_i386` and `kernel_linker_script()` with
`ALIGN(4096)` before `.bss` in both builds to control the known QEMU code/data-page
artifact. See [the earlier reproduction](KERNEL_GC_FREE_CURSOR.md) for full commands.

Run single-vCPU qemu64/TCG with 32 MiB, no disk/network/display and ISA debug exit.
Use a 25-second supervisor bound; only exit 85 is success. Run the allocator probe
separately, never as the timing workload:

```
pblang run tests/tooling/kernel_gc_scratch_probe_generate.sapp -- selected-gc.sapp 128 probe.c
```

Compile/link as above, adding `-debugcon stdio` to QEMU to read the two counters.
Repeat with 129 and with the 486 CPU model. The generator's two sizes are test
cases, not a product limit. These are QEMU-only kernels, not host executables or
installed guest OS tests; existing linker RWX warnings remain unresolved.

## Remaining work

Large traversals, generation wrap, fallback scratch allocation and unfavorable
sweep orders can still cause pauses under the same global lock with local IRQs
masked. Incremental collection needs a root/mutation barrier or another proved
snapshot contract. No hard real-time, bounded waiting or fairness guarantee is
provided. Broader SMP topologies, per-arena concurrency and physical-platform
acceptance remain open.

Runtime/FFI/PB-MCP/polyglot integration also remains open. Destructive retained-host
collection remains disabled because roots across nested VMs cannot all be proven.
Borrowing precise kernel metadata does not supply those missing host roots or
safe ownership of foreign resources. No installed PB-MCP transport or Specisoft
server was used or modified.

Graphify updates exceeded their 35-second budgets; the owned process groups
were stopped. Graph output is not semantic coverage of these PBLang modules.

For a later comparison that retains scratch reuse and removes redundant
allocator successor recovery, see [resource-cost follow-up](RESOURCE_COST_FOLLOWUP.md).
