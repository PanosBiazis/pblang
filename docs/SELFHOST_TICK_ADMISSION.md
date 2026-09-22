# Inline bounded-VM instruction admission

Baseline: `0d4f4b07461fbe0291a680f3b498e0afbb6c51cf`.
Local validation date: 2026-09-22.

## Implementation and boundaries

The self-hosted VM performs the existing per-instruction admission directly in
`exec`, instead of calling `_runtime_tick` for each guest instruction and then
`_guard_value` for its top stack value. The removed private helper had one caller
and was not exported. Other callers of `_guard_value` keep the shared helper.
All product implementation is PBLang; no Rust or foreign block is added.

The order is unchanged: sticky failure, step admission, step increment, stack
admission, local admission, peak accounting, then top-value admission. A failing
step budget is not incremented. A failing stack/local/value check retains its
previous accounting. Stack and local lengths are read again on every instruction;
no value or length is cached across guest execution, catches, calls or mutations.
The top-value check retains the same string/array/map predicate and size bound.
There are no guest callbacks, foreign calls, yielding or new shared state between
these checks. The existing mutation admission and final fall-through stack check
are unchanged. No check is disabled, quota changed or program-size cap added.

This is a call-overhead optimization, not memory reclamation, synchronization,
a new sandbox or a guarantee against concurrent host mutation. Raw `exec` inputs
and retained host/FFI boundaries keep their existing limitations. Unbounded
execution still bypasses the optional budget block.

## Tests and reproduction

The accounting regression now also checks a guest throw caught normally with a
shared six-step budget, exhaustion at handler entry, and exhaustion after return
to the parent. Both baseline and candidate must preserve the original refusal
reason and counter. Existing cases cover changing stack/local lengths, admission
before mutation, sticky failures and priority when two limits would be exceeded.

```sh
target/release/pblang run tests/semantics/selfhost_vm_tick_accounting_test.sapp --allow-all
target/release/pblang run tests/tooling/selfhost_dispatch_snapshot.sapp --allow-all
target/release/pblang run tests/tooling/selfhost_array_snapshot.sapp --allow-all
target/release/pblang run bench/selfhost_inline_blocks.sapp --allow-all -- bounded failed
target/release/pblang run bench/selfhost_tick_execution.sapp --allow-all -- bounded failed
target/release/pblang run bench/selfhost_tick_execution.sapp --allow-all -- bounded success
target/release/pblang run bench/selfhost_tick_execution.sapp --allow-all -- plain failed
```

Use clean isolated checkouts, the identical retained executable and benchmark
sources, and repository-relative locks published by `tools/pblock.sapp` after
build. The existing one-execution workload is unchanged. The added benchmark
compiles that same guest source once and executes it ten times, asserting every
result. It increases the execution share; whole-process timing still includes
frontend work and does not isolate runtime CPU perfectly. Its results must not
replace the original workload when discussing the earlier 2.8% observation.

Timing uses two warmups and seven measured samples, rotating side order.
CPU is user plus system time, RSS is process peak KiB. Nearest-rank p95 with seven
samples is the observed maximum, not a worst-case or real-time guarantee. Tests,
builds and Graphify do not run alongside timed processes; other system load is
not isolated. Historical absolute timings are not contemporaneous controls.

## Validation and measurements

**127 suites passed** in the isolated candidate, including existing emulator,
resource, lifetime, self-hosted and operator coverage. This is not a complete
platform certification. Lint passed for the VM, extended regression and new
benchmark with the same eight existing VM warnings and no new warnings. Build
and repository-relative lock normalization passed in the comparison stages.

**940 differential executions matched**, including output, failure state,
refusal reason and runtime accounting. All **60 PBCS artifacts remain identical**.
The extended catch accounting regression passes on baseline and candidate.

| Workload | Median CPU before → after (s) | p95 CPU before → after (s) | Median peak RSS before → after (KiB) |
| --- | --- | --- | --- |
| try_failed_bounded | 1.03 → 1.01 | 1.04 → 1.02 | 37,296 → 37,260 |
| try_success_bounded | 0.99 → 0.96 | 1.00 → 0.97 | 37,292 → 37,352 |
| try_failed_plain | 0.90 → 0.91 | 0.93 → 0.94 | 37,540 → 37,428 |
| creation | 1.07 → 1.07 | 1.09 → 1.07 | 37,368 → 37,440 |
| counter | 1.50 → 1.52 | 1.60 → 1.67 | 37,416 → 37,432 |
| numeric | 1.14 → 1.15 | 1.15 → 1.16 | 37,304 → 37,340 |
| ten_failed_bounded | 2.73 → 2.48 | 2.79 → 2.52 | 37,348 → 37,524 |
| ten_success_bounded | 2.25 → 2.05 | 2.52 → 2.08 | 37,580 → 37,352 |
| ten_failed_plain | 1.59 → 1.59 | 1.60 → 1.63 | 37,232 → 37,404 |
| numeric_bounded | 2.61 → 2.35 | 2.64 → 2.51 | 37,500 → 37,596 |

The original bounded failed try/catch improves **1.9%** in this session; successful
try bodies improve **3.0%**. To address the quoted earlier 2.8% regression, the
pre-small-array revision `0f7b0817a458b7592170889d91e158cf0807e225` was also measured
with the same binary and failed-try workload, rotating all three variants. Its
median is **1.04 s**, p95 **1.10 s**, RSS **37,296 KiB**; the candidate is **1.01 s**.
The earlier slowdown is not reproduced by this current comparison. This does not
prove that the historical slowdown was stable, or that all overhead is eliminated.

With ten executions per compilation, bounded failed/successful bodies improve
**9.2% / 8.9%**. The bounded numeric loop improves **10.0%**. Plain ten-execution
try/catch is unchanged. Shared-closure creation stays **1.07 → 1.07 s**; its older
representation cost is not removed by optimizing optional budget admission.

Plain single-run failed try, counter and numeric controls increase approximately
**1.1%, 1.3% and 0.9%** respectively. Small differences and p95 variation do not
establish a general speedup or a worst-case improvement. Peak RSS varies by about
−0.6% to +0.5% across these workloads; no general RAM saving is established.
No guest allocation policy, collector, generated program size, disk I/O, GPU,
NPU, peripheral or streaming optimization is claimed by these CPU measurements.

[Raw samples and commands](evidence/selfhost-tick-cost-2026-09-22.json),
[suite results](evidence/selfhost-tick-validation-2026-09-22.json), and
[differential evidence](evidence/selfhost-tick-parity-2026-09-22.json).

The initial experiment removed only the outer helper; short probes were
inconclusive. The retained change also inlines top-value admission. Final samples
above use that final code, not selected samples from either exploratory probe.
A reporting script initially treated the array snapshot as a map; correcting the
report reader exposed all 340 already-produced array records and changed no VM
behavior or fixtures. Graphify update exceeded 35 seconds and was terminated,
with no remaining Graphify process. It does not supply semantic `.sapp` evidence.
All 212 pre-existing changed files were checked by content hash and preserved.
Owned-file privacy checks, documentation links and whitespace checks passed.

## Remaining work

General cycle reclamation, long collection pauses, runtime/FFI/polyglot integration
and broader physical/guest-platform validation remain unresolved. The installed
PB-MCP desktop path does not use this VM, so this change does not establish a
live-view benefit and does not require copying unused modules into its bundle.
No desktop approval, running installation or EnterpriseOrg server is changed.
