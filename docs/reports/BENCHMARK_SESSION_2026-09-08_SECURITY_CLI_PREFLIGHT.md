# Security CLI policy admission — 2026-09-08

## Change and evidence

Base: a0528fe. Four PBLang production files changed; no Rust source or executable
rebuild. Pattern configuration and resolved dependency policies now have
outcome-only preflight. pbsecure and security-selected pbanalyze invoke it before
entry-source reads. Dependency analysis preflights pattern configuration too.
Malformed disabled_rule_ids is not silently repaired; dependency nil-as-unset
and command-line additions remain supported. Entry-only validates patterns only.
Non-security pbanalyze selections retain their previous behavior.

Regression first failed on the original implementation: malformed pattern policy
reached entry_source type checking before rejection. It now fails configuration
admission first. Both actual CLIs originally returned a missing-source error for
invalid taint configuration. Linux strace file-syscall observation saw exactly one
access mentioning the absent entry marker per CLI. After the change, each CLI
returned status 2 and the policy error for both invalid taint and pattern cases,
with zero entry-marker file syscalls in all four traces. Config loading and
reverification are expected, not counted as entry-source access. Temporary traces
were deleted; no personal paths/source are retained. This is a bounded
missing-entry experiment, not a proof for every OS or all concurrent races.

pbsecure early rejections retain config-snapshot evidence, never claim source
snapshot verification, and preserve exclusive report publication/collision guards.
Successful validation does not approve source, freeze mutable policy, authorize
execution or bypass subsequent per-module/whole-program validation.

## Paired dependency measurements

Unchanged bench/taint_policy_preflight_pair.sapp, operator-reviewed baseline
dependency module from a0528fe, same current supporting modules. No workload
reduction; every valid output compared in full. Two warmup pairs plus 12 retained
AB/BA pairs per case; 50 analyses/sample for default-small/custom-flow, three for
custom-boundary (128 custom taint sinks). Timed spans include complete dependency
calls with in-memory source, not CLI startup, config I/O or complete process time.
Initial series overlapped brief lint/metadata checks; confirmation followed
completed build/lock validation without those task checks running.

| Series | Workload | Before median ms | After median ms | Median paired ratio |
| --- | --- | ---: | ---: | ---: |
| initial | default-small | 43.295 | 43.657 | 1.0080 |
| initial | custom-flow | 95.431 | 96.956 | 1.0121 |
| initial | custom-boundary | 49.081 | 48.222 | 1.0030 |
| confirmation | default-small | 42.899 | 43.429 | 1.0252 |
| confirmation | custom-flow | 97.180 | 99.149 | 1.0382 |
| confirmation | custom-boundary | 46.510 | 47.154 | 1.0045 |

The extra policy check costs time: paired median ratios about 1.008/1.025 for
small, 1.012/1.038 for flow, and 1.003/1.004 for maximum taint policy. This is
a correctness change, not a speedup. Initial flow timing was noisy. Maximum
custom-pattern policies, CLI end-to-end performance and allocation deltas were
not benchmarked. now_ms is not monotonic; isolated component spans cannot be
added into a fabricated exclusive end-to-end trace.
Raw samples/p95/variation: bench/results/security-cli-preflight-2026-09-08.jsonl.

## Separate numeric comparison

Existing session_compare integer_kernel, one million iterations, checksum 474518
and status=ok for all nine paths. First run, warmup, five retained executions;
compiled paths first build and three repeated builds. Linux x86_64 only.

| Path | Median ms | p95 ms | Median peak RSS KiB | Artifact bytes |
| --- | ---: | ---: | ---: | ---: |
| pblang-interpreter | 276.949 | 281.595 | 8624 | N/A |
| pblang-c99 | 7.489 | 7.847 | 1572 | 14472 |
| pblang-codegen-x64 | 3.482 | 3.708 | 264 | 507 |
| c | 7.521 | 7.829 | 1568 | 14472 |
| cpp | 7.938 | 8.525 | 3380 | 14472 |
| go | 9.428 | 9.595 | 1500 | 1228952 |
| rust | 7.969 | 8.204 | 2036 | 350560 |
| python | 200.526 | 209.613 | 10316 | N/A |
| javascript | 132.937 | 135.919 | 51584 | N/A |

PBLang's retained interpreter remains slower than Python and JavaScript here.
This workload does not execute the changed analyzers; differences from previous
sessions cannot establish a causal regression or speedup from this patch.
RSS is peak process residence, not total allocation; artifact size excludes
runtime/dependencies. N/A is not zero. Short-run CPU zeros reflect measurement
resolution, not zero CPU work. No other platform/device, full C99 dynamic
analysis or embedded-polyglot combination is qualified.
Raw: bench/results/session-2026-09-08-security-cli-preflight.jsonl.

## Validation and remaining work

Eighteen targeted test runs passed: new pattern/dependency policy preflight,
new CLI admission separately for pbsecure/pbanalyze, existing security-tools CLI,
taint preflight/configuration/class reuse/limits/admission/completeness,
dependency/source/token bounds, dependency analysis, general security, dynamic
eval, manifest validation and current portable lock. Coverage includes exact
outcomes, malformed input, privacy, nested precedence, nil fallback, path/mode
precedence, CLI rule additions, selected scope and exclusive publication.

The initial expected stale dependency hash was refreshed using normal build and
PBLang-native pblock, not bypassed. Final build/portable lock passed without that
note. Touched-code lint succeeded with unused-binding warnings, including used
bindings the retained linter does not recognize. No full suite or new executable.
Independent security review found no remaining material issue in this slice.

Owned-file privacy pattern scans and git diff --check passed. Graphify reached
its 20-second deadline (124), with no completion or graph-freshness claim.
No owned test/benchmark/build/Graphify process or temporary fixture remains.
Pre-existing dirty work and the refreshed local lock stay outside this commit.

Remaining: concurrent policy snapshots; pbanalyze pre-allocation read bounds and
snapshot verification; general analyzer performance; C99 embedded-NUL parity;
native self-bootstrap/executable refresh; compiler/paradigm compatibility;
VM ownership, hard heap/preemption; typed FFI/raw bypass/hostile-code sandbox;
publisher trust, artifact handoff, revocation/rollback; broad product regression
and physical-platform qualification. PBLang 1.0.0 remains NOT READY.
