# Automatic CPU discovery avoidance for serial execution

## Behavior

The compute executor now uses the known batch length to avoid resolving an
unused default worker count. All policy fields are validated first. If the
worker count is omitted or nil, detection remains fresh whenever parallel CPU
execution is possible. Otherwise a serial internal policy is sufficient and
produces the same public route and result envelope as a fully resolved policy.

This applies to empty/singleton batches, batches below `min_parallel_items`,
disabled parallelism, and an explicit sequential preference. Accelerator
eligibility and fresh device discovery are unchanged. A small accelerator batch
can still execute a registered kernel and, if that fails, receives the same
serial CPU fallback as before. Public `policy()` always resolves the worker
default; it is not replaced with a serial-only policy. Explicit worker values
retain validation and do not acquire a new CPU-count dependency.

Controlled storage reads use the same principle for empty/singleton batches
and batches below `min_parallel_files`. Real file contents and output ordering
are unchanged, and detection still occurs at the parallel threshold. This
removes subprocess work on Linux without adding a cache, a product size limit,
or a new public API. No Rust source or polyglot block was added.

## Verification

The new regression suite compares complete routes from public `policy()` plus
`select_device()` with actual execution for eight option sets and eight batch
sizes. It covers the parallel threshold, explicit/nil/default workers, disabled
parallelism, strict accelerator failure, malformed policies even on empty
batches, registered-executor failure, chunked execution, and real serial and
parallel storage reads. The test also passes on the unmodified baseline.

Twenty-five Linux suites, build, portable lock refresh, and lint passed. Lint
retains existing style warnings and the known false unused-arguments warning
for benchmark argument handling. No broad safety or platform certification is
implied by those focused checks.

A successful-exec process trace of five calls per case recorded:

| Case | Baseline `nproc` executions | Candidate `nproc` executions |
| --- | --- | --- |
| Supplied devices, four items, default workers | 5 | 0 |
| Empty storage, default workers | 5 | 0 |
| Compute batch at the parallel threshold | 5 | 5 |
| Public policy with explicit workers | 0 | 0 |

## Measurements

Compared with the current baseline `fdd7e90`, using identical retained runtimes,
identical workloads and separate clean packages. One warmup per variant and
workload was excluded; nine Linux and seven Windows samples per variant were
collected in alternating order. Every run checked its expected output checksum.
Existing discovery and explicit-worker benchmarks were unchanged. Raw samples,
runtime hashes, checks and process counts are in
[evidence](evidence/compute-automatic-serial-cost-2026-09-23.json).

Linux CPU includes user/system time of the process and waited children; times
include startup and import verification. The table shows median CPU seconds,
nearest-rank p95 CPU seconds, and median peak RSS KiB, baseline to candidate.

| Workload (rounds) | Median CPU s | CPU change | p95 CPU s | Peak RSS KiB |
| --- | --- | --- | --- | --- |
| Supplied devices, default workers (3000) | 3.97 → 1.56 | -60.7% | 4.46 → 1.57 | 39332 → 39424 |
| Explicit worker control (1000) | 1.40 → 1.43 | +2.1% | 1.44 → 1.47 | 39136 → 39284 |
| Public policy, explicit worker (1000) | 1.39 → 1.39 | +0.0% | 1.40 → 1.42 | 39216 → 39304 |
| Default workers at threshold (500) | 1.81 → 1.81 | +0.0% | 1.84 → 2.29 | 39224 → 39384 |
| Empty storage (1000) | 2.12 → 1.36 | -35.8% | 2.16 → 1.39 | 39200 → 39304 |
| Small auto-discovery workload (50) | 1.40 → 1.35 | -3.6% | 1.43 → 1.46 | 39376 → 39380 |

The supplied-device workload improves 60.7% and empty storage 35.8% in these
Linux measurements. The explicit-worker control worsens 2.1%. The threshold
median is unchanged but its worst/p95 sample grows from 1.84 to 2.29 seconds;
the small workload also has a worse p95. Median RSS changes range from almost
zero to +0.4%; no RAM saving or general worst-time improvement is established.
Empty-storage timing does not measure disk throughput.

The authorized Windows workstation ran only in an owned directory below the
requested executable-test root. Ten suites passed on each variant (20 runs).
Both consumed the Linux-published portable v2 lockfiles with the same current
Windows retained runtime. The outer SSH wait timed out after 330 seconds, but
the complete validation and 70-sample measurement files were recovered afterward;
no test was counted from the timeout alone.

Windows CPU below is **parent process only**, in milliseconds; it is not directly
comparable with Linux child-inclusive CPU. Windows CPU-count lookup already uses
a platform hint rather than the Linux subprocess route. Windows RAM was not
measured. The same checksum gates and alternating pairs were used.

| Workload | Median CPU ms | CPU change | p95 CPU ms | Median elapsed ms |
| --- | --- | --- | --- | --- |
| Supplied devices, default workers (3000) | 3781.2 → 3796.9 | +0.4% | 3875.0 → 3906.2 | 3924.4 → 3915.3 |
| Explicit worker control (1000) | 3546.9 → 3562.5 | +0.4% | 3734.4 → 3765.6 | 3633.7 → 3634.5 |
| Public policy, explicit worker (1000) | 3515.6 → 3515.6 | +0.0% | 3578.1 → 3562.5 | 3617.1 → 3613.0 |
| Default workers at threshold (500) | 3515.6 → 3453.1 | -1.8% | 3593.8 → 3562.5 | 3627.3 → 3660.6 |
| Empty storage (1000) | 3484.4 → 3468.8 | -0.4% | 3531.2 → 3515.6 | 3606.6 → 3584.9 |

Windows medians are close, with variation; supplied-device and explicit-worker
CPU medians increase about 0.4%. Some elapsed-time tails worsen. With seven or
nine samples, the reported nearest-rank p95 is the maximum observation, not a
well-resolved population tail. These shared-machine measurements do not prove
all historical regressions are resolved, GPU/NPU gains, real-time behavior,
energy savings, or a speedup in installed applications.

Reproduction: build each isolated package, publish its lock with `tools/pblock.sapp`,
and run the following from the package root using the same retained binary:

```text
pblang run bench/compute_discovery.sapp --allow-all -- injected 3000
pblang run bench/compute_explicit_workers.sapp --allow-all -- compute 1000
pblang run bench/compute_serial_discovery.sapp --allow-all -- policy 1000
pblang run bench/compute_serial_discovery.sapp --allow-all -- threshold 500
pblang run bench/compute_serial_discovery.sapp --allow-all -- storage_empty 1000
pblang run bench/compute_discovery.sapp --allow-all -- small 50
```

The last workload was measured on Linux only. Use the same new benchmark file
with both revisions; do not change rounds or expected results between variants.

## Windows publication and installed PB-MCP boundaries

Inspection found that a Windows directory-handle implementation already exists
in `windows_dirfs.sapp`; it should not be described as entirely absent. However,
its availability contract explicitly reports unverified FFI-symbol and private
root ACL guarantees. `secure_file.status()` does not enable general publication
on Windows, while `portable_lock.publish()` also directly uses Linux directory
opening. The earlier successful cross-platform v2 lock consumption therefore
still does not establish a secure Windows publisher. These admission checks
were not relaxed in this change. A complete fix needs the bridge/package trust,
root-handle acquisition, publication integration, and adversarial live tests to
be completed together.

The currently configured PB-MCP desktop release was inspected: 77 PBLang files,
no `compute.sapp`, and no textual reference to that module. Updating this library
alone would not speed up its live view, so no installed desktop package or
approval setting was changed. That integration needs a measured application
change rather than copying an unused module.

Collector pauses, general runtime reference cycles, FFI/polyglot integration,
real GPU/NPU kernels and wider platform coverage remain open. No Specisoft
server was used. Graphify update reached its 35-second bound; its process group
was terminated, and it supplies no semantic coverage claim for this change.
