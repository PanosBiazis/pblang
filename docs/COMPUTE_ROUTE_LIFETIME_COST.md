# Routing cost and completed chunk lifetime

## Implementation

`compute.sapp` checks its seven supported device names directly, avoiding two
temporary arrays and their membership loops per policy-plus-routing request.
Its private option reader uses the map-type check already performed by each
owning public entry point. Invalid options still fail before any field reads,
including for empty batches; this is not a new trusted-policy public API.

When automatic or explicit accelerator routing falls back to CPU, it reuses
the estimate it just validated within that selector invocation. It does not
recompute the same workload profile, saturating arithmetic and estimate map.
Direct CPU routes still construct their estimate normally. Public estimates
and policies remain mutable values, so no cross-call cache is used. A later
call observes policy changes and rejects malformed values again. No external
executor runs between creation and reuse of this estimate.

Execution also avoids inventory calls for workloads that cannot match any
candidate accelerator. The gate calls the selector's existing `_matches`
function rather than maintaining a second list of supported workloads. An
explicit incompatible accelerator request retains its rejection or declared
CPU fallback; compatible requests still discover registered runtimes freshly.
Supplied device lists and explicit inventory APIs remain unchanged.

`run_chunked` clears the private reference to a completed snapshot batch after
copying its results into the output. Shared result objects and captured values
remain reachable. It preserves the existing up-front snapshot of input slots:
replacing an original array slot in a callback cannot change an unprocessed
snapshot batch. Creating batches lazily would change that behavior, so this
change deliberately retains the snapshot. Original input and complete output
remain resident. It is not a general cycle collector or an out-of-core API.

These changes are PBLang only. There are no new product size limits, runtime
permissions, device grants, caches, retries, native operations or Rust changes.

## Correctness and review

The new route test compares 420 complete route records against the baseline,
covering seven device preferences, five workload classes, six sizes and two
device lists. Both variants produce the same SHA-256 over their JSON records:
`3196fcc8e1d8eb1067df6835f786639e069b48c14f0e39674ce8f6569156e7ec`.
Additional checks cover estimate mutation, changed and nil policy fields,
saturation, strict accelerator denial, invalid option types and malformed
device names. Existing tests retain transfer-budget and worker validation.

The chunk regression checks original-input mutation, shared map aliases,
returned closures, empty input, the legacy minimum chunk size, and failure
without retry or later worker invocation. The separate failing fixture checks
that the original worker error reaches the CLI in both variants.

Twenty-nine Linux suites passed, including compute/routing, parallelism,
resource budgets, FFI/polyglot and five emulator-related suites. Build, portable
lock publication and lint passed. Lint retains existing warnings. This is focused
coverage, not general memory-safety or cross-platform certification.

The workload-discovery test covers incompatible automatic and explicit
requests even with registered runtimes, strict rejection, all four compatible
runtime kinds, and immediate unregistration. A successful-exec Linux trace
recorded 63 to 15 `lspci` executions and 42 to 10 `nproc` executions. The five
compatible requests retain their discovery; skipped calls belong to the 16
incompatible requests. Hardware queries, not actual GPU/NPU kernels, are being
avoided here.

Review confirmed that all private option-reader callers validate maps first;
CPU fallback validation is retained; no externally supplied estimate is accepted
through the public interface; result mutations cannot affect a later estimate.
The chunk clear drops a private reference without explicitly freeing shared
objects. It adds no callback or cleanup action that can replace a worker error.
Concurrent mutation of public policy maps is not a new supported synchronization
contract. Reference cycles, external-resource lifetime and native ownership are
outside this change.

## Measurements

Baseline is `5fc7953`; an additional older compute implementation from
`fdd7e90` is compared for the original explicit-worker workload. Both variants
use identical runtime binaries per platform, inputs, rounds and expected results.
One warmup per workload/variant is excluded, followed by nine Linux or seven
Windows samples in alternating forward/reverse variant order. All runs include
startup and import verification. These are shared machines. Nearest-rank p95
is the maximum observation at these sample counts, not a well-resolved tail.

Linux user-plus-system CPU includes waited child processes. CPU is seconds,
peak RSS is KiB, and all values below are baseline to candidate:

| Workload | Median CPU | CPU change | p95 CPU | Median peak RSS |
| --- | --- | --- | --- | --- |
| Explicit workers, 1,000 calls | 1.41 → 1.41 | +0.0% | 1.78 → 1.50 | 39204 → 39284 |
| Explicit workers, 10,000 calls | 2.03 → 2.00 | -1.5% | 2.06 → 2.34 | 40984 → 39328 |
| Auto routing, 1,000 calls | 1.43 → 1.42 | -0.7% | 2.01 → 1.53 | 39276 → 39316 |
| Chunk execution, 180,000 values | 3.83 → 1.81 | -52.7% | 3.96 → 2.28 | 50432 → 49868 |
| Public policy, 1,000 calls | 1.36 → 1.35 | -0.7% | 1.57 → 1.60 | 39308 → 39200 |
| Supplied devices, 3,000 calls | 1.54 → 1.51 | -1.9% | 1.79 → 1.77 | 39244 → 39340 |
| Parallel threshold, 500 calls | 1.79 → 1.77 | -1.1% | 1.82 → 2.25 | 39336 → 39300 |

The large chunk workload improves about 52.7% CPU, with 1.1% lower median peak
RSS. The complete input and output are checked element by element. The earlier
candidate that only shortened chunk lifetime did not establish a substantial
memory improvement; avoiding irrelevant hardware discovery supplies most of
this final CPU benefit.

The unchanged 1,000-call explicit-worker benchmark measures 1.41 seconds median
on the older reference, baseline and candidate. The prior +2.1% was not reproduced
in this run; that is not proof it has been eliminated for all conditions. The
10,000-call comparison adds a longer control using the same benchmark source
and equal rounds on both variants: about 1.5% less CPU and 4.0% lower peak RSS.
It supplements rather than replaces the original comparison. Several p95 values
still worsen, including the longer explicit-worker and parallel-threshold tests.
No general RAM saving or worst-time guarantee is established.

On the authorized Windows workstation, eleven suites passed on each variant,
plus the expected original-error failure on each (24 checks). The 420-route
signature matches Linux. Both packages consumed Linux-published portable v2
lockfiles with the same Windows runtime, under the authorized executable-test
root. No antivirus, account, approval or system configuration was changed.

Windows measurements below are milliseconds. CPU is the **parent process only**;
it excludes CPU consumed by hardware-query helper processes. It must not be
compared directly with Linux child-inclusive CPU. Windows RAM was not measured.

| Workload | Median parent CPU | CPU change | Median elapsed | p95 elapsed |
| --- | --- | --- | --- | --- |
| Explicit workers, 1,000 calls | 3687.5 → 3671.9 | -0.4% | 3771.9 → 3789.6 | 3930.6 → 3835.3 |
| Explicit workers, 10,000 calls | 3843.8 → 3750.0 | -2.4% | 3978.3 → 3931.4 | 4152.5 → 4114.9 |
| Auto routing, 1,000 calls | 3515.6 → 3500.0 | -0.4% | 3676.5 → 3632.2 | 3812.7 → 3807.1 |
| Chunk execution, 180,000 values | 3718.8 → 3734.4 | +0.4% | 37388.2 → 3841.1 | 38183.4 → 3953.8 |
| Public policy, 1,000 calls | 3468.8 → 3500.0 | +0.9% | 3572.6 → 3576.8 | 3660.7 → 3782.6 |
| Supplied devices, 3,000 calls | 3578.1 → 3578.1 | +0.0% | 3645.1 → 3661.2 | 4013.9 → 3748.9 |

In the Windows chunk workload, median elapsed time falls about 89.7%. This
is avoidance of repeated inventory work, not faster GPU/NPU computation.
The older explicit-worker reference has median parent CPU 3593.8 ms
and elapsed time 3795.4 ms. Other workload differences and
worse tails are preserved in the table and raw samples; no blanket speedup is claimed.

The candidate's Windows explicit-worker parent CPU is still about 2.2% above
the older reference (3671.9 versus 3593.8 ms), despite being close to the current
baseline. The public-policy control also costs about 0.9% more parent CPU than
the baseline. These costs have not been eliminated.

[Raw evidence](evidence/compute-route-lifetime-cost-2026-09-23.json) contains the
runtime/source hashes, samples, summaries, validation and process trace counts.
Preliminary results are separate and excluded from final statistics. The first
Windows benchmark was deliberately stopped after the unnecessary inventory
path was identified; complete comparisons were rerun with the final candidate.
No partial old run is presented as a final measurement.

For reproduction, build isolated packages and publish their portable locks with
`tools/pblock.sapp`. Use the same retained binary per platform and the following
commands, unchanged between variants. Copy the new chunk benchmark unchanged
into the baseline package as well.

```text
pblang run bench/compute_explicit_workers.sapp --allow-all -- compute 1000
pblang run bench/compute_explicit_workers.sapp --allow-all -- compute 10000
pblang run bench/compute_discovery.sapp --allow-all -- small 1000
pblang run bench/compute_chunk_lifetime.sapp --allow-all -- 180000
pblang run bench/compute_serial_discovery.sapp --allow-all -- policy 1000
pblang run bench/compute_discovery.sapp --allow-all -- injected 3000
pblang run bench/compute_serial_discovery.sapp --allow-all -- threshold 500
```

The threshold workload was timed on Linux only. These tests do not measure
GPU/NPU kernel throughput, energy use, disk throughput or live desktop capture.

## Remaining integration

The PB-MCP source was inspected: desktop loops use their separate
`src/desktop_memory.sapp` maintenance counter and host collector, while desktop
streaming uses its PBRemote frame path. They do not invoke this compute module.
Neither that collector nor the installed desktop package was changed. A general
runtime collector still needs proven ownership of every live VM root; copying
this compute module into a desktop release would not provide that guarantee or
improve live view.

Secure Windows lock publication still needs verified bridge/root ACL contracts
and integration beyond the existing Linux publisher. Windows consuming a
portable lock is distinct from publishing it safely. No admission check was
relaxed. Long collector pauses, general reference cycles, runtime/FFI/polyglot
integration, actual GPU/NPU throughput and broader device coverage remain open.
No EnterpriseOrg server was used.

Graphify's AST-only update exceeded its 35-second limit and its own process group
was terminated. It does not establish semantic coverage of the PBLang changes.
