# Rejected shared return-signal experiment

Date: 2026-07-14

The candidate replaced per-return one-element arrays and break/continue maps
with shared scalar sentinels. Return values were stored in the nearest call
environment so nested lexical scopes and `nil` returns remained distinguishable.
Focused correctness tests passed, but the extra environment hash insertion and
lookup cost more than the removed small allocation.

Across seven fresh CPU-pinned processes, `fib18` regressed by 5.63%,
`choose(15,7)` by 3.78%, and end-to-end `fib18` by 6.80%. The candidate and its
temporary tests were fully removed. Raw evidence is retained in `baseline.txt`,
`candidate.txt`, and `baseline_rss.txt`; summary values are in `metrics.txt`.
