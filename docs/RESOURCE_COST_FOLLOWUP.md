# Resource-cost follow-up: preserve cleanup and reduce repeated work

This slice changes the PBLang self-hosted VM and the PBLang kernel allocator.
It does not remove automatic local cleanup, reduce safety checks, raise quotas,
add program-size limits or enable destructive retained-host collection. No Rust
or product polyglot code was added. Baseline is `a8f3345`.

## Implementation and safety review

`_runtime_tick` now reads stack and local-array lengths once per tick. It still
checks sticky failure first, then the step budget, charges the step, checks stack
and locals, updates peaks and validates the top value in the same order. These
arrays cannot change during this synchronous helper: no guest callback, yield or
foreign call is introduced. Lengths are not cached across instructions or frames.
This removes repeated host builtin calls without weakening mutation admission.

`_set_slot` now calls the value-budget helper only when a bounded runtime exists.
Previously the helper was called for ordinary execution and immediately returned.
With a runtime present, negative-slot, local-budget and value checks still precede
growth and assignment in their original order. Nil stores still clear dead locals;
shared values, captures, try/catch cleanup and original bytecode remain unchanged.

`_guarded_heap_free_from` keeps the successor found by its search instead of
replacing it with a sentinel and rereading the head/predecessor link. Empty lists,
head/tail insertion, ascending/descending cursors and forward/backward coalescing
retain the same behavior. No metadata, allocation or cached pointer outlives the
existing shared memory lock. There is no new check/use gap or free operation.
Raw pointer ownership, valid arena metadata, NMI/DMA/foreign writes and incomplete
host roots remain the existing boundaries; this is a focused review, not a claim
of comprehensive memory safety or malicious-code isolation.

The existing sweep probe now supports both old and new source shapes. It counts
the nonzero stopping successor in both versions, so its visit metric remains
comparable. The fragmented fixture reports 1984 visits for both; this change
removes duplicate reads/control work, not another traversal complexity factor.

## Measurements

[All samples](evidence/resource-cost-benchmark-2026-09-21.json) record two warmups
and seven measured processes per variant, alternating order. Identical retained
executable, compiler, source workloads, build options and dependency closures were
used. Only the selected product modules differ. No test suites or Graphify ran
alongside the timed processes. Other system load was not isolated. CPU is GNU
time user+system seconds; RSS is whole-process peak KiB. P95 is nearest rank, the
maximum of seven samples, not a tail-latency guarantee.

| Workload | CPU median before / after (s) | CPU p95 before / after (s) | RSS median before / after (KiB) |
| --- | --- | --- | --- |
| Kernel 128-object live ring, borrowed scratch | 0.90 / 0.93 | 1.34 / 1.20 | 54216 / 54492 |
| Kernel 129-object live ring, allocated scratch | 1.16 / 1.14 | 1.23 / 1.27 | 54256 / 54624 |
| Kernel 128 live + 16 garbage per pass | 0.94 / 0.88 | 1.26 / 0.94 | 54276 / 54588 |
| Kernel fragmented sweep | 0.19 / 0.19 | 0.22 / 0.21 | 55884 / 55908 |
| VM bounded numeric loop | 1.92 / 1.69 | 2.05 / 2.40 | 35696 / 35728 |
| VM ordinary numeric loop | 1.05 / 1.03 | 1.07 / 1.05 | 35596 / 35664 |
| VM numeric caught failures | 0.61 / 0.61 | 0.64 / 0.62 | 35844 / 35580 |
| VM numeric successful try bodies | 0.61 / 0.61 | 0.93 / 0.78 | 35716 / 35764 |
| VM 16 successful 2 MiB blocks | 0.65 / 0.63 | 0.85 / 0.74 | 35688 / 35708 |
| VM 16 failed 2 MiB blocks | 0.60 / 0.60 | 0.70 / 0.83 | 35688 / 35664 |

The bounded numeric median decreases about 12%; ordinary numeric decreases about
2%. Mixed kernel CPU decreases about 6%, allocated-scratch CPU about 2%, while
borrowed-scratch CPU increases about 3%. Fragmented sweep and numeric try/catch
medians are unchanged. Small differences and wide p95 variation must not be
presented as universal acceleration. In particular bounded-numeric p95 worsens.

No general RAM saving is demonstrated. QEMU RSS is emulator/process memory, not
GC live bytes. VM RSS also includes frontend compilation and the retained host.
No new persistent allocator storage was added; the four ELF files are each 76
bytes smaller in these builds. This does not establish less disk I/O, less
storage for all executables, lower power or GPU/NPU/peripheral savings.

The initial VM series is retained in the JSON: it tested length reuse before the
no-op value-guard call was removed. It is not the final result. Kernel binaries
were unaffected by the later VM-only adjustment and were not rebenchmarked.

## Reproduction

Build two disposable packages from the baseline, apply the selected module changes
to one, run retained `pblang build`, then publish each local lock using the
PBLang-native `tools/pblock.sapp` before timing. Do not benchmark import-hash
failures or include build/lock maintenance in timing. Source and module inspection
must use the same dependency closure; the dirty workspace diagnostics were not
used in these checks.

For VM measurements run the committed PBLang benchmark:

```
pblang run bench/selfhost_resource_cost.sapp -- bounded numeric
pblang run bench/selfhost_resource_cost.sapp -- plain numeric
pblang run bench/selfhost_resource_cost.sapp -- plain failed
pblang run bench/selfhost_resource_cost.sapp -- plain success
pblang run bench/selfhost_resource_cost.sapp -- plain blocks
pblang run bench/selfhost_resource_cost.sapp -- plain failed_blocks
```

Each invocation compiles and executes the same source and asserts its output.
The numeric workload has 20000 iterations. The failure/success loops have 1000;
block workloads have 16 distinct 2 MiB temporaries. The explicit benchmark budgets
are not new default application limits. Use a 30-second external test supervisor.

Kernel reproduction follows [scratch reuse](KERNEL_GC_SCRATCH_REUSE.md): 500000
ring collections, 128/129 nodes; unchanged mixed and fragmented fixtures. Compile
with `-m32 -O2 -march=i386 -fno-toplevel-reorder -ffreestanding -nostdlib -fno-pie
-mgeneral-regs-only`, link as elf_i386 with ALIGN(4096) before .bss for both builds,
and use qemu64/TCG, 32 MiB, no disks/network/display and ISA debug exit. Only exit
85 is success. Supervise each process for 30 seconds. Inspect CPU, wall, RSS and
ELF size; do not select only favorable samples.

## Which previously reported costs are addressed?

| Earlier report quoted by the user | Current evidence and remaining work |
| --- | --- |
| +3% allocated-scratch CPU, unchanged mixed workload | Current allocator removes redundant successor recovery; allocated-scratch and mixed medians improve versus the immediate baseline, with a borrowed-ring regression disclosed above. |
| About +1% in other checks; CPU unchanged / RSS +0.08% | These are historical, workload-specific observations. New controls are recorded here; small process differences do not prove a universal resource cost or saving. |
| QEMU RSS +4.4% in fragmented sweep | Guest allocation strategy is unchanged in this slice. Current fragmented RSS is nearly identical; this is not proof that all of the old QEMU increase has been eliminated. |
| +5% mixed collection CPU | Mixed workload is remeasured against the current baseline and improves. This is not a controlled comparison to the much older version in the original report. |
| +26% CPU during the first iterative collector work | [Original report](KERNEL_GC_LIFETIME.md) already records a substantial QEMU code/data-layout effect. Current comparisons align .bss identically; removing bounds or reverting to recursive marking would sacrifice correctness and capacity. |
| 54% lower failed-frame RSS, +3–4% numeric CPU | Both successful and failed temporary-block workloads still pass; current numeric try/catch medians are unchanged. This slice does not claim to eliminate the earlier cleanup instruction cost. |
| About +4% numeric CPU after automatic local cleanup | Nil cleanup stores remain automatic; ordinary numeric median improves about 2% against the immediate baseline. No claimed complete recovery against the pre-cleanup compiler. |
| Automatic references at block/break/continue/match exits | Existing lifetime, exception and explicit-exit regressions pass unchanged. No manual free is introduced. |
| +4.6% CPU with active limits | Tick length reuse and redundant-call removal improve the current bounded numeric median; exact step/peak/error accounting has a new regression. It is a different workload from the historical collection-growth benchmark, not proof every bounded workload improves. |

## Validation and remaining scope

[Validation record](evidence/resource-cost-validation-2026-09-21.json) includes
33 semantic suites, seven single-vCPU fixtures on both qemu64/486, two SMP runs,
three native four-thread runs (24000 rooted lifetimes), and four source-version/
CPU combinations for the updated sweep probe. The new tick-accounting regression
also passed against the baseline, verifying an unchanged behavioral contract.
Native lint, committed kernel diagnostics, isolated build/portable lock, privacy
checks and diff checks were run. Existing linker RWX warnings remain unresolved.

Large marking/sweeping/wrap pauses, unfair global-lock waiting, incremental
barriers, broader platforms, physical-device measurements and runtime/FFI/PB-MCP/
polyglot integration remain open. This does not activate general host cycle
collection or provide a realtime guarantee. Installed PB-MCP was not updated;
these VM changes take effect for consumers loading the updated PBLang module.
No EnterpriseOrg server was used. Unrelated local files were preserved.

Graphify update timed out after 35 seconds; owned process group stopped. Its output does not establish semantic
coverage of these PBLang modules.

For the subsequent direct try/catch dispatch and single-edge traversal changes,
see [try and chain cost follow-up](TRY_AND_CHAIN_COST.md).

The later [inline instruction-admission follow-up](SELFHOST_TICK_ADMISSION.md)
removes the private tick call while retaining the ordering described above.
