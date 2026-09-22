# Optional kernel recovery under memory pressure

`stdlib/kernel/gc_low_memory.sapp` provides three PBLang kernel APIs. Concatenate
it after `kernel/heap.sapp` and `kernel/gc.sapp` when selecting this capability.
It is **not enabled by default** in the existing collector, runtime or PB-MCP.
The ordinary `gc.sapp` implementation remains byte-for-byte unchanged.

| API | Behavior |
| --- | --- |
| `kernel_gc_collect_low_memory(g)` | Explicit scratchless collection. Returns 0 on success, -1 if an owned node branches with roots registered or count admission fails. |
| `gc_alloc_low_memory(g, children)` | First attempts normal allocation. A valid failed request with automatic collection enabled triggers scratchless collection and one allocation retry. Returns the object or zero. |
| `gc_alloc_rooted_low_memory(g, children)` | Same recovery, with atomic root publication before unlock and rollback if root registration fails. |

Thus recovery is automatic for a program choosing the new allocator APIs, while
explicit collection is available for diagnostics. `gc_set_auto(g,0)` disables
allocator recovery. Invalid child counts are rejected before collecting. The
existing maximum comes from the 32-bit allocator ABI, not a new application cap.
Unrooted results have the same publication obligations as ordinary `gc_alloc`;
prefer the rooted variant when another CPU or interrupt may collect.

## Reachability and failure contract

With registered roots, the collector first checks every active owner. Each must
be a leaf or have one child. Admission checks four owners per loop iteration and
then checks the remaining zero to three owners. No owner is skipped and no mark
is changed before the entire admission pass succeeds. It then follows chains
from roots without a work queue or recursive calls.
Generation marks terminate self/mutual cycles and avoid repeating shared tails or
duplicate roots. Unreachable objects are swept with the existing free-list cursor.
No scratch allocation, persistent cache or extra object metadata is added.

With roots registered, any branching owner rejects the operation before changing
marks, even if that owner is unreachable. With **zero registered roots**, all
owned objects are unreachable under the existing explicit-root contract. That
case delegates to the ordinary collector's scratchless rootless path under the
same lock, without checking topology or marks. Branching cycles are therefore
supported when rootless, including exhausted-heap allocator recovery. Rooted
arbitrary graphs still require the ordinary collector and its scratch space.
Accepted traversal advances the generation before marking. A count-admission
failure after partial marking returns -1 without sweeping, changing ownership
entries, resetting bytes-since-collection or counting a success. Retry advances
again; generation wrap clears owned marks before reusing generation one.

One shared memory lock covers topology checks, marking, sweep, allocator retry
and root publication. Cooperating edge/root mutation cannot interleave between
inspection and use. There are no callbacks, foreign calls or temporary unlocks.
A failed root-table expansion removes and frees the unpublished object once,
matching the existing rooted allocator contract. The base kernel assumptions
still apply: valid owned pointers/metadata and explicit root publication. This
is not protection against arbitrary address corruption, foreign/DMA writes or
noncooperating code. Allocation counters are not live-memory measurements.

## Why the default was preserved

General adaptive borrowing and automatic fallback integration were tested and
rejected for default use. In the last integrated candidate, the six paired
workloads were mostly unchanged but mixed allocation/sweep rose from 0.77 to
0.82 CPU seconds and rootless allocation/sweep from 1.38 to 1.49 seconds. Some
p95 values also worsened. Keeping that candidate would introduce new regressions.

A cold fallback call also increased the ordinary worker's local stack reservation
from 40 to 60 bytes in one generated i386 build. Separating dispatch restored the
40-byte worker but did not remove the measured regressions. All exploratory
samples are separate from measurements of the retained optional APIs. This is
not evidence of a universal compiler, CPU, RAM or latency improvement.

## Validation and reproduction

`examples/kernel_gc_low_memory.sapp` checks collection of rooted cycles in an
exhausted arena; duplicate roots/shared cycles; unchanged ownership; partial mark
failure and retry; generation wrap; unreachable leaf reclamation; automatic
allocator recovery; disabled-auto and invalid-count rejection; unsupported
unreachable branches; and repeated root-publication rollback with no free-byte
loss. An arena with no free block is checked explicitly, not inferred from RSS.

The follow-up fixture checks 18 placements of unsupported branches, covering
each admission batch position and tail, including short tables. Rejection at
generation wrap leaves all marks, ownership, free bytes and counters unchanged.
After removing roots, those same graphs are reclaimed, followed by an empty
collection. Rootless branching recovery in an exhausted heap publishes the new
root atomically. The saved-table comparison was corrected to inspect exactly
129 entries, matching its 516-byte allocation, instead of reading a 130th entry.

The optional rooted allocator is also substituted in the existing SMP fixture
and four-thread native harness. The native test retains the existing checked
low-address host ABI and fake interrupt-state seam; it is not a physical-kernel
proof. Source checks use kernel-aware diagnostics. Retained CLI lint does not
recognize kernel intrinsics; existing linker RWX warnings and the i386 QEMU
qemu64 long-mode CPUID warning remain toolchain limitations.

Use an isolated package with identical retained executable/compiler sources.
Build, then publish a repository-relative lock with `tools/pblock.sapp`. Emit the
concatenated kernel sources with `compiler/ctranspile.transpile_kernel_int`.
The comparison harness prepends heap, GC, interrupts and timer modules, adding
`gc_low_memory.sapp` only when testing these optional APIs. Use the compile/link
settings in [kernel lifetime](KERNEL_GC_LIFETIME.md#reproduce), align `.bss` to
4096 on both sides, and run QEMU TCG with no disk/NIC/display/monitor. Timing
uses two warmups, seven samples, alternating order, GNU time user+system CPU,
peak QEMU RSS and nearest-rank p95. It is not a per-collection pause measurement.


[Initial validation](evidence/kernel-gc-low-memory-validation-2026-09-22.json)
records 26 passing suites, two low-memory QEMU runs (qemu64/486), two optional
allocator SMP runs (qemu64/Pentium III, two CPUs), and three four-thread runs.
Three kernel-aware source checks report zero diagnostics. Six ordinary workloads
produce byte-identical C and ELF outputs versus the baseline when the optional
module is not selected. This verifies unchanged generated kernel execution for
those cases, not compiler startup cost or every application.

[Initial explicit-collection cost](evidence/kernel-gc-low-memory-cost-2026-09-22.json),
with both variants including the optional module, uses the same 129-node live
ring and 500000 successful collections. Ordinary versus scratchless CPU medians
are **0.98 / 1.16 s** (+18.4%), p95 **0.98 / 1.18 s**. Peak QEMU RSS medians are
59260 / 59268 KiB, effectively unchanged. The scratchless operation trades a
full topology scan for not requiring work-buffer memory; it is not a faster
replacement when ordinary scratch is available. Allocator-wrapper cost was not
separately benchmarked. The exhausted-arena fixture proves the capability gain.

[Discarded integration measurements](evidence/kernel-gc-low-memory-exploratory-2026-09-22.json)
retain raw samples and patches separately; none of those default integrations
was shipped.

## Batched admission and rootless recovery follow-up

[Current validation](evidence/kernel-gc-low-memory-tuning-validation-2026-09-22.json)
records 26 passing related suites, the expanded low-memory fixture on qemu64 and
486, two-CPU SMP runs on qemu64 and Pentium III, and three four-thread native
runs. Unlike the initial parallel fixtures, these also request explicit
scratchless collection while published roots are shared. Two kernel-aware source
checks have zero diagnostics. Retained CLI lint still rejects known kernel
intrinsics/externs; it is not reported as passing. Isolated build and native
portable-lock publication passed. The ordinary collector, allocator, compiler
VM and runtime GC modules are unchanged.

[All timing samples](evidence/kernel-gc-low-memory-tuning-cost-2026-09-22.json)
include an initial two-warmup/seven-sample alternating run and a follow-up with
two warmups and fifteen measured samples per variant in rotating order. The
follow-up includes ordinary collection in the same current build as a control.
Both variants execute identical workloads and verify successful exits.

| Confirmation workload | Before CPU median / p95 | After CPU median / p95 | Median change |
| --- | --- | --- | --- |
| 129-node live ring, 500000 collections | 1.16 / 1.41 s | 1.10 / 1.70 s | 5.2% lower |
| 128 live nodes + 16 garbage leaves, 100000 rounds | 0.85 / 1.37 s | 0.86 / 1.19 s | 1.2% higher |

Ordinary collection of the ring costs 0.97 s median / 1.17 s p95. Thus the
optional collector still costs **13.4% more CPU** than ordinary collection when
scratch space is available. The initial ring improvement was 20.3%, but did not
persist at that size in the longer confirmation; both runs are retained. The
initial rootless workload improved from 1.51 to 1.48 s median (2.0%), with p95
1.64 / 1.49 s. It was not rerun in the confirmation.

Confirmation peak QEMU RSS medians are 59120 / 59244 KiB for the ring and
59196 / 59180 KiB for mixed allocation: no general RAM reduction is established.
The ring ELF grows from 13800 to 13980 bytes (+180); no extra persistent object
metadata is introduced. The higher ring p95 is retained as a limitation, not
hidden behind the better median. These are process-level QEMU measurements on
a non-isolated laptop, not worst-case collection pause bounds or physical-device
measurements. Default integration remains deferred.

## Avoiding a second collection during the recovery retry

After successful scratchless recovery, the optional allocator now retries the
allocation without triggering another automatic collection. With threshold zero,
the old retry could immediately walk the graph a second time. A pressure-cycle
probe verifies **20000 collections before versus 10000 after** for the same
10000 replacement allocations, live roots and garbage workload.

Only the retry temporarily sets the automatic-trigger field to zero. It restores
one unconditionally after the internal allocator returns, including a zero
allocation result, before root registration or unlocking. The caller already
holds the shared memory lock and the internal allocator has no callbacks or
exception exits in the kernel target. All allocation-size, table-growth, heap
and root-registration checks remain active. This is not a persistent policy
change or a suspension of memory validation. Raw invalid-pointer faults and
noncooperating memory writers remain outside the existing kernel contract.

`examples/kernel_gc_recovery_retry.sapp` covers rooted/unrooted allocations,
zero and high thresholds, preservation of the legacy negative-threshold bit
pattern, an allocation too large for the recovered space, the next ordinary
allocation obeying the original policy, and 100 root-publication rollbacks with
no free-block loss. Its post-recovery collection-count assertion fails on the
baseline and passes after the change on qemu64 and 486. The negative threshold retains the current i386
unsigned comparison behavior; this change does not redefine that API.

[Retry validation](evidence/kernel-gc-retry-validation-2026-09-22.json) records
26 passing related suites, baseline failures and corrected QEMU runs on qemu64
and 486, the existing low-memory fixture, exact collection-count probes,
two-CPU SMP checks and three four-thread checks. Concurrency checks exercise the
optional allocation entry but do not force every memory-pressure interleaving.
Three kernel-aware diagnostics checks and isolated builds pass; native CLI lint
retains its kernel-intrinsic limitation. Portable locks were regenerated only in
isolated packages.

[Retry measurements](evidence/kernel-gc-retry-cost-2026-09-22.json) retain two
warmups and fifteen alternating samples per variant for the 10000-cycle pressure
workload and explicit-collection ring/mixed controls. A longer confirmation uses
50000 identical pressure cycles on both variants, two warmups and nine samples.

| Workload | Before CPU median / p95 | After CPU median / p95 |
| --- | --- | --- |
| Recovery, 10000 cycles | 0.40 / 0.42 s | 0.39 / 0.41 s |
| Recovery, 50000 cycles | 3.03 / 3.54 s | 2.86 / 4.08 s |
| Explicit ring control | 0.87 / 1.78 s | 0.85 / 1.76 s |
| Mixed control | 0.76 / 1.23 s | 0.77 / 1.26 s |

The longer recovery median improves 5.6%, but its p95 worsens. The mixed median
costs 1.3% more. The explicit collector's algorithm was not changed, so the ring
control is not evidence that its earlier overhead was eliminated. Timing on this
non-isolated laptop varies; these are QEMU process measurements, not bounds on
individual collection pauses. Confirmation peak RSS medians are 59040 / 58960
KiB, essentially unchanged. The recovery ELF grows by 48 bytes, 14316 / 14364;
there is no new per-object metadata or persistent resource usage. Default
integration and general CPU/RAM savings are not claimed.

This removes the redundant **post-recovery** collection only. The initial normal
allocator may still attempt collection before allocation fails; if that pass
already reclaimed everything, the later fallback can remain redundant. That
separate case is not optimized here. Default runtime/VM/FFI behavior is unchanged.

## Remaining work

Default automatic integration still needs an acceptable performance result.
The earlier bounded VM numeric/try-catch overheads were not changed or resolved.
Long topology/mark/sweep passes still hold the shared lock and mask local
interrupts; there is no incremental collection or real-time guarantee. Retained
host cycles, complete runtime/FFI/polyglot integration and wider physical-platform
validation remain open. The installed desktop PB-MCP does not contain these
kernel modules or the compiler VM. It was inspected, not updated; no live-view
improvement was measured. PBLang remains alpha.

Graphify update was attempted with a 35-second local watchdog and did not
complete. Its owned process group was stopped; no owned test, benchmark or
analyzer processes remained at closeout. The 212 unrelated local files were
preserved. No Specisoft server or installed desktop bundle was modified.
