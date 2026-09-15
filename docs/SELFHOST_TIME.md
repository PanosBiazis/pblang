# Self-hosted time builtins — 2026-09-15

The PBLang bytecode VM now dispatches `clock()`, `now_ms()` and `now_iso()`.
Previously the compiler knew these host names, but the VM registry/dispatcher
omitted them, causing calls to fail. This completes the specific clock gap
reported in [cache lookup validation](CACHE_LOOKUP_VALIDATION_2026-09-15.md).

| Function | Current retained-host contract |
| --- | --- |
| `clock()` | Nonnegative number, Unix-epoch seconds, fractional precision |
| `now_ms()` | Nonnegative number, Unix-epoch milliseconds, fractional precision |
| `now_iso()` | UTC string, `YYYY-MM-DDTHH:MM:SSZ` |

All three require exactly zero arguments. Wrong arities raise the same fixed
messages as the retained host, e.g. `now_ms() takes no arguments`. Global, local
and seeded-global shadows still take precedence; first-class aliases and
higher-order calls work. Optional direct-call metadata can be absent, and safe
optimization does not change behavior. No compiler/project size limit was added.

```pblang
let F=import("stdlib:compiler/selfhost_frontend.sapp");
let result=F.frontend_run("print now_ms();");
print result.output[0];
```

## Boundary and compatibility

The dispatch is PBLang source; the actual system clock comes from the retained
host. This does not migrate the native runtime into PBLang or implement clock
syscalls independently. `clock` is wall time, not CPU time or a monotonic stopwatch.
System clock adjustments may move all these values backward. Do not use them as
trusted timestamps or cryptographic entropy.

Read-only time functions follow the retained host's existing ungated policy.
They expose real wall time, nondeterminism and timing side channels to bounded
programs; the bounded frontend has no separate time-capability switch. Step,
output and value-size checks still apply. `sleep_ms` remains unregistered and
unsupported by this VM; this change adds no blocking wait, filesystem, network,
process or environment capability. This is compatibility, not a stronger sandbox.

The cache's positive TTL insertion, reads, touch, keys and expiry sweep now work
through this frontend on the tested local retained host. Cached nil remains a
hit, and an expired entry can trigger the existing `get_or_set` producer. Expiry
remains lazy/on access or sweep; no background timer or automatic thread is added.

## Validation

The focused time suite covers all three APIs through direct, alias and higher-order
calls; raw/optimized/metadata-stripped chunks; normal and bounded execution;
arity 1/2/3/4 rejection; user/local/seeded shadows; ISO structure; and step/value
budget enforcement. It also passes without `--allow-all`. Tests do not assert
monotonicity or exact cross-call timestamp equality.

The self-hosted cache TTL suite checks all four lookup APIs with synthetic elapsed
deadlines, hit/miss/expiry counts, positive touch, deadline removal, key snapshots,
sweeps and cached-nil producer behavior. It additionally executes the complete
`cache_lookup_test.sapp` regression body through the frontend. No sleeps, private
data or real external services are required.

47 selected suites passed, including 17 emulator/input suites, eight cache/data
store/pbAI suites, two new time/TTL suites, six VM dispatch/arity/budget suites and
14 ORM/database/predicate/resource suites. Five owned PBLang files passed check,
lint and strict entry-only pbsecure (15 gates, zero reported findings). Lint
retains pre-existing warnings. Build and PBLang-native portable-lock publication
passed. Independent security review found no material blocker; these checks are
not a complete runtime/FFI security audit. Graphify update completed with
9,140 nodes; its AST analyzer still does not cover .sapp. Generated graph
output is excluded from this publication.

## Resource regression measurement

Baseline commit: `89c9387fe7277423066d339b2ec1c42c2c621f7a`.
Retained executable SHA-256:
`7f1b97c0037736553ab39d165c13c36f3fd8497bc6aca9ca94a4c680858e6500`.

The existing `bench/selfhost_vm_builtin_dispatch.sapp` was run unchanged except
for equivalent relative imports of baseline/current VM modules. Both import the
same current shared registry and compiler; these control sources use no time
builtins, so this isolates the dispatcher change rather than whole-revision
compiler startup. No benchmark workload was reduced or tuned for the candidate.
The original benchmark compiles once per case, verifies a warm result, and times
four 10,000-iteration runs of len and bit_shr loops. Each process reports their
average milliseconds. One extra process warmup per variant, then seven process
samples in alternating order. GNU time records whole-process CPU and peak RSS.
Benchmarks ran serially before broad tests/build/Graphify; unrelated host activity
was not controlled. p95 is nearest rank (maximum of seven). Samples are in
[resource evidence](SELFHOST_TIME_BENCHMARK_2026-09-15.json).

| Metric | Before median / p95 | After median / p95 |
| --- | --- | --- |
| len loop ms | 351.235 / 472.980 | 350.737 / 565.800 |
| bit_shr loop ms | 388.145 / 492.075 | 340.838 / 460.862 |
| Process user CPU seconds | 4.010 / 5.360 | 4.010 / 6.020 |
| Process system CPU seconds | 0.300 / 0.440 | 0.290 / 0.450 |
| Peak RSS KiB | 35536.000 / 35880.000 | 35408.000 / 35664.000 |

User CPU medians were identical (4.01 seconds); peak RSS was nearly unchanged
(about -0.36%). The len median was nearly unchanged; bit_shr varied downward,
while p95 fluctuated significantly. These controls do not establish a speedup or
zero overhead. The new time operations had no working baseline in this VM, so
there is no before/after latency improvement claim for them. Executable size,
disk writes, GPU/NPU, other backends and physical devices were not benchmarked.

## Remaining work

Monotonic/test-injected clocks and per-guest time policy need a separate design.
Runtime ownership for complex/shared values, native FFI/isolation, other backends,
audit/release authenticity, PBIDE and actual guest-OS validation remain open.
This is not universal platform support or completion of PBLang 1.0.0. No Specisoft
or Hetzner server was contacted for development or testing.
