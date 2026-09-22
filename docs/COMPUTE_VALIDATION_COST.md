# Preferred-device validation and bootstrap portability

## Retained change

`compute.policy` and `compute.select_device` stop scanning the seven supported
preferred-device names after the first match. CPU policy construction compares
two names instead of seven; automatic policy construction compares one. The
selector makes the same change. The checks after those loops, including worker
counts, thresholds and transfer limits, remain in place. Unknown names still
scan the complete list and fail. No new public API, cache, scheduling limit, or hardware executor is introduced.

Both `policy` and `storage_read_controlled` now query the CPU count only when
`max_workers` is omitted or `nil`. An explicit worker count already determines
the value, so computing its unused default needlessly spawned `nproc` on Linux.
Explicit counts retain the existing 1–64 validation, including counts exceeding
the detected core count. Missing and `nil` retain fresh detection and existing
defaults; malformed values still fail. This does not cache hardware state.

A process trace of five explicit-worker calls observed five successful `nproc`
executions before and zero after, separately for compute and empty storage. The
empty-storage case measures scheduling overhead, not physical disk throughput.

`compute_preferred_validation_test.sapp` exercises all seven names, uppercase
normalization at policy construction, result/route equivalence, malformed fields
after a successful match, unsupported names, and mutated unnormalized policies.
It also covers explicit, omitted, nil and malformed worker counts for compute
and storage. The same test passes on the before and after variants.

## Initial measurement: preferred-name loops only

[Raw samples and validation](evidence/compute-validation-cost-2026-09-23.json).
Before is commit `3345e61`; the older comparison is `b4f47ff`, before the previous
discovery optimization. All variants use the unchanged
`bench/compute_discovery.sapp`. Build clean packages named `pblang`, refresh the
local lock, and run:

```
pblang run bench/compute_discovery.sapp --allow-all -- injected 3000
pblang run bench/compute_discovery.sapp --allow-all -- small 50
pblang run bench/compute_discovery.sapp --allow-all -- cpu 50
```

Linux used one warmup and nine measured samples per variant, rotating variant
order. CPU is user plus system time, including waited child processes. Values
include startup and dependency validation. This was a shared laptop, not an
isolated machine; remote package transfer overlapped part of measurement.
Nearest-rank p95 is the maximum with this sample count.

| Linux workload | CPU median before / after | CPU p95 before / after | Peak RSS median KiB before / after |
| --- | --- | --- | --- |
| Supplied list, 3,000 calls | 4.48 / 4.45 s | 4.84 / 4.92 s | 39,324 / 39,208 |
| Automatic small batch, 50 calls | 1.63 / 1.44 s | 1.91 / 1.56 s | 39,148 / 39,168 |
| Explicit CPU, 50 calls | 1.44 / 1.43 s | 1.69 / 1.64 s | 39,232 / 39,312 |

The older supplied-list variant measured 4.45 s median CPU and 4.86 s p95, with
39,300 KiB median peak RSS. The previous 5–6% regression did not reproduce in
this run: the unchanged current baseline was only 0.7% slower than the older
variant. Consequently it cannot be claimed that this patch alone eliminated
the historical regression. There is no demonstrated general RAM reduction;
some p95 results worsened. Small-workload measurements also include substantial
fixed cost and variation and do not establish a universal speedup.

Windows used the authorized desktop, an isolated directory under the
user-selected executable root, one warmup and seven alternating pairs for the
same supplied-list workload (3,000 calls). Median wall time was
2,017.816 / 2,001.733 ms; median parent-process CPU was 1,968.75 / 1,906.25 ms.
Wall p95 worsened from 2,027.012 to 2,063.023 ms. Windows parent-process CPU is
not directly comparable with the Linux child-inclusive measurement. Reliable
Windows peak working-set measurements were not obtained; they are not zero.
No antivirus exclusions, quarantine restoration, or desktop-control permissions
were changed.

## Final measurements: explicit-worker discovery avoidance

After the local build and package transfer completed, the final candidate was
measured against the unchanged `3345e61` baseline: one warmup and nine
alternating pairs per workload. The original supplied-list benchmark was
retained; the new `bench/compute_explicit_workers.sapp` adds separate explicit
worker cases, rather than replacing the previous regression workload.

| Linux workload | CPU median before / after (s) | CPU p95 before / after (s) | Peak RSS median KiB before / after |
| --- | --- | --- | --- |
| Supplied list, default workers, 3,000 calls | 4.34 / 4.24 | 4.72 / 4.36 | 39,344 / 39,308 |
| Supplied list, explicit workers, 1,000 calls | 2.30 / 1.48 | 2.37 / 1.99 | 39,216 / 39,300 |
| Empty storage, explicit workers, 1,000 calls | 2.20 / 1.41 | 2.37 / 1.44 | 39,180 / 39,116 |
| Automatic small batch, 50 calls | 1.45 / 1.45 | 1.47 / 1.95 | 39,380 / 39,308 |
| Explicit CPU, default workers, 50 calls | 1.45 / 1.45 | 1.55 / 1.47 | 39,312 / 39,204 |

The explicit-worker compute and empty-storage scheduling cases use about 36%
less median CPU here. Empty storage performs no disk reads: this is not a disk
throughput claim. The original supplied-list case improved by about 2.3% against
the current baseline in this run. General RAM savings are not demonstrated.
The small automatic case retained the same median but its p95 worsened.

Reproduce the additional cases on both unmodified baseline and candidate with:

```
pblang run bench/compute_explicit_workers.sapp --allow-all -- compute 1000
pblang run bench/compute_explicit_workers.sapp --allow-all -- storage 1000
```

Final Windows comparisons used the same retained binary for both source
variants, with one warmup and seven alternating pairs. The rebuilt binary was
used for compatibility validation separately, not mixed into performance pairs.

| Windows workload | Wall median before / after (ms) | Parent CPU median before / after (ms) | Wall p95 before / after (ms) |
| --- | --- | --- | --- |
| injected | 2008.273 / 2069.920 | 1953.125 / 1953.125 | 2154.783 / 2106.240 |
| compute | 2005.361 / 1980.634 | 1890.625 / 1859.375 | 2171.602 / 2086.483 |
| storage | 1930.819 / 1910.426 | 1843.750 / 1859.375 | 1996.863 / 2031.657 |

The Windows injected case worsened about 3.1% in median wall time with unchanged
median parent CPU. Storage median parent CPU rose about 0.8%. Windows uses an
environment CPU-count hint in these tests, so the Linux process-launch saving
does not translate into the same Windows speedup.


## Lockfile diagnosis

A byte-identical synthetic package was built on both platforms. Its dependency
contains `a.txt` with `hello\n`, a directory `empty` containing `b.txt` with
`world\n`, and an additional empty directory `really-empty`. The package manifest
and source were identical. The manifest digest matched on both platforms:
`289ea7927f54d22db609e9ddbd3ec93cb4dade2181407fa69e41338a80433085`.

The Linux bootstrap emitted `content_hash_algorithm = "pblang-package-tree-v2"`
and dependency digest
`7d54e9d45c0e5caf594fb4e4b083cb26380cc0370b5a580b788c102ba36ef12a`.
The retained Windows executable emitted no algorithm field and digest
`46ad546bdb6afcebb51684e839df6674aac3bb1cbe7a07251e37960a1b1cb521`.
Thus the tested Windows artifact is a legacy lock producer. The current Rust
reference source explicitly hashes directory identities and normalizes path
separators for v2; that source must not be mistaken for behavior of the old
Windows executable.

Do not relabel a legacy digest as v2 or replace a trusted expected digest with
a mismatch value. Windows functional tests used freshly generated local test
locks, not published release locks. A compatible bootstrap and a supported
secure Windows publication backend are separate requirements. The PBLang
`portable_lock.publish` currently depends on Linux no-follow directory handling;
rebuilding the bootstrap alone does not provide that Windows backend.

The existing Rust bootstrap was rebuilt from clean tracked source at `3345e61`
with locked, offline dependencies and two build jobs. No Rust source was added
or changed. The initial 420-second attempt was stopped and resumed from its
compiled dependencies; the resumed build completed in 106 seconds. The new
Windows executable emitted v2 with exactly the Linux fixture digest above.
Nine targeted suites then passed using the Linux-produced portable lock
without regeneration on Windows. A negative publication check still rejected
the unsupported secure Windows writer. This separates the repaired test
bootstrap compatibility from the still-missing Windows publication backend.

The verified executable was retained locally at
`target/x86_64-pc-windows-gnu/release/pblang.exe`; the previous artifact was
preserved as `pblang.legacy-78035e13.exe` beside it. The new SHA-256 is
`33c9c4c6fd6dbc6e7134b0509ce220e2c9037dec0e7570c7b8e72b52e2d3fc69`.
These are local build artifacts, not a signed release or PB-MCP installation.

## Validation and outstanding work

Twenty-four Linux suites passed, covering routing, tensors, resources, FFI,
polyglot policy, emulator behavior and portable locks. The relevant build,
portable lock refresh, and lint passed. Four targeted suites passed on both
Windows variants (eight executions), as did both package builds. The Windows
fixture build also completed; its output exposed the legacy-format issue.
The rebuilt Windows bootstrap adds nine successful targeted suite executions
against the Linux lock. The final PBLang changes were checked again, including
build, lint, and all 24 Linux suites. Graphify update reached its 35-second bound
and its process group was terminated; it is not evidence of `.sapp` semantics.

The generic scheduler still needs real, parity-tested GPU/NPU execution backends.
Hardware inventory and a mock registered executor do not demonstrate hardware
acceleration. Collector pauses, runtime reference cycles, general runtime/FFI
and polyglot integration, and wider platform testing remain open. PB-MCP was
not updated and no live-view performance benefit was measured. No Specisoft
server was used.
