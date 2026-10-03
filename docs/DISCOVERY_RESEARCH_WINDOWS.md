# Research observations in time windows

`research_windows.review(spec, rows, grid)` partitions the existing bounded
`research_observations` dataset into at most 32 contiguous time windows. It reuses
that module's complete validation/statistics and `units.convert_checked`. It does
not replace index-based `array_windowing` or integer-millisecond `interval_analysis`.

The spec and rows are unchanged from `research_observations`: one measured variable,
explicit compatible units, absolute/difference conversion, relative timebase,
at most 256 rows and caller-assigned subject slots 1–64. Subject times must increase
strictly, even outside the requested range. Every row validates before partitioning;
an invalid excluded row rejects the entire call without returning partial counts.

`grid` has exactly three fields:

| Field | Contract |
|---|---|
| start | Finite number 0–1e9, in spec.time_unit relative to the same origin as rows |
| duration | Finite number greater than zero and at most 1e9, in spec.time_unit |
| count | Integer 1–32 |

Start and duration convert to seconds. Edges use normalized_start + index ×
normalized_duration. Every edge must stay in 0–1e12 seconds and advance strictly
at floating-point precision. Unrepresentable/tiny steps at a large origin fail.
Each window includes its left edge and excludes its right edge. The final right
edge is outside the requested grid. Comparisons use the computed floating-point
values exactly: no tolerance, rounding, snapping or resampling is applied. Caller
origin alignment and clock accuracy are not verified.

The result includes the canonical overall summary, per-window descriptive metrics,
before/included/after row counts and these mutually exclusive window states:

| State | Meaning |
|---|---|
| empty | No supplied rows fell in the window |
| all_missing | Rows were supplied, but every value was nil |
| partial | Both observed and missing values were supplied |
| observed | Every supplied row has an observed value |

Observed is not a completeness certificate: no expected sampling cadence, sample
count or missingness mechanism is inferred. Means remain per-observed-row, not
time-weighted or per-subject means. Missing rows are not synthesized; nil values
are not imputed. Subject counts cannot be summed across windows to count distinct
subjects. Window time_span_seconds describes supplied rows, not window duration.

No raw rows, subject slots or individual observation timestamps are returned.
Grid boundaries and measurement statistics are returned and may still be sensitive,
especially in narrow windows. `safe_to_log`, `coverage_verified`,
`expected_cadence_verified`, `scientific_validity_verified`, `acquisition_performed`
and `monitoring_started` remain false. This is not anonymization or a live monitor.
Input objects and previously returned summaries are preserved. Method provenance
and scientific limits remain those of `research_observations`.

Run `pblang run tests/semantics/research_windows_test.sapp` and
`pblang run examples/discovery_research_windows.sapp` from a prepared checkout.
The synthetic environmental example has all four states, explicit temperature/time
units and a sample exactly at the excluded final boundary. No device, server,
network, file ingestion or global installation is used.

This adds computation supporting requirements PB-05-021 (research datasets) and
PB-01-017/PB-05-020 (bounded monitoring infrastructure); it does not complete device
acquisition, scheduled monitoring, alarms, GUI integration or hard-real-time support.
