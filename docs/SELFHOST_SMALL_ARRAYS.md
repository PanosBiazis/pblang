# Small arrays and closure allocation in the self-hosted VM

Baseline: `0f7b0817a458b7592170889d91e158cf0807e225`.
Local validation date: 2026-09-22.

## Implementation

`stdlib/compiler/vm.sapp` constructs arrays of zero through three items directly
instead of filling a growable array with nil values and overwriting each slot.
The one-item case is also the cell used for a shared closure binding. Pops still
occur in reverse order, so the resulting array retains source evaluation order.
Each construction owns a fresh array; element references are not cloned.

A one-reference closure capture vector is likewise constructed directly. Its
descriptor still selects a local or transitive capture with the same index and
missing-local behavior. Empty and multiple-reference vectors retain the generic
path. Closure factories, shared cells, captured globals and owner cleanup are
unchanged; independent activations never reuse an argument or capture array.

Small-arity calls allocate only their final argument array. Previously the VM
created an empty array and immediately replaced it for calls with one to three
arguments. A zero-argument call still receives a fresh empty array.

All implementation changes are PBLang. No Rust, polyglot block, new opcode, code
generator change, file-format change, array pool or application size cap is added.

## Compatibility and review

Array-size admission remains before construction; per-instruction ticks, stack
accounting, frame/arity checks, sticky budget errors and mutation guards remain
in place. This is not a new memory collector or a change to the default budgets.

Direct array construction is restricted to ordinary numeric counts. Raw exact
numeric objects (`bigint`, `decimal`, `rational`) follow the previous host-index
path, including its errors. Differential tests caught the initially broader
acceptance and the final implementation preserves the baseline behavior. The
raw VM historically supplies nil values when an array instruction has too few
stack values; this optimization preserves that behavior. The raw execution API
is not a hostile-input sandbox, and serialized-reader admission is separate.

Focused review covered array identity, source/pop order, missing and transitive
captures, zero and multiple captures, constructor admission before allocation,
type-sensitive malformed counts, independent default-argument frames, and
successful/error budget accounting. No mutable guest data is cached between
instructions, no permissions change, and no new concurrency guarantee is made.

## Reproduction

```sh
target/release/pblang run tests/semantics/selfhost_small_arrays_test.sapp --allow-all
target/release/pblang run tests/semantics/selfhost_shared_capture_test.sapp --allow-all
target/release/pblang run bench/selfhost_shared_captures.sapp --allow-all -- creation
target/release/pblang run bench/selfhost_retained_captures.sapp --allow-all -- single
target/release/pblang run bench/selfhost_retained_captures.sapp --allow-all -- four
target/release/pblang run bench/selfhost_array_construction.sapp --allow-all -- 0
target/release/pblang run bench/selfhost_array_construction.sapp --allow-all -- 3
target/release/pblang run bench/selfhost_array_construction.sapp --allow-all -- 16
```

Run both `tests/tooling/selfhost_dispatch_snapshot.sapp` and
`tests/tooling/selfhost_array_snapshot.sapp` on the baseline and candidate and
compare JSON structurally. They omit cyclic closure/global objects from output.

The original shared-capture benchmarks are unchanged. Additional benchmarks keep
10,000 closures or arrays simultaneously, checking scalar results without printing
the retained object graph. The same benchmark sources and retained executable
must be used on both sides. Measurements include compilation, with two warmups,
seven measured samples and rotating side order. CPU is user plus system time;
RSS is process peak memory, not an isolated VM heap measurement. At this sample
size nearest-rank p95 is the largest observed sample, not a worst-case bound.

## Validation and measurements

**127 suites passed** in an isolated clean checkout, including self-hosted, runtime, lifetime, operator, resource and emulator coverage. Temporary paths were isolated inside disposable test directories. Lint passed for all five touched PBLang files with ten warnings (eight existing VM warnings and two benchmark warnings). Build and repository-relative lock normalization passed.

**940 differential executions matched**: 600 closure/assignment runs across budget configurations and 340 raw array-construction cases. Outputs, failure state, refusal reason and runtime accounting match. **60 PBCS artifacts remain byte-identical.** The new focused regression passes on baseline and candidate, including identity/aliasing, argument defaults, transitive captures, missing locals, generic captures and malformed exact-number counts.

| Workload | Median CPU before → after (s) | p95 CPU before → after (s) | Median peak RSS before → after (KiB) |
| --- | --- | --- | --- |
| creation | 1.07 → 1.07 | 1.12 → 1.10 | 36,596 → 36,524 |
| counter | 1.48 → 1.48 | 1.51 → 1.52 | 36,436 → 36,528 |
| numeric | 1.14 → 1.15 | 1.17 → 1.16 | 36,464 → 36,560 |
| retained_single | 1.61 → 1.58 | 1.66 → 1.60 | 41,264 → 37,768 |
| retained_four | 2.08 → 2.05 | 2.13 → 2.12 | 47,856 → 42,024 |
| empty_arrays | 1.12 → 1.13 | 1.22 → 1.14 | 36,436 → 36,368 |
| three_items | 1.18 → 1.16 | 1.23 → 1.19 | 36,424 → 36,532 |
| sixteen_items | 1.36 → 1.36 | 1.41 → 1.39 | 36,484 → 36,628 |
| try_failed_bounded | 1.45 → 1.49 | 1.73 → 1.65 | 36,568 → 36,580 |

For 10,000 simultaneously retained closures, peak RSS fell **8.5% with one capture** and **12.2% with four captures**. CPU fell approximately 1.9% / 1.4% in those same workloads. These are process measurements on these fixtures, not universal heap-reclamation or resource-saving guarantees.

The small creation workload remains **1.07 → 1.07 s**. In this measurement session the pre-shared-binding reference (`27f2ef9`) was 1.03 s; the residual cost is not eliminated. The earlier approximately 2% estimate is not a stable bound across sessions. Counter CPU is unchanged; numeric and empty-array CPU rose approximately 0.9%. Bounded failed try/catch CPU rose **2.8%**, while its observed p95 fell. Timing variation prevents a general speedup or worst-case claim.

For retained single-capture closures, the older reference measured 1.45 s / 36,480 KiB, versus the candidate 1.58 s / 37,768 KiB. Shared-binding semantics therefore still carry representation cost relative to that older implementation, even after reducing it versus the immediate baseline. The older revision does not provide correct general shared mutation. Only equal-output read-only workloads are compared.

Ordinary small workloads show broadly similar peak RSS; the compiler and retained runtime contribute to the process peak. Generic arrays of 16 items remain supported and have unchanged median CPU in this sample. No general RAM saving is claimed.

[Raw samples](evidence/selfhost-small-arrays-cost-2026-09-22.json), [suite results](evidence/selfhost-small-arrays-validation-2026-09-22.json), and [differential evidence](evidence/selfhost-small-arrays-parity-2026-09-22.json).

Whitespace, documentation links, privacy checks and preservation of the 212 unrelated files passed. Graphify exceeded its 35-second update bound and was terminated; its query did not supply semantic PBLang coverage.

## Remaining scope

This change addresses allocation work in the self-hosted interpreter. It does
not reclaim reference cycles, reduce all collector pauses, complete runtime/FFI
or polyglot integration, provide real-time guarantees or establish universal
platform compatibility. Physical-device and guest-OS tests remain separate work.

The installed PB-MCP desktop path does not contain or execute this interpreter.
Adding an unused file to its bundle would not improve live view. That integration
and its measured desktop benefit remain open; the installed bundle, consent and
approval state are preserved. No EnterpriseOrg server was used.

See [shared binding behavior](SELFHOST_SHARED_CAPTURES.md),
[previous dispatch measurements](SELFHOST_DISPATCH_GROUPS.md), and
[stable-release gates](STABLE_1_0_READINESS.md).

The subsequent [bounded instruction-admission follow-up](SELFHOST_TICK_ADMISSION.md)
measures the unchanged try/catch workload against this revision. It does not
retroactively replace the measurements above.
