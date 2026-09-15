# Structured predicate validation — 2026-09-15

This slice adds an optional pure SQL predicate compiler and an ORM/SQLite
cleanup example. The [API contract](SQL_PREDICATES.md) and
[idea intake](IDEA_INTAKE_PREDICATES_2026-09-15.md) describe the supported subset
and remaining work. No existing execution API or native runtime was changed.

## Correctness and security

The final implementation emits bracket-quoted ASCII identifiers, exact
whitelisted operators and positional value parameters. NULL equality uses
IS NULL / IS NOT NULL. It rejects unsupported shapes/operators/types and
nonfinite numbers with fixed value-free errors. There is no recursive tree
walk or database effect in compilation.

An independent review caught the SQLite double-quoted-string fallback before
publication. [SQLite documents that an unresolved double-quoted name may become
a literal](https://www.sqlite.org/quirks.html#double_quoted_string_literals_are_accepted).
The final implementation uses
[bracket identifier quoting](https://www.sqlite.org/lang_keywords.html), and a
real in-memory regression verifies an unknown column equal to its own name
fails instead of producing a constant-true filter. The final security recheck
found no remaining blocker within this limited contract.

31 suites passed on the final source, including the three new predicate suites,
existing ORM/DB/resource checks, and 17 emulator/device suites. Tests cover:

- All seven operators and parameter order; nil/bool/string/fraction/large finite
  values; NaN and both infinities; malformed/unknown inputs and exact term arity.
- Identifier grammar and 64/65 boundary, operator injection, value binding,
  keyword columns, missing columns, cyclic/nested values, independent results
  and unchanged input arrays.
- A 4,096-term pure compilation case (not a claim of SQLite execution support
  for that many terms), late-invalid rejection and live SQLite comparison,
  NULL, boolean and wildcard behavior.
- Retained/self-hosted output parity for the new module. SQLite execution was
  local Linux in-memory only; C99/other backends and guest OSes were not run.

Six changed PBLang files passed self-hosted source checks, lint and strict
entry-only security scans: 18 gates, with zero static findings. Lint has
non-blocking warnings. These scans do not certify the whole runtime. One
repeated scan initially reused an existing JSON output path: pbsecure's
exclusive-create report writer refuses replacement. Final scans used fresh
report paths and passed. An example prints `1`.

## Construction cost

The optional module had no prior implementation to benchmark against. These
are absolute costs, not a speedup or overhead comparison against trusted raw
SQL. The benchmark includes validation and assembly, but no database access.
One warmup process is discarded, followed by five serial samples per workload;
no test suite or Graphify process ran concurrently. The same workload yielded
identical output hashes across samples. Clock intervals are converted to ms.

| Terms × repetitions | Median ms | p95 ms | Median peak process RSS KiB |
| --- | ---: | ---: | ---: |
| 1 × 1,000 | 5.562 | 7.371 | 21,768 |
| 16 × 100 | 7.059 | 7.242 | 21,804 |
| 1,024 × 5 | 23.539 | 24.247 | 21,940 |

p95 uses nearest rank (the maximum of five samples). Median total process CPU
was 0.02 / 0.02 / 0.04 seconds at 0.01-second reporting precision. RSS includes
runtime startup/import overhead, not just the helper's allocations. No
zero-overhead, RAM saving, executable-size or temporary-disk saving is claimed.
The retained executable is unchanged; adding source/tests/docs costs repository
space. The previous small-row ORM timing difference was not repaired here.

Reproduce in a checkout containing the final module:

```sh
pblang run bench/sql_predicate.sapp --allow-all -- 1 1000
pblang run bench/sql_predicate.sapp --allow-all -- 16 100
pblang run bench/sql_predicate.sapp --allow-all -- 1024 5
```

Discard the first run per workload, then collect five samples using
`/usr/bin/time -f '%U %S %M'` for process CPU and peak RSS. Raw samples,
source/executable identity and suite/gate lists are published alongside this
report. Runtime allocation/preemption/FFI, full dialect compatibility, nested
filters, audit/release authenticity, PBIDE and guest-OS work remain open.

## Completion checks

Clean retained build and PBLang-native portable-lock publication passed in the
isolated tree and main. Three focused predicate suites passed again in main.
All 211 unrelated pre-existing file hashes were preserved; the main local lock
was intentionally refreshed, while the staged portable lock comes from the
isolated checkout. No active configured executable hook was present.

Owned-file privacy and diff whitespace checks passed. Graphify's AST-only
refresh completed with 9,002 nodes and 13,168 edges; it contains no `.sapp`
nodes, so direct PBLang review remains necessary. The isolated graph snapshot
was retained outside publication and the user's graph was preserved. No test,
benchmark or Graphify background process remains. PBLang 1.0.0 is incomplete.

[Raw benchmark samples](evidence/sql-predicate-benchmark-2026-09-15.json) ·
[Source identity and validation](evidence/sql-predicate-validation-2026-09-15.json)
