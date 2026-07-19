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

## Polyglot policy

`stdlib:polyglot_policy.sapp` validates runners when a policy is constructed.
Every mode, including explicit `unrestricted`, retains the code-size and
language-allowlist limits. `report` is the only mode that needs no runner.
`secure` scanning remains defense in depth, not proof that foreign code is
safe; untrusted execution still needs an OS/container sandbox with explicit
filesystem, process, and network capabilities.

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
- `tests/semantics/hardware_cost_routing_test.sapp`
- `tests/semantics/accelerator_test.sapp`
- `tests/semantics/polyglot_policy_test.sapp`
- `tests/semantics/ffi_universal_enrich2_test.sapp`
- `bench/controlled_execution_stages.sapp`
- `bench/hardware_cost_routing_stages.sapp`
- `examples/controlled_compute/main.sapp`
- `examples/hardware_routing/main.sapp`
