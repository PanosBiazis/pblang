# Lazy defaults in the public compute estimator

## Change and contract

The public `estimate_work` previously constructed a workload-profile map on every
call, even when both per-item hints were already present. It now reads those two
fields once and constructs that temporary profile only for absent or `nil`
values. Explicit zero transfer is retained. Numeric type, integrality, bounds,
item count, policy format, saturating arithmetic, result shape and validation
order (work before transfer) are unchanged. Every call returns a fresh estimate;
no caller map is changed, trusted, or cached across calls.

The internal validated route of `run_controlled` already bypassed this public
estimator and is unchanged. Its small and large execution benchmarks are
controls, not evidence of a newly optimized internal algorithm. This PBLang-only
change neither alters retained binaries nor adds product limits, foreign code,
native capabilities, automatic retries or new concurrency guarantees.

## Correctness and security review

The new suite covers all 12 workload/default groups and independent absent,
`nil`, explicit-work and zero-transfer combinations (108 estimate records).
It also checks missing/unknown workload values, malformed numeric hints,
fractions, NaN/infinity, empty input, mutations between calls, detached results
and preservation of caller-owned nil fields. Both versions produce the digest
`b2e0337196a001b8bdbc65d64ef88816f9f5b5148c573089dd9ff59451d8424c`.
The two existing 420-route parity suites retain their complete before/after
output digests. Checks that caller-supplied markers cannot select the private
validated route remain passing.

Review of the changed estimator found no input-validation bypass: constants
passed as validator fallbacks are reached only after nil hints were replaced
with the same workload defaults. No filesystem, FFI, resource ownership or
permission path changed. Simultaneously mutating public maps across native
threads is still outside the proven synchronization contract. This review is
scoped to the change, not certification of the compiler/runtime or entire product.

34 Linux suites passed, including compute, tensors, CPU parallelism, memory
contracts, FFI/polyglot, five emulator-related suites, portable locks and Windows
ABI structure contracts. Twelve Windows suites passed on both versions. Linux
build, PBLang-native portable lock publication and lint passed in an isolated
package. Lint reports existing style warnings, an unused-argument false positive,
and the intentional infinity-minus-infinity NaN test. The initial build refreshed
an inherited dependency hash; the final build succeeded after lock publication.

## Paired measurements

Baseline: `a9846dc818679fc183471e897be746cd66cc5b08`. Candidate source and retained
binary hashes are in the [raw evidence](evidence/compute-lazy-estimate-defaults-2026-09-23.json).
Same scripts, dependencies and binary per OS; one excluded warmup per workload
and variant; alternating order. Main Linux runs have nine samples per variant,
supplemental Linux and Windows runs seven. Startup/imports are included and
machines are shared. Nearest-rank p95 is the maximum with these small sample
counts, not a robust bound on tail latency.

Linux CPU is user+system seconds, including waited children; RSS is median peak
process KiB. Supplemental estimate loops do not replace the original workloads.

| Workload | Median CPU s before → after | Change | p95 CPU s before → after | Median peak RSS KiB before → after |
| --- | --- | --- | --- | --- |
| public10000 | 1.60 → 1.58 | -1.2% | 1.94 → 1.88 | 39208.00 → 39380.00 |
| public100000 | 4.10 → 3.86 | -5.9% | 4.60 → 3.92 | 67436.00 → 55732.00 |
| explicit1000 | 1.36 → 1.35 | -0.7% | 1.55 → 1.43 | 39448.00 → 39204.00 |
| explicit100000 | 7.03 → 7.10 | +1.0% | 8.00 → 7.85 | 124612.00 → 124648.00 |
| auto10000 | 2.25 → 2.23 | -0.9% | 2.74 → 2.39 | 48024.00 → 48048.00 |
| missing100000 | 2.42 → 2.22 | -8.3% | 2.62 → 2.30 | 46676.00 → 46596.00 |
| estimate100000 | 2.05 → 1.94 | -5.4% | 2.20 → 2.37 | 46676.00 → 39328.00 |

The large public selector reduces median CPU by 5.9% and median peak RSS by
17.4% (67,436 → 55,732 KiB). The original 10,000-call selector improves only
1.3% CPU while RSS increases about 0.4%. Explicit direct estimates reduce median
CPU 5.4%, but their p95 worsens from 2.20 to 2.37 s. Missing-default estimates
improve median CPU 8.3%, with almost unchanged RSS. The unchanged 100,000-call
execution control costs about 1.0% more CPU. There is no general speedup or
RAM-saving claim, and no guarantee on collector pauses or worst-case latency.

Windows CPU is parent-process milliseconds; child CPU and RAM were not measured.
Elapsed milliseconds include waits. These measurements are not directly
comparable with Linux child-inclusive CPU.

| Workload | Median parent CPU ms before → after | Change | p95 elapsed ms before → after | Median elapsed ms before → after |
| --- | --- | --- | --- | --- |
| public10000 | 3875.00 → 3640.62 | -6.0% | 4188.06 → 4259.26 | 4067.64 → 4068.97 |
| public100000 | 5062.50 → 5000.00 | -1.2% | 5975.28 → 5823.89 | 5553.26 → 5299.17 |
| explicit1000 | 3656.25 → 3593.75 | -1.7% | 4204.71 → 4233.20 | 3967.24 → 3953.05 |
| explicit100000 | 6578.12 → 6500.00 | -1.2% | 7555.80 → 7266.27 | 7146.25 → 6994.47 |

The Windows public selector improves median parent CPU 6.0% at 10,000 calls and
1.2% at 100,000 calls. The smaller run's elapsed median is essentially unchanged
and its p95 is worse. The small execution control shows -1.7% median CPU, but its
internal path did not change; this is not proof that the older +2.2% regression
was eliminated. Prior percentages came from a different measurement session and
must not be subtracted from these paired results to claim cumulative gains.

## Reproduction

Prepare isolated `before/pblang` and `after/pblang` packages from the baseline,
changing only the candidate compute module. Copy the new test and supplemental
benchmark identically to both. Run `pblang build` and
`pblang run tools/pblock.sapp --allow-all` on Linux to publish relative locks.
Run with the same retained binary for each variant:

```text
pblang run bench/compute_route_validation.sapp --allow-all -- public 10000
pblang run bench/compute_route_validation.sapp --allow-all -- public 100000
pblang run bench/compute_explicit_workers.sapp --allow-all -- compute 1000
pblang run bench/compute_explicit_workers.sapp --allow-all -- compute 100000
pblang run bench/compute_discovery.sapp --allow-all -- small 10000
pblang run bench/compute_estimate_defaults.sapp --allow-all -- missing 100000
pblang run bench/compute_estimate_defaults.sapp --allow-all -- explicit 100000
```

Windows ran the first four workloads below the user-authorized executable-test
root; child `TMPDIR` was explicitly confined to its owned scratch directory.
No antivirus, account, machine setting or local desktop approval was changed.
Source and executable hashes matched at cleanup; no owned executable remained
running, and the entire unique Windows test directory was removed.

## Remaining work

The current Windows portable-lock publication fixture was run separately and
failed with `secure file backend authority unavailable`; it is an unsupported
operation, not a passing publication test. The current publisher also uses the
Linux directory backend for dependency roots. A correct Windows implementation
still needs verified symbol/ABI handling, protected-root/ACL and handle-relative
path contracts, plus publication integration and adversarial live tests. No
readiness flag was relaxed.

The retained-host runtime still cannot prove roots across same-thread nested
VMs. Automatic destructive collection, runtime reference cycles, long collection
pauses, FFI lifetime, polyglot integration and wider platform/device coverage
remain unresolved. Actual GPU/NPU acceleration requires kernels and measured
result parity; test runtime hooks do not establish hardware acceleration.

PB-MCP's installed desktop pipeline was not changed and does not gain a proven
live-view benefit from this estimator. No Specisoft server was used. The 170
pre-existing status entries (212 hashed files) were preserved. Graphify's
AST-only update exceeded 35 seconds; its owned process group was terminated
and reaped. No semantic PBLang graph coverage is claimed.
