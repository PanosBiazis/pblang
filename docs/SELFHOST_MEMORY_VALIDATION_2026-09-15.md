# PBLang-owned memory engine — validation, 2026-09-15

## Implemented slice

The PBLang bytecode VM now implements scalar allocation, access, mutation,
borrowing, release and lifetime reclamation in PBLang source. The new module
itself compiles and executes on that VM. Imported module initializers and the
main program share private ownership state. The retained Rust executable and
its ownership tracker are unchanged; no Rust source was added.

`memory.scope` and `memory_safety.scope` remove supported scalar allocation
entries on balanced normal/caught-failure exits. `MemoryTracker` allocation
methods now use the supported `array_push` builtin. An explicit nullable-number
narrowing also allows `memory_safety.sapp` to pass the self-hosted type checker.
[The runtime contract](SELFHOST_MEMORY_RUNTIME.md) documents execution routes,
compatibility and unsupported values.

## Reproducible measurements

Baseline revision: `ab59351ab6a8814ba878dbe680ed16b78e07dbdd`. Both versions ran
on the same local Linux x86-64 host with retained executable SHA-256
`7f1b97c0037736553ab39d165c13c36f3fd8497bc6aca9ca94a4c680858e6500`.
Only VM/registry imports were redirected to unchanged baseline copies for the
before call benchmark. Fixture, compiler pipeline, expected outputs, iteration
counts and executable were identical. No compiler or benchmark acceleration is
assumed from these data.

`bench/selfhost_vm_call_stages.sapp` checks fib(14), 2,000 zero-argument calls
and 2,000 two-argument calls. One whole-process warmup per version, then five
measured processes per version, alternating order. Every process also performs
its fixture's internal warmups. GNU time records process user+system CPU and
peak RSS; PBLang records batch elapsed time. p95 is nearest rank (the maximum
for five samples). The desktop was not an isolated benchmark host, and samples
show substantial variation. No universal speedup or zero-overhead claim follows.

| Metric | Before median / p95 | After median / p95 |
| --- | ---: | ---: |
| Process CPU, seconds | 3.25 / 3.59 | 3.22 / 3.29 |
| Peak RSS, KiB | 39864 / 40136 | 39896 / 40140 |
| 40 fib batches, ms | 1654.42 / 1910.26 | 1609.84 / 1658.20 |
| 8 zero-argument batches, ms | 647.89 / 667.59 | 640.43 / 660.72 |
| 8 two-argument batches, ms | 697.67 / 738.97 | 712.02 / 761.06 |

Median process CPU is about 0.9% lower and RSS about 0.08% higher; the two-argument
batch median is about 2.1% slower. These mixed observations support similar
cost on these workloads, not a demonstrated general improvement. No GPU/NPU,
power, temperature, aggregate temporary-disk or executable-size improvement was
measured in this slice.

`bench/selfhost_memory_scopes.sapp` performs 2,000 scalar allocate/borrow/read/
scope-close iterations. Every run verifies checksum 1999000, zero remaining
owners/borrows, 2,000 allocations and reclamations, and peak one live owner.
One process warmup plus five measured processes, each with internal warmup:

| Metric | Median / p95 |
| --- | ---: |
| Measured loop body, ms | 367.14 / 371.34 |
| Whole process CPU, seconds | 1.40 / 1.62 |
| Peak RSS, KiB | 35644 / 35812 |

This is a correctness/cost observation of the new execution route. It is not a
speed comparison against native host memory calls. Source example commands:

```sh
target/release/pblang run bench/selfhost_vm_call_stages.sapp --allow-all
target/release/pblang run bench/selfhost_memory_scopes.sapp --allow-all
```

[Machine-readable measurements and suite results](evidence/selfhost-memory-2026-09-15.json)
retain samples, warmup flags and success markers without personal paths.

## Validation coverage

35 suites passed: 18 memory/VM/module/operator suites plus 17 emulator/device/
input-model suites. The new memory suite covers scalar types, immutable and
moved owners, shared/exclusive borrows, both release forms, double-free,
nested scopes, malformed handles/kinds/arities, rejected composite values,
first-class aliases and user shadowing, both real memory libraries, module
initializer exports, bounded store/allocation, fresh state after an uncaught
failure, and explicit host context close. Host ownership counters remain
unchanged during the self-host memory cases.

The engine is also loaded and executed as guest PBLang source, including legacy
release selection and scope close. Existing arithmetic/operator, small-call,
try/arity, module differential and budget suites still pass. Independent security
review identified immutable-move, borrow-kind and allocate-arity issues during
iteration; corrections and regression tests were included before publication.
The final review found no blocker within the documented scalar-only scope.

Ten touched PBLang files passed self-hosted type checks and retained lint.
Lint returned success with nonblocking style/unused-variable warnings. Ten
strict entry-only scans completed their mandatory checks with zero blocking
findings and stable source snapshots; these are not dependency-wide semantic
security proofs. Retained build and repository-relative lock refresh passed.

Graphify's AST-only update completed (521 supported files, 8,807 nodes and
24,393 edges) in an isolated snapshot. The installed extractor does not parse
`.sapp`, so it does not supply a complete graph of this new PBLang engine. The
existing workspace graph was preserved. No analyzer or test service is left
running by this slice.

These are local Linux checks. Emulator results validate their declared model
subsets, not execution of the new ownership engine on every modeled device.
No guest operating system or remote workstation was used in this slice. The
approved workstation was off. No EnterpriseCore/Hetzner server was accessed.

## 1.0.0 work still open

This slice is not a 1.0.0 readiness certificate. The package remains alpha.
Prior [project/platform evidence](DEEP_LINK_RESOURCE_VALIDATION_2026-09-15.md)
remains dated evidence, not a fresh revalidation of every subsystem.

- Runtime: identity-aware composite ownership and aliases, escaped values,
  deterministic uncaught-error cleanup, balanced lifetime guarantees, complete
  native/bootstrap replacement, and backend-specific memory/FFI contracts.
- Resource accounting: cumulative import-graph work/heap budgets for explicitly
  bounded profiles, workload-based CPU/RAM/disk measurements, and program-size
  scaling without arbitrary general-purpose restrictions.
- Compiler/platforms: parity across C99, interpreter, WASM and native backends;
  corrected local VM-lab/controller contracts and guest OS tests, including old
  and constrained devices. Hardware acceleration still needs measured fallbacks.
- Libraries/PBIDE: wider library ownership coverage, file/Run/Designer workflows
  and consistent behavior across supported interfaces.
- Audits/releases: complete authenticated evidence and publisher/artifact chains,
  rollback policy and current end-to-end release gates.
- Idea intake: earlier bounded inventories and selected content scans remain
  partial. No claim that all conversations or all mounted disk contents were
  read. This slice converts the scoped-resource idea into tested PBLang code;
  broader streaming-output, document-model and device-library ideas remain open.
