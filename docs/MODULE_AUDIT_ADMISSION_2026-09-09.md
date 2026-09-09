# Canonical module audit admission — 2026-09-09

This continuation builds on the local quality-gate counter fix over 16169b9.
The prior fix is preserved. All implementation is PBLang; no Rust source or
executable is rebuilt. PBLang 1.0.0 remains NOT READY.

## Context recovery

The app catalog, additional older turns, and a read-only streaming search
through the three core locally stored PBLang/pbAI/PBRemote transcripts informed
the continuation. Earlier audit fixes, platform work and runtime blockers were
reconciled with the live repository. This does not claim access to every chat
in the account or review of every tool-output byte; unavailable catalog-only
entries remain distinct from readable messages.

The starting worktree had 198 status entries, including the preceding local
counter fix. Their ordinary files were hashed before this continuation. The
next gap selected was validation before module accumulation and checkpoint
reuse, followed by consistent incomplete-coverage decisions.

## Before/after observations

The exact pre-change text-audit module and current module were executed using
the same retained runtime, current dependencies and isolated checkout fixture.
The baseline therefore already includes the prior quality-gate scalar check.

| Fixture | Before | After |
| --- | --- | --- |
| One positive high/source/security finding plus negative counters in a second module | complete=true, total=0, retained=1, gate passed=true | ok=false, invalid module report at index 1 |
| Canonical clean module with complete=false | outer incomplete but gate passed=true | outer incomplete, aggregate and gate both fail |
| Nine counted high findings with one retained row and explicit truncation | total=9, retained=1, gate blocked | same observed values |

These are live observations, not assumptions about NaN behavior. Temporary
baseline copies and probes were removed from the checkout after comparison.

## Implementation

`module_audit_contract` validates module scalars, fixed-width category maps,
safe sums, joint/marginal consistency and retained-row lower bounds. The full
batch is admitted before cross-module arithmetic, with checked aggregate sums.
Public validation results carry no persistent authority; every consumer checks
its input again. No caller report is modified during admission.

`project_text_audit` uses this contract before aggregation and snapshot
inventory. Finding-index appends stop once the output cap is reached, while
the truncation flag and full counts remain accurate. This loop change has no
claimed benchmark speedup. Incomplete coverage blocks both nested gate and
aggregate success, in direct scanning and recombination.

`pbaudit` validates loaded checkpoint modules before reuse and checks combination
success before writing. Malformed resume input is rejected with a fixed exit-2
diagnostic and preserved byte-for-byte. Since completeness rules changed, the
engine advances to v7; unmodified v6 checkpoints require a fresh scan and are
preserved on resume rejection. This is a documented correctness change.

## Validation

The new pure suite covers every consumed scalar and count map with invalid
numeric/type values, canonical shapes, flags, key/row/root bounds, cross-map
inconsistency, retained categories, cancellation, overflow, sparse zero maps,
unknown path roles, mutable input, index truncation, incomplete gates and
policy-error precedence. Existing module scan/resume suites also passed.

The new Linux integration suite creates and successfully resumes a genuine v7
checkpoint, then injects negative counters, malformed flags/maps, inconsistent
counts and unaccounted retained rows. Each resume exits 2 with the exact fixed
diagnostic, no supplied-marker leakage, no rescan message and byte-identical
checkpoint preservation. The source fixture is unchanged. An old-engine
identity is also rejected without rewriting the file. Children have explicit
deadlines and owned fixtures are cleaned on success and caught failure.

Independent defensive review found no blocking implementation or live-test
issue and confirmed that the documented limits match the implementation.

Twenty selected suites passed: module_audit_admission,
module_audit_admission_live, module_audit, module_audit_resume,
project_audit_gate_counters, project_audit_gate_counters_live, project_audit,
project_audit_limits, project_audit_gate_policy, project_audit_gate_policy_live,
project_text_audit, project_text_audit_gate_policy,
project_text_audit_gate_policy_live, project_text_audit_finding_preflight,
pbaudit_cli, pbaudit_limits, pbaudit_numeric_preflight, security_analysis,
manifest_validate and portable_lock_current. Execution was sequential through
the trusted-checkout runner. No full suite or physical-platform run is claimed.

All seven touched PBLang files passed source checks with zero errors. Lint
completed with warnings, including the known misleading advice on intentional
non-finite comparisons and unused-variable/style diagnostics. No protective
comparison was removed. After stdlib edits the dependency hash was refreshed;
the final project build had no stale-hash diagnostic, native portable-lock
publication passed, and portable_lock_current passed again.

Added-line/new-file privacy checks found no matched host-path or credential
patterns. Whitespace and `git diff --check` passed. All 198 initial status
entries remain present. Of the 192 initially hashed ordinary files, only the
already-dirty canonical security documentation and lock changed: the former
received the new contract section; the latter received the current stdlib hash.
The preceding local quality-gate implementation/tests remain byte-identical.
The eight newly changed/untracked paths make this a ten-file continuation,
including those two already-dirty files. No commit or push was performed.

Graphify queries completed, but the final AST graph update exceeded its
45-second limit (exit 124, no output). The graph is therefore not confirmed
current. The update process terminated; no task-owned test, build, analyzer,
or server process remained. Owned temporary comparison files were removed.

## Remaining boundaries

Canonical counter consistency is not report authenticity. An attacker able to
rewrite a report may remove rows and alter all matching counts consistently.
Checkpoint provenance, source-to-report authenticity, duplicate/requested-root
identity, arbitrary metadata validation and completeness of detection remain
open. Findings with unsupported categories require a reviewed schema extension.

Admission permits at most 256 modules with 100000 retained rows each. It can
therefore still perform 25.6 million row checks for a direct call. Fixed-width
maps bound auxiliary count work, but there is no new hard wall-time or VM heap
guarantee. CLI checkpoint reads retain their separate 64 MiB byte ceiling.
Direct producer root/numeric bounds and a shared whole-work budget remain open.

Compiler/C99 compatibility, self-bootstrap and executable refresh, VM-enforced
resources and typed FFI, hostile-code sandboxing, publisher trust and independent
physical-platform verification remain broader product work.
