# Android execution cleanup validation — 2026-09-16

`stdlib/android_emulator.sapp` now owns a private VM execution context in
`run_pbcs`, closing it immediately after the existing caught execution call,
before returning failure, recording an event or returning success.
`compile_and_run` inherits this behavior. Supported scalar owner/value, borrow
and lifetime indexes are cleared even after a guest throw or runtime budget
failure. Previously failure bypassed the VM's successful-execution close and
final reclamation depended on host GC.

The context is never inserted into guest globals. Result schemas, fixed error
text, event policy, runtime budgets and compiler limits are unchanged. No retry,
background worker or rollback was added. Top-level seeded globals are copied;
mutations through nested aliased maps/arrays can remain after failure.

## Validation

The new `android_emulator_cleanup_test.sapp` covers root/inner allocations and
borrows followed by throw, failure shape and private-output suppression,
step-budget exhaustion, invalid options, fresh execution, success metadata/nil,
compile-and-run delegation, shallow-seed behavior, malformed bytecode and a full
event queue followed by recovery. The Android example demonstrates contained
failure and a subsequent fresh ownership state.

A scratch observer delegates to the real VM, retaining the ownership state
around execution. Swapping only the Android module's VM import gave the baseline
`owners=2, borrows=2, closed=false`; the change gave `0,0,true`, one close and a
nil context memory pointer. This establishes cleanup sequencing under that
instrumentation. The public API tests alone cannot observe the private context;
no test-only telemetry was added to the product. Observer copies are excluded
from publication. Logical cleanup is not a measurement of OS memory release.

- 57 selected semantic suites passed, including 17 emulator suites.
- 12 gates passed: check, lint and strict entry-only pbsecure for the module,
  regression suite, benchmark and example. The example also executed successfully.
- Independent defensive review found no material blocker; its scope was static
  review, not an independent rerun of all suites.
- Final build and Graphify outcome are recorded below.

## Reproducible measurements

Baseline module: `cf72c48373507f55bbe4867788954efa47ac093f`.
Retained executable SHA-256:
`7f1b97c0037736553ab39d165c13c36f3fd8497bc6aca9ca94a4c680858e6500`.

```sh
pblang run bench/android_emulator_cleanup.sapp --allow-all -- 200 failure 16
```

Arguments are repetitions, success/failure, scalar owner count. The benchmark
compiles and serializes once; timed work includes bytecode decoding, execution,
cleanup and successful event recording. A synthetic emulator/app is created
before timing. Identical workloads use baseline/current Android modules and the
same current VM/executable. One warmup per variant and seven measured samples,
alternating order; p95 is nearest rank (maximum of seven). GNU time measures
whole-process CPU and peak RSS, including initialization. Python/GNU time only
orchestrate the PBLang workload. An initial run overlapped tests and was discarded;
the reported run was restarted after all test/example processes completed.
Unrelated system activity was not controlled.

[Raw samples and source hashes](ANDROID_CLEANUP_BENCHMARK_2026-09-16.json).

| Outcome | Owners × repetitions | Before median / p95 ms | After median / p95 ms | Median change | Before / after median RSS KiB |
| --- | --- | --- | --- | --- | --- |
| success | 0 × 500 | 298.937 / 310.843 | 299.661 / 386.862 | +0.24% | 37644 / 37788 |
| success | 16 × 200 | 448.812 / 458.135 | 443.206 / 823.184 | -1.25% | 37664 / 37700 |
| success | 1024 × 5 | 534.692 / 720.072 | 517.182 / 607.669 | -3.27% | 37632 / 37600 |
| failure | 0 × 500 | 313.027 / 315.904 | 304.418 / 309.491 | -2.75% | 37628 / 37656 |
| failure | 16 × 200 | 444.206 / 511.260 | 450.235 / 666.622 | +1.36% | 37708 / 37712 |
| failure | 1024 × 5 | 517.998 / 941.399 | 522.894 / 532.645 | +0.95% | 37628 / 37620 |

Median elapsed changes ranged from about -3.3% to +1.4%, with median peak RSS
within 0.4%. Sample variation is substantial in some cases.
This is a cleanup correctness change, not evidence of a general speedup or
zero overhead. No executable-size, storage-write, GPU/NPU or whole-compiler
improvement was measured. The emulator retains its normal successful event
history; the change does not remove those application-level records.

## Remaining boundaries

Only supported scalar ownership is covered. Direct VM/frontend uncaught-failure
cleanup, composite/shared values, native runtime/FFI, backend parity, broader
library work, audit/release authenticity, PBIDE Run/Designer and guest-OS tests
remain open. Android tests run the userspace conformance model locally, not a
real Android OS or device; they do not prove universal compatibility or hostile
code isolation. Version 1.0.0 remains incomplete. No Specisoft server was used.

## Final local gates

Retained-bootstrap build and portable-lock publication passed; the final build
reported no dependency-hash warning. Graphify update completed with exit 0.
Its AST analyzer does not support `.sapp`, so completion is not semantic or
call-graph coverage of the changed PBLang code. Generated graph/probe files
remain excluded from the commit.
