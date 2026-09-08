# Profiler lifecycle and measurement contract

The PBLang-only profiler is a lightweight, explicitly enabled wall-clock tool.
It is not a VM sandbox, monotonic clock, allocation tracker or automatic trace of
every compiler stage. Existing bench/pblang_phase_stages.sapp and its runner
remain the broad compiler-phase tools; this change does not replace them.

## Corrected clock interpretation

The former private time_monotonic_ms function already called now_ms. It was
NOT a missing builtin dependency. A previous benchmark incorrectly tried to call
that private helper globally; prior continuation/benchmark reports are corrected.
Internal calls now use the honest name _wall_time_ms. New clock_info() returns
exactly {source:"now_ms",monotonic:false,unit:"ms"}. The returned map is fresh.
System-clock changes can produce negative durations; the profiler does not clamp
them to zero or claim reliable monotonic elapsed time. It is not a deadline API.

## Sections, marks and counters

Existing names/signatures and report shape are retained. Missing sections/marks/
counters now follow their intended zero behavior instead of throwing on an absent
map key. Starting the same section name still replaces its previous measurement.
end() uses one timestamp for both its returned elapsed time and stored end_time.
Negative counter values and explicitly supplied counter values remain unchanged.

## Benchmark and warmup

Both functions validate ordinary finite integer iterations in 0..1000000 before
any callback/time read. The callback must have a supported fn/closure/builtin
runtime tag, including for zero iterations. Invalid values throw constant messages
without embedding the supplied value. This intentionally tightens formerly
coerced/fractional/unbounded input; callers above one million must split batches.

benchmark(name, callback, 0) does not execute the callback or read the clock.
It returns the unchanged seven keys: name, total_ms, avg_ms, min_ms, max_ms,
iterations, result. All numeric metrics are zero and result is nil. Previously
zero produced a nonfinite average and a sentinel minimum of 999999999.

Positive runs call the callback exactly N times, retaining only its last result.
Durations accumulate directly rather than retaining an N-element timing array.
The order of arithmetic/min/max comparisons and the historical min/max
initializers remain; no clipping of clock anomalies is introduced. Callback
exceptions propagate, and no fabricated success report is returned.
For full samples/percentiles, use the existing benchmark.stats/run API.

## Trace semantics

Enabled state is sampled at entry. Disabled trace calls execute the callback
once and return its result without collecting timestamps/logs, even if the
callback enables profiling. Enabled calls retain their completed record even if
the callback disables profiling. Each call owns its start timestamp, so nested
traces cannot replace the outer start through the former shared trace_start mark.
Exceptions propagate without adding a success record. Trace records retain
time, elapsed_ms and result_type; callback result content is not stored.

## Memory evidence

mem_snapshot() remains the raw memory_stats() result. mem_diff keeps exactly
allocations_delta and borrows_delta:
- canonical counters: total_allocs / active_borrows;
- legacy fixtures: allocations / borrows;
- no comparable measurement: nil, never a fabricated zero.

For each counter, presence of the canonical key on either side chooses that
schema for the entire pair. Canonical and legacy counters are never subtracted
from each other. Present selected values must be ordinary finite nonnegative
numbers; missing means an absent key, not a present nil. Negative differences
remain valid. Inputs are not modified.

Raw memory_stats exposes active_borrows but not total_allocs; therefore comparing
raw snapshots returns allocations_delta:nil. total_locations is NOT an allocation
counter. Use the already implemented resources.snapshot/diff for the richer
allocation view. No collection, heap policy or runtime implementation changed.
This fixes the old mem_diff logical-or arithmetic and invalid field assumptions;
consumers must now handle unavailable measurements explicitly.

## Validation and remaining limits

profiler_lifecycle, profiler_admission and profiler_memory exercise real runtime
behavior, including zero and maximum counts, malformed inputs, callback types,
nested/disabled tracing, exceptions, missing names, real memory snapshots,
canonical/legacy precedence and input nonmutation. Existing benchmark/resources/
phase-contract tests also run; see BENCHMARK_SESSION_2026-09-08_PROFILER.md.

Iteration admission does not limit callback execution time, allocations or
effects. Global section/mark/counter/log growth, mutable trace_log exposure,
unvalidated names/counter values, thread isolation and wall-clock rollback remain.
The historical positive-run minimum sentinel remains. No universal platform,
hard resource isolation or PBLang 1.0.0 readiness guarantee is established.
