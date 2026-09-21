# Direct reclamation when the kernel collector has no roots

The precise PBLang kernel collector now takes a dedicated path when its registered
root count is zero. It returns every active owned object to the existing allocator
without reading object mark words, traversing child edges or compacting survivors.
On generation wrap it also omits the full mark-reset pass: there are no surviving
objects whose old marks could match a reused generation. The generation still
advances, including empty collections; collection count and bytes-since-collection
retain their previous successful-pass behavior.

This applies automatically to manual, automatic and admitted try-collect calls.
No caller option, object-count limit, persistent metadata, new allocation or Rust
code was added. Rooted collection, scratch admission and failed-mark handling keep
their existing path. The allocator still performs real frees and coalescing; the
collector does not discard or reset the arena.

## Ownership and concurrency review

Root count, ownership entries and reclamation stay under the same shared memory
lock. No callback or unlock was added between checking the root count and freeing
objects. The free-list cursor is private to the same free-only sequence and starts
at zero. Descending object addresses retain the allocator's existing safe restart.
After successful reclamation, the active count is zero; subsequent allocations
initialize their mark and active table entry before publishing the new count.

Only this collector's active ownership entries are freed. Heap allocations outside
that table remain intact, as tested with a separately allocated sentinel. The
contract still requires valid owned pointers, accurate metadata and explicit root
publication. Direct pointer writes, forged entries, cross-collector ownership,
uncooperative FFI/DMA and NMI/fault reentry are not validated by this raw API.
An unregistered reference does not keep an object alive. This optimization must
not be applied to the retained host's incomplete root inventory.

## Validation

The [new kernel fixture](../examples/kernel_gc_rootless.sapp) checks repeated
cyclic allocation/reclamation, duplicate root removal, descending ownership order,
exact free-byte recovery, an unrelated heap allocation, generation wrap with
objects and without objects, new-object mark initialization, subsequent rooted
survival and automatic collection before publishing the next allocation.
It passes on both baseline `3862a09` and candidate: these are preserved semantics.

[Validation evidence](evidence/kernel-gc-rootless-validation-2026-09-22.json)
records 26 semantic suites, including 17 emulator suites, retained-runtime refusal
and self-hosted resource/lifetime checks. Eight kernel fixtures pass on each of
qemu64 and 486 (16 executions), covering cycles/OOM, generations, interrupts,
mixed/live sweep, free cursors, leaf marking, scratch reuse and the new rootless
path. Two additional two-vCPU runs pass on qemu64 and Pentium III. Kernel-aware
diagnostics report zero issues for the collector, fixture and benchmark; the probe
generator passes native lint. Isolated builds and portable lock publication pass.

The separate [operation probe](../tests/tooling/kernel_gc_rootless_probe_generate.sapp)
instruments only the sweep's mark-comparison site in generated test source. A
single pass over 512 unrooted self-cycles performs **512 mark comparisons before
and zero after**, on both qemu64 and 486. The allocator still reads allocation
headers and free-list links; this is not a claim of zero memory access or zero
work. The probe is not included in timing runs and reserves low RAM only in its
disposable QEMU fixture.

## Reproduction and measurements

Use identical isolated baseline and candidate packages with the same retained
compiler, heap module, workloads and toolchain. Build and publish portable locks
before translation. Concatenate heap, selected collector and fixture, translate
with `compiler/ctranspile.transpile_kernel_int`, and align `.bss` to 4096 in both
linker scripts. Compile with `-m32 -O2 -march=i386 -fno-toplevel-reorder
-ffreestanding -nostdlib -fno-pie -mgeneral-regs-only`; link with `ld -m elf_i386`.
Existing linker RWX warnings remain unresolved.

Run qemu64/TCG with one vCPU, 32 MiB, no disk/network/display, and ISA debug exit.
Only exit 85 is success; a 30-second external test timeout is not a language limit.
Use two warmups and seven measured samples per variant in alternating order.
GNU time measures process user+system CPU, wall time and peak RSS. Nearest-rank
p95 is the maximum of seven observations. Tests, builds and Graphify must not run
alongside the measurements.

Workloads:

- `bench/kernel_gc_rootless.sapp`: 10,000 passes of 512 unrooted self-cycles,
  reusing the same collector and arena without resets.
- Small rootless control: existing `bench/kernel_gc_mixed_sweep.sapp`, removing
  only `gc_add_root(g,first);` and changing expected live count from 128 to zero.
  It allocates 16 garbage leaves per pass for 100,000 passes.
- Rooted ring: existing `bench/kernel_gc_collect.sapp`, replacing both 50,000
  literals with 500,000, identically in both variants.
- Mixed control: existing `bench/kernel_gc_mixed_sweep.sapp` unchanged.

[Raw timing samples](evidence/kernel-gc-rootless-benchmark-2026-09-22.json):

| Workload | CPU median / p95 before, s | CPU median / p95 after, s | Peak RSS median before / after, KiB |
| --- | ---: | ---: | ---: |
| rootless_batch | 1.99 / 2.28 | 1.95 / 2.25 | 59028 / 59364 |
| rootless_small | 0.54 / 0.68 | 0.53 / 0.66 | 59216 / 59268 |
| rooted_ring | 0.92 / 0.93 | 0.93 / 0.95 | 59380 / 59268 |
| mixed | 0.84 / 0.86 | 0.82 / 0.83 | 59212 / 59400 |

Rootless medians decrease about 2%; mixed decreases about 2.4%, while the rooted
ring increases about 1.1%. These small changes and sample variation do not establish
universal acceleration. Peak RSS is nearly unchanged, with increases in several
workloads; no general RAM saving is established. QEMU totals include allocation
and emulation, not just time inside a collection critical section.

ELF byte sizes (before / after):

- rootless_batch: 11388 / 11496
- rootless_small: 11636 / 11744
- rooted_ring: 11656 / 11764
- mixed: 11656 / 11764

The executable size increase is part of the cost of the separate path. No
reduction of disk writes or temporary space was demonstrated.

## Remaining work

This removes unnecessary work from one phase and does not implement incremental
collection. Large freeing sequences, unfavorable free-list order, rooted marking,
scratch allocation and rooted generation wrap can still pause under the lock with
local interrupts masked. No bound on the largest pause, fairness or real-time
completion is established. QEMU process RSS is not guest live heap, device memory,
energy consumption or GPU/NPU utilization.

General closure-capture compatibility, runtime/FFI/polyglot ownership and wider
physical-platform testing remain open. Kernel ownership is not the retained VM's
ownership: these modules are absent from the installed PB-MCP desktop bundle.
That installation was not changed or restarted. No live-streaming acceleration
is claimed, and no Specisoft server was used.

A current closure diagnostic is retained in the validation evidence: two sibling
closures capturing the same scalar produce `[1,1]` on the retained runner and
`[1,0]` on the self-hosted frontend after one closure increments it. The latter
result reflects snapshot captures. Correcting this generally also requires
resolving repeated-declaration lifetime semantics; no closure behavior changed
in this kernel slice.

Graphify AST refresh exceeded its 35-second bound and its owned process group
was terminated. Its output does not establish semantic `.sapp` coverage.
