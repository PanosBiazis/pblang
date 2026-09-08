# Taint configuration admission and measurements — 2026-09-08

Baseline e0cb6de, same Linux x86-64 checkout and retained executable.
PBLang-owned code only; no Rust changes or native executable refresh.

## Reproduced correctness failures

A custom fs_write sink with severity HIGH, class fixture and args [0] produced
one finding but passed=true. Omission of args produced zero findings and
passed=true. Fractional index 0.5 was accepted. These are malformed-policy
failures; they do not show that analyzed source can modify an operator policy.

Strict bounded custom validation now rejects these shapes before direct source
admission/tokenization. Valid lowercase severities, explicit empty policies/args
and other deliberate replacements retain their meaning. Private default policy
is constructed once, never exported/mutated or returned in reports. No parsed
source, findings, caller policy or analysis summaries are cached across calls.
See TAINT_COMPLETENESS.md for limits, compatibility and threat boundaries.

## Paired full-analysis comparison

Existing benchmark.stats, taint fixture and alternating paired methodology are
reused in bench/taint_configuration_pair.sapp. Reference is an exact locally
reviewed copy of security_taint.sapp at e0cb6de. Both versions use current identical
tokenizer/parser modules. Importing a reference executes its code: never use an
untrusted reference path. Two warmup pairs, twelve retained AB/BA pairs per case.
Full result JSON must match; no retained sample/outlier is removed.

Each default-small/default-full/custom-full sample performs 100 analyses.
Default-small analyzes let value=1; default-full uses the unchanged interprocedural
source/sink fixture. Custom-full uses one valid custom source/sink and no sanitizer.
The custom comparison now includes deep validation absent in the baseline.

| Operation | Initial paired median ratio | Confirmation | Boundary-expanded |
| --- | ---: | ---: | ---: |
| default-small | 0.7409 | 0.7164 | 0.7362 |
| default-full | 0.9151 | 0.9294 | 0.9118 |
| custom-full | 0.9813 | 0.9769 | 0.9671 |

Below one means less elapsed time. Default full analysis improves approximately
7–9% by the median of paired ratios on this fixture, not across all applications.
Confirmation medians per 100 analyses: reference 75.469 ms, candidate 71.088 ms.
The ratio of those medians is not the median of ratios. The earlier 5.97 -> 7.22 ms
observation remains an OPEN recovery question: different harnesses/process states
must not be directly equated.

## Unfavorable boundary case retained

The extended harness additionally validates 128 custom sinks, each with 32
argument indexes and a 256-byte class label, against empty source. Fixture
construction happens after earlier measurements and outside the timed calls.
Each boundary sample performs five complete analyses. Reference/candidate reports
remain equal, but the candidate now checks all policy fields.

Median totals: 0.993 -> 42.103 ms per five analyses (approximately 0.199 -> 8.421
ms per analysis). Paired median ratio: 41.9143. This is a substantial real overhead,
not hidden by default-path gains; the old implementation skipped the nested work.
Further optimization must preserve validation and mutation/provenance guarantees.
No unsafe cross-call cache of mutable caller policy was introduced.
This boundary case has one twelve-pair series, not independent confirmation.

## Stage and end-to-end boundaries

The existing independent lexer/parser diagnostics are kept separately. Initial
per-100 medians: lexer 26.217 ms, parser 18.833 ms; confirmation 33.619 / 28.485 ms.
These variations underline uncontrolled scheduling/thermal/allocation effects.
They are standalone workloads with prepared inputs, NOT exclusive spans from a
single analyzer call; do not sum/subtract them to invent other stage times.
The complete analyze rows include configuration, all admissions, lexing, parsing,
flow traversal and result construction, but exclude imports/caller file reads.

Whole-harness wall time, peak RSS and CPU are in the raw record. They include
both versions, imports, warmups, assertions and reporting—not a single-analysis
latency or before/after heap measurement. Cached defaults retain a small policy
tree for the module lifetime; no hard heap reduction is claimed.

Raw pairs and baseline probes: bench/results/taint-configuration-2026-09-08.jsonl.
The boundary-expanded series retains its earlier rows too. now_ms is a system
clock, not monotonic. All samples are retained, including negative performance
comparisons. Ratio statistics are dimensionless despite *_ms statistic keys.

## Separate numeric comparison

Unchanged session_compare integer_kernel: one million iterations, all nine
paths status=ok and checksum 474518. One first run, one warmup, five retained
runs; compiled paths one first build and three repeated builds.

| Path | Median ms | p95 ms | Median peak RSS KiB | Artifact bytes |
| --- | ---: | ---: | ---: | ---: |
| pblang-interpreter | 274.650 | 275.573 | 8640 | N/A |
| pblang-c99 | 6.635 | 6.962 | 1508 | 14472 |
| pblang-codegen-x64 | 3.321 | 3.732 | 264 | 507 |
| c | 7.760 | 8.044 | 1572 | 14472 |
| cpp | 8.129 | 8.613 | 3364 | 14472 |
| go | 9.061 | 9.522 | 1500 | 1228952 |
| rust | 8.175 | 8.472 | 2000 | 350560 |
| python | 202.060 | 207.210 | 10264 | N/A |
| javascript | 135.023 | 139.119 | 51628 | N/A |

Raw: bench/results/session-2026-09-08-taint-configuration.jsonl.
This workload does not execute the changed taint analyzer. The retained PBLang
interpreter is slower than Python and JavaScript here. Artifact sizes omit runtime
and dependencies, RSS is not total allocation, and N/A is not zero. No universal
language superiority, embedded-polyglot combination or other-platform/device claim.

## Validation and reproduction

Fifteen targeted suites passed: configuration, limits contract, admission,
completeness, dependency bounds, source bounds, token bounds, dependency analysis,
general security, dynamic eval, tokenizer bounds, parser depth, manifest,
portable lock and security CLI. New tests first reproduced rejection failure,
then passed for malformed/unused policies, bounds, UTF-8/control text, private
defaults, input/report mutation, intentional empty/low policies and all modes.
Independent defensive review found no blocking issue.

The initial test draft used an unsupported chained if-expression; it was changed
to the existing statement syntax. No parser change was made. Lint completes with
warnings for existing/nonfinite checks and bindings/shadowing. No full suite ran.

Reproduce with PB_TAINT_REFERENCE pointing at a reviewed baseline module and run
bench/taint_configuration_pair.sapp sequentially. Supervisors use 2 GiB address
space, disabled core dumps, 35-second paired limits and 2-second kill grace.
Numeric rows use 60-second outer / 10-second child limits and 64-MiB file caps.

Remaining: pre-I/O deep validation in dependency analysis (currently happens at
per-module taint), concurrent caller-policy snapshots, boundary validation cost,
broader analyzer performance, native self-bootstrap/executable refresh, compiler/
paradigm compatibility, runtime hard heap/preemption/ownership, FFI/sandbox,
package authenticity and physical-platform/product regression evidence.
PBLang 1.0.0 remains NOT READY.

Final project build and PBLang-native portable-lock publication passed without
the initial stale-content-hash note. Touched-file lint, privacy pattern scan and
git diff --check passed (lint warnings retained). Graphify update reached its
20-second deadline without completion; graph freshness is not claimed. Temporary
reviewed reference/probe copies were removed; baseline source remains in Git.
Unrelated dirty work was preserved and no owned validation workers remained.
