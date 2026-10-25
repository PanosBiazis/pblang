# Bounded sensor diagnostics

`sensor_contract()` and `sensor_analyze(request)` are identical on both pbAI facades.
They call the shared pure PBLang `stdlib:pbai_sensor.sapp` implementation.
This is a deterministic numerical specialist, not trained neural sensor inference.
It reads caller-supplied numbers only; it never connects to a device, invokes a tool,
trains a model, saves samples or authorizes a physical action.

## Exact request

```pblang
let AI=import("stdlib/pbai.sapp");
let result=AI.sensor_analyze(map{
 values:[10,12,14,30], period_seconds:2, sigma:3, absolute_tolerance:0.5
});
```

All four fields are mandatory; extra fields are rejected. `values` is a flat array
of 4–256 finite numbers in [-1000000,1000000]. `period_seconds` is an integer 1–86400;
`sigma` is a finite number 1–10; `absolute_tolerance` is finite 0.000001–1000000,
in the same units as the values. Uniform spacing and common units are declarations
by the caller, not verified acquisition metadata. Missing samples, irregular times,
multiple channels and arbitrary metadata are unsupported; preprocess explicitly.
No implicit sorting, interpolation, timestamp inference or missing-value filling.

## Meaning of the result

For `n = len(values)-1`, fit ordinary least squares to prefix indices 0..n-1.
The final observation is excluded from the mean, slope and residual scatter.
`expected_last` evaluates that fitted line at index n; `deviation` is the final
observation minus this expected value. `slope_per_second` divides the per-index
slope by the declared period. `residual_sd = sqrt(SSE/(n-2))` uses the prefix
regression residuals. `threshold = absolute_tolerance + sigma * residual_sd`;
`outlier` is true only when `abs(deviation) > threshold` (equality is within).
Constant and exactly linear baselines remain well-defined through the positive
absolute tolerance. The example yields expected value 16, slope 1, deviation 14,
threshold 0.5 and direction `above`.

This threshold is an engineering diagnostic, not a calibrated confidence interval,
an alarm policy, a future forecast or a probability. Short/noisy/trending/seasonal
series can cause false positives or negatives. Physical decisions require separate
domain validation and action authorization. No AGI/ASI capability is established.

The response has `format: pbai.sensor.result.v1`, `status: result|error`, `value`
and `error`. Malformed input returns `invalid_request` with nil value and never
includes the rejected content. Successful output contains derived numerical data
but no raw array, device identity or timestamp. **Derived values can still reveal
sensitive measurements** (expected_last + deviation reconstructs the last sample);
treat them with the same privacy controls as the source. Caller logging/export is
outside this function. No input/output references or cross-call state are retained.

## Verification and next steps

Run `pblang run tests/semantics/pbai_sensor_test.sapp --deny-net --deny-http
--deny-exec --deny-ffi --deny-spawn --deny-sqlite` from either product root (one line).
The test covers analytic linear/noisy oracles, exclusion of the last sample,
threshold equality, negative anomalies, maximum size/extremes, nonfinite values,
cycles, callbacks, wrong types, exact schema, privacy and mutation isolation.
`bench/pbai_sensor.sapp` records one warmup and five wall-time samples at 4/64/256
values, with 32 calls per sample and deterministic output digests. It is a local
cost baseline, not a before/after optimization or a model-quality benchmark.

Roadmap: explicit timestamp/missingness contracts, labeled independent sensor
corpora, calibrated evaluation, then an optional trained specialist compared with
this deterministic baseline. No automatic collection or training is enabled.
