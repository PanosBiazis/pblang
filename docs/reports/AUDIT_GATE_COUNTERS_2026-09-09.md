# Audit gate counter validation — 2026-09-09

Baseline: 16169b9. This continuation follows the gate-policy preflight work.
Product changes are confined to `stdlib/project_audit.sapp`, in PBLang.
No Rust source or executable was rebuilt. PBLang 1.0.0 remains NOT READY.

## Recovered context and selected scope

The available relevant conversation messages and local history establish prior
work on audit policy admission, project intake, polyglot compatibility, VM and
emulator validation, and platform adapters. The latest repository commit
confirms that gate policy preflight is already implemented. The next recorded
audit gap is report validity. Catalog entries with no readable turns were not
treated as full transcripts; this is not an exhaustive export of all chats.

The initial worktree contained 193 pre-existing status entries. This slice
preserves that work and adds one bounded report-counter check with focused
regressions. The local dependency lock is refreshed for the changed stdlib.

## Observed failure and repair

An exact copy of the baseline module and the current module were run with the
same retained runtime and shared dependencies, using a temporary checkout
fixture. The input was `finding_count:1` with only `counts_severity.high` set.

| Counter value | Baseline | Current |
| --- | --- | --- |
| -1 | exit 0, gate passed=true | exit 1, fixed counter-admission error |
| parse_number("NaN") | exit 1, comparison expects numbers | exit 1, fixed counter-admission error |

The negative-counter bypass is live-reproduced. NaN is **not** a reproduced
pass bypass on this runtime; it already failed during comparison. Its change
is deterministic admission and a specific error instead of an incidental
runtime error. Initial probes outside the checkout lacked manifest context
and are not counted as validation. Temporary probes and copied source were
removed from the checkout after execution.

Quality gating now admits all 19 consumed counters, five optional counter
maps, the report wrapper, and the fallback array shape. Present nil is rejected;
absent sparse keys retain their prior defaults. Source high-plus-medium uses a
checked sum. The canonical contract and exact errors are in
`Security_Analysis.md`, under Quality-gate counter admission.

## Compatibility and limitations

The check visits only fixed decision inputs. It neither enumerates extra map
keys nor traverses finding rows. Full aggregate counts remain separate from
bounded presentation indexes, and a valid explicit count still takes precedence
over the retained array. Returned count maps remain independent from inputs;
every call checks mutable input again. Valid thresholds and output schemas are
unchanged. No performance improvement is claimed or benchmarked.

This is not a report-integrity or checkpoint-authentication solution. A caller
can still lie using valid nonnegative counters; module aggregation can combine
values before the quality gate sees them. Cross-map/row consistency, input
admission before aggregation, whole-work/row limits, truncation, critical
severity policy and authenticated checkpoint reuse remain open. Comparison and
progress gates were not changed. A caller-provided report remains a trusted
embedding input, never a proof of source safety.

## Validation

The new pure suite exercises all 19 fields against 16 invalid values, directly
and through project wrappers with source-only policy. It also covers inclusive
valid bounds, container/report/fallback shape, checked sum, sparse maps,
retained-index compatibility, input mutation, output independence, unrelated
metadata, generated reports and release readiness.

The new Linux live suite runs seven deadline-bounded children, verifies exact
failure statuses and fixed diagnostics, checks that supplied private-marker
values are absent from output, and confirms malformed-policy precedence.
Owned fixture files are removed on success and caught test failure.

Independent defensive review found no blocking issue in the implementation
or tests.

All 16 selected suites passed: project_audit_gate_counters,
project_audit_gate_counters_live, project_audit, project_audit_limits,
project_audit_gate_policy, project_audit_gate_policy_live, project_text_audit,
project_text_audit_gate_policy, project_text_audit_gate_policy_live,
project_text_audit_finding_preflight, module_audit, module_audit_resume,
pbaudit_cli, security_analysis, manifest_validate and portable_lock_current.
They ran sequentially through the trusted-checkout test runner. This is not a
full-suite, binary-refresh or physical-platform validation claim.

Source checks on the three touched PBLang files reported zero errors. Lint
completed successfully with warnings, including existing style/unused-variable
diagnostics and the known incorrect `eq_op` advice for deliberate non-finite
checks. Those checks were retained. After an initial stale dependency-hash
diagnostic, the lock was refreshed; the final project build completed without
that diagnostic and PBLang-native portable-lock publication passed.

Added-line/new-file privacy patterns had zero matches; tracked diff and new
file whitespace checks passed. The baseline's 187 ordinary files were hashed:
only the already-dirty lockfile changed, with its stdlib hash refreshed. All
193 pre-existing status entries remain present. Temporary checkout fixtures
were removed. No owned test, build, lint or analyzer process remains.

Graphify query/explain completed during navigation. `graphify update .` reached
its 45-second deadline (exit 124, no output), leaving no process. Graph freshness
is not established. Changes remain local and uncommitted.

## Next work

1. Validate module reports before aggregation and reconcile count maps with
   retained findings without mistaking presentation truncation for full data.
2. Define report/checkpoint provenance, comparison bounds and completeness
   semantics before allowing untrusted reports to authorize a gate.
3. Continue compiler/C99/catch-variable compatibility, self-bootstrap and
   runtime refresh with independent execution evidence.
4. Keep VM resource enforcement, typed FFI, hostile-code sandboxing, publisher
   trust/revocation and physical-platform verification as explicit blockers.
