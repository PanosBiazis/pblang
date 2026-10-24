# Kernel heap regression coverage and rejected optimization experiments

This slice adds reproducible allocator tests and benchmarks in PBLang. **No
allocator or collector implementation change is shipped.** Several candidate
optimizations passed correctness checks but did not demonstrate a performance
benefit. The original `stdlib/kernel/heap.sapp` was restored byte-for-byte.
No runtime limit, permission, lock, persistent metadata or foreign code was added.

## New regression coverage

[The kernel fixture](../examples/kernel_heap_merge.sapp) exercises all 120 orders
of freeing five differently sized blocks, with/without intervening live guards and
with/without cursor reuse: 480 cases. After each free it checks total free bytes,
address ordering, extent bounds and absence of adjacent unmerged nodes. It also
verifies live guard values, the returned cursor and subsequent full-arena
allocation. It passes with the unchanged allocator on qemu64 and 486.

The fixture also passed against each explored candidate. Eight existing kernel
fixtures passed on baseline and measured candidate with both CPUs: free cursor,
cycles/OOM, generation wrap, interrupts, live sweep, leaf queue, scratch reuse
and rootless reclamation. Candidate two-vCPU runs passed on qemu64 and Pentium III.
These are QEMU kernel runs, not guest-OS or physical-device certification.

Twenty-six semantic suites pass on the final unchanged allocator plus new tools,
covering kernel translation, runtime/resource/lifetime checks and platform/device/
Android emulators. Isolated builds and PBLang-native portable lock publication
pass. Kernel-aware diagnostics report zero issues on heap, fixture and both new
benchmarks. Both PBLang generators pass native lint. Ordinary host lint does not
resolve freestanding-only intrinsics; kernel-aware diagnostics are the applicable
source gate. An initial diagnostic harness counted fields of the result map;
it was corrected to inspect its diagnostics array.

[Validation evidence](evidence/kernel-heap-merge-validation-2026-09-22.json)
separates experimental-candidate results from final-source validation. The general
closure diagnostic was rerun and still differs: sibling scalar closures produce
`[1,1]` on the retained runner and `[1,0]` on the self-hosted frontend.

## Reproduction

Use a clean isolated package with the retained executable, run `pblang build`,
then publish a portable dependency lock with `pblang run tools/pblock.sapp --allow-all`.
Generate one of `fixture`, `merge`, `fragmented`, `rooted`, or `mixed`:

```
pblang run tests/tooling/kernel_heap_merge_generate.sapp --allow-all -- fixture build/heap-fixture
```

The output parent must already exist. The tool emits C and a linker script, but
never compiles or executes them. It rejects unknown workload names before writing
output. The four generated benchmark sources reproduce the measured baseline
sources byte-for-byte. `rooted` lengthens the existing rooted-ring workload from
50,000 to 500,000 passes, identically for both variants.

Compile the generated C with `-m32 -O2 -march=i386 -fno-toplevel-reorder
-ffreestanding -nostdlib -fno-pie -mgeneral-regs-only`, and link with
`ld -m elf_i386 -T <generated.ld>`. The script aligns `.bss` to 4096. Existing
linker RWX warnings remain unresolved. Run only inside disposable QEMU with
TCG, 32 MiB, no disk/network/display, one vCPU and ISA debug exit at port 0xf4;
only exit 85 is success. A 30-second external test timeout is not a language limit.

[The separate operation probe](../tests/tooling/kernel_heap_merge_probe_generate.sapp)
accepts a selected heap source and output C path. It instruments `peek32` only
inside the free-list function, using fixture-owned low RAM. It is not part of
timed binaries and must never run on the host or a real device.

## Measured read-reuse candidate: not shipped

The smallest candidate cached the predecessor size under the existing shared
lock, removing its repeated volatile read during backward coalescing. The exact
patch is recorded in [raw timing evidence](evidence/kernel-heap-merge-cost-2026-09-22.json).
The operation probe observed 16 -> 14 reads for three ascending frees; an isolated
free remained 2 -> 2. The number and order of stores were unchanged.

Two warmups and seven measured processes per variant alternated order. GNU time
records process user+system CPU, wall time and peak RSS. Nearest-rank p95 is the
largest of seven samples. No builds, tests or Graphify ran alongside timing.

| Workload | CPU median / p95 before, s | CPU median / p95 candidate, s | RSS median before / candidate, KiB |
| --- | ---: | ---: | ---: |
| Merge, 1,000,000 rounds | 0.76 / 0.91 | 0.78 / 0.93 | 59,260 / 59,384 |
| Fragmented middle-block reuse, 1,000,000 rounds | 0.27 / 0.28 | 0.27 / 0.28 | 59,256 / 59,256 |
| Rooted ring, 500,000 collections | 0.90 / 1.06 | 0.91 / 1.17 | 59,212 / 59,360 |
| Mixed live/garbage, 100,000 rounds | 0.78 / 0.79 | 0.78 / 0.80 | 59,372 / 59,212 |

The merge median increased approximately 2.6%; the rooted median increased 1.1%.
Two other medians were unchanged. These short, variable process measurements do
not establish a speedup or RAM saving. Therefore the candidate was not retained.
The prior rooted-collector overhead is not claimed to be fixed.

An earlier deferred-header-write candidate reduced the three-free probe from
9 to 6 writes but raised merge median CPU from 0.74 to 1.09 seconds and mixed-GC
median from 0.77 to 0.81 seconds. [Its separate samples](evidence/kernel-heap-merge-rejected-writes-2026-09-22.json)
are not combined with the later experiment. Two alternative layouts also failed
exploratory timing checks. All experimental source changes were discarded.
Fewer source-level memory operations alone do not imply faster generated code.

## Contract review and remaining work

The complete heap allocator and collector callers were reviewed. The existing
shared lock spans reads, writes and free-cursor reuse; direct API callers must
provide a live pointer from the correct arena, exactly once, with coherent
metadata. Arbitrary pointers, double frees, forged headers, uncooperative FFI/DMA
and NMI/fault reentry remain outside this raw API's validation contract. This
slice neither weakens nor certifies those boundaries.

Large collection pauses, general closure semantics, runtime/FFI/polyglot ownership
and wider platforms remain open. No collection-pause bound, streaming benefit,
GPU/NPU gain, disk reduction or universal resource saving is established.
The kernel modules remain absent from the retained PB-MCP desktop package;
its installation is unchanged. No external corporate server was used.

See [1.0 readiness and planning estimate](STABLE_1_0_READINESS.md) for release
criteria, remaining blockers and the assumptions behind calendar estimates.

Graphify AST refresh exceeded its 35-second bound; its owned process group was
terminated and no member remained. It does not provide semantic `.sapp` coverage.
