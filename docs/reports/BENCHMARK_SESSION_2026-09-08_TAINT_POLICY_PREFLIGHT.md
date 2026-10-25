# Dependency taint-policy preflight — 2026-09-08

Baseline 3c97dde; Linux x86-64, same retained executable and supporting modules.
PBLang-only implementation; no Rust edit or executable replacement.

## Correctness and scope

The dependency analyzer previously read entry source or resolved imports before
rejecting malformed nested taint policy. An absent-entry baseline probe with an
invalid unused sink produced ok=true, passed=false, unresolved_count=1 rather
than an immediate policy error. Linux strace observed one target-file syscall.
This is an ordering defect, not demonstrated arbitrary code execution.

The new outcome-only security_taint.validate_policy uses the existing bounded
configuration validator. Dependency analyze_entry calls it after numeric limits
but before path normalization, source handling or import probes. Invalid policy
returns exactly ok=false, passed=false, format and a constant error. Valid reports
remain unchanged. Direct analysis and each later module/linked analysis still
validate; no mutable map identity cache or trusted-policy flag is accepted.

The regression first failed against baseline. After the fix, all 14 targeted
suites passed. A Linux file-syscall trace of the full new preflight test found zero
accesses to both the absent-entry and unresolved-import marker paths. The module
imports needed to run the analyzer are outside this claim. The first temporary
probe used stdlib: from outside a project and failed import resolution; it was
corrected to an absolute operator-reviewed module import before taking baseline
evidence. A stale dependency hash initially blocked the changed test; normal
project build and portable-lock publication refreshed it, without disabling checks.

This boundary is the library call, not the CLI: pbsecure and pbanalyze still
read entry/config first. Pattern-policy preflight, the legacy non-map outer
security fallback, and concurrent mutation/snapshot safety remain separate.
See TAINT_COMPLETENESS.md for API examples and compatibility.

## Paired complete dependency-analysis measurements

bench/taint_policy_preflight_pair.sapp imports a locally reviewed baseline copy
of the dependency module. Its six stdlib import paths were changed only to point
to the identical current supporting modules because the reference lived outside
the project. No algorithm or workload change in the baseline. The reference
import executes code: it must remain operator-controlled, never an untrusted path.

Two independent series, two warmup AB/BA pairs and twelve retained alternating
pairs per case. Full result JSON equality is checked outside timing; valid
reports, source identity and finding counts must match. No outlier is dropped.

| Case | Analyses/sample | Initial paired median ratio | Confirmation |
| --- | ---: | ---: | ---: |
| default-small | 50 | 1.0145 | 1.0110 |
| custom-flow | 50 | 1.0152 | 1.0154 |
| custom-boundary | 3 | 1.4641 | 1.4851 |

Ratios above one mean slower. This is a security ordering improvement, NOT a
speed optimization. Small cases show approximately 1–1.5% paired median overhead,
near observed timing variation. At maximum custom policy the repeated validation
adds approximately 46–49%: confirmation totals 81.168 -> 121.003 ms per three
analyses, approximately 27.06 -> 40.33 ms per dependency analysis. Unlike the prior
direct-analyzer benchmark, this includes per-module AND linked taint passes, and
the candidate adds preflight. Historical direct timings are not comparable.

default-small analyzes let value=1; custom-flow analyzes one env_get to fs_write
with a custom source/sink and requires two findings (module plus linked).
The maximum policy uses 128 sinks, each 32 argument indexes and a 256-byte UTF-8
class label, against empty source. All fixture preparation is outside timing.
No actual source file I/O occurs in these valid in-memory workloads; imports,
fixture creation and assertions are outside the measured analyze_entry calls.

Complete calls include admissions, path normalization, import tokenization,
pattern analysis, module taint, linking, linked taint and report construction.
These are not separate exclusive stage timings; no synthetic stage sum is made.
now_ms remains a non-monotonic clock. Raw p95, sample arrays and variation are in
bench/results/taint-policy-preflight-2026-09-08.jsonl. Whole-harness wall/RSS/CPU
measure both implementations plus setup and are not separate allocation evidence.

The maximum-policy cost and earlier analyzer regression remain OPEN. Safe future
reuse needs ownership/immutability and mutation guarantees; validation is not
skipped merely because a policy was previously accepted.

## Separate numeric comparison

Unchanged session_compare integer_kernel, one million iterations. All nine
paths returned status=ok and checksum 474518. One first run, one warmup, five
retained runs; compiled paths one first build plus three repeated builds.

| Path | Median ms | p95 ms | Median peak RSS KiB | Artifact bytes |
| --- | ---: | ---: | ---: | ---: |
| pblang-interpreter | 278.254 | 287.180 | 8576 | N/A |
| pblang-c99 | 7.537 | 7.960 | 1564 | 14472 |
| pblang-codegen-x64 | 3.598 | 3.818 | 264 | 507 |
| c | 7.375 | 8.003 | 1508 | 14472 |
| cpp | 8.267 | 8.427 | 3380 | 14472 |
| go | 9.254 | 9.551 | 1500 | 1228952 |
| rust | 8.131 | 8.566 | 2072 | 350560 |
| python | 203.645 | 204.789 | 10240 | N/A |
| javascript | 163.686 | 219.408 | 51496 | N/A |

Raw: bench/results/session-2026-09-08-taint-policy-preflight.jsonl. This workload
does not call the changed analyzer. The retained interpreter is slower than
Python and JavaScript here; no universal language superiority is claimed.
Artifact bytes exclude runtime/dependencies, RSS is not total allocation and N/A
is not zero. No additional device, platform or embedded-polyglot combination ran.

## Validation and remaining work

Fourteen targeted suites: policy preflight, configuration, limits contract,
admission, completeness, dependency bounds, source bounds, token bounds,
dependency analysis, security analysis, dynamic eval, manifest, portable lock,
and security tools CLI. Independent defensive review found no blocking issue.

Final build and portable-lock publication passed with no validator-unavailable
note. Touched-code lint passed with warnings for existing bindings, deliberate
nonfinite checks, boolean style and benchmark loop bindings. No full suite ran.
Owned-file privacy scans and git diff --check passed. Graphify reached its
20-second deadline without completing; no graph freshness claim is made. No
owned test/build/benchmark/Graphify process remained. Temporary reference/probe
and syscall trace files were removed; path-free observations are retained in
the raw evidence. Unrelated dirty work remains outside this slice's commit.

Remaining: CLI entry pre-read and pattern-policy admission, concurrent policy
snapshots, maximum custom-policy/analyzer performance, native self-bootstrap and
executable refresh, compiler/paradigm compatibility, runtime ownership/hard heap/
preemption, typed FFI/raw bypass/sandbox, publisher trust/artifact handoff/
revocation/rollback, broad product regression and physical-platform verification.
PBLang 1.0.0 remains NOT READY.
