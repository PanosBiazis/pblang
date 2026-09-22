# Self-hosted VM dispatch groups

Local validation on 2026-09-22 against
`78f34ab7001589041eb1800281a3e721a72f2e6f`.

For the subsequent allocation changes and their separate measurements, see
[small arrays and closure allocation](SELFHOST_SMALL_ARRAYS.md).

## Change and compatibility

`stdlib/compiler/vm.sapp` now groups instruction handlers by opcode range after
the frequent constant/local accesses. Indexed reads/writes and captured-variable
accesses are near the start of their groups; calls and returns lead the control
group. This reduces repeated comparisons in the PBLang interpreter. It adds no
Rust, polyglot block, opcode, dispatch cache, function-body specialization or
bytecode format change.

All 43 handler bodies are token-identical to the baseline, ignoring comments and
whitespace. The current instruction is still read on every iteration. This is
not instruction fusion: each executed instruction receives the same tick and
resource checks, including sticky failures, checks before writes, arity checks,
frame admission and value checks. No guest state or mutable instruction metadata
is cached between iterations. Shared closure cells and their factory frames are
retained, including the independence of declarations in loop iterations.

The range guards preserve ordinary numbers and exact numeric types accepted by
the raw VM's existing equality comparisons. Unknown raw opcodes in the tested
legacy cases remain ignored. This does not broaden the serialized reader's
admission rules or turn the raw `VM.exec` API into a hostile-code sandbox.

## Validation

- **126 suites passed**, including the tracked self-hosted suites and the prior
  runtime/operator/lifetime/resource/emulator selection. Tests ran against a
  clean staged checkout; temporary test paths were redirected inside a disposable
  directory. Unrelated working-tree changes were not test dependencies.
- **600 before/after executions matched**: 30 closure/assignment fixtures,
  ordinary and optimized chunks, and ten budget configurations. Comparison
  includes output, failure state, refusal reason, charged steps, resource peaks
  and remaining local-slot count. Stack/step/frame/local/value failures match.
- **60 PBCS artifacts are byte-identical** between revisions. No code generator
  or native translator was changed.
- The new dispatch regression also checks exact numeric opcode representations,
  early refusal before a cell write, raw scalar upvalue writes, and unknown raw
  opcodes. It passes on both baseline and candidate.
- Clean build and repository-relative lock normalization passed. Lint passed for
  the three touched PBLang files, with eight warnings in the existing VM module.

Reproduce the semantic checks with:

```sh
target/release/pblang run tests/semantics/selfhost_vm_dispatch_test.sapp --allow-all
target/release/pblang run tests/semantics/selfhost_shared_capture_test.sapp --allow-all
target/release/pblang run tests/semantics/selfhost_vm_tick_accounting_test.sapp --allow-all
```

Run `tests/tooling/selfhost_dispatch_snapshot.sapp` under both revisions and
compare its JSON structurally. The snapshot deliberately excludes cyclic globals
and closure objects; printing those remains unsafe in the retained formatter.

## Measurement protocol

The existing benchmarks are unchanged. Use the same retained executable in clean
before/after checkouts, two warmups and seven measured runs per workload and side,
rotating side order. Time the entire process with `/usr/bin/time`; CPU is user
plus system time and peak RSS includes compilation. No tests, builds or Graphify
run alongside timing. At seven samples, nearest-rank p95 is the maximum observed
sample, not a worst-case bound.

The closure workloads also compare against `27f2ef9`, the revision before shared
bindings. Those workloads have identical expected outputs on all three versions;
the earlier incorrect sibling-sharing behavior is not used as equivalent work.

## Measured cost

| Workload | Median CPU before → after (s) | p95 CPU before → after (s) | Median RSS before → after (KiB) |
| --- | --- | --- | --- |
| counter | 1.82 → 1.51 | 2.06 → 1.52 | 36,740 → 36,484 |
| creation | 1.14 → 1.04 | 1.15 → 1.07 | 36,668 → 36,448 |
| numeric | 1.15 → 1.15 | 1.19 → 1.22 | 36,692 → 36,596 |
| overload | 2.12 → 1.95 | 2.14 → 2.00 | 36,672 → 36,632 |
| try_success | 0.90 → 0.89 | 1.11 → 1.01 | 36,592 → 36,524 |
| try_failed_bounded | 1.04 → 1.02 | 1.06 → 1.03 | 36,756 → 36,468 |

Counter CPU decreased **17.0%** and closure creation **8.8%** versus the shared-binding baseline. Operator-overload CPU decreased **8.0%** in its workload. The numeric median is unchanged; its p95 increased from 1.19 to 1.22 s. The small try/catch differences do not establish a general speedup.

The contemporaneous pre-shared-binding reference (`27f2ef9`) measured counter CPU 1.60 s and creation CPU 1.02 s, versus 1.51 s / 1.04 s now. Thus the earlier counter overhead is covered in this sample, while creation remains approximately **2.0%** slower than that older reference. This is not a claim that every closure workload is faster.

Median peak RSS changed by less than 0.8% in all six workloads, with overlapping samples. No general RAM saving is established, and the previous representation cost is not proven eliminated. Process measurements include compilation and do not isolate VM heap usage.

[Raw timing samples](evidence/selfhost-dispatch-groups-cost-2026-09-22.json), [suite results](evidence/selfhost-dispatch-groups-validation-2026-09-22.json), and [differential parity](evidence/selfhost-dispatch-groups-parity-2026-09-22.json).

## Scope and remaining work

This improves the tested self-hosted interpreter route. It does not reclaim
reference cycles, shorten kernel collector pauses, replace the retained native
runtime, complete FFI/polyglot integration, or establish compatibility with all
physical platforms. There is no real-time latency guarantee.

The configured PB-MCP desktop bundle was inspected read-only again: it contains
`resource_scope.sapp` but not `compiler/vm.sapp` or `compiler/codegen.sapp`.
Installing this module there without a consuming execution route would not make
the desktop faster. The bundle and approvals were left intact; there is no
measured live-view speedup from this change. No EnterpriseOrg server was used.

Graphify update exceeded its 35-second bound and was terminated. Its query did
not provide semantic coverage of this PBLang implementation.

See also [shared binding semantics](SELFHOST_SHARED_CAPTURES.md) and
[stable release gates](STABLE_1_0_READINESS.md).
