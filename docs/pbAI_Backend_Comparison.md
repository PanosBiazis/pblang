# Backend comparison admission

`stdlib/pbai_optimization.sapp` is a pure advisory selector. It consumes caller
reports and does not measure, probe, activate hardware, load a model or train.
The standalone module and the shared PBLang module expose the same contract.
The shared module is an explicit import; this addition does not extend the
embedded legacy facade or alter `pbai_accel` hardware detection.

## Same artifact, valid measurements

An eligible accelerator must have the exact same `model_digest` string as the
selected verified deterministic CPU report, as well as the existing workload,
quality, runtime, precision and operator gates. A faster report for a different
model is ignored. If no matching eligible accelerator passes, CPU remains the
fallback. The existing selection order is retained: the last eligible CPU report
is the reference and the lowest eligible accelerator median wins. Callers should
submit reports for one artifact/workload. Digests and verification flags are
caller assertions, not independently authenticated benchmark receipts.

Warmups and samples must be finite exact integers, at least 2 and 5 respectively.
`peak_memory_bytes` and `artifact_bytes` must be positive finite exact integers.
All four count fields are bounded by 9007199254740991, the largest safe integer
for this numeric representation. The upper bound validates representation, not
a resource reservation or permission to run that many samples.

Median/p95 latency and throughput must be positive finite numbers. Fractional
measurements remain valid; fractional counts do not. p95 cannot precede median.
An overflowed CPU/candidate speedup is rejected instead of publishing infinity.
Valid existing same-model report values and the selection output shape are
unchanged. Missing, wrong-type, nonfinite or out-of-range report fields cannot
be used as measurement evidence. Invalid CPU reports do not satisfy the baseline
requirement; invalid accelerator reports fall back to an eligible CPU.

This deliberately tightens admission of previously accepted malformed reports.
No new training/deployment-plan validation or proof of real hardware speedup is
claimed. NPU compute remains ineligible. Historical performance regressions and
quality comparisons require their own current measurements.

## Verification

Run the two pure tests from either product with the seven runtime capability
categories denied:

```sh
pblang run tests/semantics/pbai_optimization_test.sapp --sandbox
pblang run tests/semantics/pbai_optimization_comparison_test.sapp --sandbox
```

The original suite covers selection/fallback, quality, NPU, planning and status.
The comparison suite covers digest mismatch and ordering, all count fields on
CPU and GPU reports, minimum/maximum boundaries, malformed types, NaN/infinity,
finite metric admission, speedup overflow, nonmutation and recovery. Fixtures
are synthetic reports; GPU is a report label, not a device execution.
