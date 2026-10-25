# Reusing validation within one compute request

## Implementation and compatibility

`compute.run_controlled` constructs a new policy through `_policy` on every
request. That constructor normalizes names, applies defaults and validates all
numeric configuration fields, even for empty input. The private selector can
therefore consume these scalar values directly rather than repeating their
validation and workload-profile construction in the same synchronous call.

The execution route uses `_select_device(..., true)` only with its own fresh
policy. The public three-argument `select_device` always uses the checked
route. An input `validated` field, a copied policy, or a format string does not
select the internal path. Item counts, device-list type and size, and individual
device usability remain checked. There is no cross-call cache, global trusted
policy, new native grant, product limit or disabled protection.

A shared private estimate constructor retains saturating arithmetic and the
same public result shape. The public `estimate_work` still validates inputs.
CPU and accelerator cost calculations retain all formulas and routing reasons.
The fallback CPU route is constructed before an external accelerator hook runs.
Neither a hook nor a worker receives the internal policy; changing the original
options is observed and validated on the next request. No retry is added.

This is PBLang source only; neither the retained runtime nor its binaries changed.
The optimization does not add synchronization for concurrently mutated public
maps and is not a new isolation boundary between hostile native callers.

## Verification

`compute_validated_route_test.sapp` compares execution against public selection
for seven preferences, five workloads, six item counts and two device lists:
420 complete routes and ordered output arrays. Registered test hooks exercise
all four accelerator kinds without claiming GPU/NPU kernel execution. Both
baseline and candidate produce the same route digest:
`89813efce71035c9de2e63066f02ea21ead04fbd2989003b9ed32d76334aaa95`.

Additional checks cover all 13 numeric policy fields, seven invalid values per
field, malformed input on empty requests, absence of worker effects on failure,
explicit nil defaults, mutable public policy revalidation, worker/hook changes
between calls, fallback after hook failure, and the absence of private helper
exports. The unchanged route-estimate suite also checks 420 records, saturation,
mutation and policy validation with its prior digest.

Thirty Linux suites passed, including compute, CPU parallelism, tensors,
resource/device policy, FFI/polyglot, portable locks and five emulator-related
suites. Build, portable lock publication and lint passed in an isolated package.
The original chunk-worker error still reaches the CLI. Lint retains existing
warnings. These are focused tests, not full platform or memory-safety certification.
An initial lint invocation used an unsupported CLI spelling; the documented
`pblang lint` command was then run successfully on all touched PBLang files.

## Measurements

One warmup is excluded, followed by nine Linux or seven Windows samples per
variant, alternating the variant order. Startup and imports are included. These
are shared machines; nearest-rank p95 is the maximum observation with this
sample count, not a well-resolved tail distribution. Preliminary trials are
recorded separately and not pooled into the final statistics.

Linux CPU is user plus system seconds, including waited children. RSS is KiB.

| Workload | Median CPU before → after | Change | p95 CPU before → after | Median RSS before → after |
| --- | --- | --- | --- | --- |
| explicit1000 | 1.39 → 1.40 | +0.7% | 1.51 → 1.64 | 39332 → 39264 |
| explicit10000 | 1.98 → 1.90 | -4.0% | 2.09 → 1.93 | 39228 → 39340 |
| explicit100000 | 8.47 → 7.13 | -15.8% | 9.04 → 7.49 | 133964 → 124512 |
| auto10000 | 2.34 → 2.26 | -3.4% | 2.57 → 2.67 | 48632 → 48032 |
| public10000 | 1.55 → 1.59 | +2.6% | 1.85 → 1.83 | 39256 → 39252 |
| chunk180000 | 1.79 → 1.79 | +0.0% | 2.24 → 1.82 | 49716 → 49788 |

Windows CPU is parent-process milliseconds; helper-process CPU and RAM were not
measured. It cannot be directly compared with Linux child-inclusive CPU.

| Workload | Median parent CPU before → after | Change | Median elapsed before → after | p95 elapsed before → after |
| --- | --- | --- | --- | --- |
| explicit1000 | 3500.0 → 3578.1 | +2.2% | 3702.0 → 3656.0 | 3842.9 → 3890.5 |
| explicit10000 | 4093.8 → 3968.8 | -3.1% | 4209.8 → 4153.4 | 4511.5 → 4236.7 |
| explicit100000 | 6812.5 → 6406.2 | -6.0% | 7115.1 → 6692.0 | 7182.8 → 6990.7 |
| auto10000 | 4046.9 → 3921.9 | -3.1% | 4161.0 → 4136.5 | 4368.1 → 4268.3 |
| public10000 | 3640.6 → 3640.6 | +0.0% | 3725.0 → 3741.7 | 3952.0 → 3903.4 |
| chunk180000 | 3781.2 → 3734.4 | -1.2% | 3890.5 → 3843.6 | 4064.0 → 4068.6 |

On the 100,000-call workload, Linux median CPU improves 15.8% and peak RSS
falls 7.1%; Windows parent CPU improves 6.0%. Short workloads differ: the
1,000-call candidate is 0.7% above the current Linux baseline, and 2.2% above
the current Windows baseline. The public-selector Linux control costs 2.6%
more CPU. Chunked Linux CPU is unchanged. Some p95 values worsen. The change
therefore favors repeated execution, not every use of the public routing API.

For the older `fdd7e90` compute reference, the 1,000-call Linux median is 1.38 s
versus 1.40 s candidate (+1.4%); Windows parent CPU is 3546.875 versus
3578.125 ms (+0.9%). The earlier +2.2% versus that older reference is not
reproduced at the same size here, but a remaining cost is still observed.
This is not proof of eliminating the regression across conditions.

Eleven Windows suites passed on both variants, plus the expected original-error
fixture on each. The secure lock-publication test fails on both variants with
the existing unavailable-backend error; this is recorded as unsupported, not a
passing publication test. The first Windows validation run stopped at that
known unsupported operation before benchmarking. The final comparison records
it separately and does not relax the backend guard. This existing fixture used
its default root-level scratch directory on Windows; all three exact fixture
directories created by these attempts were content-checked and removed, as was
the dedicated executable-test directory. Future live fixture runs should set
`TMPDIR` to their owned scratch directory explicitly.

[Raw evidence](evidence/compute-validation-reuse-2026-09-23.json) includes every
final sample, identities, validation attempts, preliminary measurements and
cleanup observations. No general speedup, RAM saving, maximum-pause bound or
energy-saving claim follows from these workloads.


## Reproduction

Use baseline compute source `9ef1f35` and the candidate with identical retained
binaries per OS and identical scripts. The supplementary older reference uses
only the compute module from `fdd7e90` with the same dependency snapshot; it is
not a rebuild of every older component. Prepare isolated package directories,
run `pblang build`, and publish their lockfiles with `tools/pblock.sapp` on Linux.
Windows tests consume those portable lockfiles below the user-authorized
executable-test directory; no antivirus, system or approval setting is changed.

```text
pblang run bench/compute_explicit_workers.sapp --allow-all -- compute 1000
pblang run bench/compute_explicit_workers.sapp --allow-all -- compute 10000
pblang run bench/compute_explicit_workers.sapp --allow-all -- compute 100000
pblang run bench/compute_discovery.sapp --allow-all -- small 10000
pblang run bench/compute_route_validation.sapp --allow-all -- public 10000
pblang run bench/compute_chunk_lifetime.sapp --allow-all -- 180000
```

The 100,000-call case supplements the original small workload rather than
replacing it. New benchmark and tests are copied unchanged into the baseline.
No pure-CPU benchmark proves energy savings or GPU/NPU/SSD utilization.

## Remaining work

Windows still consumes a Linux-published lock; secure publication on Windows
remains unimplemented. The Windows filesystem backend still reports unverified
FFI-symbol and private-root ACL contracts. No readiness flag or admission check
was relaxed to bypass that blocker.

Long collector pauses, runtime reference cycles, owner isolation across VMs,
external resource/FFI lifetime, polyglot integration, real accelerator kernels
and broader platform coverage remain open. The retained-host GC contract still
cannot prove roots across nested VMs on the same host thread. This optimization
does not solve those problems or justify enabling destructive collection.

The installed PB-MCP does not invoke this compute route. Its desktop maintenance
and frame pipeline remain unchanged; no live-view benefit was measured. The
unrelated workspace changes were preserved. No external corporate server was used.

Graphify's AST-only update exceeded 35 seconds; its owned process group was
terminated and reaped. It does not establish semantic PBLang coverage.
