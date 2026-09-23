# Controlled polyglot and parallel execution

PBLang exposes a PBLang-written control plane for bounded parallel work,
heterogeneous device routing, polyglot execution, and FFI. The control plane
does not turn hardware detection into an execution claim: a device is
`present` when detected and `usable` for generic compute only when an
equivalent runtime hook has been registered.

## Bounded CPU execution

`stdlib:thread_pool.sapp` accepts 1 to 64 workers and at most 100,000 submitted
tasks. Submission after shutdown and non-callable tasks fail closed. Parallel
workers return immutable outcome records; the parent VM updates task status,
results, and success/failure counters in deterministic priority order. Worker
closures are created by top-level factories so only closed, transferable
upvalues cross the VM-thread boundary.

`stdlib:compute.sapp` provides:

- `policy(options)` for `preferred_device`, workload, worker and size
  thresholds, accelerator permission, and CPU fallback;
- `estimate_work(count, policy)` for the bounded, saturating cost estimate used
  by every route;
- `select_device(count, devices, policy)` for a deterministic dry-run route;
- `run_controlled(items, worker, options)` for an auditable result envelope;
- `storage_read_controlled(paths, options)` for bounded serial or chunked disk
  reads;
- `run(items, worker)` as a backward-compatible result-only helper.

Small or cheap jobs run serially. CPU parallelism is selected only when the
estimated work amortizes a conservative thread-launch cost and each selected
worker receives useful work. The worker count is capped by item count, policy,
and a hard limit of 64; parallel execution uses bounded contiguous chunks
instead of one operating-system thread per element.

GPU, NPU, SIMD, and distributed routes require a matching workload, minimum
item count, permission, registered equivalent runtime, bounded transfer size,
and enough estimated work to amortize launch plus transfer cost. An explicit
accelerator request either fails or reports that CPU fallback was used; it
never changes devices silently. Runtime failure is re-routed through the same
CPU cost model rather than blindly starting the maximum worker count.

`run_controlled` automatically skips hardware discovery for empty batches,
explicit CPU/sequential requests, disabled accelerators, and batches below
`min_accelerator_items`. It also skips discovery when the requested workload
cannot match the preferred accelerator (or any automatic candidate), using the
same workload-matching rules as the selector. In those cases availability cannot affect the
route. Validation, rejection reasons, worker ordering, and strict fallback
behavior are unchanged. Caller-supplied device lists are still validated,
including on empty batches. Eligible accelerator calls retain fresh discovery;
there is no device cache that could hide runtime registration or removal.
Preferred-device membership uses direct exact-name checks without constructing
a temporary list; all subsequent policy and transfer-limit checks still run.
Automatic routing reuses its fresh work estimate when it falls back to CPU;
estimates are never cached across calls or external executor invocations.
Explicit `max_workers` also avoids
querying the CPU count to compute an unused default, for both compute and
controlled storage reads. Execution also skips that query for batches guaranteed
to remain serial: zero/one item, a batch below its parallel item threshold,
disabled compute parallelism, or an explicit sequential compute request.
Omitted or `nil` worker counts still trigger fresh detection whenever parallel
execution is possible. Public `policy()` always resolves its worker default,
including for sequential preferences. No device or policy cache is added.
See [automatic serial discovery measurements](COMPUTE_AUTOMATIC_SERIAL_COST.md).
See [validation cost measurements](COMPUTE_VALIDATION_COST.md) for the measured
cost and platform limitations.

This also benefits `run` and small batches in `run_chunked`. It does not change
the explicit inventory functions `devices`, `hardware`, or `summary`, and does
not enable a GPU/NPU executor merely because a driver is installed.

`run_chunked` releases each completed private input batch after copying its
results into the output. It still snapshots all input slots before invoking
workers, preserving behavior when a worker changes the original input array.
Shared values and returned closures retain their references. The input, initial
snapshot and complete result still require memory; this API is not a streaming
or out-of-core iterator. See [routing and chunk lifetime measurements](COMPUTE_ROUTE_LIFETIME_COST.md).

```sapp
let compute = import("stdlib:compute.sapp");
let result = compute.run_controlled([1, 2, 3, 4], fn(x) { return x * x; }, map{
    preferred_device: "auto",
    max_workers: 2,
    min_parallel_items: 4,
    fallback_to_cpu: true,
});
```

The default workload profiles and their `work_units_per_item` and
`transfer_bytes_per_item` values are deterministic relative estimates, not
nanoseconds or benchmark claims. Advanced callers can calibrate these hints,
the launch costs, transfer ratio, minimum useful work per worker, and transfer
budget for a known kernel. Arithmetic saturates at pblang's exact-integer
ceiling instead of wrapping, and malformed post-construction policies fail
closed.

`storage_read_controlled` validates at most 10,000 paths of at most 4,096 bytes
each, stays serial below `min_parallel_files` (four by default), and otherwise
uses at most `max_workers` contiguous chunks (eight by default, 64 maximum).
The compatibility `storage_read_parallel(paths)` returns only the ordered
results. The current `fs_read` builtin still obtains each complete finite file;
this API bounds scheduling metadata and worker count, not file bytes or physical
I/O bandwidth. Sized streaming reads need a separate native backend.

Runtime hooks are registered through `stdlib:accelerator.sapp`. `dispatch`
contains runtime exceptions and invokes a caller-supplied equivalent CPU
implementation only when fallback is enabled. Supported hook classes are
`gpu`, `npu`, `simd`, and `distributed`.

### Revalidation within one compute request

`run_controlled` builds a fresh validated scalar policy for each call, then uses
an internal selector that avoids revalidating those same scalar fields. Public
`policy`, `select_device` and `estimate_work` retain their input checks and
mutable-policy behavior; a caller-supplied `validated` field or format marker
cannot activate the internal route. Device lists and item counts remain checked.
No policy is cached across calls. The CPU fallback route is prepared before an
accelerator hook executes, so a hook changing the original options does not
change that call's policy. A subsequent call validates the new options again.

See [validation reuse measurements](COMPUTE_VALIDATION_REUSE.md) for parity,
resource measurements and unresolved limits. This change neither grants native
capabilities nor changes the collector, thread isolation or accelerator kernels.

## Polyglot policy

`stdlib:polyglot_policy.sapp` validates runners when a policy is constructed.
Every mode, including explicit `unrestricted`, retains the code-size and
language-allowlist limits. `report` is the only mode that needs no runner.
`secure` scanning remains defense in depth, not proof that foreign code is
safe. The mode now fails closed unless `with_sandbox` supplies an explicit
runner, and scan-safe code is executed only through that runner. The convenience
`secure` function treats its runner as the explicit sandbox runner. The caller
must still provide a real OS/container boundary with explicit filesystem,
process, and network capabilities; a callback label is not proof of isolation.
Accordingly, generic callback results report `sandbox_runner_used=true` but keep
`sandbox_verified`, `sandboxed`, and `effects_confined` false. Policy callbacks,
mode, limits, and allowlists are held in a bounded private registry behind opaque
handles; returned descriptions are detached snapshots.

The direct polyglot APIs now require `unsafe_polyglot: true` together with an
`unsafe_polyglot_sha256` that matches the exact foreign source bytes. This is an
explicit, tamper-evident acknowledgement that foreign code is outside PBLang's
memory-safety contract. It is still not user authorization. `trusted_source`
and toolchain-effect acknowledgements remain separate, and authority is scoped to the VM host's
capability profile. A service running mutually untrusted principals must use a
separate least-privilege VM, or an external authenticated execution broker, for
each principal. Results expose `authority_scope: "vm_capability"`,
`multi_principal_safe: false`, and `principal_isolation_required: true` so a
wrapper cannot infer a stronger boundary. Opaque policy handles have a bounded
idle lease, and one-shot helpers close them on both success and exceptions.

`polyglot_router.plan` is analysis-only. It always reports `allowed=false` and
`executable=false`. Its legacy `sandbox_ready` argument is accepted only as a
strict boolean declaration and cannot become authority or verified evidence.
Execution still requires a separate capability-gated backend.

On current Linux targets, race-resistant secure publication also performs
owner/mode verification with a fixed `stat` executable against a validated held
descriptor path. It therefore needs `allow_ffi` and `allow_exec`; no shell or
caller-selected executable is involved. Removing that exec dependency requires
a future PBLang-native, ABI-specific `fstat`/`statx` structure decoder rather
than skipping the ownership check.

`stdlib:polyglot_isolate.sapp` is the Linux namespace substrate for a future
dedicated authenticated execution broker. Its first complete adapter is Python;
other languages fail closed until they have equivalent fixed request-file
wrappers. Caller source and input arrive through a bounded descriptor and a
mode-0400 request file inside an 8 MiB tmpfs, never through runtime argv. The
fixed wrapper closes inherited descriptors other than bounded stdout/stderr,
enables no-new-privileges, compiles the source, and emits a nonce-bound dispatch
marker immediately before calling Python `exec`. The marker has a separate
128-byte internal reserve; returned application stdout/stderr are still clipped
to the caller's exact combined budget. Results distinguish process completion
from dispatch-point observation. `executed` and `execution_confirmed` remain
false because this boundary cannot prove that the first caller instruction ran.

The descriptor-held fixed bubblewrap launcher requests private
user/PID/network/IPC/UTS/mount namespaces, UID/GID 65534, read-only system
runtime mounts, empty `/home`, a cleared environment, dropped capabilities,
disabled nested user namespaces, and inherited bounded process limits. It never
mounts a host workspace or credential store. Ordinary `run` results report
these as configured rather than verified. The separate live self-test publishes
granular observations for UID/GID, 8 MiB scratch size, selected host-path
absence, `/usr` write denial as UID 65534, environment allowlisting, all five
Linux capability sets, no-new-privileges, payload PID, sandbox hostname,
loopback-only interfaces, argv privacy, and selected host-path descriptor
absence above FD 2. The `/usr` result does not independently prove a read-only mount;
the path checks do not cover every socket or memfd. User, IPC, and mount
namespace separation therefore remain explicitly unverified by this self-test.

The Python runtime is still selected from a read-only but broad host runtime
tree and its identity is not descriptor/digest verified. Seccomp and cgroup
enforcement also remain explicitly false. Therefore
`production_untrusted_ready` remains false and this module is not yet the
complete broker boundary.

`stdlib:polyglot_broker_protocol.sapp` now provides the effect-free
authentication and state-machine layer for that future boundary. It uses a
canonical length-delimited binary frame, request/response-separated HMAC keys,
audience and epoch binding, exact principal/organization/credential context,
strict next-sequence processing, broker-owned time, bounded request lifetimes,
and per-request reauthorization against the IAM authority pinned when the
broker is created. Dispatch never accepts a caller-selected IAM state.
Malformed or unauthenticated requests are
silent and do not advance state. Authenticated replay and gap responses are
signed without advancing state or consuming the terminal credential-attempt
quota; transport rate limiting remains a separate outer-layer responsibility.
Exact expired, invalid, or unauthorized
requests consume their sequence. Revocation signs one exact denial and then
scrubs and removes the binding.

Broker handles and public descriptor identities are separate random values. A
descriptor cannot be reconstructed into a privileged handle. Broker and client
registries are bounded; both have idle and non-refreshing absolute lifetimes
capped at one hour, and broker close/expiry retires linked client keys. Client
response verification checks the request digest, full context, MAC direction,
effect flags, and status-specific sequence transition before changing state.
Administrative and dispatch capabilities are also separate: only the admin
handle can bind, unbind, describe, mint a dispatch handle, or close; only the
dispatch handle can process frames.

This protocol module imports no listener, filesystem, process, polyglot, or
isolation API. Every descriptor and result therefore reports
`executable=false`, `executed=false`, and `effects_applied=false`. It does not
yet connect authenticated requests to `polyglot_isolate.sapp`; transport
framing, worker ownership, cancellation, response output framing, and
end-to-end deny-exec client profiles remain the next integration slice.

`stdlib:process.sapp` now supplies the opaque asynchronous lifecycle needed by
that integration. `start_fixed_bounded` reserves one of 128 private registry
slots before launch, requires a Linux pidfd identity, and returns no PID,
process-group number, descriptor, directory, or raw supervisor state. `poll`,
`request_cancel`, `finish`, and `close` accept only the opaque handle. Clean
launch failures release their reservation; a startup failure that may still
own a process instead returns a recovery-capable opaque handle. The lower-level
Linux process API also requires pidfd identity for every start; it has no
cancellable numeric-PID fallback.

`maintain_bounded` performs one bounded, nonblocking pass over the registry. It
reaps completed abandoned jobs and requests deadline recovery, but removes an
entry only after the supervisor proves the process group absent. There is no
background thread. Cancellation pins the exact leader generation with pidfd
`SIGSTOP` while the fixed group-signal helper runs, preventing numeric PGID
reuse between identity validation and signalling. The generated supervisor
launcher self-stops before exec, while its parent publishes readiness only after
`wait4(..., WUNTRACED)` observes that exact fork child stopped. The generated
supervisor atomically publishes direct-child status and then uses
`kill(0, SIGKILL)` to
remove itself and any same-group descendants; post-fork status-publication
errors use the same teardown path. Bounded output is read only after group
absence is verified.

This remains process supervision, not a complete sandbox. A descendant that
successfully creates another session/process group is not confined because
cgroups and a PID namespace are not enforced here. Background-descendant
userspace buffers may be lost during forced teardown, and status publication
is not power-loss durable without file and directory `fsync`. The future
broker worker must combine this lifecycle with `polyglot_isolate.sapp` and keep
`production_untrusted_ready=false` until runtime identity, seccomp, cgroup or
equivalent whole-tree confinement, and platform parity have current evidence.

## Safe FFI control plane

`stdlib:ffi_universal.sapp` adds `ffi_policy`, `decide_load`, `decide_call`,
`load_checked`, and `call_checked`. Checked loading uses an exact library path
and never guesses an extension, prefix, or search-path alias. Calls require an
exact symbol, matching argument/type arity, a bounded argument count, supported
types, and explicit opt-in for raw pointer types.

This PBLang-side policy can only narrow access. It does not grant `allow_ffi`
and cannot widen the VM host's native `FfiPolicy`; production embeddings must
configure both layers.

## Current boundary

Automatic parallelization is limited to explicit combinators and controlled
compute calls whose functions are declared thread-safe by the caller. The
compiler does not yet prove purity, absence of data races, associativity, or
device-kernel equivalence. Generic GPU/NPU/SIMD/distributed code generation,
driver isolation, remote authentication, cancellation of losing race tasks,
and dynamic work stealing remain separate implementation work. CPU execution
is the portable baseline. Network requests are not automatically reordered or
batched because that could change ordering, retry, and idempotency semantics;
callers must use the explicit async/network APIs that match their protocol.

Verification:

- `tests/semantics/controlled_execution_test.sapp`
- `tests/semantics/thread_pool_enrich_test.sapp`
- `tests/semantics/compute_test.sapp`
- `tests/semantics/compute_discovery_test.sapp`
- `tests/semantics/compute_preferred_validation_test.sapp`
- `tests/semantics/compute_serial_discovery_test.sapp`
- `tests/semantics/compute_validated_route_test.sapp`
- `tests/semantics/compute_route_estimate_test.sapp`
- `tests/semantics/compute_workload_discovery_test.sapp`
- `tests/semantics/compute_chunk_lifetime_test.sapp`
- `tests/semantics/hardware_cost_routing_test.sapp`
- `tests/semantics/accelerator_test.sapp`
- `tests/semantics/polyglot_policy_test.sapp`
- `tests/semantics/polyglot_broker_protocol_test.sapp`
- `tests/semantics/process_bounded_async_test.sapp`
- `tests/semantics/linux_pidfd_test.sapp`
- `tests/semantics/linux_process_test.sapp`
- `tests/semantics/ffi_universal_enrich2_test.sapp`
- `bench/controlled_execution_stages.sapp`
- `bench/hardware_cost_routing_stages.sapp`
- `bench/compute_discovery.sapp`
- `bench/compute_route_validation.sapp`
- `bench/compute_explicit_workers.sapp`
- `bench/compute_serial_discovery.sapp`
- `bench/compute_chunk_lifetime.sapp`
- `examples/controlled_compute/main.sapp`
- `examples/hardware_routing/main.sapp`
