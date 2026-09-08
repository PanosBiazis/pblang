# Audit CLI numeric admission and startup cost — 2026-09-08

Base CLI: 0c6fe65. PBLang-only changes; no Rust source or executable replacement.

## Correctness and security repair

An owned synthetic previous report was overwritten by the old CLI with an
empty blocked report for --max-files=0. This is not a clean audit, but it still
destroys the previous report. Malformed final numeric strings also silently
retained the preceding/default value. In module mode, NaN/fractional file limits
passed range-only checks; NaN could bypass the intended file-count limit up to
independent inventory/deadline ceilings. Independent defensive review verified
these control-flow weaknesses and approved the final repair.

project_audit.pbaudit_numeric_error(name,value,module_mode=false) is the single
pure validator for the eight non-finding numeric CLI flags. An empty string
means scalar acceptance, not filesystem permission or reusable scan authority.
Unknown names and non-boolean mode values produce a fixed generic error; numeric
errors name only the whitelisted flag and constant range, never the input value.
The CLI calls the same function as the exhaustive pure test.

pbaudit rejects malformed final values with status 2 before root normalization,
report removal, checkpoint loading, snapshot verification or audit traversal.
The prior max-findings check retains first numeric-error precedence. Missing
options preserve defaults; integral syntax such as 1.0 and final-option-wins
remain supported. File and byte ceilings remain mode-specific, including when
--module-by-module appears after the numeric option. See the canonical numeric
range table in Security_Analysis.md.

Display options retain positive safe integers up to 9007199254740991 rather than
an arbitrary smaller cap. Their work stops at bounded findings/module arrays;
nothing allocates up to the requested scalar. Zero/negative display values,
fractions, non-finite numbers and unsafe integers now reject instead of fallback
or inconsistent behavior. The four fixed summary lines remain unchanged, so
--lines is not a promise to truncate the fixed header below four lines.

Valid options, detector behavior, reports and checkpoint identity are unchanged.
Engine v6 stays v6; this change does not require fresh valid v6 checkpoints.
Existing resume tests pass. Old v5 rejection from the preceding change remains.

## Tests and file-effect evidence

The pure test exercises all eight fields in both modes, inclusive and rejected
boundaries, nil, bool, string, array, map, function, bigint/rational/decimal,
NaN, both infinities and fractions; it also checks exact names/mode and defaults.

The live test defaults to four subprocess checks so the normal test suite does
not inherit an exhaustive subprocess matrix. Explicit single-field selections
cover all eight CLI routes, three modes, reverse duplicates, valid-last numeric
syntax, mode-specific min/max and late mode selection: 66 successful child runs
plus the four-call smoke. After relocation the pure matrix and smoke were
repeated against the final exported helper. Limits at minimum depth/entry/time
may legitimately yield incomplete audits; accepting the number is not proof of
complete coverage.

A file-syscall trace of rejected module resume with max-runtime-ms=NaN produced
the exact status-2 error, no traced target/checkpoint access and an unchanged
owned marker. Static ordering independently confirms admission before those
calls. The trace used the initial equivalent validator placement; final live
smoke verifies report preservation after relocation. Startup/trusted imports
still perform I/O. Raw traces with host paths are not retained.

An initial all-in-one live design timed out under 45-second supervision and was
not accepted as coverage. Its owned fixtures were cleaned. Exhaustive scalar
tests plus opt-in field groups replaced it. A nested value-producing expression
in the new test also failed; explicit assignments corrected that test without
changing compiler semantics. No claim of fixing the broader block-semantics
issue is made. Existing heavier finding-preflight regression was separately
supervised at 90 seconds here, not qualified for a default 20-second full suite.

Fourteen focused suites passed on the final source: pbaudit limits/live smoke/
CLI contract, project audit/core limits, source analysis, security analysis,
project text audit, module audit/resume, library/CLI finding preflight, manifest
validation and current portable lock. No full suite or physical-device run.
The final project build passed without a dependency-hash note; the native lock
publisher/current-lock check passed. This is not an executable rebuild. During
development, adding/moving the stdlib helper required refreshing the manifest
lock before imports; those earlier mismatches were not accepted as test passes.
Lint passed on all five touched code files with warnings, including existing
unused-variable/style warnings and incorrect eq_op advice for intentional
NaN/infinity checks. Those checks were preserved.
Added-line/new-file privacy scans found no matched host-path/credential patterns;
git diff --check passed. Owned fixtures, old reference and raw traces were
removed. No owned test/benchmark/analyzer process remained. Graphify reached
its supervised 20-second limit (status 124) without completing or leaving a
process; graph freshness is not claimed. The 193 pre-existing dirty status
entries remain outside this commit, including the locally regenerated lock.

## Identical-workload CLI benchmark

bench/audit_cli_numeric_pair.sapp uses the old reviewed CLI and current CLI,
identical one-file fixture, flags, runtime, two warmup pairs and eight retained
AB/BA pairs. Exit status must be zero, stderr empty and complete stdout exactly
equal for every run. Startup, imports, validation, traversal, analysis and
summary output are timed; JSON report writing is not part of this benchmark.
Both versions use the current shared dependencies in each series. The final
comparison therefore does not isolate the parse cost of the additive helper
inside project_audit from all its other importers.

| Variant | Mode | Before median ms | After median ms | After p95 ms | Median paired ratio |
| --- | --- | ---: | ---: | ---: | ---: |
| separate-helper-import | ordinary | 1801.941 | 1872.060 | 1888.940 | 1.0381 |
| shared-audit-helper-first | ordinary | 1815.600 | 1783.738 | 1972.846 | 0.9958 |
| shared-audit-helper-modules | modules | 1668.856 | 1713.704 | 1820.990 | 1.0003 |
| shared-audit-helper-confirmation | ordinary | 1782.804 | 1768.596 | 1844.352 | 0.9826 |

The first implementation added a separate one-function stdlib import and showed
a +3.8% paired slowdown. It was not retained. The exact validator was moved next
to the existing finding admission in the already-imported project_audit library,
removing that extra import. The final ordinary paired ratios were about -0.4%
and -1.7%, with module mode about +0.03%. These support avoiding the observed
extra-import regression on this host, not a general speedup claim. Report all
samples, including outliers and the initial slower variant. Separate medians can
move differently from the median of paired ratios when host timing varies.

Linux x86_64, warm filesystem cache, shared host, non-monotonic now_ms.
No test/build/other benchmark worker ran concurrently with measurements.
Timing alone says nothing about improved allocation/RSS.
Raw: bench/results/audit-cli-numeric-2026-09-08.jsonl.

## Separate numeric comparison

Unchanged session_compare integer_kernel, one million iterations. All nine
paths returned status=ok and checksum 474518. First run, one warmup, five
retained runs; compiled rows also first compilation and three repeated builds.
Each language ran sequentially, with a 40-second outer deadline, 32-GiB virtual
address-space ceiling, OMP/BLAS thread limits of one and GOMAXPROCS=2.
This address-space ceiling is not a PBLang heap-safety guarantee.

| Path | Median ms | p95 ms | Median peak RSS KiB | Artifact bytes |
| --- | ---: | ---: | ---: | ---: |
| pblang-interpreter | 270.818 | 273.083 | 8576 | N/A |
| pblang-c99 | 4.653 | 4.873 | 1572 | 14472 |
| pblang-codegen-x64 | 3.282 | 3.593 | 264 | 507 |
| c | 7.815 | 8.063 | 1572 | 14472 |
| cpp | 8.069 | 8.554 | 3380 | 14472 |
| go | 9.144 | 9.430 | 1500 | 1228952 |
| rust | 8.103 | 8.253 | 1968 | 350560 |
| python | 200.127 | 207.264 | 10252 | N/A |
| javascript | 140.184 | 148.064 | 51488 | N/A |

Interpreter remains slower than Python/JavaScript on this workload. This
unchanged numeric kernel does not exercise the audit fix, so session-to-session
differences are not causal patch results. C99/native measurements cover the
fixture, not the complete dynamic analyzer. RSS is residence, not allocation;
artifact bytes exclude runtime/dependencies; N/A is not zero. Short-run CPU
measurements have timer granularity limits. No universal device/platform/
language/polyglot or whole-application performance claim.
Raw: bench/results/session-2026-09-08-audit-cli-numeric.jsonl.

## Remaining release work

Direct module/snapshot/aggregate option admission and generic gate input/
threshold checks; whole-input/record/suppression/work budgets; root/argv and
non-numeric CLI admission; report/source path collisions and publication
semantics; snapshot/namespace consistency; trusted security_report ownership;
compiler/paradigm/C99 parity and linter float reasoning; older heavy live-test
supervision; self-bootstrap/executable refresh; VM heap/time/ownership/cleanup;
typed FFI/raw bypass/sandbox; package trust/artifact handoff/revocation/rollback;
broad product compatibility and physical platforms. PBLang 1.0.0 remains NOT READY.
