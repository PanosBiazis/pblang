# Self-hosted conversion cost: separate imports from execution

A whole-CLI conversion benchmark includes launcher admission, dependency/module
loading, compilation, guest execution, and process teardown. Do not interpret its
wall time as the cost of `parse_number`, cleanup, or one VM opcode.

The frozen workload compiles a program that allocates two scalar owners, opens a
lifetime, borrows one owner, and fails `parse_number("bad")`. It executes the chunk
100 times, catches each escaping error, and explicitly closes every context.
Both variants must print exactly `escape_benchmark_pass` and exit successfully.
The explicit close makes this a cost fixture, not proof of automatic cleanup.

## Isolation protocol

- Keep the primary fixture SHA-256
  `4d1ade5412d8b04539c409ddf8cebf14b259f00097143849368a32e61e208a83`.
- Use 2 warmups and 11 measured samples per variant, alternating AB/BA order.
  Serialize workload groups through the shared benchmark lock. Keep all samples.
- Preserve 5 seconds/sample, 50 seconds/active group, 512 MiB address space,
  5 CPU seconds and 64 KiB captured output. Lock waiting is outside active time.
- Run source-hidden, with each immutable release and private fixture mounted
  read-only, network namespaces isolated and unrelated capabilities denied.
- Record actual CLI child `wait4` user+system CPU, CLI peak RSS, whole-invocation
  wall time, load averages, fixture/manifest hashes and cleanup outcomes.
- Test a nearby committed revision and a private counterfactual changing only
  `stdlib/compiler/vm.sapp`. The latter is a diagnostic artifact, never a release;
  its inherited SOURCE_REVISION does not identify the modified VM contents.

The separate frozen diagnostic fixture is
`tests/benchmarks/selfhost_conversion_stages.sapp` (SHA-256
`5348b4beed7b8f6311e6c7af29f6af57f463fdead43d6b3a2e4b955b6095fde1`).
It inserts `now_ms()` boundaries around imports, compilation and the 100-run loop.
Its three values are **wall milliseconds**, not phase CPU measurements. They
exclude launcher admission and process startup/teardown outside the script.
Keep its bytes unchanged when comparing these receipts; formatting changes need
a new fixture identity. Its instrumentation makes it a separate workload.

## Recorded local observations (2026-10-08)

Historical cycle20 compared `514dd620d7bd` with `f4bcc7d863c7`: median CPU
+9.65%, wall +20.65%, peak RSS +1.88%. Those negatives remain recorded.
The cycle21 groups below are new observations; they do not erase that run.

| Comparison | Median CPU change | Median wall change | Median peak RSS change | Paired CPU / wall change |
|---|---:|---:|---:|---:|
| Nearby `5a7142697456` → `f4bcc7d863c7` | +12.43% | +11.85% | -0.65% | +7.01% / +6.76% |
| Only initialization reverted → current VM | -8.91% | -9.50% | -1.03% | -4.56% / -4.34% |
| Instrumented `514dd620d7bd` → `f4bcc7d863c7` | -2.52% | -2.43% | -0.83% | +0.45% / +0.25% |

All 78 invocations passed (66 measurements plus 12 warmups). Median import wall
intervals were 857.11 ms and 833.59 ms, compilation 1.57 ms and 1.45 ms, and
100-run execution 12.68 ms and 12.33 ms. In both versions, the median per-sample
import share of the instrumented interval was about 98.45%.

Nearby revisions had CPU ranges 0.743–1.317 s and 0.845–1.553 s, with overlapping
ranges and changing signs across the isolation groups. Shared-host load was
about 7.65–9.50 (one-minute load average); memory/swap pressure was present.
These are observations on a shared i7-1195G7 Linux host, not causal attribution,
a general speedup, or evidence that initialization cleanup should be reverted.
No VM micro-optimization was justified by this experiment. Import work dominates
this fixture, but parsing, filesystem reads and package verification were not
individually profiled. Preserve module-origin and capability contracts in any
future import optimization.

RSS is peak process resident memory, not physical reclamation or a leak proof.
For 11 samples, nearest-rank p95 equals the maximum; it is not an independent
tail estimate. GPU/NPU/TPU utilization, energy and other operating systems were
not measured. Device availability must not be reported as measured zero usage.

## Validate existing receipts

```sh
python3 tests/tooling/selfhost_cost_summary.py escape-results.json
python3 tests/tooling/selfhost_cost_summary.py stage-results.json --stages
python3 -m unittest discover -s tests/tooling -p test_selfhost_cost_summary.py
```

The summary tool requires the complete successful frozen receipt and recomputes
statistics from raw samples. It rejects changed fixtures/bounds, reordered or
missing pairs, misclassified warmups, failed or timed-out invocations, invalid
CPU accounting and malformed stage durations. It reads at most 1 MiB and writes
JSON to stdout; it starts no workload, modifies no release and sends no data.
Median ratios and medians of paired ratios are reported separately because they
answer different questions. `causal_attribution` and `speedup_established` remain
false. Receipt validation cannot authenticate how someone collected the data or
independently prove recorded sandbox/manifest assertions. Retain original raw
receipts, measuring scripts, frozen contracts and host context privately.

For automatic cleanup contracts and remaining host/FFI limits, see
[SELFHOST_HOST_ERROR_CLEANUP.md](SELFHOST_HOST_ERROR_CLEANUP.md).

## Narrow module-cache dependency (cycle22)

The full frontend imports `compiler/module_cache.sapp` even when a caller only
compiles source. Previously that cache imported `secure_file.sapp`, pulling in
its Linux, macOS, Windows and SHA-256 modules. The cache only called five of its
private-parent operations, and only on Linux.

`compiler/module_cache_file.sapp` now specializes that internal contract using
`linux_dirfs.sapp`. Its imports remain literal top-level declarations, preserving
module origin and the source inliner's existing import rules. Package resolution
and complete locked dependency-tree verification are unchanged; no trusted-path
shortcut or cross-process validation cache was added. The helper is part of the
cache's default toolchain revision, so upgrading it invalidates old entries.

The helper always requires a current-user-owned mode-0700 parent. Creation is
exclusive, only one missing parent component is admitted, each component is
opened without following links, reads retain the bounded descriptor snapshot,
and writes retain staged fsync/rename/parent-fsync publication. It never changes
an existing parent's mode or falls back to ordinary path I/O. Atomic replacement
of an existing final symlink or hard link replaces that directory entry without
writing through it; this remains a replaceable cache, not a no-overwrite archive.
Non-Linux module-cache branches and the general secure-file library are unchanged.

The intentionally small specialization duplicates the five-operation private
contract; changes to that contract must update the differential tests and both
implementations. This does not add exception-safe cleanup to the underlying
LinuxDirFS operations or repair the existing non-Linux post-read quota limitation.

Qualification (prepared Linux releases, disposable source-hidden fixtures):

```sh
python3 tests/tooling/module_cache_file_check.py --candidate /path/to/candidate \
  --reference /path/to/reference --report parity.json --group parity
python3 tests/tooling/module_cache_file_check.py --candidate /path/to/candidate \
  --reference /path/to/reference --report denials.json --group denials
python3 tests/tooling/module_cache_file_check.py --candidate /path/to/candidate \
  --reference /path/to/reference --report integration.json --group integration
```

These checks compare full structured results with the previous secure-file path,
exercise UTF-8 byte boundaries, malformed paths/limits, missing and non-private
parents, symlinks, hard links, FIFOs, repeated descriptor closure, denied FFI
capabilities, cold/warm cache use, corruption, and source invalidation. The tests
use explicit existing libc symbols and retain the existing `stat` subprocess
policy checks; this change does not claim an exec-free persistent cache.

The isolated cycle22 comparison retained the active `942d13f77be4` payload and
native binary, changing only the cache module/helper and regenerated lock
(plus the new qualification fixtures in the candidate). All 26 invocations
passed: two warmups and 11 measured samples per variant, alternating AB/BA.
Two leases contained pairs 0–6 and 7–12, respectively, retaining the 5-second
sample and 50-second active-lease limits. The earlier single-lease attempt
retained four successful warmups then failed to produce the required inner-usage
record; it remains incomplete and was not used in the statistics below.

| Actual CLI metric | Baseline median | Candidate median | Median change | Paired median change |
|---|---:|---:|---:|---:|
| CPU, user + system | 1.206815 s | 1.067380 s | -11.55% | -18.74% |
| Whole invocation wall | 1.252567 s | 1.122340 s | -10.40% | -18.16% |
| Peak RSS | 37,128 KiB | 34,792 KiB | -6.29% | -6.25% |

Nearest-rank p95 (the maximum at n=11): CPU 2.079428 → 1.487513 s;
wall 2.662505 → 1.613796 s; RSS 37,320 → 35,044 KiB. The variation is substantial
on the shared host: these are bounded local observations for this exact import
workload, not a guarantee for arbitrary programs, platforms, or the later combined
release. Existing cycle20 negative evidence is retained. Lower peak RSS does not
prove physical memory reclamation. No safety check was removed to obtain these
measurements.

The real cache dependency closure also passed `inline_source_prefixed` before
and after the change: 15 linked modules became 11. This is covered by
`tests/semantics/module_cache_link_test.sapp`. It is distinct from the existing
nested import in `zipfile.extract` that blocks that unrelated whole-program
link route; this change neither modifies `zipfile` nor relaxes linker rejection.

### Follow-up: do not report success after unconfirmed parent close

Fault injection after the initial import optimization confirmed that the inherited
facade ignored false close results. The cache adapter now attempts close once and
requires a boolean `true`. False, nil, non-boolean and opaque close exceptions
produce `ok:false, cleanup_uncertain:true`. A successful write also reports
`publication_may_have_occurred:true`; callers must not infer rollback or blindly
retry an effectful operation. A known primary error map and its extra fields are
retained when cleanup also fails. Newly created parents are rejected when their
first parent close cannot be verified. No shared LinuxDirFS or secure-file code
was changed for this correction.

Run `module_cache_file_check.py --group faults` with the same candidate/reference
arguments to exercise the 15 fault scenarios. The fixture replaces exactly one
import in private source copies with a deterministic backend model; it acquires
no native descriptors. Actual descriptor-bound I/O and close-count checks remain
in the separate parity/integration groups.

Five injected unexpected backend exceptions still preserve the original
exception and skip the adapter's subsequent parent close, as in the original
facade. This remains open. PBLang catch currently exposes no original exception
payload (`resource_scope.sapp` documents the same limit), and retrying effectful
operations to reconstruct that exception is unsafe. This change verifies normal
return/cleanup-failure handling; it does not claim generalized exception-safe
cleanup, physical resource reclamation, or a closed whole-runtime ownership audit.
The preceding timing table describes the initial specialization; subsequent
cleanup changes require their own frozen measurement before using those numbers
for the newer source revision.
