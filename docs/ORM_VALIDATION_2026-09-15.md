# ORM validation — 2026-09-15

This update closes an ORDER BY grammar bypass and improves SQL construction
for wide insert/upsert calls. See the [API contract](ORM_QUERY_BUILDERS.md)
and [incremental idea intake](IDEA_INTAKE_ORM_2026-09-15.md).

## Reproducible measurements

Local Linux x86-64 retained executable, with the exact executable/source hashes
in [validation evidence](evidence/orm-validation-2026-09-15.json). The unchanged
previous ORM module is from commit `7b6f9b3`. Both alternatives run the same
`bench/orm_builders.sapp` workload with a recording connection. Each repetition
builds one INSERT and one UPSERT; identifier validation and value gathering are
included. Database execution is not timed. The 10,000-column case measures
builder scalability, not SQLite acceptance of that many columns.

One warmup process per mode/workload was discarded. Five measured samples ran
serially with alternating mode order; no concurrent suite or Graphify job ran.
All sample pairs have identical final SQL/parameter SHA-256. Medians follow:

| Columns × repetitions | Previous ms | Current ms | Previous/current RSS KiB |
| --- | ---: | ---: | ---: |
| 4 × 2,000 | 87.841 | 91.079 | 22,132 / 22,264 |
| 100 × 200 | 202.658 | 194.465 | 22,116 / 22,236 |
| 2,000 × 10 | 439.881 | 388.993 | 22,064 / 22,340 |
| 10,000 × 2 | 628.774 | 429.654 | 22,132 / 22,336 |

The 10,000-column builder workload is about 31.7% faster; median process CPU
fell from 0.66 to 0.46 seconds. Small-row wall time increased about 3.7%, while
median process CPU stayed at 0.11 seconds (coarse 0.01-second precision).
Peak RSS increased about 0.5–1.3% across these samples. There is no RAM-saving,
zero-overhead or universal project-speedup claim. The 2,000-column samples show
substantial timing variation. Full samples and nearest-rank p95 (the largest
of five samples) are in [benchmark evidence](evidence/orm-benchmark-2026-09-15.json).

The retained executable is unchanged. No executable-size or temporary-disk
reduction is claimed. The builder materializes SQL in memory; new source,
tests and documentation add repository bytes.

Reproduce in an isolated checkout:

```sh
mkdir -p probe
git show 7b6f9b3:stdlib/orm.sapp > probe/orm-before.sapp
pblang run bench/orm_builders.sapp --allow-all -- ../probe/orm-before.sapp 10000 2
pblang run bench/orm_builders.sapp --allow-all -- ../stdlib/orm.sapp 10000 2
```

Repeat with `4 2000`, `100 200` and `2000 10`. Discard a warmup per mode,
alternate mode order for five samples, and measure process user/system seconds
and peak RSS with `/usr/bin/time -f '%U %S %M'`. The benchmark prints seconds;
multiply by 1,000 for milliseconds. Keep the baseline module unchanged.

## Validation

- 28 passing suites: three new ORM construction/ordering/self-hosted tests,
  existing ORM, DB cleanup/placeholders/self-hosted placeholders, DB enrichment,
  persistence, resources, polyglot DB security and 17 emulator/device suites.
- Six touched PBLang files passed self-hosted source checks, lint (non-blocking
  warnings remain), and strict entry-only security checks: 18 gates. Static
  reports have zero findings; these rules are not a full runtime audit.
- Exact SQL/value-order checks cover 0/1/4/8/9/16/17/100/2,000 keys, unchanged
  caller map keys, original row-ID behavior and invalid-key rejection before
  exec. SQLite in-memory tests verify insert/upsert and parameter round trips.
- Ordering tests cover valid byte preservation and SQLite results, rejection of
  comments/extra LIMIT/UNION/COLLATE tokens, malformed commas/whitespace/NUL,
  invalid directions and non-string inputs. Invalid cases never call the
  recording connection; the old comment-based LIMIT bypass is rejected.
- The self-hosted VM test covers the >8-key reuse path, caller map preservation
  and ORDER admission. `examples/orm_owned.sapp` prints `updated` and combines
  the ORM with the earlier optional automatic connection cleanup library.
- The independent security review found no new blocker and confirmed fresh
  `map_keys` array semantics in retained/self-hosted/C99 implementations.
  C99 behavior was source-reviewed, not executed for this ORM change. Existing
  unchecked C99 allocation arithmetic remains open. Guest operating systems
  and real devices were not tested; emulators cover selected modeled behavior.

Raw WHERE clauses, schema definitions and raw query APIs remain deliberate
trusted-SQL boundaries. This slice does not close runtime/FFI isolation,
composite/shared ownership, authentic audit/release, PBIDE or guest-OS gaps.
PBLang 1.0.0 remains incomplete.

## Completion checks

Clean build and PBLang-native portable-lock publication passed in the isolated
checkout and main; four focused ORM/DB suites passed again in main. All 211
unrelated pre-existing file hashes were preserved. The main local lock was
refreshed deliberately; the staged release lock comes from the isolated tree
and uses repository-relative dependency paths. No executable configured hook
was present. Owned-file privacy scans and diff whitespace checks passed.

AST-only Graphify completed: 8,956 nodes / 13,130 edges from 532 extracted
files. The installed extractor still contributes no `.sapp` nodes; the refresh
does not validate PBLang source relationships. Its snapshot and exploratory
probes were retained outside publication; the original working graph remains
unchanged. No background test, benchmark or Graphify process is retained.
