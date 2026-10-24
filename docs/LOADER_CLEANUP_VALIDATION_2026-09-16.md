# Loader failure cleanup validation — 2026-09-16

The shared `_run_chunk` boundary in `stdlib/selfhost_loader.sapp` now creates
and owns a private execution context, passes it explicitly to the bounded VM,
and closes it immediately after the existing runtime-error catch. This clears
scalar owner/value, borrow and lifetime indexes before returning either success
or failure. It fixes cleanup for file, bytes, seeded and options-based loader
entry points, and consequently the pbrun and PBSELF routes that call them.

Previously only successful internally-owned VM runs closed the context explicitly;
an uncaught runtime error propagated past that close and the loader caught it
later. Reclamation then depended on host GC. The change uses the same existing
close operation and adds no callbacks, retry, background worker or new public API.
Result schemas, bounded defaults/options and fixed runtime-error text are unchanged.

## Evidence and tests

`selfhost_loader_cleanup_test.sapp` first retains a host-only alias to an explicit
ownership state. Its positive control sees two owners and two active borrows after
an uncaught guest throw. Closing the context clears the aliased maps, reduces all
owner/borrow/scope counts to zero, records two reclaimed owners, rejects later
state use and remains idempotent. The test then exercises real loader byte/file,
seeded/options, invalid-budget, step-limit and CLI-result failure routes, fresh
execution, guest-caught errors, nil output and normal results. Runtime failures
retain empty output and do not include partial globals, locals or the context.

A separate scratch instrumentation experiment swapped only the loader's VM import
for a delegating observer, retaining a state alias around the real VM call. The
baseline loader reported `[owners=2, borrows=2, closed=false]`; the candidate
reported `[0,0,true]`, with one explicit close and a nil context memory pointer.
This is an instrumented sequencing experiment, not public product telemetry or
an OS-RAM measurement. The observer/probe copies are excluded from publication.

The loader does not undo mutations already made to a caller-supplied globals map.
A regression verifies exactly one marker increment on failure, with neither retry
nor rollback. This matters for automation: retry policy is a separate decision
and must account for already-performed effects.

56 selected suites passed with zero exit status and their final success markers,
including 17 emulator/input suites, cache/database regressions, time/VM suites,
scalar-memory tests and nine added memory/loader/serialization suites. The loader
runtime-error suite also exercised actual local pbrun subprocess exit codes.
Three owned PBLang files passed check, lint and strict entry-only pbsecure (nine
gates, zero reported findings). Existing lint warnings remain. Independent
security review found no material blocker; this is not a complete security audit.
Build and PBLang-native portable-lock publication passed. Graphify update
completed; its AST analyzer still does not cover .sapp. Generated graph files
and scratch probes are excluded from the commit.

## Reproduce the resource comparison

Baseline: `e1a3afaa26e9a8dc0501b454a526641ee3e32563`.
Retained executable SHA-256:
`7f1b97c0037736553ab39d165c13c36f3fd8497bc6aca9ca94a4c680858e6500`.

`bench/selfhost_loader_cleanup.sapp` arguments are repetitions, success/failure,
and scalar owner count. For example:

```sh
pblang run bench/selfhost_loader_cleanup.sapp --allow-all -- 200 failure 16
```

The benchmark compiles and serializes once, then repeatedly deserializes/executes
identical bytes, checking every result. Compilation and serialization are outside
the timed interval; decoding, execution and cleanup are inside. Before/after use
the same benchmark with equivalent relative imports of baseline/current loader
modules and the same current VM. One warmup per variant, seven measured process
samples per case, alternating order; p95 is nearest rank (maximum of seven).
GNU time records whole-process CPU and peak RSS, including startup. Runs were
serial before the broad tests/build/Graphify/CLI inventory; unrelated host activity
was not controlled. Python/GNU time only orchestrated PBLang executions.
[Raw samples and module hashes](LOADER_CLEANUP_BENCHMARK_2026-09-16.json).

| Outcome | Owners × repetitions | Before median / p95 ms | After median / p95 ms | Median time change | Before / after median RSS KiB |
| --- | --- | --- | --- | --- | --- |
| success | 0 × 500 | 296.893 / 319.733 | 235.886 / 303.269 | -20.55% | 35616 / 35488 |
| success | 16 × 200 | 433.319 / 573.022 | 438.611 / 736.135 | +1.22% | 35624 / 35564 |
| success | 1024 × 5 | 506.557 / 551.583 | 512.673 / 532.084 | +1.21% | 35620 / 35620 |
| failure | 0 × 500 | 297.337 / 304.606 | 301.497 / 303.732 | +1.40% | 35652 / 35588 |
| failure | 16 × 200 | 447.379 / 488.827 | 458.473 / 566.896 | +2.48% | 35476 / 35568 |
| failure | 1024 × 5 | 525.764 / 567.181 | 518.139 / 558.610 | -1.45% | 35540 / 35528 |

The allocating success cases cost about 1.2% more time. Failure cases range from
about 1.5% faster to 2.5% slower. The zero-owner success case measured about 20.5%
faster, but does not establish a general speedup; sample variability and host
activity remain confounders. Median peak RSS was nearly unchanged (within 0.4%).
No general RAM saving or zero-overhead claim is justified. Logical reclamation
before return does not guarantee immediate reduction in process RSS. No executable
size, disk-write, GPU/NPU or whole-compiler improvement was measured.

## Remaining boundaries

Only supported scalar owners are covered. Composite/shared object ownership,
native allocations, host GC, FFI and backend-wide isolation remain unresolved.
Direct VM/frontend helpers and Android-emulator execution still need a separate
uncaught-failure cleanup strategy; their original exceptions were not replaced
with generic errors by this loader-only change. A host process kill, fatal OOM or
failure inside the runtime itself is not covered by a language catch boundary.

The bounded loader's existing step/value limits remain in place; this is not a
new aggregate-memory limit or resource-budget policy. It is not a compiler/project
size restriction. Canonical [memory runtime documentation](SELFHOST_MEMORY_RUNTIME.md)
describes the different execution routes. PBLang 1.0.0 is not complete; audits/
releases, PBIDE, other backends and actual guest-OS tests remain open. No EnterpriseCore
or Hetzner server was used.
