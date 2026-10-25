# Exact audit gate policy and cost — 2026-09-08

Baseline: 9d2eb02. Production changes are confined to stdlib/project_audit.sapp.
No Rust source or executable replacement. This is a narrow policy-admission
repair, not completion of the 1.0.0 readiness audit.

## Reproduced behavior and correction

A temporary exact baseline module and the current module ran in the same
retained executable with the same current shared dependencies and synthetic
reports. Positive infinity across numeric ceilings previously returned passed=true
from both quality and comparison gates. On a separate single low-severity
security row with max_new_total=1, block_new_security=0 passed before the fix;
the current gate rejects it. Exact false still intentionally passes and true
still blocks. These are malformed-policy weaknesses when options cross a
misconfigured or untrusted boundary, not a new privilege beyond a caller who
is already authorized to choose lenient valid policy.

Correction to the initial analysis: NaN did NOT produce a clean gate pass in
this runtime. It raised the late generic error "comparison expects numbers".
The patch produces an early fixed admission error instead. Numeric booleans
and strings likewise threw generic baseline errors; fractional/negative ceilings
were accepted but blocked the tested finding. Do not infer a universal IEEE
comparison behavior from a source-level expression.

## Implementation and compatibility

Gate-specific private helpers require map-or-nil options, exact booleans, and
ordinary finite integral numeric ceilings in 0..9007199254740991. Extended numeric
representations, non-finite numbers, fractions, negatives and unsafe integers
are rejected, with fixed input-value-free exceptions. Error messages do not echo
supplied values; normal runtime stack diagnostics may still name source files.

Nil/omission keeps legacy defaults and source-ceiling inheritance. Extra keys in
shared option maps are ignored as before. Each quality policy is a fresh map,
detached from caller options; its old threshold key order and result schema are
preserved. The private resolved implementation reuses that fresh map in its
result to avoid a second allocation. Nil quality calls return a fresh default
map directly, avoiding needless field lookups while retaining isolation.

compare_reports resolves its three ceilings and three flags before finding
traversal, using a scalar snapshot. progress_gate propagates errors unchanged.
release_readiness preserves max_findings admission precedence, snapshots its
quality policy before audit_project, then reuses that snapshot. Backlog and
other non-gate options are not covered by this change.

The full result JSON matches baseline for all four valid benchmark policies.
The focused suites also retain valid legacy gate behavior. Engine-v6 checkpoint
semantics remain unchanged; this private/additive repair does not bump the
engine. This does not promise compatibility for previously malformed policy.

## Targeted validation

Both new suites passed: project_audit_gate_policy_test (pure exhaustive field
admission, valid boundaries, defaults/inheritance, exact flags, input/output
non-aliasing, fresh defaults and progress propagation) and
project_audit_gate_policy_live_test (five supervised child executions checking
exact diagnostics, exit status, no supplied marker echo, release finding-limit
precedence and rejection before poisoned record/finding traversal).

Twelve existing focused suites passed: project_audit, project_audit_limits,
pbaudit_limits, pbaudit_numeric_preflight (four-child smoke), pbaudit_cli,
source_analysis, security_analysis, project_text_audit, module_audit,
module_audit_resume, manifest_validate and portable_lock_current. Total: 14
targeted suites. No full suite or physical-device test.

During test development, the documented catch-variable syntax failed with
"undefined global 'e'" in the available executable. Tests now use supported
catch expressions; separate live children verify exact errors. This is a
documented-syntax/runtime compatibility discrepancy, not a compiler fix in
this patch. Intermediate stdlib edits required build/portable-lock refresh;
dependency-hash mismatches were not accepted as passing tests.

Independent defensive review approved the final narrow diff, explicitly
correcting the NaN claim and retaining the wrapper/report limitations below.

## Identical-workload paired benchmark

bench/audit_gate_policy_pair.sapp loads the exact 9d2eb02 project_audit module
through operator-supplied PB_GATE_REFERENCE and the current module. Reproduce
with a temporary reviewed baseline copy alongside bench, from repository root,
using the same shared dependencies. Do not execute an unreviewed reference.
Imports/report construction are outside timing. Each operation measures 300
calls, two warmup pairs and 12 retained alternating AB/BA pairs. Complete result
JSON must match and elapsed time must be positive. No filesystem work is inside
the measured batches.

| Variant | Operation | Before median ms/300 | After median ms/300 | Median paired ratio |
| --- | --- | ---: | ---: | ---: |
| initial-validation | quality-default | 13.398 | 14.023 | 1.0918 |
| initial-validation | quality-explicit | 14.265 | 16.798 | 1.1786 |
| initial-validation | compare-default | 23.333 | 23.211 | 0.9879 |
| initial-validation | compare-explicit | 21.048 | 23.369 | 1.1044 |
| final-fresh-default | quality-default | 12.384 | 10.783 | 0.8495 |
| final-fresh-default | quality-explicit | 12.169 | 14.609 | 1.1826 |
| final-fresh-default | compare-default | 22.653 | 23.199 | 1.0281 |
| final-fresh-default | compare-explicit | 20.690 | 22.036 | 1.0975 |
| final-confirmation | quality-default | 13.821 | 12.401 | 0.8723 |
| final-confirmation | quality-explicit | 12.759 | 15.260 | 1.2663 |
| final-confirmation | compare-default | 23.160 | 23.111 | 1.0105 |
| final-confirmation | compare-explicit | 20.693 | 22.665 | 1.1089 |

The initial implementation slowed the nil quality path by about 9.2% paired.
The fresh-default path removed that avoidable work: final paired ratios are
0.8495 and 0.8723 (about 15.0% and 12.8% faster on this microbenchmark).
Explicit quality policy still costs about 8.1–8.3 microseconds more per call
using differences of batch medians; paired slowdowns are 18.3% and 26.6%.
Explicit comparison costs about 4.5–6.6 microseconds more per call, paired
slowdowns 9.7% and 10.9%. Default comparison varies by roughly +1.0% to +2.8%
paired. These extra type/range checks are retained for correctness; no blanket
"no regression" or end-to-end speedup claim is made.

All three series, including the slower initial version, are retained in
bench/results/audit-gate-policy-2026-09-08.jsonl. The first initial row preserves
raw arrays recovered from tool output plus a summary subset; later rows have
the complete emitted summaries. Ratio summaries reuse the statistics helper's
_ms field names but represent dimensionless ratios. Separate medians can differ
from the median of paired ratios.

Linux x86_64, shared host, warm filesystem cache, non-monotonic now_ms. Each pair
run had a 40-second outer deadline and 768-MiB virtual address-space ceiling.
No owned test/build/other benchmark ran concurrently. This is not a PBLang heap
guarantee and timing alone proves no allocation/RSS improvement.

## Separate numeric comparison

Unchanged integer_kernel workload: one million iterations, checksum 474518 in
all nine rows. One first execution, one warmup and five retained runs; compiled
rows also record first compilation and three repeated builds. Per-language
40-second supervision, 32-GiB virtual address-space ceiling, OMP/BLAS one thread
and GOMAXPROCS=2. Sequential execution, not an isolated machine.

| Path | Median ms | p95 ms | Median peak RSS KiB | Artifact bytes |
| --- | ---: | ---: | ---: | ---: |
| pblang-interpreter | 561.223 | 568.622 | 8716 | N/A |
| pblang-c99 | 8.938 | 9.010 | 1572 | 14472 |
| pblang-codegen-x64 | 5.073 | 5.873 | 264 | 507 |
| c | 9.646 | 10.529 | 1572 | 14472 |
| cpp | 11.082 | 12.131 | 3380 | 14472 |
| go | 11.611 | 12.223 | 1500 | 1228952 |
| rust | 10.476 | 10.603 | 1908 | 350560 |
| python | 262.769 | 269.720 | 10236 | N/A |
| javascript | 179.616 | 194.673 | 51504 | N/A |

Interpreter is slower than Python and JavaScript on this workload. The numeric
kernel does not exercise this audit change, so changes from earlier sessions
are not causal patch results. C99/native rows cover this numeric fixture, not
the full dynamic analyzer. RSS is resident memory, not allocation; artifact
sizes omit runtime/dependencies; N/A is not zero. No universal language,
polyglot, device or platform claim. Raw samples and compilation/CPU statistics:
bench/results/session-2026-09-08-audit-gate-policy.jsonl.

## Remaining release work

Gate report shape/counter integrity, bounded comparison rows and truncation;
earlier gate-policy admission in project_text_audit filesystem/module wrappers;
other direct module/snapshot/aggregate options; whole-input/record/suppressed-row
and total-work budgets; source/config/namespace snapshots and trusted report
ownership; compiler/paradigm/C99/catch-variable parity and linter float reasoning;
native self-bootstrap/executable refresh; VM heap/time/ownership/cleanup;
typed FFI/raw bypass and hostile-code sandbox; publisher trust/artifact handoff/
revocation/rollback; broad product compatibility and physical-platform evidence.

PBLang 1.0.0 remains NOT READY.

## Final verification

Final project build completed with no dependency-hash note. PBLang-native lock
publication and portable_lock_current_test passed; this does not rebuild the
executable. Lint completed on all four touched code files with warnings:
unused-variable/style warnings (including used variables under negation/catch)
and incorrect eq_op suggestions for deliberate NaN/infinity checks. The finite
checks were retained. Added-line/new-file privacy scans found no matched
host-path/credential patterns, and git diff --check passed.

The temporary baseline module, probes and owned live-test fixtures were removed;
no owned test/benchmark/Graphify worker remained. Graphify was limited to one
worker, a 1-GiB virtual address-space ceiling and 20 seconds. It ended at the
deadline with status 124 and no output; graph update completion is not claimed.

Only this slice's nine source/test/benchmark/documentation files are selected
for commit. The 193 pre-existing dirty status entries remain outside the commit,
including the locally republished lock. Repository privacy was reverified before
publication.
