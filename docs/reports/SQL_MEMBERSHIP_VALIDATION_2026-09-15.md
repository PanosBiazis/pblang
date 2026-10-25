# SQL membership validation — 2026-09-15

Scope: optional `sql_predicate.compile_all` membership and its ORM example.
Baseline: `41c0b86328efada96571e8b298a8eaac3fc37966`. All execution was local.
This is not a 1.0.0 readiness or universal backend/device report.

## Reproducible measurement

Retained executable SHA-256:
`7f1b97c0037736553ab39d165c13c36f3fd8497bc6aca9ca94a4c680858e6500`.

Use `bench/sql_predicate.sapp` with arguments `count repeats mode`, where mode
is `scalar` or `membership`. For before/after comparison, copy the baseline
and current predicate modules into a temporary directory and redirect the same
benchmark's import to each local module. Both sides must use equivalent local
imports: mixing a local import with a stdlib import changes manifest-loading
work and makes whole-process RSS incomparable. An initial mixed-import pilot
was excluded from the table for that reason. No workload was changed to improve
results. Product code and benchmark are written in PBLang; orchestration used
the local Python runner and GNU time.

Each case has one warmup per variant and seven measured executions. Baseline
and current order alternates each sample for scalar cases. Timing is the
`clock()` interval around repeated construction, excluding input setup and
output hashing. RSS is whole-process peak from GNU time; it includes startup
and is not live heap size. p95 uses the nearest-rank statistic (maximum with
seven samples). Identical scalar SQL plus parameter JSON hashes were required
for every paired sample. No test/Graphify process was launched during final
measurements; unrelated host activity and scheduling were not controlled.

| Scalar terms × repeats | Before median / p95 ms | After median / p95 ms | Median change | Before / after median RSS KiB |
| --- | --- | --- | --- | --- |
| 1 × 10000 | 117.155 / 129.023 | 121.957 / 133.378 | +4.10% | 12584 / 12572 |
| 16 × 1000 | 121.335 / 123.690 | 129.749 / 133.779 | +6.93% | 10336 / 10368 |
| 1024 × 50 | 381.646 / 389.725 | 392.156 / 395.391 | +2.75% | 10560 / 10600 |

The existing scalar cases cost approximately 2.8–6.9% more time in these samples.
Their median RSS differences range from -0.1% to +0.4%, which does not establish
memory savings. Membership adds functionality, not a demonstrated optimization
of scalar filters. These microbenchmarks do not measure compiler throughput,
GPU/NPU use, filesystem writes or executable size.

| Membership values × repeats | Median / p95 ms | Median peak RSS KiB |
| --- | --- | --- |
| 1 × 10000 | 124.795 / 252.597 | 14004 |
| 16 × 1000 | 34.755 / 39.621 | 10320 |
| 1024 × 50 | 57.288 / 63.358 | 10288 |

Membership has no preexisting implementation to compare. One-element membership
showed a high p95 outlier; no zero-overhead or stable-latency claim is made. The
linear marker/parameter loops avoid repeated growing-prefix concatenation but
still allocate and copy. Raw samples and portable module hashes are in
[benchmark evidence](SQL_MEMBERSHIP_BENCHMARK_2026-09-15.json).

## Correctness and boundaries

- Exact operators, ordered bindings across list/scalar terms, hostile strings,
  nil/bool/finite numbers, duplicate preservation and caller/result independence.
- Empty-list semantics, SQL NULL matrix and missing-column errors tested using
  actual in-memory SQLite, including both empty IN and NOT IN.
- Malformed lists, nested/cyclic/compound/function/nonfinite members rejected;
  late invalid members/terms do not dispatch a query. Pure helper errors use
  fixed messages rather than embedding member data.
- A 4,096-member list compiles with exact size/order; no new list-count cap.
  This is construction coverage, not a claim that every database accepts it.
- Self-hosted frontend parity for membership output and malformed input.
- Existing primitive/scalar behavior retains identical measured output hashes.

Tests cover the retained local runtime and selected self-hosted execution.
Native OOM, C99 allocation arithmetic, database parameter/depth limits and
caller-controlled resource policy remain independent constraints. Empty NOT IN
selects all rows including NULL; NULL in a nonempty exclusion list can suppress
nonmatching rows. See the [API contract](SQL_PREDICATES.md).

## Gates

31 selected suites passed, including 17 emulator suites, database/ORM/resource
regressions and the three predicate suites. Each required exit code zero and
its declared final success marker. The changed test was rerun after the final
additional duplicate-value assertion. Six touched code files passed 18 gates
(check via PBLang, lint, strict entry-only pbsecure); all security JSON reports
passed with zero findings. Lint may report existing-style unused warnings;
this is not a zero-warning claim. The ORM/owned-connection example printed 1.
An uncaught synthetic invalid-member probe returned a fixed error without the
synthetic member value. The retained build completed, and the lock was rewritten
by the PBLang-native portable-lock tool.

Independent security review found no blocker in the owned diff. Source/privacy
checks and `git diff --check` passed. Tests are selected regression coverage,
not the entire repository suite; no actual guest OS or remote hardware execution
was performed. The external sqlite3 executable is not needed by the in-memory
native SQLite tests in this slice.

`graphify update . --no-cluster` completed: 9,049 nodes and 13,209 edges. The
installed AST extractor produced zero `.sapp` source nodes, so this is not
semantic PBLang graph coverage. Its generated snapshot stays outside the
publication and the preexisting main-worktree graph is preserved.
