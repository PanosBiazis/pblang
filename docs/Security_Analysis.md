# Source security and selectable analysis

PBLang-native smart builds run bounded dependency-wide security analysis by
default. The analyzer follows every reachable literal import with token-aware
resolution, securely reads modules without following symlinks on Linux, applies
policy rules to every module, and runs flow-sensitive semantic taint and
control-flow analysis over an in-memory linked whole-program snapshot. The
default `protect` mode blocks unsuppressed high-severity findings and reports
medium, low and informational findings before artifact publication.

```bash
pblang run tools/pbbuild.sapp --allow-all -- app.sapp build/app
```

## Modes

- `--security-mode=protect`: default; block high severity and report the rest.
- `--security-mode=strict`: block every enabled severity.
- `--security-mode=audit`: configurable findings are report-only; mandatory
  runtime invariants and graph completeness still block publication.
- `--security-mode=off` or `--no-security-analysis`: disable configurable
  pattern/taint enforcement while retaining mandatory runtime and graph gates.

Every build result contains a `security_analysis` report. Disabling the gate is
therefore visible to build automation rather than a silent global setting.
Artifact-producing builds require `security_scope=dependencies`. Entry-only
analysis remains available in the read-only analysis tooling, but cannot publish
an executable or portable artifact because it cannot inspect imported code.

After code generation, `pbbuild` re-reads every analyzed module and compares its
SHA-256 identity with the analysis snapshot. If any module changed, the
artifact is removed and publication fails. This detects replacement of the
analyzed source modules before publication. It does not by itself eliminate
every filesystem or toolchain TOCTOU boundary outside that retained snapshot.

## Scoped suppressions

Suppressions require a rule ID and a reason. They remain present in the report.

```pblang
// pblang-security-ignore-next-line: network.plaintext-http reason=loopback integration fixture
let endpoint = "http://127.0.0.1";

// pblang-security-ignore-begin: process.shell-command reason=migration fixture with fixed input
// reviewed source block
// pblang-security-ignore-end: process.shell-command
```

Blocks are capped at 200 lines. Unknown rule IDs, malformed directives,
duplicate starts, unmatched ends and unterminated blocks fail closed. High
severity suppressions still block by default. They require both a recorded
reason and the explicit `--allow-high-security-suppressions` policy.

## Custom configuration

The default `code.dynamic-eval` rule with needle `eval(` uses tokenizer
identifiers rather than raw substring matching. Comments, string examples and
longer names such as self_eval are inert; bare/member/optional-member eval calls
are detected across whitespace and comments. Findings are attributed once to
each callee's physical source line, including after multiline strings. This is
conservative lexical call-shape detection (an eval declaration may also match),
not complete alias/computed-call resolution. Malformed lexical input still blocks
the mandatory pass. The same exact ID/needle pair inherits this behavior when
severity/message is customized; different IDs or replacement needles remain
literal rules. See `SELFHOST_LEXICAL_BUILD_PROGRESS_2026-09-08.md`.

`--security-config=security-rules.json` accepts a bounded JSON object. Custom
rules with an existing ID replace that rule; new IDs add rules. IDs listed in
`disabled_rule_ids` remove rules. Invalid configuration fails closed.

```json
{
  "mode": "protect",
  "custom_rules": [
    {
      "id": "device.unsafe-mode",
      "severity": "high",
      "needle": "unsafe_device_mode(",
      "message": "unsafe device mode requires review"
    }
  ],
  "disabled_rule_ids": ["network.plaintext-http"],
  "taint_sanitizers": ["checked_project_path"],
  "allow_high_suppressions": false
}
```

`taint_sources`, `taint_sanitizers`, and `taint_sinks` in the same policy are
forwarded to dependency-wide semantic analysis. A configured local sanitizer
terminates taint only on its return value; its body is still analyzed with
tainted arguments, so it cannot hide a security-sensitive side effect.

`max_taint_source_bytes` optionally caps each taint input, including linked
whole-program input (finite integer, 1..134217728 bytes). Explicit values are not
widened during linking. Without an override, dependency analysis retains its
existing module and linked-output budgets; direct security_taint calls default
to 1 MiB. Rejection occurs before tokenizer allocation. See
`TAINT_COMPLETENESS.md` for UTF-8 boundaries and remaining memory/time limits.

`max_taint_tokens` bounds tokenizer output for import discovery and taint analysis,
including comments and EOF (default 65536; integer 1..1048576). Exhaustion discards
partial tokens and rejects all dependency modes. The same budget applies to
whole-program taint without automatic widening; it does not bound the linker's
internal tokenization or replace source-byte/VM-memory limits.

One rule can also be disabled for one invocation with
`--disable-security-rule=RULE_ID`. Configuration is limited to 256 KiB, 128
rules, 256-byte match strings and 512 findings per source.

## Standalone and selectable analysis

`pbsecure` performs strict dependency-wide security analysis by default:

```bash
pblang run tools/pbsecure.sapp --allow-all -- app.sapp --mode=strict --config=security-rules.json
```

Use `--project-root=DIR` for an entry outside the current project root.
`--entry-only` is the explicit bounded compatibility mode.

Both tools now reject malformed effective security policy with status 2 before
reading entry source. Configuration loading and CLI overrides happen first.
Dependency mode checks dependency limits, mode, taint limits, taint schema and
pattern schema; entry-only checks only the pattern policy it actually uses.
In pbanalyze this preflight runs only when security/all is selected, so an unused
deep-invalid security policy does not prevent performance-only analysis.

`security_analysis.validate_policy(options=nil)` reuses the analyzer's pattern
configuration validator; `security_dependency_analysis.validate_policy(options=nil)`
validates the resolved combined policy. Success is a fresh `{ok:true}` outcome,
not a source approval or a reusable validation capability. Errors contain constant
diagnostics (pattern failures also contain configuration_errors), not policy values.
Dependency nested nil-as-unset and nested-over-top-level precedence remain intact;
direct pattern calls reject explicit nil fields. Analyzers still validate again.
Malformed non-array disabled_rule_ids is no longer silently repaired by pbsecure;
dependency-mode nil still accepts CLI rule additions. Invalid pattern policy is
now a configuration failure, not a completed dependency report with findings.

Early pbsecure config rejections retain configuration snapshot verification;
they do not claim that any source snapshot was obtained. Existing exclusive
report publication and input-collision guards remain active. This does not provide
atomic policy snapshots, universal path-free reports or hard runtime resource
limits. See BENCHMARK_SESSION_2026-09-08_SECURITY_CLI_PREFLIGHT.md.

pbanalyze now uses secure_file.read_bytes with a 16 MiB ceiling before source
bytes are materialized in the VM, then retains its existing UTF-8 decoding.
Read failures (including backend exceptions) produce the existing generic
status-2 error; UTF-8 failures keep their separate existing message. Backend
diagnostics/paths are not echoed and no ordinary path-read fallback is allowed.
This intentionally rejects symlink traversal and unsupported special files even
for performance-only analysis. The built-in backend bounds individual reads;
it does not enforce total VM heap limits or guarantee cleanup on all exceptions.

Linux absolute/relative paths are supported by the existing descriptor backend.
macOS absolute secure paths are currently unsupported; Windows secure input
must be relative to a supported root. Such inputs now fail closed, whereas the
old ordinary reader could accept them. Use supported relative paths; broader
secure-root support needs implementation and physical testing, not a fallback.
Post-analysis source/config snapshot verification remains absent in pbanalyze.
See BENCHMARK_SESSION_2026-09-08_ANALYSIS_BOUNDED_READ.md for evidence and cost.

The opt-in `pbanalyze` tool composes selected analyses without adding them to
the default build hot path:

```bash
pblang run tools/pbanalyze.sapp --allow-all -- app.sapp --analysis=security,quality,performance
pblang run tools/pbanalyze.sapp --allow-all -- app.sapp --analysis=all --fail-on-findings
```

Available categories are `security`, `quality`, `performance`, `completeness`
and `roadmap`. The same security JSON config controls added, replaced and
disabled security rules.

Category names, including `all`, are case-insensitive: `ALL,performance`
selects all five categories in canonical order and retains security results.
Without all, recognized names keep first-requested order and duplicates are
removed. Unknown names are ignored in mixed requests; unknown-only selection
still fails. Existing empty/non-array library selections default to security.

The source_analysis module keeps a private canonical catalog. Public AVAILABLE,
report.available and report.analyses are detached arrays; editing a prior result
or the public compatibility catalog cannot change later category admission.
AVAILABLE is informational, not an analysis-registration API. The private array
is not mutated by this implementation; no runtime deep-freeze or concurrent
snapshot guarantee is claimed.

Finding limits are admitted by `source_analysis.limits(options=nil)`. Missing
options/key default to 256; an explicit value must be a finite integral ordinary
number in 1..1024. Explicit nil, extended numeric types, NaN/infinity, fractions
and out-of-range values return a constant value-free error. Successful outcomes
are fresh maps containing only ok and max_findings. For example:

```pblang
let A=import("stdlib:source_analysis.sapp");
let admitted=A.limits(map{max_findings:32});
let report=A.analyze("let value=1;", "example.sapp", map{max_findings:32});
```

The direct analyzer preserves source/options and category error precedence,
then validates before downstream work. The CLI's `--max-findings=1..1024`
preflight occurs before config/source I/O, returns status 2 for malformed final
values and preserves omitted=256 and last-option-wins. Integral syntax such as
1.0 remains accepted. This intentionally replaces silent fallback to 256 and
fractional caps with rejection; bad limits now precede config errors.
Limits bound retained findings, not total analysis work or VM memory/time.
Direct project_audit calls admit finite integral ordinary-number limits in
1..4096. Invalid values throw the constant error
`project audit max_findings must be a finite integer in 1..4096` before text
conversion or record traversal; scan_text and audit/readiness wrappers propagate
it. Do not catch this error and substitute an empty/clean audit. Successful
report schemas have no new ok field. Omitted or explicit nil preserves the
legacy text default 200 / record default 500; source_analysis retains its
separate stricter explicit-nil convention. Previously accepted fractions,
non-finite/extended numeric values, values below one and values above 4096
are deliberately rejected. The maximum matches existing module per-file limits.

For example, a bounded direct call remains:

```pblang
let audit=import("stdlib:project_audit.sapp");
let rows=audit.scan_text("example.sapp", "let value=1;", 32);
```

This bound does not cover suppressed rows, input size or total work/heap/time.
The narrow public `project_audit.require_finding_limit(value=nil)` returns an
admitted scalar (nil/omitted means 500) or throws the same fixed limit error.
It exposes no caller-controlled fallback or reusable authorization capability.
audit_roots now uses it before collection, preserving missing=200 and nil=500.
audit_modules rejects malformed max_findings_per_file, including nil, before
inventory and returns its existing invalid-module-bounds error; omission is 256.

pbaudit rejects malformed final --max-findings values with a value-free status-2
diagnostic before audited-root/report/checkpoint effects. The old output remains
untouched for these rejected requests, including resume. Default 160, numeric
1.0 and last-option-wins are preserved. The remaining numeric CLI options now
use the pure `project_audit.pbaudit_numeric_error` rule before root normalization,
report removal or checkpoint reads. Malformed final values no longer silently
fall back; all require finite integral ordinary numbers. Defaults remain unchanged:

| CLI option | Ordinary audit | Module audit |
| --- | --- | --- |
| max-depth | 0..32 | 0..32 |
| max-entries | 1..50000 | 1..50000 |
| max-children | 1..4096 | 1..4096 |
| max-files | 1..10000 | 1..100000 |
| max-file-bytes | 1000..1048576 | 1000..16777216 |
| max-runtime-ms | 10..300000 | 10..300000 |
| backlog-limit, lines | 1..9007199254740991 | 1..9007199254740991 |

The display options retain large exact integer requests: their loops also stop
at the bounded findings/module arrays, not at an allocated array of that size.
Zero/negative display values previously fell back or behaved inconsistently;
they now reject, as do fractions, non-finite values and unsafe integers.
Mode-specific limits follow the final mode flag, regardless of argument order.
The existing max-findings diagnostic retains first precedence. Invalid numeric
requests preserve old reports byte-for-byte, including --resume. Valid CLI
checkpoint identities and engine v6 remain unchanged by this numeric-only fix.
Direct module/snapshot/aggregate APIs still need broader option validation;
CLI admission is not a hard VM memory/time guarantee or a sandbox.
Startup/import I/O is not eliminated. Module engine v6 rejects unmodified v5 checkpoints on resume;
start a fresh audit rather than treating the old checkpoint as current.
See BENCHMARK_SESSION_2026-09-08_AUDIT_PREFLIGHT.md for compatibility and evidence.
For the underlying direct-call contract, see
BENCHMARK_SESSION_2026-09-08_PROJECT_AUDIT_LIMITS.md for compatibility and evidence.
See BENCHMARK_SESSION_2026-09-08_ANALYSIS_LIMITS.md for tests and measured costs.

The optional caller-supplied security_report remains a trusted embedding input;
its finding maps may be annotated by source_analysis. It is not an untrusted
proof/admission interface. The current CLI creates this report itself.
See BENCHMARK_SESSION_2026-09-08_ANALYSIS_SELECTION.md for regressions and costs.

## Exact project-audit gate policy

`quality_gate`, `compare_reports` and `progress_gate` accept a map or nil.
Missing/nil fields retain legacy defaults; unrelated keys remain supported for
shared options maps. The following ceilings require ordinary-number finite
integers in 0..9007199254740991:

| Gate | Numeric fields | Defaults |
| --- | --- | --- |
| quality | max_high, max_medium, max_total | 0, 5, 50 |
| quality | max_source_high, max_source_medium, max_source_total | corresponding resolved overall ceiling |
| comparison/progress | max_new_total, max_new_high, max_new_medium | 0, 0, 0 |

Quality flags `source_only_blockers` (false), `allow_roadmap` (true) and comparison
flags `block_new_security`, `block_new_reliability`, `block_new_performance` (true)
require exact booleans. Nil preserves the stated default. Explicit false remains
an intentional policy choice; numeric zero is not a substitute.

Invalid policy throws one of these fixed messages, without embedding its value:

- `project audit gate options must be a map or nil`
- `project audit gate numeric limits must be finite integers in 0..9007199254740991`
- `project audit gate flags must be booleans`

Do not convert these exceptions to empty/clean reports. Thresholds are resolved
before direct report inspection. `release_readiness` validates max_findings
first (preserving its diagnostic precedence), then snapshots quality policy
before scanning records. Returned thresholds do not alias the input options;
default maps are fresh on each call. Successful schemas and valid-policy
semantics remain unchanged; malformed policies deliberately no longer work.

This validates policy, not report authenticity, consistency of counters with
rows, completeness/truncation or total comparison work. Reports are still
trusted caller inputs. The private resolved helper is not an authorization
capability. Valid engine-v6 checkpoint semantics are unchanged. Tests,
corrected runtime evidence and measured costs:
`BENCHMARK_SESSION_2026-09-08_AUDIT_GATE_POLICY.md`.

`project_audit.require_quality_gate_options(options=nil)` exposes the same
validator as a plain detached canonical map. Its result is not a trusted token:
`quality_gate` always validates it again. Missing/nil inputs use the general
defaults above. Callers may change the returned map, but cannot use it to bypass
later admission.

The three project_text_audit gate consumers now preflight a detached policy:
`audit_roots` after finding-limit admission, before collection; `audit_modules`
after its existing root/bound checks, before inventory; `combine_module_reports`
after outer shape/index/requested-root checks, before module aggregation.
Their absent defaults remain 0/8/80 overall, 0/4/40 source and true/true flags.
Explicit nil retains the existing general fallback rather than the absent
wrapper default: max_medium:nil resolves to 5, max_source_medium:nil inherits
the resolved overall medium ceiling, and source_only_blockers:nil is false.
Other shared options remain supported and the input map is not modified.

Only quality policy is covered here. Startup/import I/O, other direct module
numeric bounds, snapshot validation and whole-work guarantees are not covered.
Report schemas, detector semantics and valid v6 checkpoint identities are
unchanged. Evidence and optional path-filtered trace instructions:
`BENCHMARK_SESSION_2026-09-09_TEXT_GATE_PREFLIGHT.md`.

### Quality-gate counter admission

After policy admission, `quality_gate` requires a direct report map or a
project wrapper whose present `report` member is a map. A sparse direct `{}`
still represents zero counters. A present `report:nil` is rejected.

The 19 scalar inputs used by the quality decision require ordinary-number
finite integers in 0..9007199254740991:

| Input | Consumed keys |
| --- | --- |
| report | finding_count |
| counts_severity | high, medium, low, info |
| counts_kind | security, reliability, performance, incomplete, roadmap |
| counts_role | source, docs, test, config |
| counts_role_severity | source:high, source:medium |
| counts_role_kind_severity | source:security:high, source:reliability:high, source:performance:high |

Absent containers/keys retain sparse zero defaults. Each present container
must be a map; each present consumed scalar is checked, including explicit
nil. The source high-plus-medium sum must also fit the exact integer domain;
overflow is rejected before addition. Every consumed counter is checked even
when the selected policy would not otherwise use it to block.

If `finding_count` is absent, an absent `findings` means zero and a present
`findings` must be an array whose length supplies the count. A supplied valid
`finding_count` remains authoritative: public/module indexes may retain fewer
rows than the total. No caller-sized map or finding rows are traversed here.
The validation work depends only on the fixed inputs in the table.

Errors use fixed messages without including supplied values:

- `project audit gate report must be a map`
- `project audit gate counter containers must be maps`
- `project audit gate findings fallback must be an array`
- `project audit gate counters must be finite integers in 0..9007199254740991`

These exceptions must propagate; do not replace them with a passing/empty
report. Valid policy defaults, gate output schemas, detached output counts,
and sparse aggregate behavior are preserved. Malformed policy errors retain
precedence over malformed report errors. `release_readiness` uses the same
admission after its existing finding-budget and policy preflight.

This closes malformed numeric decision inputs, not report forgery. Individually
valid but false counters, cross-counter/row consistency, completeness, critical
severity policy, checkpoint authenticity and module admission before aggregation
remain outside this change. Reports remain trusted embedding inputs. The
comparison/progress/backlog APIs were not changed. Valid engine-v6 checkpoint
identities remain unchanged. See `AUDIT_GATE_COUNTERS_2026-09-09.md` for the
observed negative-counter bypass and the separate NaN diagnostic improvement.

## Module-by-module repository audit

The current module engine is **v8**. Earlier v6 compatibility notes above
describe the preceding policy/counter fixes. V7 added canonical module admission
and consistent failure of incomplete coverage; v8 binds coverage to unique
requested root identities. Unmodified older-engine checkpoints have a different
audit identity and require a fresh scan. Rejected resume files
are preserved, not deleted or overwritten by the rejection path.

`stdlib:module_audit_contract.sapp` is a pure validation layer used before
`combine_module_reports` accumulates any counts, before snapshot inventory,
and before the CLI reuses a loaded checkpoint. Its `module_error(module)` and
`modules_error(modules)` return an empty string on admission or a fixed error;
successful validation is not a trusted token. Consumers validate mutable input
again on each call.

Canonical modules require their existing format tag, a nonempty root string
of at most 4096 code points without NUL, exact `complete` and
`finding_index_truncated` booleans, and four ordinary-number finite integers
in 0..9007199254740991: `files_scanned`, `finding_count`, `suppressed_count`,
and `encoding_degraded_files`. Degraded files cannot exceed scanned files.
Missing canonical fields and explicit nil are rejected.

The required count maps use fixed domains and maximum widths:

| Map | Domain | Maximum keys |
| --- | --- | ---: |
| counts_kind | security, reliability, performance, incomplete, roadmap | 5 |
| counts_severity | high, medium, low, info | 4 |
| counts_role | source, docs, test, config, unknown | 5 |
| counts_role_severity | role:severity | 20 |
| counts_role_kind_severity | role:kind:severity | 100 |

Widths are checked before `map_keys` allocation, and keys are bounded to 64
code points before splitting. Every value and sum uses the same exact integer
domain. Each map totals `finding_count`; the joint map must reproduce all four
marginal maps. Empty maps and explicit zero entries are equivalent for zero
categories. Unsupported categories are rejected instead of silently ignored.

The retained `findings` arrays are capped at 100000 rows across the entire
batch (and per module). A shape/length-only preflight runs before any row
content is inspected; excess total rows return `module audit whole-report
finding budget exceeded`. Each row
must be a map with supported role/kind/severity, and its category cannot occur
more often than the full joint count permits. `finding_count` cannot be smaller
than the retained length. Equality is required when the index is not marked
truncated; a truncated index remains a lower bound on the full counts.
Other finding metadata and source provenance are not validated by this layer.

The complete batch, capped at 256 modules, is admitted before cross-module
arithmetic; checked additions reject aggregate overflow before accumulation.
Failure returns `ok=false, complete=false` without a gate or partially combined
module result. Malformed gate-policy errors retain precedence over module
admission. `max_finding_index` requires a finite integer in 1..100000.

Incomplete scan/combine results retain their outer `status=incomplete`, while
both `aggregate.passed` and `quality_gate.passed` are false. The nested gate
has `status=blocked` and the fixed blocker `module audit coverage is incomplete`.
Empty batches with requested roots are incomplete initial checkpoints; the
legacy empty batch without requested coverage remains supported.

CLI checkpoint rejection uses `checkpoint module reports are invalid`, exits
2, and happens before snapshot verification and checkpoint writes. Snapshot
admission also protects direct callers. The CLI checks unsuccessful combination
results before every subsequent write. Source hashing still has its separate
purpose: detecting changed source bytes, not authenticating report counters.

This closes cancellation and inconsistent canonical category counts. It does
not prove that all source findings were recorded, establish checkpoint
authenticity or validate arbitrary metadata graphs. Engine v9 limits canonical
row traversal to 100000 retained rows across the batch. This is an algorithmic
work bound, not a hard CPU, heap or wall-time guarantee. Decoding and other
metadata require separate bounds; the CLI retains its 64 MiB checkpoint-byte
limit. See `MODULE_AUDIT_ADMISSION_2026-09-09.md` for earlier admission evidence
and `module_audit_work_budget_test.sapp` for the shared-row regression.

### Requested module coverage (engine v8)

The pure contract now exposes `roots_error(roots)` and
`coverage_error(modules, requested_roots)`. Root labels are exact strings,
nonempty and capped at 4096 code points without NUL; lists have at most 256
entries. Requested roots and module roots must each be unique. When a nonempty
requested list is supplied, every module must belong to it. A unique subset
is a valid incomplete checkpoint, and module ordering does not matter. Equal
cardinality establishes full requested coverage only after these checks and
only when all module reports are complete. Empty requested lists retain the
legacy unspecified-coverage convention, including an empty combined report.

Requested-list admission runs before canonical module row/counter traversal.
The combine path preserves outer module/index/requested-array errors and gate
policy precedence, then rejects invalid/duplicate requested root labels before
`modules_error`. The CLI preserves format, audit-id and outer module-index
precedence, then validates the saved requested list and its count/order against
the invocation before inspecting module contents. Thus a checkpoint with both
bad saved roots and bad counters returns the fixed coverage diagnostic. Module
membership still follows canonical report admission. That preflight-only change
preserved engine v8 identities; the shared budget described below advances to
v9. Rejection ordering alone is not a measured speedup or a whole-work bound. Regression: `module_audit_preflight_order` and the
mixed-invalid cases in `module_audit_coverage_live`.

No filesystem normalization or lookup occurs in the pure contract. Case,
relative/absolute labels and lexical aliases remain distinct for direct API
callers. `audit_modules` rejects invalid and duplicate exact roots before
inventory, without converting nonstrings to strings. Snapshot verification
also respects `options.requested_roots` when supplied. This is a label identity
contract, not proof that two paths name distinct physical directories, nor a
symlink, hardlink, mount or concurrent-filesystem identity guarantee.

The CLI validates roots before and after its existing absolute lexical
normalization, before removing an earlier output or loading a checkpoint.
Thus normalized aliases reject with exit 2 and `pbaudit: invalid module roots`.
Resume compares the checkpoint's top-level requested list, including order,
against the trusted invocation roots and admits its module membership before
snapshot inventory. Module order remains unrestricted. Foreign roots or an
inconsistent saved requested list produce `checkpoint module coverage is
invalid`; duplicate modules produce `checkpoint module reports are invalid`.
Diagnostics do not echo supplied roots. Rejection leaves checkpoint bytes
unchanged. The unkeyed audit identity is not treated as authentication.

Tests: `module_audit_coverage` and `module_audit_coverage_live`; the latter has
an opt-in `PB_MODULE_COVERAGE_TRACE=1` exact-path syscall check with a positive
control. See `MODULE_AUDIT_COVERAGE_2026-09-09.md` for observed evidence and limits.

### Direct module numeric preflight

`module_audit_contract.bounds_error(options=nil, snapshot=false)` checks only
consumed numeric fields. Whole options nil/omission retains defaults; a present
numeric field set to nil is invalid. Each value must be an ordinary-number
finite integer in the inclusive range below. Extra fields remain supported.

| Option | Default | Inclusive range |
| --- | ---: | ---: |
| max_files_per_module | 10000 | 1..100000 |
| max_file_bytes | 1048576 | 1000..16777216 |
| max_depth | 32 | 0..32 |
| max_entries_per_module | 50000 | 1..50000 |
| max_children_per_dir | 4096 | 1..4096 |
| max_runtime_ms_per_module | 300000 | 10..300000 |
| max_findings_per_file | 256 | 1..4096 |
| max_finding_index | 20000 | 1..100000 |
| max_findings_per_module_index | 20000 | 0..100000 |
| max_suppressed_per_module_index | 1000 | 1..10000 |

Engine v9 adds a shared 100000-row retention ceiling. An explicit per-module
index limit of zero keeps scanning and exact counts while retaining no rows;
explicit nil is still invalid. The CLI subtracts retained rows in requested-root
order, including reused checkpoints. Stable truncated modules are rescanned
when their retained length differs from the currently available cap, preserving
fresh-run retention after earlier modules change. Old engine v8 checkpoints
require a fresh scan because their audit identity differs.

The producer validates all ten fields after outer root-array admission and
before gate policy, root identity and inventory. Failure returns the existing
`ok=false, complete=false` module result with `invalid module audit bounds`.
Snapshot verification validates the first six fields before inventory, using
its existing `ok=false, stable=false, error="invalid module snapshot"` result.
The four finding/index fields are not consumed by snapshot verification and
remain ignored there, even when malformed. Validation does not enumerate
caller-supplied option keys, mutate input or include supplied values in errors.

For example, `audit_modules(project, roots, map{max_files_per_module: 1.5})`
now rejects before scanning; `max_files_per_module: 1` remains valid. Valid
v8 CLI checkpoint identities and report semantics are unchanged by this
preflight-only extension. It establishes numeric admission, not hard wall-time,
heap or aggregate-work enforcement. Evidence: `MODULE_AUDIT_BOUNDS_2026-09-09.md`.

The previous global audit stopped traversing when its shared file/finding cap
was reached. `--module-by-module` scans every eligible file independently and
retains findings under each module. Its bounded top-level finding index is only
a convenience view and never stops traversal:

```bash
pblang run tools/pbaudit.sapp --allow-all -- \
  --project=pblang --module-by-module --resume --json-output=module-audit.json \
  stdlib tools editors tests examples src
pblang run tools/pbmoduletriage.sapp -- module-audit.json --limit=30 --high-limit=100
```

The report is an atomic checkpoint after every module. `--resume` re-inventories
and securely re-reads every previously completed module, compares its full
eligible-file content snapshot, reuses only unchanged modules, and rescans any
changed or incomplete module. The output file is explicitly excluded from its
own audit. On Linux both source and report reads reject symlinks and use bounded
descriptor-relative access. Report findings contain location, classification
and a SHA-256 fingerprint, never the matched source line.
The checkpoint identity also includes a versioned analyzer revision; changes to
detector semantics, report fields, or completeness rules invalidate old module
results instead of silently reusing stale classifications.

`complete=true` requires every module inventory and eligible text file to be
read and scanned without a depth, time, file-size, file-count, or per-file
finding limit. Heuristic findings remain review candidates, not proof of a
vulnerability. Detector/catalog literals and test fixtures are retained as
audited suppressions with reasons rather than silently deleted.
Engine v5 reads raw bytes through the no-follow backend, hashes those exact
bytes for resume/TOCTOU verification, and scans UTF-8 exactly. BOM-tagged
UTF-16 and bounded legacy single-byte text receive an explicitly reported
ASCII-preserving pass, which retains language/security control tokens and line
numbers without pretending to reconstruct an unknown historical code page.
`encoding_degraded_files` keeps that reduced textual fidelity visible.
Eligible formats cover PBLang and common native, JVM, .NET, mobile, web,
scripting, assembly, database, serialization, build, and configuration source
text. Binary executables, archives, media, bytecode, and generated output remain
outside this source audit and require separate artifact scanners.
Vendored package trees (`node_modules`, virtual environments, `site-packages`,
`vendor`, CocoaPods, Gradle caches) are likewise excluded from first-party
heuristics and require dependency/SBOM and advisory analysis. Dataset corpora,
model files, and databases require separate data-provenance and artifact scans.

A whole-AAP checkpoint can be run from the PBLang checkout with explicitly
approved top-level modules:

```bash
pblang run tools/pbaudit.sapp --allow-all -- \
  --project=AAP --module-by-module --resume \
  --json-output=docs/AAP_MODULE_AUDIT.json \
  --exclude=products/pblang/docs/PBLANG_MODULE_AUDIT_2026-08-09.json \
  --max-depth=32 --max-entries=50000 --max-children=4096 \
  --max-files=100000 --max-file-bytes=1048576 --max-runtime-ms=300000 \
  ../../archive ../../docs ../../library ../../products ../../projects ../../training
```

An interrupted run leaves the latest complete atomic checkpoint. A quality-gate
exit code of `1` does not discard that report; use `pbmoduletriage` to classify
its heuristic candidates module by module.
Every repeated `--exclude=PATH` is exact-path matched, capped, recorded in the
checkpoint identity, and intended only for generated reports or other separately
audited artifacts. Broad path patterns are not accepted.

## Bounds and current limits

Taint budgets now require ordinary finite integers within their documented
intervals; invalid resolved numeric policy rejects before dependency reads.
Unterminated lexical tokens reject direct taint analysis before parsing.
Finding-index truncation and AST/call-budget exhaustion are blocking incomplete
results, even if retained findings have only medium severity. Per-module and
whole-program taint failures block protect as well as strict; audit still requires
complete coverage. Deduplication happens before the retained finding cap, so a
duplicate is not evidence loss. See [Taint completeness](TAINT_COMPLETENESS.md)
for defaults, compatibility tightening, tests and remaining resource boundaries.

This is not a proof that a program is vulnerability-free. Default dependency
bounds are 256 modules, depth 64, 1 MiB per module, and 8 MiB total source;
larger explicit bounds remain hard-capped. Literal imports, parser coverage,
linking, AST work, call expansion, recursion, and finding limits are all
reported. `semantic_complete=false` or `interprocedural_complete=false` cannot
pass strict mode. Dynamic imports and native/binary behavior require separate
runtime or binary analysis.
Per-file heuristic passes poll the module deadline every 128 lines. A large or
pathological file therefore returns an explicit incomplete reason instead of
holding the audit process beyond its configured device/resource budget.

The retained host `pblang compile` and `run` paths do not yet invoke the
PBLang-written gate; use `pbbuild` for default-on artifact gating until the
bootstrap CLI is replaced. The Linux no-follow reader uses the existing
PBLang system-API capability and therefore `pbbuild`, `pbsecure`, and repository
audits are launched with explicit build-tool capabilities, not inherited by
the produced application.

For large or automated reports, `pbsecure --json-output=FILE` publishes one
bounded UTF-8 JSON document through the no-follow secure-file backend and emits
nothing to stdout. Every platform uses exclusive creation and fails if the
destination already exists, preventing replacement through alternate path
spellings, aliases, symlinks, or case-folded mounts. The parent directory must
already exist; source/config/module collisions are also rejected with
case-insensitive comparison on Windows/macOS. Entry/dependency source snapshots
and an optional security-policy config snapshot are reverified immediately
before publication and their expected/actual digests are retained in the
report. Exit status remains `0` for a
passing report, `1` for a successfully published blocking report, and `2` when
no trustworthy report could be written. Exclusive creation does not yet claim
atomic visibility or power-loss durability: consumers must parse the complete
JSON and fail closed on a partial file, which an operator may remove only after
inspection. Report paths/hashes/suppression reasons should be stored in a
private directory.

An immutable token-structural policy additionally reserves the seven retained
raw `polyglot_*` VM builtin identifiers. It runs independently of configurable
pattern rules, scans every token even after the normal finding index is full,
and cannot be disabled, replaced, source-suppressed, or mutated through module
exports. Comments, strings, dot/optional member calls, and quoted map keys are
inert. Bare calls, aliases, unquoted reserved keys, and ambiguous colon syntax
are blocking. Tokenization
failure or an unterminated token makes the mandatory pass incomplete and blocks
protect/strict publication. Audit mode retains the evidence without claiming
that the direct bootstrap `run`/`compile` commands enforce this policy.

The reproducible `bench/security_analysis_stages.sapp` benchmark records entry
pattern, semantic AST, and dependency-wide whole-program costs separately.
`bench/module_audit_stages.sapp` separately measures a full module scan,
unchanged-snapshot verification, and bounded report recombination.

## Current measured repository evidence

The 2026-08-09 engine-v4 PBLang report at
`docs/PBLANG_MODULE_AUDIT_2026-08-09.json` covers 18 modules and 2,535 files
with `complete=true`. Its 1,771 rows are heuristic triage candidates; the
source/security/high count is zero after focused manual classification and
regression tests, while reliability/performance backlog keeps the quality gate
blocked. The active AAP report is intentionally incomplete and is documented in
the workspace `docs/AAP_SECURITY_QUALITY_AUDIT_2026-08-09.md`; no whole-AAP
security claim is made from that partial source scan.

Engine-v5 focused migration evidence scanned `projects/desktop` completely
(55 files) and `training/delphi` completely (116 files), closing their previous
legacy-encoding blockers. Splitting `projects/ai` into eight natural modules
completed seven; `projects/ai/WEB` remains an explicit deadline-bound shard and
must be subdivided before the whole-AAP checkpoint can be called complete.
## Retained host GC isolation

The retained native runtime currently keeps arrays, maps, and modules in
thread-local registries shared by multiple VM instances. A nested imported VM
cannot prove that it has marked every suspended caller root before sweeping
that registry. Therefore `stdlib:runtime_gc.sapp` rejects destructive host
collection, `resources.sapp` and `debugger.sapp` expose that refusal explicitly,
and the default security analyzer immutably rejects every bare host `gc_collect`
reference or declaration. The freestanding collector uses the unambiguous
`kernel_gc_collect` name, so security does not depend on declaration order.

`audit`, `off`, and `--no-security-analysis` can disable or de-escalate
configurable pattern and taint policy, but they cannot disable mandatory runtime
invariants or dependency-graph completeness. This distinction prevents noisy
policy checks from becoming an all-or-nothing switch for known corruption
primitives.

This is containment, not owner-scoped heap enforcement. Native `pblang run`
must not execute untrusted dependency graphs. The complete PBLang-only repair is
to move execution authority to `stdlib:compiler/vm.sapp`, give each runtime
instance an owner-scoped heap domain, run imports in the correct domain, and
retire retained native execution for untrusted programs. A future true-fix gate
must prove that collection reclaims unreachable cycles while preserving caller
state across nested imports and two same-thread VM instances.

## Immutable smart-build publication

`stdlib:build.sapp` treats the dependency analyzer's linked program as private
build state. It removes `linked_source` from the public security report before
any success or failure return, so blocked and snapshot-failed JSON cannot expose
the flattened project source.

Direct x86-64 and hosted C99 artifacts are generated under a private mode-0700
stage and loaded through descriptor-held bounded reads. C99 transpilation is
performed in memory and generated C reaches a fixed system compiler only via
the bounded Linux process backend's private stdin descriptor; project source
and generated C are never reopened by pathname. Secure compiler selection does
not execute `which`, `os_which`, or another PATH-controlled discovery helper;
the recipe and executable are selected together from allowlisted absolute
system directories. Host architecture probing likewise uses only fixed absolute
`uname` paths before passive environment fallback. PBCK serialization consumes
the same analyzed linked source entirely in memory and rejects any unresolved
real import instead of falling back to disk.

No final artifact name becomes visible until dependency hashes pass the final
snapshot check. Publication then uses the cross-platform secure-file atomic
writer, executable permissions are applied through a descriptor-held Linux
parent, and the published bytes are rehashed against the staged digest. If
portable-lock publication then fails, cleanup targets the canonical output path
reported by the backend, including `.pbck` or `.exe`, uses descriptor-oriented
file-only removal, and reports removal only after the held-parent unlink confirms
namespace removal. It never infers absence from path-following `fs_exists`, which
cannot distinguish a dangling symlink from a missing directory entry.

The exported immutable-source C99 API also compiles to a private bounded blob
before opening any caller output path. Its final bytes and the immutable-source
PBCK APIs use the same atomic secure-file writer and digest recheck, so a
preplanted output symlink is replaced rather than followed. Tool discovery is
non-executing; build paths, omitted-target selection, architecture probes, and
optional compressors execute only allowlisted absolute system binaries.

Before transpilation, immutable C99 source is admitted as valid UTF-8 text with
a hard 10 MiB byte ceiling aligned with the VM string ceiling. A cheap character
ceiling precedes the bounded UTF-8 byte conversion. Private C99 cleanup removes
only the exact allowlisted compiler output through the held directory, verifies
descriptor closure, and uses final empty-directory removal as proof that no
unexpected stage entry exists. It performs no unbounded directory enumeration.
Publication failures expose `output_removed` and `cleanup_verified`; callers
must treat either false value as an operator-visible cleanup incident.
When dependency verification fails before publication, the report instead sets
`publication_attempted=false` and makes no claim that a pre-existing output was
removed.

Build wrappers never resolve the PBLang runtime through ambient `PATH`. On
Linux they reuse `/proc/self/exe`; `PBLANG_BIN` is an explicit absolute,
existing-path operator override. Other hosts currently fail closed until an
equivalent verified current-process path is available. The override grants the
named executable compiler authority and must therefore be set only by the
operator or a protected service configuration.

The immutable C99 transport currently requires the Linux x86-64 bounded process
backend. Other hosts must use the in-memory PBCK route until they have an
equivalent descriptor/stdin process backend; the legacy `dist.native(path, ...)`
API remains a compatibility API and is not the secure smart-build path. The
PBLang-written in-memory C transpiler also does not yet cover every stdlib
construct. General dynamic string operations, for-in lowering, and exception
semantics remain outside its current hosted-numeric subset; unsupported linked
programs fail before publication and never fall back automatically to the
retained path.

## Invalid numeric source

Malformed numeric tokens make the mandatory scan incomplete and block admission,
independently of configurable severity or source suppressions. Taint analysis
also rejects the parser failure. See [self-hosted numeric literal diagnostics](SELFHOST_NUMERIC_LITERALS.md)
for the accepted subset, fixed diagnostics and remaining bootstrap differences.
