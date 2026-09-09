# Text-audit gate preflight — 2026-09-09

Baseline: c7a304d. PBLang-only changes in project_audit and project_text_audit.
No Rust source or executable refresh. PBLang 1.0.0 remains NOT READY.

## Repair and measured execution order

The previous gate-policy fix already rejected malformed/infinite policy.
This slice does not fix another infinity bypass: it prevents unnecessary
filesystem/aggregation work before that rejection.

A reviewed exact baseline copy and current module were exercised against the
same one-file fixture, same retained executable and shared current dependencies.
Both returned a fixed error for malformed max_high. Bounded syscall traces
showed the difference:

| Probe | Baseline audited-target file events | Current events | Diagnostic |
| --- | ---: | ---: | --- |
| audit_roots | 6 | 0 | same fixed numeric admission error |
| audit_modules | 6 | 0 | same fixed numeric admission error |
| combine_module_reports with poisoned counts | 0 | 0 | generic aggregation error becomes fixed policy rejection |

The filesystem events included opening the fixture directory and its source
file. No source was executed. These are targeted observations, not a universal
no-I/O claim; runtime startup and trusted imports still access files.
Raw traces containing machine paths were deleted after verification; only
counts and outcomes are retained here.

## API and compatibility

require_quality_gate_options(options=nil) returns a fresh ordinary map using
the existing validator. It is not an authorization token: quality_gate always
revalidates, including maps produced by this API. A regression mutates an
admitted map to infinity and confirms rejection by the consuming gate.

The filesystem layer centralizes its wrapper defaults and snapshots policy
before collection/aggregation, after each entry point's existing early
structural/finding-bound checks. All later gate calls consume that detached
snapshot, not the caller's original fields. The source options are not mutated.

Absent defaults remain overall 0/8/80, source 0/4/40, and both flags true.
Explicit nil keeps its previous general fallback: medium becomes 5, source
medium inherits resolved medium, source_only_blockers becomes false. Generic
quality-gate defaults are unchanged. Other shared options remain accepted.

Valid schemas, gate behavior, detector semantics and engine-v6 checkpoint
identity inputs remain unchanged. Invalid gate policy now wins before individual
module traversal errors, but established outer root/index/finding-bound errors
keep precedence. Snapshot verification has no quality gate and was not changed.

## Validation

Sixteen focused suites passed: the two new project_text_audit gate-policy
suites, both existing project_audit gate-policy suites, project_audit,
project_audit_limits, project_text_audit, project_text_audit_finding_preflight,
module_audit, module_audit_resume, pbaudit_limits, pbaudit_numeric_preflight
(four-child smoke), pbaudit_cli, security_analysis, manifest_validate and
portable_lock_current. No full suite or physical-platform test.

The pure suite covers malformed/valid ceilings and flags, map shape, fresh
default/output maps, revalidation, inheritance, wrapper missing-vs-nil behavior
and structured error precedence. The live default performs three supervised
children and checks fixed diagnostics without echoing the supplied marker.

Optional syscall testing selects one case with PB_TEXT_GATE_TRACE=roots,
modules or combine when running project_text_audit_gate_policy_live_test.sapp.
It requires Linux, strace and prlimit at the explicit system paths. A positive
control reads the known fixture file and must appear in the trace before a
zero-access assertion is accepted for the rejected request. The final roots
selection with positive control passed; all three modes also have the separate
before/after execution evidence above. Each traced child has a 20-second
deadline and 1 MiB output-file cap; secure trace reads share that byte ceiling.
Ordinary children have five seconds each. An outer supervisor should allow the
selected trace/control pair plus the two ordinary children and startup.

Initial all-file tracing was unsuitable as a default test: it generated about
27 MB per child from imports, and later the bounded reader correctly rejected
that log. Earlier diagnostic assertion failures were not counted
as coverage. The retained version filters only the exact fixture directory/file,
caps log growth and makes tracing opt-in per case. All owned temporary raw logs
(about 162 MB across the initial six probes) and failed-test fixtures were
removed. The bounded reader was not weakened.

Independent defensive review approved production and test changes, including
the positive control, no trusted-map bypass, compatibility and residual limits.

## Paired benchmark

bench/text_gate_preflight_pair.sapp imports the reviewed c7a304d text-audit copy
selected by PB_TEXT_GATE_REFERENCE and the current module, with identical shared
dependencies. It uses the existing one-file module fixture. Imports/setup are
outside timing; scans include inventory, bounded reads, analysis and gating.
Aggregation cases process the same module report. Every full result is compared
after recursively zeroing only duration_ms fields, outside the measured batch.
No other result field is ignored. All comparisons passed.

Two warmup pairs and eight retained AB/BA pairs per operation; full raw samples,
percentiles and dimensionless paired ratios are retained. The statistics helper's
ratio _ms field names are reused but do not denote milliseconds.

| Series | Operation | Calls/batch | Before median ms | After median ms | After p95 ms | Median paired ratio |
| --- | --- | ---: | ---: | ---: | ---: | ---: |
| first | roots | 1 | 3.203 | 2.952 | 3.378 | 0.9273 |
| first | modules | 1 | 3.837 | 3.481 | 4.334 | 0.9688 |
| first | combine | 100 | 10.363 | 11.530 | 12.683 | 1.1248 |
| first | combine-nil | 100 | 9.323 | 11.168 | 15.265 | 1.2458 |
| confirmation | roots | 1 | 3.013 | 3.511 | 4.300 | 1.1732 |
| confirmation | modules | 1 | 3.243 | 3.775 | 4.596 | 1.0683 |
| confirmation | combine | 100 | 11.083 | 14.340 | 18.715 | 1.2501 |
| confirmation | combine-nil | 100 | 11.418 | 14.375 | 17.326 | 1.2544 |

No consistent scan speedup is established: the small I/O cases reverse direction
between runs. Aggregation has a measured validation cost: roughly 12–25% paired,
or about 12–33 microseconds per invocation from batch-median differences.
That extra preflight is retained; skipping consumer validation would make the
public mutable map an unsafe trusted-policy shortcut. No zero-overhead or
end-to-end acceleration claim.

Linux x86_64, warm cache, shared host, non-monotonic now_ms. Each series had a
45-second outer deadline and 1 GiB virtual address-space ceiling. No owned
test/build/other benchmark ran concurrently. Initial repository status briefly
waited in kernel I/O and later completed. The host reported an unclean-unmount
warning for the project volume; no new I/O errors were observed in the inspected
kernel log. No filesystem repair/unmount was attempted, and causation of the
delay or benchmark variation is not established.
Raw: bench/results/text-gate-preflight-2026-09-09.jsonl.

## Separate numeric comparison

Unchanged integer_kernel, one million iterations: all nine rows returned ok and
checksum 474518. First run, one warmup and five retained runs; compiled paths
also measure first build and three repeated builds. Sequential 40-second
supervision per language, 32 GiB address-space ceiling, OMP/BLAS one thread and
GOMAXPROCS=2. These host limits are not VM heap-safety claims.

| Path | Median ms | p95 ms | Median peak RSS KiB | Artifact bytes |
| --- | ---: | ---: | ---: | ---: |
| pblang-interpreter | 448.780 | 499.455 | 8592 | N/A |
| pblang-c99 | 8.370 | 8.978 | 1496 | 14472 |
| pblang-codegen-x64 | 4.692 | 5.235 | 264 | 507 |
| c | 8.018 | 8.396 | 1496 | 14472 |
| cpp | 10.680 | 11.502 | 3392 | 14472 |
| go | 8.605 | 9.438 | 1500 | 1228952 |
| rust | 8.656 | 9.035 | 1984 | 350560 |
| python | 279.726 | 363.971 | 10316 | N/A |
| javascript | 136.795 | 141.759 | 51396 | N/A |

The interpreter remains slower than Python/JavaScript on this fixture. This
numeric kernel does not exercise the audit repair; differences from preceding
sessions are not attributable to this patch. C99/native rows qualify this
numeric fixture only, not full dynamic analyzer parity. RSS is resident memory;
artifact bytes exclude dependencies/runtime; N/A is not zero.
Raw compilation, CPU and run samples:
bench/results/session-2026-09-09-text-gate-preflight.jsonl.

## Remaining 1.0.0 work

Report shape/authenticity/counter integrity, comparison row/work bounds and
truncation; other direct module/snapshot/aggregate options; whole-input, record
and suppressed-row budgets; source/config/namespace consistency; compiler,
paradigm, C99 and documented catch-variable parity; linter float reasoning;
self-bootstrap and executable refresh; VM heap/time/ownership/cleanup; typed
FFI/raw-bypass closure and hostile-code sandbox; publisher trust, artifact
handoff, revocation/rollback; full product compatibility and physical platforms.

This patch does not establish complete security or universal platform support.

## Final local gates

The final project build completed without a dependency-hash note. PBLang-native
lock publication and the current portable-lock test passed; this is not a binary
rebuild. Lint completed on both owning modules, both new tests and the paired
benchmark, with existing/known style and unused-variable warnings and incorrect
eq_op advice for intentional non-finite checks. No such safety check was removed.
Intermediate manifest-hash mismatch diagnostics after stdlib edits required a
lock refresh and were not accepted as test passes.

Added-line/new-file privacy scans found no matched host-path/credential patterns;
git diff --check passed. All owned test/trace/reference fixtures were removed and
no owned test, benchmark or Graphify process remained. Graphify was limited to
one worker, 1 GiB address space and 20 seconds; it exited 124 with no output.
Its update did not complete, so graph freshness is not claimed.

The private remote was verified. Only this slice's ten files are selected for
commit; the 193 pre-existing dirty status entries remain outside it, including
the locally republished lock. The filesystem unclean-unmount warning described
above remains an operator follow-up, not a repaired PBLang issue.
