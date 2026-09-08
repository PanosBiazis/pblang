# Local taint-class validation reuse — 2026-09-08

Baseline b1db0b2, same Linux x86-64 host and retained executable. Changes are
PBLang-only; no Rust/C runtime edits, compiler changes or executable refresh.

## Implementation and evidence boundary

security_taint._config_sinks keeps one local previously validated class string.
It checks ordinary string type before equality, and only records a value after
the original bounded text validator succeeds. Equal later class values can skip
the repeated UTF-8 conversion/control-byte scan. Every key, spec shape, severity,
argument index and control_flow flag is still checked. No caller-owned map/rule
is cached and nothing survives the validation call. No API or accepted-policy
schema change; direct, preflight and linked calls retain their existing checks.

The optimization regression passed before and after implementation: this is
semantic preservation, not a newly failing security test being fixed. Coverage
includes repeated/alternating/independently built Unicode labels, exact byte caps,
all C0 bytes and DEL, valid-prefix NUL/newline/DEL suffixes, nil/container/overload
inputs, malformed other fields despite a matching class, call-to-call mutation,
input preservation, exact errors and dependency pre-I/O rejection in all modes.

The small bounded PBLang-written VM test separately verifies exact string
equality, Unicode, NUL suffix inequality and persistence of the old value after
reassignment. This is a primitive-semantics check, not a full analyzer bootstrap.
Independent defensive review found no new blocking issue.

## Backend limitation found during review

The generic C99 runtime uses strcmp for string equality, and strlen-based length
and str_to_bytes (crates/pblang-ctranspiler/src/pblang_rt.h, comparison helper and
pb_builtin_str_to_bytes). Embedded-NUL suffixes therefore have a pre-existing
full-string parity gap. Both baseline and candidate text validation see the
truncated prefix: this is not a new acceptance difference from the optimization,
but C99 analyzer safety is NOT established. The separate PBLang-owned minimal
pointer/numeric C transpiler cannot run the full dynamic analyzer and is not
qualified by these tests. No fix for either broader backend boundary is claimed.
Concurrent in-call host mutation of policy containers remains an open snapshot
guarantee; ordinary sequential mutation is covered and triggers fresh validation.

## Paired measurements

Two independent series per harness. Two warmup AB/BA pairs and twelve retained
alternating pairs per case; no outlier removed. Every complete direct/dependency
result must match the baseline's full JSON outside the timed loops. Isolated
preflight requires the exact successful outcome. Same fixtures and supporting
tokenizer/parser/pattern/linker modules; no reference algorithm change.

| Workload | Initial paired median ratio | Confirmation |
| --- | ---: | ---: |
| Isolated single short class | 0.9927 | 1.0074 |
| Isolated 128 repeated short classes | 0.9252 | 0.9176 |
| Isolated 128 repeated 256-byte classes | 0.3574 | 0.3756 |
| Isolated 128 distinct 256-byte classes | 1.0041 | 1.0042 |
| Direct default-small | 0.9660 | 1.0022 |
| Direct default-full | 0.9905 | 1.0360 |
| Direct custom-full, one sink | 1.0095 | 1.0124 |
| Direct repeated maximum policy | 0.3799 | 0.3878 |
| Dependency default-small | 1.0270 | 1.0111 |
| Dependency custom-flow, one sink | 1.0022 | 1.0127 |
| Dependency repeated maximum policy | 0.3921 | 0.3222 |

Below one means less time. The repeated-large-policy benefit is substantial:
approximately 62–64% lower isolated validation time, 61–62% lower complete direct
analysis time, and 61–68% lower complete dependency analysis time in these fixtures.
Confirmation direct totals per five analyses: 63.516 -> 24.705 ms (approximately
12.703 -> 4.941 ms per call). Dependency per-three totals: 153.632 -> 49.310 ms
(approximately 51.211 -> 16.437 ms per call). These contemporaneous baselines,
not historical measurements from another session, establish the comparison.

Unfavorable results remain explicit: distinct long classes add about 0.4% by
paired median, single-sink custom direct analysis about 0.9–1.2%, and confirmation
default-full is 3.6% slower. Defaults do not execute the changed custom-sink path;
timings vary with allocation, scheduling and thermal state, and these small
differences are not evidence of a general speedup or fully diagnosed regressions.
The earlier default-analyzer 5.97 -> 7.22 ms recovery question remains OPEN.

The isolated harness uses 200 validations/sample for one short class and 10 for
128-sink cases, all with 32 argument indexes per sink. Distinct labels have a
253-byte common prefix plus three differing digits; they cannot use value reuse.
Existing direct harness uses 100 analyses/sample for small/full cases, five for
the repeated maximum policy, plus independent lexer/parser diagnostics. Existing
dependency harness uses 50 analyses/sample for small/flow and three for maximum.
Max cases have 128 sinks with 256-byte labels. No workload was weakened.

Direct calls include configuration, bounds, lexing, parsing, flow and reporting;
dependency calls additionally include normalization, pattern analysis and linking.
Fixtures are in-memory; source creation/import/setup are outside each timed call.
Isolated lexer/parser measurements are not exclusive spans of one analyzer call
and must not be added to manufacture an end-to-end trace. now_ms is not monotonic.
Whole-harness wall/RSS/CPU include both implementations, warmups and assertions;
no before/after heap-allocation reduction or VM-wide memory bound is established.

Raw samples/p95/variation: bench/results/taint-class-reuse-2026-09-08.jsonl.
Reproduce the isolated benchmark with bench/taint_class_validation_pair.sapp,
and the unchanged taint_configuration_pair.sapp / taint_policy_preflight_pair.sapp
for complete analysis. Reference imports execute code: use only reviewed b1db0b2
copies, route their imports to identical support modules (and reference taint for
the reference dependency copy), never an untrusted environment-selected path.

## Separate numeric comparison

Unchanged session_compare integer_kernel: one million iterations, status=ok and
checksum 474518 on all nine paths. First run plus warmup and five retained runs;
compiled paths first build plus three repeated builds.

| Path | Median ms | p95 ms | Median peak RSS KiB | Artifact bytes |
| --- | ---: | ---: | ---: | ---: |
| pblang-interpreter | 269.136 | 275.836 | 8608 | N/A |
| pblang-c99 | 4.502 | 4.927 | 1572 | 14472 |
| pblang-codegen-x64 | 3.537 | 3.791 | 264 | 507 |
| c | 7.602 | 7.912 | 1572 | 14472 |
| cpp | 8.294 | 8.911 | 3360 | 14472 |
| go | 9.552 | 9.871 | 1500 | 1228952 |
| rust | 7.723 | 8.229 | 1968 | 350560 |
| python | 207.480 | 217.438 | 10344 | N/A |
| javascript | 135.946 | 138.685 | 51608 | N/A |

Raw: bench/results/session-2026-09-08-taint-class-reuse.jsonl. This workload does
not execute the taint analyzer. PBLang's retained interpreter remains slower than
Python/JavaScript on it. Artifact size excludes runtime/dependencies; RSS is not
total allocation, and N/A is not zero. No other device/platform, full C99 analyzer,
or embedded-polyglot combination is qualified by this numeric comparison.

## Validation and remaining work

Sixteen targeted suites passed: class reuse, self-host string equality, policy
preflight, configuration, limits contract, admission, completeness, dependency
bounds, source bounds, token bounds, dependency analysis, general security,
dynamic eval, manifest, portable lock and security tools CLI. No full suite.
The initial build refreshed the expected changed dependency hash through the
normal build/pblock flow; integrity checks were not bypassed.
Final build and portable-lock publication passed without the stale-hash note.
Touched-code lint completed with existing binding/nonfinite/style warnings and
no errors. Owned-file privacy scans and git diff --check passed. Graphify reached
its 20-second deadline without completing; no freshness claim is made. No owned
test/build/benchmark/Graphify process remained. Temporary reviewed references
were removed; baseline source remains recoverable from Git. Pre-existing dirty
work, including frontend edits and the local lock, stays outside this commit.

Remaining: CLI entry pre-read and pattern-policy admission, concurrent policy
snapshots, distinct-policy and general analyzer performance, C99 embedded-NUL
parity, native self-bootstrap/executable refresh, compiler/paradigm compatibility,
runtime ownership/hard heap/preemption, typed FFI/raw bypass/sandbox, publisher
trust/artifact handoff/revocation/rollback, broad product regression and physical
platform verification. PBLang 1.0.0 remains NOT READY.
