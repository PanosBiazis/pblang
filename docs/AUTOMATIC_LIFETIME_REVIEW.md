# Focused automatic-lifetime review, 2026-09-20

Scope: PBLang resource scopes, latest-update buffering, owned scalar callbacks,
retained GC diagnostics, and the kernel collector/allocator. This is a local
source review plus the listed regressions, not a certification of all PBLang,
all native libraries or installed PB-MCP transports. No unreviewed external code was
executed for this review. No EnterpriseOrg server was accessed.

| Component | Verified behavior | Remaining boundary |
| --- | --- | --- |
| `resource_scope` | Result envelope snapshot precedes cleanup; callback removed before invocation; registration closes before cleanup; later callbacks run after a failure | Trusted synchronous callbacks only; payload/error objects retain shared aliases; no thread exclusion, deadline or general OOM guarantee |
| `latest_updates` | Batch detaches before callbacks; reentrant updates go to the next batch; recursive flush/discard refused; failed work is not replayed | Shared payloads remain mutable; unbounded distinct keys can consume memory; it is not a permission-check queue or an ordered-event journal |
| `memory_owned` | Scope-depth/token mismatches are detected; normal/error scalar cleanup preserves caller scope in tested cases; escaped freed handle becomes invalid | Raw handles/builtins are available to callbacks; this is not isolation from malicious callbacks; composite/native ownership and general exceptions remain incomplete |
| Kernel GC | Table capacity/size checks, failed allocation before writes, no sweep after scratch failure, iterative traversal, generation retry/wrap, cycle reclamation, current-CPU IRQ serialization, atomic rooted allocation and size-overflow refusal | Raw pointer validity and owner identity are supplied by the kernel; SMP/NMI/DMA and external mutation still require synchronization |
| Retained GC | Standard-library destructive collection remains refused because same-thread nested VM roots are incomplete | Specialized desktop maintenance does not prove a safe general collector |

## TOCTOU and concurrency

The resource-scope result snapshot closes the tested callback-mutation window
for scalar envelope fields. It is a shallow snapshot: a nested object can still
change through another reference. Such reports must not be used as immutable
authorization evidence. Likewise, a queued update is state data, not a guarantee
that permissions checked at submission still hold when its consumer runs.
Effectful consumers must check the relevant authority at the effect boundary.

The kernel collector has no callbacks during marking. Public GC/free-list
operations now save and mask interrupts on the current CPU, with compiler memory
barriers and restoration on success/failure. Nested operations preserve the
incoming disabled state. `gc_alloc_rooted` closes the allocate-then-publish
window, rolling back object registration/allocation if root growth fails.
Existing `gc_alloc` remains unrooted; subsequent `gc_add_root` is not atomic with
it. Callers must root values throughout their use and protect compound graph
operations. Internal underscore helpers require an already-held critical section.

This addresses cooperative maskable-interrupt races on **one active x86 CPU**.
It does not serialize other CPUs, NMIs, DMA, arbitrary bump-allocator users,
external `poke32` writes or foreign runtimes. A saved IF bit is not an SMP lock.
Whole-collection masking delays interrupts and gives no hard real-time bound.
Public collector operations do not validate arbitrary pointer ownership and the
heap does not detect general invalid/double frees. No general concurrent-GC or
untrusted-native isolation claim follows from these changes.

The collector control block still comes from the retained kernel bump arena.
Failed `gc_new` attempts can consume that 48-byte block even though free-list
metadata is unwound, and there is no general collector-destruction API. This
existing lifecycle gap remains separate from rooted-object rollback; repeated
collector creation is not claimed to be leak-free.

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
