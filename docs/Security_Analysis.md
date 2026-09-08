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
Direct project_audit callers have separate admission limitations.
See BENCHMARK_SESSION_2026-09-08_ANALYSIS_LIMITS.md for tests and measured costs.

The optional caller-supplied security_report remains a trusted embedding input;
its finding maps may be annotated by source_analysis. It is not an untrusted
proof/admission interface. The current CLI creates this report itself.
See BENCHMARK_SESSION_2026-09-08_ANALYSIS_SELECTION.md for regressions and costs.

## Module-by-module repository audit

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
  --max-depth=32 --max-entries=50000 --max-children=10000 \
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
