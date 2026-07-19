# Tree-interpreter builtin resolution

Date: 2026-07-17

## Accepted change

The pure-pblang AST preparation pass records whether an identifier call names a
known builtin that cannot be shadowed by any supported source binding. The
runtime skips its scope-chain lookup only for that proven case. If a builtin
name is used by a variable, named function, parameter, function literal, or loop
binding, all calls with that name retain the previous dynamic lookup.

No Rust implementation was added or changed.

## Method

- Release executable: `target/release/pblang`.
- CPU affinity: `taskset -c 3`.
- Seven fresh benchmark processes.
- Each process warms every AST, records nine direct-path samples, then records
  seven paired dynamic/direct samples.
- Pair order alternates within each process to reduce ordering and frequency
  bias.
- The control is a separately compiled, structurally asserted copy of the same
  AST with only the private unshadowed marker disabled.
- Every source has an exact result guard; parser-shape drift aborts the control
  setup instead of silently measuring a different call.

The primary result is the median across seven processes of each process's
paired p50 percentage. Full absolute process medians were not used as evidence:
the host showed substantial frequency/thermal drift between process groups.

## Results

| Workload | Calls per execution | Median paired p50 change |
|---|---:|---:|
| `len_15000` | 15,000 | -7.87% |
| `map_has_15000` | 15,000 | -6.95% |
| `str_starts_with_15000` | 15,000 | -7.42% |
| `mixed_collections_5000` | 15,000 total | -11.52% |

Negative change means the accepted direct path completed faster than the
dynamic-scope control. These percentages describe only these interpreter
fixtures on this host; they are not a claim about arbitrary programs or other
languages.

The conservative binding scan was also measured against an otherwise identical
temporary no-scan control. Across seven paired runs, its median compile-only
increment on the small `fib18` source was 0.03076171875 ms. Control p50 values
were 0.643-0.657 ms and scanned p50 values were 0.675-0.688 ms. Relative compile
overhead is visible on this deliberately tiny source, but it is below 0.05% of
the roughly 77-80 ms end-to-end recursive fixture.

One standalone accepted benchmark process used 12,764 KiB peak RSS and 17.61 s
elapsed time. There is no matched pre-change RSS series, so this report makes no
memory-improvement claim.

## Correctness and neighboring paths

- `interp_builtin_resolution_test.sapp`: all 16 current builtins plus global,
  local, function, parameter, function-literal, and loop shadowing; declaration
  order; member/index callees; prepared metadata; stale-marker rebuilding.
- Full semantics: 675 passed, 0 failed, 0 ignored, 0 filtered.
- Closure smoke: `fib18` p50 79.531 ms, `choose15_7` p50 189.766 ms,
  closure capture p50 0.0254 ms; suite status `ok`.
- Control-flow smoke: recursive `fib18` p50 79.333 ms, nested-scope return p50
  13.281 ms, break/continue p50 20.202 ms; suite status `ok`.

The first serial full-semantic command reached its 900-second outer timeout
while slow tests were still active. It is not counted as a pass. The explicit
eight-worker run completed and supplies the 675/675 result above.

## Rejected experiment

A numeric builtin-ID dispatcher was implemented and measured, then removed.
Across three paired processes and nine workload comparisons, movement was
mostly 0-2.4% with one 0.16% regression. That evidence was too small and
inconsistent to justify a second dispatch representation.

## Limits

- `perf` hardware counters were unavailable because this host has
  `perf_event_paranoid=4`; the system setting was not changed.
- This optimization is deliberately whole-program conservative. A builtin name
  bound in an unrelated nested scope disables the direct path for that name.
  More precise lexical proof is future work and requires its own measurements.
- The interpreter item remains active in the vision ledger. This slice does not
  establish universal runtime, memory, or cross-language performance leadership.

## Artifact map

- `final-1.txt` through `final-7.txt`: accepted implementation and primary
  paired evidence.
- `accepted-smoke.txt` and `resources.txt`: one final standalone run and its
  process-level resource observation.
- `baseline-1.txt` through `baseline-7.txt`: exploratory pre-control benchmark
  shape. These absolute timings are not compared with the accepted series.
- `candidate-1.txt` through `candidate-7.txt`: numeric-ID candidate runs. The
  candidate was rejected and these files are retained only as negative evidence.
- `metrics.txt` and `verification.txt`: compact accepted metrics and validation
  summary.
