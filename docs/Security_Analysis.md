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
- `--security-mode=audit`: report findings without blocking publication.
- `--security-mode=off` or `--no-security-analysis`: explicit full opt-out.

Every build result contains a `security_analysis` report. Disabling the gate is
therefore visible to build automation rather than a silent global setting.
`security_scope=entry` remains an explicit compatibility option for callers
that cannot yet provide a resolvable dependency graph; it is never reported as
dependency-wide or semantically complete.

After code generation, `pbbuild` re-reads every analyzed module and compares its
SHA-256 identity with the analysis snapshot. If any module changed, the
artifact is removed and publication fails. This closes the analysis/build
check-to-use window instead of relying on an earlier path check.

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

The opt-in `pbanalyze` tool composes selected analyses without adding them to
the default build hot path:

```bash
pblang run tools/pbanalyze.sapp --allow-all -- app.sapp --analysis=security,quality,performance
pblang run tools/pbanalyze.sapp --allow-all -- app.sapp --analysis=all --fail-on-findings
```

Available categories are `security`, `quality`, `performance`, `completeness`
and `roadmap`. The same security JSON config controls added, replaced and
disabled security rules.

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
