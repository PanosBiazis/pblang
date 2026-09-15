# SQL column reuse validation — 2026-09-15

Scope: `sql_predicate.compile_all` call-local reuse of adjacent column names.
Baseline: `99ef7a4acbb7d1271468d7e9dee5a7f28d5a425b`. No EnterpriseOrg server or remote
runtime was used. This is a selected regression/performance slice, not 1.0.0
readiness, a full-repository audit or universal backend validation.

## Implementation and invariants

The cache contains only the previous validated string name and bracket-quoted
spelling. It has constant entry count, adds no source/filter limit and does
not retain parameters, rows or state between calls. Every term still validates
its shape, operator and value. Type rejection happens before name comparison:
PBLang map equality is overloadable and must not run during column validation.
The private `_quote_string_column` helper has one guarded caller and validates
length/ASCII before the cache is updated. SQLite still resolves schema names.

A preliminary variant repeated the type check on cache misses. It was replaced
with one explicit guard before equality. The retained measurements below are
for that final variant only, against the same baseline. The pilot is not used
to claim an improvement from moving the type check; host timing varied.

## Reproduce

Retained executable SHA-256:
`7f1b97c0037736553ab39d165c13c36f3fd8497bc6aca9ca94a4c680858e6500`.

Use `bench/sql_predicate.sapp` arguments `count repeats mode`:
`scalar` repeats one column, `alternating` changes between two equal-length
names, `unique` uses a new name per term, and `membership` builds one IN term
with count values. The preexisting scalar and membership workloads are unchanged;
alternating and unique modes were added to expose cache misses, not to select
only favorable inputs. Count/repeats for each run appear below.

For paired measurements, copy the baseline module from the baseline commit and
current module to sibling scratch files; make two copies of the same benchmark
with equivalent relative imports to those modules. This avoids confounding
whole-process RSS with different stdlib-manifest loading. Both variants run on
the same retained executable. The module hashes and raw samples are in
[benchmark evidence](SQL_COLUMN_REUSE_BENCHMARK_2026-09-15.json).

One warmup per variant, seven measured samples per case, alternating execution
order. `clock()` measures repeated construction after input preparation and
before output hashing. GNU time records process CPU and peak RSS, which includes
startup and is not live heap. p95 uses nearest rank (the maximum of seven).
Every before/after sample required identical SQL+parameter JSON SHA-256.
Benchmarks ran serially without simultaneous tests, Graphify or CLI inventories;
unrelated host activity was not controlled. The benchmark is PBLang; external Python/GNU time
only orchestrated executions and statistics.

| Mode | Count × repeats | Before median / p95 ms | After median / p95 ms | Median time change | Before / after median RSS KiB |
| --- | --- | --- | --- | --- | --- |
| scalar | 1 × 10000 | 63.145 / 69.376 | 68.048 / 69.488 | +7.76% | 12556 / 12512 |
| scalar | 2 × 5000 | 57.628 / 58.668 | 46.102 / 48.484 | -20.00% | 11428 / 11076 |
| scalar | 16 × 1000 | 115.874 / 140.662 | 61.121 / 74.271 | -47.25% | 10332 / 10284 |
| scalar | 1024 × 50 | 444.421 / 456.022 | 213.639 / 221.697 | -51.93% | 10656 / 10672 |
| alternating | 16 × 1000 | 131.396 / 135.705 | 135.164 / 142.130 | +2.87% | 10208 / 10388 |
| alternating | 1024 × 50 | 394.825 / 541.981 | 412.776 / 543.438 | +4.55% | 10624 / 10584 |
| unique | 16 × 1000 | 146.085 / 151.091 | 149.597 / 160.960 | +2.40% | 10332 / 10308 |
| unique | 1024 × 50 | 511.762 / 527.102 | 523.839 / 527.156 | +2.36% | 10840 / 10660 |
| membership | 1 × 10000 | 127.250 / 134.143 | 134.386 / 140.007 | +5.61% | 13908 / 13864 |
| membership | 16 × 1000 | 29.003 / 29.945 | 30.145 / 31.050 | +3.94% | 10452 / 10300 |
| membership | 1024 × 50 | 53.390 / 102.672 | 53.266 / 95.626 | -0.23% | 10208 / 10296 |

Repeated two-column-bound terms (same name twice) improved about 20.0%; 16 and
1,024 same-name terms improved about 47.3% and 51.9%. The one-term scalar case
cost about 7.8% more time. Alternating columns cost about 2.9–4.5% more and the
unique-column cases about 2.4% more. Membership with one/16 values cost about
5.6%/3.9% more; the 1,024-value case was nearly unchanged and had high p95
variance. Membership does not benefit from adjacent reuse because it contains
only one term. These results are not a claim that every filter becomes faster.

No zero-overhead or universal acceleration claim is justified. Median RSS
changes ranged from about -3.1% to +1.8%, with no general RAM saving established.
The two cache references have constant count, while SQL/parameter output still
grows with input. No executable-size, disk-write, GPU/NPU or full-compiler
throughput improvement was measured. Existing native/C99 allocation and resource
policy limitations remain.

## Validation

31 selected suites passed, including 17 emulator suites, the three predicate
suites and ORM/database/resource regressions. Every suite required exit zero
and its declared final success marker. After the final string-accumulator
initialization adjustment, all three directly affected predicate suites were
rerun successfully. The broad run is selected coverage, not the full repository.

Five changed code files passed 15 final gates: check via PBLang, lint and strict
entry-only pbsecure. All security reports passed with zero findings. Lint
retains warnings for used variables and the intentional `value - value` finite
number test; this is not a zero-warning claim. An initial nil accumulator
caused false nil-addition lint errors; explicit string initialization resolved
them. Final benchmarks use the string-initialized code.

New tests cover same-name ranges, alternating names, malformed names after a
valid seed, malicious map equality callbacks remaining uncalled, cyclic values,
invalid operators/values on cache hits, independent calls after input mutation,
self-hosted parity and real SQLite missing-column resolution. The uncaught
synthetic invalid-column probe produced the fixed error without revealing its
value. Independent security review approved the type boundary before equality.

Retained build passed and the PBLang-native portable-lock tool refreshed the
repository-relative dependency lock. Privacy scan and diff whitespace checks
passed. Graphify AST update completed with 9,069 nodes and 13,226 edges, but zero
`.sapp` source nodes: it is not semantic PBLang graph coverage. The snapshot is
kept outside publication, preserving the main worktree's existing graph.

No guest OS, device, remote workstation or EnterpriseOrg server was used. Native
runtime/FFI, composite ownership, backend allocation, audit/release authenticity,
PBIDE Run/Designer and broader platform evidence remain open.
