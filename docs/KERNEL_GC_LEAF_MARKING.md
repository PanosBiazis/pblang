# Mark leaves without queueing a second visit

The PBLang kernel collector now marks objects with zero children immediately and
omits them from the traversal queue. Both root admission and branching-child
admission use this path. Single-child chains retain their existing direct
traversal. Objects with children are still queued and traversed; cycles and shared
references are still terminated by the current mark generation.

This automatically applies to explicit and automatic kernel collection, with no
new public API, configuration requirement, object-count limit or persistent heap
metadata. The existing temporary work allocation remains `owned_count * 4` when
roots and owned objects are present. Fewer queue writes do **not** mean that this
scratch allocation has been reduced. A leaf-only rooted graph can still encounter
scratch OOM under the unchanged allocation policy.

## Invariants and review

Marked count and queue length now intentionally differ. Every new mark increments
`marked`, including leaves; only objects needing traversal increment `tail`.
Admission therefore checks `marked >= owned_count`, including during root marking.
This preserves `tail <= marked <= owned_count` and the work-table capacity bound.
Using queue length for root admission would silently miss an inconsistent count
when roots are leaves. Duplicated roots and shared children increment neither
counter after the first mark in the same generation.

The graph, child counts and mark bits remain under the same global memory lock
from enumeration through sweep. No check/use interval, callback, cached leaf
classification or lock release was introduced. Supported `gc_set_child` calls
cannot resize the object's child table. Valid zero-child objects therefore have
nothing further to traverse during this locked pass. Nonzero child counts retain
the previous traversal behavior. Valid owned pointers and unmodified internal
metadata remain preconditions of this raw kernel API; it does not validate
arbitrary addresses, direct memory writes or foreign heaps.

Scratch OOM still happens before generation/mark changes. Partial-mark failure
still releases scratch, skips sweep, retains the successful-collection counter
and advances generation for a safe retry. Generation wrap, all-live sweep skipping,
free-list cursor reclamation, root publication, IRQ state and try-collect busy
behavior are unchanged. Rootless collection still requires no scratch.

## Validation

[Current results](evidence/kernel-gc-leaf-validation-2026-09-21.json) include:

- 21 related semantic suites, including retained-runtime refusal and self-hosted
  resource/exception/lifetime contracts.
- Six kernel fixtures on qemu64 and 486, covering cycles/OOM, generation wrap and
  retry, IRQ/root ownership, live/mixed sweeping, cursor reclamation and leaf
  admission; twelve successful single-vCPU runs.
- Two successful two-vCPU runs on qemu64 and Pentium III, including interruptible
  and masked lock waiting and both CPUs' rooted lifetimes.
- Three successful native four-thread runs, totaling 24000 rooted lifetimes.
- Three touched kernel files with zero diagnostics against committed diagnostics,
  probe-generator native lint, isolated build and portable lock publication.

`examples/kernel_gc_leaf_queue.sapp` combines leaf roots, duplicate roots, shared
leaves, a single-child bridge and a parent cycle. It checks repeated collection,
removing a root while a child remains reachable, later branch loss and final
reclamation. A fixture-only inconsistent owned count with two leaf roots must
fail without recording success; restoring the count permits retry. Leaf roots
also survive generation wrap. Attempting to attach a child to a leaf is rejected.

A deliberately incorrect test mutation that substitutes queue length for marked
count in root admission fails this fixture with exit 3. The final implementation
passes with exit 85. This is regression-test sensitivity, not a claim that the
previous release had that mutated bug.

The PBLang generator `tests/tooling/kernel_gc_leaf_probe_generate.sapp` instruments
queue writes in generated test source only. With a parent and 256 leaf children,
baseline `89467c3` writes 257 queue entries and the new collector writes one. Both
qemu64 and 486 produce those counts and retain all 257 objects. The probe reserves
low RAM 0x7000 and emits through QEMU port 0xe9; it is a single-CPU test kernel,
not a host executable or an instrumentation feature enabled in applications.

## Measurements and reproduction

[Raw samples](evidence/kernel-gc-leaf-benchmark-2026-09-21.json) compare the selected
GC versions with identical current heap/compiler/workloads. Two warmups and five
measured samples alternate order per variant. GNU time records QEMU user+system
CPU, wall time and process peak RSS. Nearest-rank p95 of five samples is the
maximum; this small sample is descriptive, not a latency guarantee.

| Workload | CPU median before / after | CPU p95 before / after | Peak RSS median KiB before / after |
| --- | --- | --- | --- |
| One parent, 256 leaves, 200000 collections | 1.90 / 1.05 s | 2.06 / 1.08 s | 57140 / 57208 |
| 128-object ring, 500000 collections | 1.06 / 1.07 s | 1.07 / 1.08 s | 59340 / 59108 |
| 128 live objects plus 16 garbage leaves, 100000 passes | 0.88 / 0.89 s | 1.17 / 1.03 s | 59188 / 59200 |

CPU falls about 45% in the leaf-heavy workload. The controls have roughly 1%
higher medians, with run variation; there is no demonstrated universal speedup.
RSS is nearly unchanged, with no demonstrated general RAM saving. ELF files are
48 bytes larger in these builds. These are process totals including setup, not
physical-device pause, power, GPU/NPU or installed-application measurements.

Use `bench/kernel_gc_leaf_mark.sapp` unchanged, the existing mixed fixture
unchanged, and the existing ring fixture with both 50000 literals replaced by
500000 identically for both variants. Concatenate current heap + selected GC +
fixture and translate with `compiler/ctranspile.transpile_kernel_int`. Use
`kernel_linker_script()` with `ALIGN(4096)` before `.bss` for both builds, controlling
the known QEMU same-page code/data artifact. Compile with `-m32 -O2 -march=i386
-fno-toplevel-reorder -ffreestanding -nostdlib -fno-pie -mgeneral-regs-only`; link
with `ld -m elf_i386`. See [the preceding report](KERNEL_GC_FREE_CURSOR.md) for
kernel commands and the aligned linker setup.

Time single-vCPU qemu64/TCG with 32 MiB, no disk/network/display and ISA debug exit.
Use a 25-second supervisor bound; only exit 85 is success. For the separate queue
probe, not included in timing samples:

```
pblang run tests/tooling/kernel_gc_leaf_probe_generate.sapp -- selected-gc.sapp probe.c
```

Compile/link the generated C as above and add `-debugcon stdio` to read its count.
Repeat the probe with the 486 CPU model and 32-bit QEMU executable. Existing linker
RWX warnings remain unresolved; these are QEMU kernel fixtures, not guest OS or
physical-device acceptance tests.

## Remaining integration and pauses

The collector still holds one lock with local interrupts masked throughout its
work. Large child arrays, scratch allocation, generation wrap and other sweep
orders can still pause execution. Incremental collection needs a root/mutation
barrier or another proved snapshot contract before releasing this lock mid-pass.
No hard real-time, scheduling fairness or bounded waiting guarantee is provided.
Wider SMP topologies, per-arena concurrency and physical-platform tests remain open.

General runtime/FFI/PB-MCP/polyglot integration remains open. The retained-host
runtime contract still cannot prove all roots across nested same-thread VMs, so
destructive host collection stays unavailable. Leaf skipping in a precise kernel
graph does not provide those missing roots or safe foreign-object ownership.
No installed PB-MCP transport or EnterpriseOrg server was used or modified.

Graphify update exceeded its 35-second budget; the owned process group was
stopped. Its graph does not establish semantic coverage of these PBLang modules.
