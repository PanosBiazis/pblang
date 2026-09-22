# Self-hosted method admission and direct result dispatch

Validation date: 2026-09-22. Baseline:
`34ba46d91795311a409beda3aaf815e81ad6d061`. Product changes are PBLang, in
`stdlib/compiler/vm.sapp`. No bytecode version, source limit, execution permission
or native runtime implementation changes.

## Correctness defect

A closure called by an overloaded operator used to skip the minimum/maximum
argument checks applied to ordinary closure calls. For example:

```sapp
let calls = 0;
let obj = map{__add__: fn(a) { calls = calls + 1; return 9; }};
let result = try (obj + 3) catch 0;
print result;
print calls;
```

The operator supplies two arguments to a one-argument function. Baseline
self-hosted execution printed `[9, 1]`: the invalid body ran and mutated state.
Corrected execution gives `[0, 0]`. The retained native executable independently
prints `0` then `0` for the dynamic-call fixture in
`tests/fixtures/selfhost_method_arity_program.sapp`.

The method route now validates the function's optional `min_arity` and `arity`
before entering its chunk, evaluating its default prologue, or propagating an
import cache into lexical globals. The generic raw-function-constant OP_CALL
route now enforces the same metadata. Valid defaults, lexical namespaces,
inheritance lookup, forward/reflected precedence and builtin-descriptor priority
remain. Metadata absent on legacy values is not invented.

Compatibility change: raw function constants and operator methods with an
invalid argument count now throw instead of running their bodies. A present
forward method that rejects or throws does not fall back to a reflected method.
This is consistent with ordinary call admission; applications relying on the
previous invalid-call behavior must fix their signatures. This does not validate
all possible forged function maps or normalize builtin arity behavior.

## Avoided temporary allocations

Previously `_try_overload` returned a fresh `{found, value}` map for each map
operator dispatch. Each operator wrapper now performs lookup and returns the
method's result directly. There is no success/sentinel ambiguity: nil, false,
zero and containers can be real results. Scalar VM operations retain their
existing fast path and do not enter these helpers.

This removes one result-envelope map allocation per overloaded method dispatch
(25,000 in the valid overload benchmark). That count follows directly from the
unchanged five executions of 5,000 method calls, not from a heap profiler.
It does not imply that 25,000 maps were simultaneously live, that all method
allocations disappeared, or that process peak RSS must fall.

The generic closure-call checks also reuse the already validated `arg` count
instead of repeatedly measuring the newly built argument array. That array is
private and exactly `arg` items long; no guest callback executes before checking.
No mutable cross-call cache or deferred validation was introduced. This is a
focused review, not a claim of general concurrent/TOCTOU or FFI safety.

## Validation

- **52 suites passed** in a clean archived checkout: runtime call admission,
  operators, closure/try behavior, local/exception lifetimes, explicit memory,
  resource scopes, serializer, loaders/bundles, optimizer and five emulator suites.
- The new suite runs **71 source scenarios** with plain and optimized chunks,
  unbounded/bounded VM execution, PBCS roundtrips and the loader. It covers all
  11 binary operators in forward/reflected directions, too many/few parameters,
  inherited methods, defaults, nil/false returns and no fallback after rejection.
- Seeded/raw tests check call counts 0–4, body side effects, lexical state,
  import-cache propagation only after admission, valid raw methods and absent
  legacy arity metadata. The suite fails against baseline on its first invalid
  operator-call case.
- The retained native fixture agrees on rejection before body effects.
- Build passed in the clean tree and the generated lock was normalized with
  `tools/pblock.sapp`. Lint exited 0 with 29 style/unused-variable warnings across
  five files; warnings were not suppressed. Whitespace and privacy gates passed.

Evidence: [suite results](evidence/selfhost-method-validation-2026-09-22.json),
[native fixture](evidence/selfhost-method-native-2026-09-22.json).
These are focused local checks, not tests inside every guest OS or on physical
GPU/NPU/peripheral devices.

## Reproducible measurements

Use the unchanged workload on both revisions:

```sh
target/release/pblang run bench/selfhost_method_admission.sapp --allow-all -- overload
target/release/pblang run bench/selfhost_method_admission.sapp --allow-all -- numeric
target/release/pblang run bench/selfhost_method_admission.sapp --allow-all -- rejected_allocation
```

Same retained executable, isolated clean trees, compilation once and five VM
runs per process. Two warmups and seven alternating samples per side/workload;
CPU is user+system, peak RSS includes compilation and the retained runtime.
No tests, builds or Graphify ran concurrently with timing. Nearest-rank p95 at
seven samples is the maximum, not a guaranteed worst-case bound.

| Workload | CPU median before → after, s | CPU p95 before → after, s | RSS median before → after, KiB |
| --- | --- | --- | --- |
| Valid overloaded addition | 2.12 → 2.12 | 2.19 → 2.24 | 36,568 → 36,592 |
| Valid numeric control | 1.49 → 1.49 | 1.51 → 1.50 | 36,500 → 36,492 |
| Invalid call attempting an 8 MiB string | 0.80 → 0.78 | 0.83 → 0.80 | 40,092 → 36,528 |

The valid workloads have unchanged median CPU and nearly unchanged RAM; the
overload p95 is slightly higher. No general speedup or RAM saving is established.
The negative fixture's **8.9% lower peak RSS** measures work avoided by rejecting
an inadmissible call before its body. Baseline attempted allocation and then
threw; the new VM rejects the signature first. Both fixtures catch failure and
print zero, but their error causes differ intentionally. This is not a speedup
for an equivalent valid program. [All samples](evidence/selfhost-method-cost-2026-09-22.json)
retain the variation.

## Remaining integration

The configured installed PB-MCP desktop bundle was checked read-only: it includes
`resource_scope.sapp`, but not `compiler/vm.sapp` or `selfhost_pbck.sapp`. It was
not updated or restarted; no live-view/streaming performance benefit is claimed.
The existing desktop enrollment and permissions are unchanged. No Specisoft
server was used. The 212 pre-existing dirty files were preserved by hash.

Update: source-level shared scalar captures and owner cleanup are now addressed
by [shared capture lowering](SELFHOST_SHARED_CAPTURES.md), including loop binding
identity and ordinary/optimized native-bytecode checks. See that report for
representation compatibility, measured costs and remaining runtime limits.

Large GC pauses, rooted-collector overhead, general runtime/FFI/polyglot
integration and broader platforms remain open. The earlier codec's extra metadata
and its RAM cost remain; this method change does not remove them. The project is
still alpha; [1.0 release gates](STABLE_1_0_READINESS.md) remain applicable.

Graphify update exceeded its 35-second limit and was terminated. No semantic
`.sapp` coverage or updated complete graph is claimed.
