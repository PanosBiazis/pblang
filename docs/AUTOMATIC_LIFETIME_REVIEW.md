# Focused automatic-lifetime review, updated 2026-09-21

Scope: PBLang resource scopes, latest-update buffering, owned scalar callbacks,
retained GC diagnostics, and the kernel collector/allocator. This is a local
source review plus the listed regressions, not a certification of all PBLang,
all native libraries or installed PB-MCP transports. No unreviewed external code was
executed for this review. No Specisoft server was accessed.

| Component | Verified behavior | Remaining boundary |
| --- | --- | --- |
| `resource_scope` | Result envelope snapshot precedes cleanup; callback removed before invocation; registration closes before cleanup; later callbacks run after a failure | Trusted synchronous callbacks only; payload/error objects retain shared aliases; no thread exclusion, deadline or general OOM guarantee |
| `latest_updates` | Batch detaches before callbacks; reentrant updates go to the next batch; recursive flush/discard refused; failed work is not replayed | Shared payloads remain mutable; unbounded distinct keys can consume memory; it is not a permission-check queue or an ordered-event journal |
| `memory_owned` | Scope-depth/token mismatches are detected; normal/error scalar cleanup preserves caller scope in tested cases; escaped freed handle becomes invalid | Raw handles/builtins are available to callbacks; this is not isolation from malicious callbacks; composite/native ownership and general exceptions remain incomplete |
| Kernel GC | Table capacity/size checks, failed allocation before writes, no sweep after scratch failure, iterative traversal, generation retry/wrap, cycle reclamation, shared atomic lock with IRQ preservation, atomic rooted allocation, size-overflow refusal and constructor-failure rollback | Raw pointer validity and owner identity are supplied by the kernel; two-vCPU bootstrap tested, general SMP OS/NMI/DMA and external mutation are not covered |
| Retained GC | Standard-library destructive collection remains refused because same-thread nested VM roots are incomplete | Specialized desktop maintenance does not prove a safe general collector |

## TOCTOU and concurrency

The resource-scope result snapshot closes the tested callback-mutation window
for scalar envelope fields. It is a shallow snapshot: a nested object can still
change through another reference. Such reports must not be used as immutable
authorization evidence. Likewise, a queued update is state data, not a guarantee
that permissions checked at submission still hold when its consumer runs.
Effectful consumers must check the relevant authority at the effect boundary.

The kernel collector has no callbacks during marking. Public GC/free-list
operations now disable local interrupts, acquire a shared atomic x86 memory lock,
release that lock and restore the original local interrupt state. Internal
helpers require existing ownership and no longer reacquire the lock. This avoids
self-deadlock in automatic collection, heap growth and rooted-allocation rollback.
`gc_alloc_rooted` closes the allocate-then-publish window; existing `gc_alloc`
remains unrooted, requiring caller-managed publication and complete roots.

`kernel_gc_try_collect` returns 1 without waiting or traversing when another
owner holds the lock, and restores IF. The native concurrency harness verifies
this with a separate thread while the main thread deliberately holds the lock.
If admitted, it still performs a complete collection; it is not an incremental
collector or a real-time deadline guarantee. After failed acquisition, blocking operations restore the original IF while
polling and disable IRQs again before retrying. Already-masked callers remain
masked; interruptible callers can service IRQs while another CPU owns the lock. The single global lock serializes independent arenas
and has no fairness or bounded-wait guarantee. Raw invalid pointers/faults or
NMI reentry while holding it can prevent progress; it is not hostile-code isolation.

The atomic primitives and emitted PBLang algorithms passed a four-host-thread
harness, and 32-bit kernel IRQ behavior passed QEMU qemu64/486. The native harness
uses a 64-bit host ABI, checked low-address arenas and a thread-local IF test seam;
it is not an SMP kernel boot. A separate QEMU fixture now boots APIC ID 1 from
real mode with its own stack, exercises the shared GC with BSP ID 0, and verifies
IRQ progress and masked-state preservation under contention. The old wait path
fails the IRQ-progress case. General topology discovery, AP interrupt handling,
more than two CPUs, physical-device behavior, NMI/DMA, raw writes and foreign
runtimes remain unverified or unsupported. See `KERNEL_GC_SMP.md`.

Failed `gc_new` now frees its 48-byte control payload and any earlier metadata
through the free-list heap. Ten thousand failed constructions in a small arena
leave its full allocation available. Successful collectors still have no general
destruction API; replacing their whole lifetime remains a separate gap. Initialization
of a live arena and general invalid/double frees remain caller errors, not made
safe by the lock.

Filesystem handles/path replacement, native handle reuse across independent
owners, GPU/DMA buffers, cancellation during FFI and asynchronous callbacks
need boundary-specific review. Generic `defer` registration does not validate
those identities or make arbitrary cleanup callbacks safe. No new ambient
permission or automatic retry was introduced to disguise those gaps.

## Correctness fixes and evidence

The previous kernel tables could overflow after 256 object or 64 root entries.
Growth now allocates before publication and preserves existing state if it fails.
Object allocation checks null before header writes; child-index and allocation
arithmetic checks reject unsupported values. The 4096-object cyclic QEMU fixture
exceeds both old table capacities and verifies live preservation followed by
reclamation. Separate exhausted arenas exercise object/root growth refusal,
failed scratch allocation without sweeping, and rootless recovery.

Existing regression suites additionally cover malformed callback result maps,
mutated result envelopes, cleanup exceptions, recursive flush, scope-token
replacement and repeated callback failures. These are deterministic cooperative
cases, not a multithreaded race detector. The mechanisms remain unsuitable for
untrusted concurrent native code without additional runtime support.

The new generation fixture simulates a failed partial mark and verifies retry
uses a fresh generation, avoiding accidental reclamation of indirect descendants.
Wrap clears stale marks before generation reuse. The IRQ fixture runs actual
PIT interrupts on qemu64 and 486, including collection between publication and
later object use, nested state restoration, root-table failure rollback and
invalid-size refusal with a nonempty free list. These extend cooperative local
coverage; they are not a multithreaded race detector. Throughput/layout controls
and the interrupt-latency limitation are recorded in `KERNEL_GC_LIFETIME.md`.
