# Caller-owned student workflows

The plain session, loaded checkpoint, distillation, continuation and reviewed
continuation handles already expose explicit close operations. Exporting a
checkpoint does **not** close its producing handle. Dropping a local variable or
finishing a benchmark sample is not an explicit lifecycle event either.

This migration covers exactly these three runnable examples in PBLang's
`examples/` and standalone pbAI's `demo/`, plus the matching `bench/` files:

- `pbai_student_checkpoint.sapp`: student export, decode/load, detached evaluation.
- `pbai_student_continuation.sapp`: parent export, separate continuation and parent load.
- `pbai_student_review.sapp`: parent export, historical review, accept and rollback.

## Examples: acquire, register, export, close

Each example enters `resource_scope.with_scope` before acquisition and captures
its bundled closer immediately after a handle is returned. Export/use can then
fail without leaving previously acquired handles unclosed. An acquisition failure
has no new handle to close; earlier handles still receive their registered cleanup.
The close callback uses a direct-parameter factory because the retained runtime
has limitations with inherited nested closure captures.

The seed student closes immediately after exporting its detached checkpoint.
The registered final cleanup may call its closer again and receives `false`, a
valid already-closed result. Continuation, loaded and reviewed handles close on
scope exit, including after a caught application exception. A failed body or
cleanup makes the example fail; it does not report successful completion.

Already exported checkpoints, histories, metrics and predictions retain their
own lifetimes. A source checkpoint can explicitly load another model after its
producer closes. The printed successful numerical results remain unchanged.
For persistence, save the exported checkpoint, not a new export from a closed
student. Export any continuation checkpoint before leaving its scope.

These are trusted synchronous bundled-provider examples. The scope does not
establish concurrency safety, arbitrary-provider guarantees, secure zeroization,
physical garbage collection, RSS savings or cleanup after OOM/hard termination.
An independent caller model remains independently owned.

## Benchmarks: unchanged measured work

Every warmup, measured reload/candidate/review and seed now has a corresponding
explicit close on successful execution. Cleanup occurs after the last use and
outside the existing timed sections. The checkpoint experiment retains three
objectives, one warmup and seven samples; continuation retains one warmup and
five samples; review retains positive/rejected cases and five samples each.
Datasets, options, quality assertions and output schemas are unchanged.

These benchmark scripts demonstrate successful-run ownership, not exception-safe
benchmark recovery. Their timing samples reflect shared desktop load and do not
establish a speedup. The fixed quality outputs, including checkpoints, labels,
metrics, acceptance decisions and source-preservation assertions, are compared
before and after the migration.

## Real caller observation

`tests/tooling/pbai_caller_lifetime.py` runs the actual selected caller body in a
catchable test function inside private copies of the selected product and shared
PBLang dependency. A test-only provider hook records the original prediction,
checkpoint and close methods of real bundled handles. It does not replace the
model or numerical algorithms. The copies have their own dependency manifests;
owner-relative `stdlib:` imports require the PBLang manifest even in the standalone
fixture. The original source roots are hidden with Linux bubblewrap and network
access is denied. Every temporary fixture is removed in a `finally` block.

Example probes cover success, checkpoint-export exception, malformed evaluation
or prediction, failure acquiring a later handle, then recovery in the same process.
They inspect saved original methods **before** diagnostic cleanup and keep an
independent active model unchanged. The observer deliberately retains aliases;
counts establish explicit closure, not ordinary-process reachability or RAM use.
Benchmark probes run the unchanged workloads and inspect every acquired handle.

From the trusted product directory, set `PBLANG_BIN` to the trusted runtime and run:

```sh
python3 tests/tooling/pbai_caller_lifetime.py --product pblang --target checkpoint
python3 tests/tooling/pbai_caller_lifetime.py --product pblang --target continuation
python3 tests/tooling/pbai_caller_lifetime.py --product pblang --target review
```

Use `--product pbai` from the standalone checkout and `--kind benchmark` for each
matching benchmark. The standalone checkout needs its sibling PBLang dependency,
as declared by its manifest. Default mode requires all acquired handles to close;
`--phase baseline` characterizes historical caller source only. The observer needs
Linux, Python 3 and bubblewrap. It is not an all-platform acceptance test. Direct
unmodified example/benchmark executions and core semantic regressions supplement
these instrumented observations.

See [the scoped evidence](evidence/pbai_caller_ownership-2026-10-08.json).
Other callers, including legacy file-origin measurements and checkpoint-file
read/reload failure paths, still require individual ownership assessment.
Existing historical benchmark evidence is not rewritten.

## Review close and reviewed-reload preparation (cycle 19)

Two more examples are migrated in both products:

- `pbai_student_review_close.sapp` registers the seed, reviewed handle and restored
  handle before export or use. The seed closes immediately after detached export.
  Its existing demonstration still explicitly closes the reviewed handle twice,
  observes terminal prediction, and reloads the earlier export. Final scope cleanup
  treats already-closed bundled handles as valid no-ops and closes the restored one.
- `pbai_student_checkpoint_reload_reviewed.sapp`, **prepare branch only**, gives
  each trained model its own scope. It closes that model before starting the next
  file, including after export or exclusive-save failure. A failed later acquisition
  has no new returned handle and all preceding models are already closed.

The prepare operation is still nontransactional: earlier explicitly saved files
remain if a later operation fails. Existing destinations are never overwritten.
Cleanup changes neither serialized checkpoint bytes nor training options. A failed
body/cleanup makes the example fail; the current scope does not expose caught
exception text, so the outer assertion reports workflow failure. The
recover and review-error branches are unchanged. Their `with_file` callback borrows
its candidate from the checkpoint scope; that scope remains its owner. The active
model and any independently reconstructed replacement have separate ownership.
The teacher training result is an artifact/report, not a closeable model handle.

The same observer now supports:

```sh
python3 tests/tooling/pbai_caller_lifetime.py --product pblang --target review_close
python3 tests/tooling/pbai_caller_lifetime.py --product pblang --target checkpoint_reload_reviewed
```

Use `--product pbai` in standalone. These two targets are example-only. The second
runs prepare against a private harness-created directory, preserving the real file
adapter and exclusive-create behavior. Its save-failure case puts a synthetic
existing destination at the second name and checks that its bytes remain unchanged.
The harness removes only its own temporary fixture after observation.

The review-close observer covers success, seed export failure, reviewed export
failure, malformed reviewed/restored prediction, failure acquiring the reviewed or
restored model, and recovery success. Preparation covers success, export failure,
real save failure, later acquisition failure, and recovery in the same process.
Saved original methods are inspected before diagnostic cleanup, with exactly one
first close required per real handle, including the review constructor's already
owned intermediate continuation. Serialized exports and saved-file byte digests
are compared before/after; direct unwrapped executions also check recover and
review-error compatibility. No model algorithm or handle API is changed.

See [cycle 19 evidence](evidence/pbai_remaining_caller_ownership-2026-10-08.json).
The observer retains aliases and demonstrates explicit closure only, not physical
memory release, zeroization, hard-kill recovery or universal device support.

## Tool and distillation examples (cycle 20)

Three additional callers now register owned handles before using them:

- `pbai_student_tools.sapp` owns its checkpoint-producing seed and independently
  loaded active model. The seed closes immediately after detached export; both
  captured closers are registered before later operations can fail. The batch
  provider continues to own its separate private loaded handle. Its prediction
  worker borrows that handle and does not acquire caller ownership.
- `pbai_neural_distillation.sapp` and
  `pbai_specialist_supervised_distillation.sapp` register their student's close
  method before evaluation or prediction. Their explicit successful closes remain;
  scope exit safely tolerates an already-closed bundled handle.

These files are in PBLang's `examples/` and standalone pbAI's `demo/`. Teacher
training returns a detached artifact/report, not another closeable handle.
Successful stdout is byte-for-byte unchanged across both products. On catchable
failure, `resource_scope` attempts the captured cleanups before the final assertion
reports workflow failure; the retained catch mechanism does not preserve raw
exception text. There is no returned handle when construction fails.

```sh
python3 tests/tooling/pbai_caller_lifetime.py --product pblang --target tools
python3 tests/tooling/pbai_caller_lifetime.py --product pblang --target distillation
python3 tests/tooling/pbai_caller_lifetime.py --product pblang --target supervised
```

Use `--product pbai` from the sibling standalone checkout, with the same trusted
`PBLANG_BIN` setup above. These are example-only targets. They run with net, http,
exec, ffi, spawn, gfx and sqlite explicitly denied. The Python test driver creates
and builds the private copied fixtures; the example process needs no process or
FFI grant. Older file-backed observer profiles are unaffected.

The cases cover success, detached-export failure, invalid active prediction,
later acquisition failure, a private tool prediction failure returned as the
existing error envelope, soft evaluation/prediction failures, supervised evaluation
failure, initial constructor rejection and recovery. The tool-error case checks
that the provider closed its private handle and the caller can continue. Every
case keeps an independently owned model and its checkpoint unchanged.

Across both products, 30 cases per phase observe 40 actual acquired handles.
Before migration 20 were still usable at observation time; afterwards none were,
and all 40 had exactly one successful first close before diagnostic cleanup.
Original method aliases, detached-export digests and partial/successful outputs
are compared. Six direct executions per phase independently preserve exact stdout.
See [cycle 20 evidence](evidence/pbai_tool_distill_caller_ownership-2026-10-08.json).
This proves explicit cleanup for these callers; it does not establish physical RAM
reclamation, zeroization, performance gains, AGI/ASI or universal platform support.
Other test/benchmark callers and file workflows remain individually scoped work.


## Checkpoint-file write ownership (cycle 25)

The `write` branch of `pbai_student_checkpoint_file.sapp` now opens a resource
scope before training and registers the returned student's captured closer before
exporting its checkpoint. Export and exclusive-save failures retain fixed primary
messages even if cleanup also fails. Unexpected constructor/assertion exceptions
remain opaque. Cleanup runs once; only boolean results are valid, including the
idempotent `false`. A throwing or nonboolean closer prevents success output.
The successful output schema, training options and saved checkpoint bytes remain
unchanged. The teacher is an artifact/report, not a second owned handle.

A successfully saved file remains if subsequent cleanup fails: this example is
not a transaction. Existing destination bytes are never replaced. A close that
throws before release leaves the model's release unconfirmed; there is no automatic
retry. The test's later diagnostic cleanup is separate from application cleanup.
The read/reload/close branches are unchanged and still need separate exception-path
ownership review.

```sh
python3 tests/tooling/pbai_caller_lifetime.py --product pblang --target checkpoint_file
```

Use `--product pbai` in the sibling standalone checkout. The example-only observer
runs ten cases per product: success, export failure, real exclusive-save failure,
initial construction failure, close failures before and after release, a nonboolean
close, combined export/close and save/close failures, and recovery success. Saved
original operations are inspected before diagnostic cleanup, and real checkpoint
bytes and existing-file sentinels are checked. This uses the existing Linux
source-hidden file observer profile, not a new sandbox or cross-platform guarantee.
No training algorithm, model API, physical-memory or AGI/ASI claim follows from
these lifecycle tests. See [cycle 25 evidence](evidence/pbai_file_write_ownership-2026-10-09.json).


## Guarded checkpoint-file read/reload (explicit PBCK profile)

`pbai_student_checkpoint_file_guarded.sapp` is a companion compilation input in
`examples/` (PBLang) or `demo/` (standalone). It exposes `checkpoint_work` and
`checkpoint_release` for the existing PBLang-native compiler cleanup boundary.
It supports `read` and `reload`; continue using the original example for `write`
and `close`. The original direct-source read/reload branches retain their
historical exception-cleanup limitation. Running the companion as ordinary source
does not invoke its declared entry. Use the explicit guarded compilation below.

From the trusted PBLang project directory:

```sh
pblang run tools/pbc.sapp --allow-all -- examples/pbai_student_checkpoint_file_guarded.sapp --target pbck --entry checkpoint_work --cleanup checkpoint_release --out checkpoint-guarded.pbck
PBAI_CHECKPOINT_TEST_DIR="$PRIVATE_CHECKPOINT_DIR" PBAI_CHECKPOINT_TEST_PHASE=read pblang run checkpoint-guarded.pbck --allow-all --deny-net --deny-http --deny-spawn --deny-gfx --deny-sqlite
PBAI_CHECKPOINT_TEST_DIR="$PRIVATE_CHECKPOINT_DIR" PBAI_CHECKPOINT_TEST_PHASE=reload pblang run checkpoint-guarded.pbck --allow-all --deny-net --deny-http --deny-spawn --deny-gfx --deny-sqlite
```

For standalone, run from its project directory, use the sibling provider compiler
`../pblang/tools/pbc.sapp` and source `demo/pbai_student_checkpoint_file_guarded.sapp`.
Keep the generated artifact inside its product project so its imports can find
the product manifest; moving it to an unrelated directory without that layout is
unsupported. Compile to a new private output filename. The existing compiler output is not an
exclusive-create transaction. The artifact retains module imports and requires
the qualified product/provider layout and trusted working directory; this is not
a relocatable dependency-free executable. The private checkpoint directory must
already meet the existing Linux adapter's private/no-follow rules. No capability
allowlist is strengthened by this example; the file profile retains exec/FFI
permissions, while networking, spawn, graphics and SQLite are denied in tests.

The guard captures cleanup before entering the workflow. Each returned handle's
closer is saved immediately and its attempt flag is set before calling it.
A rejected candidate leaves the previous model intact. A successfully loaded
replacement is separately owned before the retired model is closed. If retired
close throws, cleanup still attempts the replacement exactly once. Other active
models remain caller-owned. Success output retains the old read/reload schema and
is printed only after cleanup and terminal-prediction checks.

If the work throws, the compiled boundary preserves the original native message,
including when cleanup also fails. Cleanup continues through remaining handles;
throwing/nonboolean cleanup on an otherwise successful body prevents success
output. Boolean false is a valid idempotent response and is reported as such.
There is no automatic retry after a before-release exception. Initializer/import
failures before entry, hard termination, OOM and nonreturning cleanup remain out
of scope. No runtime/compiler/shared primitive was changed.

The standalone and embedded qualification driver is:

```sh
PBLANG_BIN="$TRUSTED_PBLANG_RUNTIME" python3 tests/tooling/pbai_guarded_file_lifetime.py --product pblang
```

Use `--product pbai` in standalone. It compares the actual historical caller to
the companion using real private files, saved original model aliases, independent
active-model preservation and synthetic faults. Observation precedes diagnostic
cleanup. Unmodified companion runs verify success/fallback output parity. The
private copies hide the original source roots and deny networking. This is Linux
qualification of the explicit profile, not universal source cleanup or physical
RAM reclamation. See [guarded evidence](evidence/pbai_guarded_file_ownership-2026-10-10.json).

## Direct checkpoint-file read/reload ownership (`defer`)

The ordinary `read` and `reload` branches of `pbai_student_checkpoint_file.sapp`
now run inside `read_phase`/`reload_phase`. Each acquired student handle gets its
own owner record (`handle_owner`), released once through
`defer handle_owner_release(owner);`. The owner records are never reassigned,
because PBLang closures capture variables by reference: a reassigned
`active_release` variable would redirect a pending cleanup (the reverted PR #92
bug). Native `defer` runs on unwind and keeps the primary error; a non-boolean
close result raises `invalid checkpoint close result`. The `write` and `close`
branches are unchanged.

The self-hosted PBCK frontend has no `defer` statement, so this direct example
cannot be compiled to PBCK; the guarded PBCK companion above remains the
compiled profile. Its gate now uses the frozen pre-defer caller
`tests/fixtures/pbai_student_checkpoint_file_baseline.sapp` as its baseline.

Linux/bwrap source-hidden check (20 read/reload fault cases):

```bash
PBLANG_BIN="$TRUSTED_PBLANG_RUNTIME" python3 tests/tooling/pbai_checkpoint_file_direct_lifetime.py --product pblang --phase baseline
PBLANG_BIN="$TRUSTED_PBLANG_RUNTIME" python3 tests/tooling/pbai_checkpoint_file_direct_lifetime.py --product pblang
```

The baseline must report unclosed fault cases (7 observed); the fixed run must
report `unclosed_fault_cases: 0`. In the standalone `pbai` repository use
`--product pbai` with sibling `../pblang`. This verifies handle close calls only,
not physical RAM reclamation or hosted CI.
