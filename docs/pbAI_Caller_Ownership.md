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
Other callers, including legacy file-origin measurements and separate review-close,
file-preparation and tool examples, are not automatically migrated by this change.
Their remaining ownership work must be assessed individually; existing historical
benchmark evidence is not rewritten.
