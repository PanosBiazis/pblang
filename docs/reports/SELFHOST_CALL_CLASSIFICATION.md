# Self-hosted call classification and return cost

Baseline: `f1538f81466d22bf50218a9b287726cfc792f56e`.
Local validation date: 2026-09-22.

## Implementation

The VM classifies a small-arity callee map once. A builtin tag has precedence over
a closure tag, including descriptors that carry both. Zero-argument builtins and
imports with non-unary argument counts still enter the general dispatch route.
Larger arities and legacy raw function constants keep the existing fallback.
The callee is read again on every call: descriptors are mutable and classification
must not survive changes to their tags. No callable cache or new persistent state
is added. Count/stack admission still occurs before any pop or body side effect.
Arity checks, lexical globals, import-cache propagation and fresh argument arrays
are unchanged. Reordering map membership checks invokes no guest callback.

An ordinary unbounded return now skips the call to `_guard_value`, whose first
operation was to return immediately when the optional runtime is nil. Bounded
returns execute the same string/array/map size predicate directly after popping
the value, without the intermediate helper frame; order and failure reason remain
intact. Per-instruction admission is unchanged.
No protection, quota, source-size policy, bytecode format or shared cell is removed.
All implementation changes are PBLang; no Rust or product polyglot code is added.

The call admission regression now covers arities zero through four, exact-number
objects rejected as raw argument counts, unknown builtin tags and a descriptor
changed from closure to builtin and back. It also preserves the previous negative
arity/underflow cases, dual-tag precedence, lexical globals and inherited caches.
Both baseline and candidate must pass the extended fixture.

## Reproduction and measurement scope

```sh
target/release/pblang run tests/semantics/selfhost_vm_call_admission_test.sapp --allow-all
target/release/pblang run tests/tooling/selfhost_dispatch_snapshot.sapp --allow-all
target/release/pblang run tests/tooling/selfhost_array_snapshot.sapp --allow-all
target/release/pblang run bench/selfhost_shared_captures.sapp --allow-all -- creation
target/release/pblang run bench/selfhost_shared_captures.sapp --allow-all -- counter
target/release/pblang run bench/selfhost_retained_captures.sapp --allow-all -- single
target/release/pblang run bench/selfhost_retained_captures.sapp --allow-all -- four
```

Use clean isolated packages, identical retained executables, unchanged benchmark
sources and repository-relative locks published with `tools/pblock.sapp` after
build. Timing includes frontend work. Two warmups precede seven measured samples
per side, alternating side order. CPU is user plus system time; RSS is process
peak KiB. Nearest-rank p95 is the maximum of seven samples, not a worst-case bound.
No tests/build/Graphify run alongside timed processes; other system load is not
isolated. Earlier absolute timings are historical rather than controls.

## Final validation and results

**127 suites passed** on the final candidate in a fresh isolated package, including
self-hosted, lifetime, resource, operator and existing emulator coverage. Build
and portable lock normalization passed. Lint passed with eight VM warnings and
five existing test warnings. This is focused coverage, not a universal platform
or security certification.

**940 differential executions match**, including output, failure state, runtime
counters and refusal reason. All **60 PBCS artifacts remain identical**. The
extended call-admission fixture passes on both revisions. The retained executable,
benchmark sources and all other stdlib files were verified identical by hash.

| Workload | Median CPU before → after (s) | p95 CPU before → after (s) | Median peak RSS before → after (KiB) |
| --- | --- | --- | --- |
| creation | 1.08 → 1.06 | 1.13 → 1.27 | 37,312 → 36,364 |
| counter | 1.48 → 1.47 | 1.50 → 1.53 | 37,600 → 36,440 |
| numeric | 1.14 → 1.14 | 1.17 → 1.17 | 37,492 → 36,572 |
| retained_single | 1.55 → 1.53 | 1.59 → 1.57 | 38,608 → 37,788 |
| retained_four | 2.05 → 2.04 | 2.35 → 2.06 | 42,988 → 42,236 |
| try_failed_bounded | 0.99 → 1.00 | 1.03 → 1.01 | 37,496 → 36,536 |
| try_failed_plain | 0.90 → 0.90 | 0.91 → 0.91 | 37,500 → 36,528 |
| ten_failed_bounded | 2.51 → 2.49 | 2.80 → 2.97 | 37,328 → 36,528 |
| ten_failed_plain | 1.57 → 1.56 | 1.58 → 1.58 | 37,524 → 36,552 |
| numeric_bounded | 2.30 → 2.33 | 2.34 → 2.39 | 37,500 → 36,608 |

The original closure creation workload improves **1.9%** against the current
baseline. Retaining 10,000 closures improves median CPU **1.3% with one capture**
and **0.5% with four captures**. Counter CPU improves **0.7%**; ordinary numeric
and short unbounded try/catch are unchanged. These are small measured differences,
not evidence that the older representation overhead has been universally removed.

The short bounded try/catch increases **1.0%** and bounded numeric increases
**1.3%**. Ten-execution bounded try/catch improves **0.8%**. Observed p95 is worse
for creation (**1.13 → 1.27 s**), counter, repeated bounded try and bounded numeric.
Thus the change does not establish improved worst-case latency or eliminate all
previously reported overhead. No samples were discarded to remove these results.

Peak process RSS is **1.7–3.1% lower** across the ten measured workloads. This
includes compilation and the retained runtime, not just live closure cells.
The ownership representation is unchanged; this is not proof of cycle reclamation
or general application RAM savings. The precise allocator/compiler contribution
was not isolated. No disk-I/O, energy, GPU/NPU/peripheral or live-view gain is claimed.

An earlier candidate classified once and skipped the no-op unbounded return helper,
but still called that helper for bounded returns. Its ten-execution bounded test
showed **+1.6% median CPU**, motivating the final inline predicate. That complete
exploratory series and its patch are retained separately; it is not mixed into the
final table. Final code was revalidated and remeasured, rather than reusing old
numbers after changing the source.

[Final samples and commands](evidence/selfhost-call-cost-2026-09-22.json),
[exploratory samples and patch](evidence/selfhost-call-cost-exploratory-2026-09-22.json),
[final validation](evidence/selfhost-call-cost-validation-2026-09-22.json), and
[differential evidence](evidence/selfhost-call-cost-parity-2026-09-22.json).

Graphify exceeded its 35-second bound on both implementation revisions; no semantic
PBLang coverage or completed refresh is claimed. The 212 unrelated dirty files
were preserved by content hash. Owned-file privacy, links and whitespace checks
passed. The 1.0 readiness document now acknowledges the tested shared-binding
support without declaring cycles, native runtime migration or backend parity done.

## Memory and integration work still required

Shared binding cells and closure factories remain necessary for correct sibling
mutation and fresh loop bindings across the retained/native and self-hosted paths.
Removing them would reintroduce result-changing bugs. This patch does not reclaim
reference cycles, change the allocator or shorten collection pauses.

`stdlib/runtime_gc.sapp` still refuses destructive retained-host collection:
nested VMs share a thread-local heap and cannot prove that suspended caller roots
have all been marked. Clearing globals or captured cells on return could destroy
escaped closures; resetting all host objects is not a safe workaround. A general
collector needs explicit object identities, ownership and complete root tracking
at VM/foreign boundaries before automatic destructive reclamation is enabled.
The PBLang scalar lifetime state in `compiler/vm_memory.sapp` is separate and
still rejects composite/reference-valued allocations. The current catch expression
also does not expose the original host error, so blindly catching and rethrowing a
replacement would not complete exception-preserving cleanup.

The installed desktop bundle was inspected read-only for this VM/code generator.
No installation, consent state or desktop transport is changed. General
runtime/FFI/polyglot integration, physical/guest-platform validation, cycles and
long pauses remain open; this does not establish better live view. No EnterpriseCore
server or the external `sw` reference tree is used or modified.
