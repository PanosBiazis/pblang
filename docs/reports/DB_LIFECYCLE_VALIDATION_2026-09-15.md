# DB library validation — 2026-09-15

This slice adds optional SQLite callback cleanup and optimizes placeholder
materialization. It does not complete PBLang 1.0.0 or port the native runtime.
The [API contract](DB_OWNED_CALLBACKS.md) describes its exact limits; the
[incremental idea intake](IDEA_INTAKE_DB_2026-09-15.md) separates implementations
from proposals.

## Measurements

Local Linux x86-64 retained executable; exact executable/source hashes and
platform metadata are recorded in
[evidence](evidence/db-validation-2026-09-15.json). Both alternatives import
the same modules. One process warmup per mode/workload precedes five measured
samples, alternating mode order. The table uses medians; raw samples and
nearest-rank p95 (the maximum of five samples) are published. No GPU/NPU path
is used. Measurements are local microbenchmarks, not a compiler-wide speedup.

| Placeholder count × repetitions | Previous loop ms | Doubling ms | Previous/current peak RSS KiB |
| --- | ---: | ---: | ---: |
| 1,000 × 100 | 19.775 | 0.462 | 10,024 / 10,148 |
| 10,000 × 20 | 56.368 | 0.167 | 10,172 / 10,164 |
| 50,000 × 5 | 368.615 | 0.169 | 10,168 / 10,316 |

Every mode/sample produced the same output SHA-256 for the same count. The
50,000 case used median process CPU 0.36 seconds before; the current value
rounded to 0.00 at the measurement tool's 0.01-second precision, which does
**not** mean zero CPU cost. Sub-millisecond samples should not be treated as
precise universal speedup ratios. RSS differences are small (up to about 1.5%
more in this sample); no RAM-saving claim is supported. The reference is the
frozen previous loop in `bench/reference/db_placeholders_linear.sapp`.

For 2,000 SQLite open/query/close cycles, manual cleanup measured median
36.113 ms, p95 60.716 ms, and 16,516 KiB peak RSS. The optional automatic helper
measured 36.978 ms, p95 58.604 ms, and 16,724 KiB. Median process CPU was 0.03
seconds for both. The observed median increase is about 2.4% time / 1.3% RSS;
variability overlaps, so neither zero overhead nor a statistically established
regression is claimed. Import/callback bookkeeping is opt-in.

Raw measurements:
[placeholders](evidence/db-benchmark-results-2026-09-15.json) and
[SQLite cleanup](evidence/db-benchmark-owned-results-2026-09-15.json).

Reproduce from the repository root with the same retained executable:

```sh
pblang run bench/db_placeholders.sapp --allow-all -- reference 50000 5
pblang run bench/db_placeholders.sapp --allow-all -- current 50000 5
pblang run bench/db_owned.sapp --allow-all -- manual
pblang run bench/db_owned.sapp --allow-all -- automatic
```

Repeat with `1000 100` and `10000 20`, discard the first run per mode, alternate
order, and take five samples. Use `/usr/bin/time -f '%U %S %M'` for process
user/system seconds and peak RSS KiB. The benchmark's `clock()` interval is in
seconds; multiply by 1,000 for the table's milliseconds. Measurements were
serial, without a concurrent test suite or Graphify run.

No executable-size or temporary-disk reduction is claimed: the retained
executable was unchanged, and this slice generates strings and uses in-memory
SQLite. Source/docs/fixtures increase repository size.

## Validation and review

- 28 passing suites: new DB ownership, placeholder and self-hosted-placeholder
  parity tests; existing DB/ORM, persistence, resources, scalar memory helpers,
  polyglot DB security, application facade, and 17 emulator/device suites.
- One explicit skip: external `sqlite3` CLI bounded-query suite, because its
  fixed executable is unavailable. Native in-memory SQLite tests did run.
- New tests cover callback success/throw/non-callable input, open failure,
  nil/false/compound results, escaped handles/prepared closures, manual close,
  field replacement and preservation of a distinct live connection.
- Placeholder tests cover legacy fractional/negative/non-number behavior,
  exact-integer overflow rejection, powers-of-two boundaries, SQL execution
  and a 10,599,999-byte result without introducing a 10 MiB result cap.
- Self-hosted PBLang and retained routes agree for the tested placeholder
  inputs. This is not SQLite execution parity across all backends.
- Nine touched PBLang files passed self-hosted source checks, lint (with
  non-blocking warnings), and strict entry-only security gates: 27 checks.
  Static security reports contain zero findings; these limited rules are not
  a whole-runtime security certification. The example prints `42`.
- An independent security review found no new blocker under the documented
  trusted synchronous callback/caller-resource contract. Existing native
  global-handle, concurrent-close and ignored-lock-error limitations remain.
- `db.sapp` line endings were normalized to LF under the repository's existing
  `.gitattributes`; the substantive edit is confined to `sql_placeholders`.

Build/portable-lock, privacy, publication and Graphify checks are recorded in
the completion section below. Unrelated main-worktree changes are preserved.

## Completion checks

Clean retained-bootstrap builds and PBLang-native portable-lock publication
passed in both the isolated checkout and the working main tree. The four
focused DB/ORM/self-hosted suites were also rerun successfully in main. All
211 unrelated pre-existing file hashes remained unchanged; the local lock was
refreshed intentionally, while publication uses the isolated repository-relative
lock. No active executable hook exists at the configured hooks path.

Owned-file privacy checks and `git diff --check` passed. No native executable
or Rust source changed. AST-only Graphify completed: 8,899 nodes / 13,070 edges
from 528 extracted files. The installed extractor contributed no `.sapp` nodes,
so this refresh does not replace the direct PBLang module review. Its isolated
snapshot was kept outside publication; the user's existing graph was preserved.

An exploratory diagnostic that printed the entire self-host frontend result
produced excessive recursive output and was stopped. Subsequent probes printed
only status and output fields. General cyclic-value diagnostic rendering is a
remaining runtime investigation, not a repaired behavior in this slice.
