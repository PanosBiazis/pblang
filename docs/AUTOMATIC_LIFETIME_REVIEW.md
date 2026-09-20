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
| Kernel GC | Table capacity/size checks, failed allocation before writes, no sweep after scratch failure, iterative traversal and cycle reclamation | Raw pointer validity and owner identity are supplied by the kernel; concurrent graph mutation requires external synchronization |
| Retained GC | Standard-library destructive collection remains refused because same-thread nested VM roots are incomplete | Specialized desktop maintenance does not prove a safe general collector |

## TOCTOU and concurrency

The resource-scope result snapshot closes the tested callback-mutation window
for scalar envelope fields. It is a shallow snapshot: a nested object can still
change through another reference. Such reports must not be used as immutable
authorization evidence. Likewise, a queued update is state data, not a guarantee
that permissions checked at submission still hold when its consumer runs.
Effectful consumers must check the relevant authority at the effect boundary.

The kernel collector has no callbacks during marking, but that does not exclude
interrupt handlers or other threads. Concurrent mutation can invalidate a root,
edge, table capacity or allocator list between reading and using it. All access
to a shared collector, its graph and underlying heap must be serialized by the
kernel's existing critical-section/locking discipline. No check-before-use
sequence in this module substitutes for that synchronization. This requirement
is explicit; the change does not claim to solve concurrent GC or all TOCTOU.

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
