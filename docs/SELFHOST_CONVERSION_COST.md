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
