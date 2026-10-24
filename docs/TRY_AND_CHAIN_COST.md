# Direct exception dispatch and single-edge collection work

This PBLang-only follow-up compares against `4d6a12a`. Automatic local-reference
cleanup, bounded execution, cycle recognition and the existing memory lock remain
active. No public API, bytecode opcode, persistent GC metadata or program-size
limit is added. No Rust product source or new polyglot block is introduced.

## VM exception dispatch

`OP_TRY` now uses the host language's catch dispatch directly:
`try exec(body) catch exec(handler)`. Every legitimate result, including nil,
false, maps, shared arrays and the former sentinel string, passes through without
an intermediate result-envelope map. Previously each successful body allocated
one wrapper map. This removes that transient allocation; it does not prove a
whole-process RAM reduction or remove the body/handler frame cost.

The retained host removes the body's exception handler before evaluating its
catch expression. A failure of the guest handler therefore propagates outward,
retaining its original payload. Nested and reentrant calls retain independent
exception state. The locals, globals, output, upvalues and memory context passed
to both subchunks are unchanged. Shared runtime limits and frame depth are
unchanged, and exhaustion still prevents a guest catch from resuming effects.
The compiler's automatic cleanup prelude still executes before the user handler.

A first candidate used a private boolean failure flag. It passed focused checks
but did not show a clear timing improvement and was replaced by direct dispatch.
Its samples are retained separately; they are not the final implementation.

## Collector chain traversal

The single-edge loop now reads the edge inside its loop body, stopping on null
or an already marked target before any further dereference. It avoids repeated
single-child tests/reset assignments. When the loop finishes on a single-edge
object, that edge is already known to be null or visited: the generic child loop
is skipped. When it reaches a branching object, all children still enter the
ordinary bounded traversal. A leaf remains complete immediately after marking.

This proof depends on continuous ownership of the same memory lock. No lock is
released between observing the edge and using that observation. Mark admission
still checks the owned-object count before writes, failed passes still skip
sweep, scratch ownership is unchanged, and generation-wrap behavior is retained.
The expanded fixture queues null-ended and self-cyclic roots before other roots,
then follows a chain into a shared fork. Later queued work, convergent edges and
reclamation must all remain correct. The new benchmark tests 128 separate rooted
self-cycles as well as the existing ring, mixed and leaf workloads.

Raw invalid pointers, forged metadata, unsynchronized native writes, NMI and DMA
remain outside the kernel contract. The change does not provide a general host
collector, cancellation barrier, fair scheduling or a maximum collection pause.

## Reproduce and interpret the measurements

[Raw samples](evidence/try-chain-benchmark-2026-09-21.json) use identical isolated
packages from the baseline, with only the selected modules updated in the new
variant. Each has two warmups and seven measured processes in alternating order.
Tests and Graphify do not run alongside timing; general host load is not isolated.
GNU time records process user+system CPU seconds, wall seconds and peak RSS KiB.
P95 is nearest rank, the maximum of seven samples, not a latency guarantee.

VM tests use the unchanged `bench/selfhost_resource_cost.sapp`: plain success,
plain failed, plain numeric, bounded numeric, plain blocks and plain failed_blocks.
This includes frontend compilation and VM execution. It is not an isolated
allocation counter or a comparison with cleanup disabled. Each checks its output.

Kernel tests concatenate the selected heap/GC with the fixture and use
`compiler/ctranspile.transpile_kernel_int`. Compile with `-m32 -O2 -march=i386
-fno-toplevel-reorder -ffreestanding -nostdlib -fno-pie -mgeneral-regs-only`; link
elf_i386 with ALIGN(4096) before .bss in both variants. Use qemu64/TCG, 32 MiB,
no disks/network/display and ISA debug exit. Only exit 85 succeeds; each process
has a 30-second external supervisor. These are test bounds, not language limits.

Use 500000 passes for `kernel_gc_collect.sapp`, with 128 and 129 objects in both
variants; use the existing mixed-sweep and leaf-mark benchmarks unchanged. Use
`kernel_gc_self_roots.sapp` unchanged (128 rooted self-cycles, 200000 passes).
The exact compilation and workload recipe is also described in
[the preceding resource-cost report](RESOURCE_COST_FOLLOWUP.md).

| Workload | CPU median before / after (s) | CPU p95 before / after (s) | RSS median before / after (KiB) |
| --- | --- | --- | --- |
| kernel ring | 1.01 / 0.93 | 1.02 / 0.94 | 59184 / 59268 |
| kernel fallback | 1.09 / 1.02 | 1.40 / 1.07 | 59188 / 59244 |
| kernel mixed | 0.86 / 0.85 | 0.87 / 0.95 | 59272 / 59284 |
| kernel leaves | 1.04 / 1.03 | 1.15 / 1.04 | 59308 / 59248 |
| kernel self_roots | 1.41 / 1.09 | 1.43 / 1.26 | 59308 / 59304 |
| vm plain_success | 0.87 / 0.88 | 1.01 / 0.98 | 35848 / 35804 |
| vm plain_failed | 0.88 / 0.87 | 1.25 / 0.88 | 35652 / 35708 |
| vm plain_numeric | 1.47 / 1.45 | 1.50 / 1.50 | 35684 / 35816 |
| vm bounded_numeric | 2.58 / 2.58 | 2.62 / 2.64 | 35736 / 35844 |
| vm plain_blocks | 0.83 / 0.84 | 1.25 / 1.04 | 35772 / 35744 |
| vm plain_failed_blocks | 0.84 / 0.85 | 0.99 / 0.95 | 35736 / 35692 |

Kernel medians decrease about 8% for the borrowed-scratch ring, 6% for the
allocated-scratch ring and 23% for separate rooted self-cycles. Mixed and leaf
medians differ by about 1%; small differences and tail variation warrant caution.
These compare the current baseline within this series, not absolute timings from
earlier sessions. They do not prove that every previously reported cost vanished.

The try/catch end-to-end measurements remain effectively unchanged: some medians
shift by 0.01 s in either direction. Direct dispatch removes a transient wrapper
per successful body, but it does not establish a meaningful speedup or lower
peak RSS. No general RAM saving or universal improvement of worst times is
demonstrated. No physical-device, disk-I/O, power, GPU/NPU or peripheral benefit
was measured. QEMU RSS includes emulator overhead, not just the guest heap.

## Validation, installed desktop and remaining scope

[Validation record](evidence/try-chain-validation-2026-09-21.json):

- 39 semantic suites, including limits, local/exception cleanup, serialization,
  optimizer, resource scopes and five emulator suites.
- Seven kernel fixtures on both qemu64 and 486, including the expanded queued
  chain-endings case, full-heap recovery, OOM, partial failure and generation wrap.
- Two two-vCPU QEMU runs and three native four-thread runs (24000 rooted lifetimes).
- Native lint, committed kernel diagnostics, isolated build and portable lock.
- Exact original handler error payload and absence of later output on both
  baseline and updated self-hosted VM. Existing linker RWX warnings remain.

The currently configured desktop PB-MCP release was examined read-only: its
manifest matches the external configured pin and all 93 member sizes/hashes match
(34762819 payload bytes). Neither compiler/vm.sapp nor kernel/gc.sapp is shipped
in that inventory. Copying these files into it would not activate them and would
violate the closed package inventory. No installation, enrollment, permission,
transport or running desktop session was changed. This inventory check is not a
new live desktop acceptance test or a universal package safety certification.

General runtime/FFI/PB-MCP/polyglot integration remains unfinished; the precise
kernel graph cannot safely substitute for missing retained-host roots. Marking,
sweep and generation wrap still run under a global lock and can cause long
pauses. Broader platforms, physical-device testing, fairness and incremental
barriers remain open. No realtime guarantee is provided. No EnterpriseCore server was
used. Other local changes were preserved.

Graphify update timed out after 35 seconds; owned process group stopped. Its graph is not semantic
coverage of these PBLang modules. The five kernel ELF artifacts were each 28
bytes smaller; this is not a claim of general storage or disk-I/O savings.

Native VM lint retains the same eight advisory warnings as the baseline;
no new VM lint warning was introduced. The new outcome test is lint-clean, and
the three touched kernel sources have zero diagnostics.
