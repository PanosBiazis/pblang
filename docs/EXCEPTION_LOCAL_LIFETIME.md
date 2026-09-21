# Automatic cleanup before exception handlers

The PBLang-written compiler now clears non-captured temporary local references
from a failed `try` body **before evaluating its `catch` handler**. No manual
`free` or new library call is required. Existing success-path scope cleanup
remains unchanged.

## Implementation and boundaries

Each try body receives new, monotonic local slots beyond the caller's existing
slots. The compiler emits a cleanup prelude in its handler covering exactly
that body-owned interval, including slots belonging to nested failed handlers.
Caller variables, global aliases and captured slots are preserved.

A declaration after the failing operation might never have executed. The prelude
first reads its slot and uses the existing nil-peek branch to skip absent/nil
values. It does not extend the local array merely to overwrite an absent value.
The native translator implements this test with its non-overloadable nil test;
no user equality function or destructor is called by cleanup.

Only existing bytecode instructions are emitted. The serialized PBCS format and
native PBCK translator need no format/opcode changes. Compile-time capture
tracking and the existing patchable cleanup stores protect native open captures
as well as PBLang VM value captures. No Rust or polyglot source was added.

The user's handler receives the same control flow and computes the same result.
If the handler itself throws, its error is not caught or replaced by a cleanup
wrapper. In direct VM and translated-native subprocess tests, `handler-original`
remained the diagnostic payload before and after this change. A body error that
a handler deliberately replaces is still replaced according to the existing
language semantics; this change does not add an exception-object API.

## Verification

- 40 local suites passed, including exception/local lifetime, closure and scope
  regressions, optimizer, serialization, bounded VM, scheduler and five emulator
  suites. This is not execution inside all guest operating systems.
- Eight additional old/new native-bytecode program pairs produced equal output.
- Four subprocess error checks (old/new, self-hosted/native) retained the original
  handler diagnostic and stopped before the unreachable output.
- A caller-owned frame inspection confirms that body slots are nil **at handler
  entry**, not merely after final execution.
- The tests cover aliases, inherited caller slots, escaped closures, nested
  failing handlers, explicit and called-function throws, out-of-range array access, ordered effects and
  skipped declarations under an existing one-local bounded-VM quota.

Regression source: [selfhost_exception_lifetime_test.sapp](../tests/semantics/selfhost_exception_lifetime_test.sapp).
Error fixture: [selfhost_handler_error.sapp](../tests/fixtures/selfhost_handler_error.sapp).

## Measurements (2026-09-20)

Reference code generator: `f168738`. Same retained executable, remaining compiler
modules, workloads and isolated dependency closure. Build and portable-lock
publication were outside timing. Two warmups and seven interleaved measured runs
per version; GNU time collected total process CPU (user+system), elapsed time and
peak RSS for compilation plus execution. P95 is nearest-rank, the largest of
seven samples. CPU resolution is 0.01 seconds; small differences include noise.
No test suites or Graphify ran concurrently with the measurement.

| Workload | Version | CPU median / P95, s | Wall median / P95, s | RSS median / P95, KiB |
|---|---|---:|---:|---:|
| 16 failed blocks, 2 MiB temporary each | before | 0.16 / 0.17 | 0.16 / 0.17 | 56840 / 57096 |
| 16 failed blocks, 2 MiB temporary each | after | 0.15 / 0.16 | 0.16 / 0.16 | 26404 / 26468 |
| 1,000 failed numeric iterations | before | 0.27 / 0.28 | 0.27 / 0.28 | 24196 / 24460 |
| 1,000 failed numeric iterations | after | 0.28 / 0.29 | 0.28 / 0.29 | 24288 / 24420 |
| 1,000 successful numeric iterations | before | 0.26 / 0.27 | 0.26 / 0.27 | 24224 / 24328 |
| 1,000 successful numeric iterations | after | 0.26 / 0.27 | 0.27 / 0.27 | 24320 / 24484 |

Peak RSS fell about 54% for distinct failed blocks. Repeated numeric failures
cost about 4% more median CPU; the successful-loop CPU median was unchanged.
No general speedup, zero overhead, GPU/NPU, energy or disk-I/O savings are claimed.
Handler bytecode grows with the number of body-owned slots, even for handlers
that are not taken. Existing instruction/stack budgets still apply to cleanup;
resource exhaustion can prevent a handler from running. No quota is bypassed.

Reproduce by compiling and executing these sources through `frontend_compile`
and `vm.run`, with identical old/new dependency closures. Print only the output
array, never cyclic VM globals/chunks. Collect `/usr/bin/time -f '%U %S %e %M'`
for two warmups and seven alternating measured runs.

- Failed blocks: start with `let total=0;`, repeat this **source text** 16 times:
  `total=total+len(try (if true {let v=str_repeat("x",2097152);throw 77;v;} else {"";}) catch "");`
  and finish with `print total;`. Expected output: 0.
- Failure loop: `let total=0;for i 1000 {let x=try (if true {let v=i+1;throw 1;v;} else {0;}) catch 0;total=total+x;} print total;`.
  Expected output: 0.
- Success loop: remove only `throw 1;` from the previous source. Expected output:
  500500.

## What remains

This closes the caught-body temporary-reference gap, not every runtime lifetime
issue. [Explicit throws and returns](EXPLICIT_EXIT_LIFETIME.md) now also clear
their own active non-captured references. Implicit uncaught failures still depend
on existing frame/context teardown; general error-preserving cleanup of external
resources is unfinished.
A handler that fails is cleaned by this mechanism when an enclosing compiled
try owns its temporary slots, not by a universal finally/destructor facility.

Captured slots, cyclic graphs, explicit numeric owner handles and FFI allocations
are not reclaimed by this prelude. Current `runtime_gc.sapp` still reports
`owner_isolation: false` for retained thread-local heaps: enabling collection
without all suspended caller roots could destroy live objects. The scalar
`vm_memory.sapp` engine rejects composite ownership rather than guessing aliases.
Neither protection was weakened. An identity-aware owner-scoped representation
and resource-specific ownership/destructors remain necessary.

Source C99/WASM/ARM64 and other independent backends are not implemented or
certified by these native-bytecode checks. The full runtime/FFI and cross-platform
backlog remains open.

Code generator SHA-256: `a447de37d649192f3c85be6ad4f4c673363616c2f99529dcbeea6c743658cb4e`.
Retained executable SHA-256: `7f1b97c0037736553ab39d165c13c36f3fd8497bc6aca9ca94a4c680858e6500`.

The isolated build, native portable-lock publication and six lint/self-hosted
syntax checks passed. Lint reports one unused-result advisory in the deliberate
uncaught-error fixture. Graphify update exceeded 35 seconds and was terminated;
no new semantic graph coverage is claimed. All 212 unrelated dirty files were
preserved. No Specisoft server was used.

For the subsequent direct try/catch dispatch and single-edge traversal changes,
see [try and chain cost follow-up](TRY_AND_CHAIN_COST.md).
