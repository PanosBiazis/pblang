# Scoped checkpoint file use

`stdlib:pbai_checkpoint_scope.sapp` provides `with_file(path, body)`: one explicit,
synchronous use of a loaded student checkpoint with cleanup after the callback.
It reuses `resource_scope.sapp`; it does not implement another cleanup engine.
The shared module serves embedded and standalone pbAI.

The ordinary sequence `load_file`, application work, `close` can leave a live
handle when the work throws and its caller catches the error. The new scope owns
that handle during one callback and attempts the captured close before returning
its result, including after a catchable exception or malformed callback result.

## Explicit setup and use

```pblang
let AI=import("stdlib/pbai.sapp");
let S=import("stdlib:pbai_checkpoint_scope.sapp");
let scoped=AI.neural_specialist_checkpoint_scope_bind(S);
let result=scoped.with_file(path,fn(loaded){
    return map{ok:true,value:loaded.predict([6,0])};
});
if result.ok {print result.value.token;}
```

These are trusted top-level imports with an application manifest/materialized
lock. The scope owner captures the selected file adapter and resource-scope
module when imported. Calling it later from a foreign working directory cannot
select dependencies from that caller directory. Direct `S.with_file` has the
same contract. No new lazy-import facade is added.

The new binding accepts a native module with a callable `with_file` export and
returns exactly that captured operation plus `report()`. It neither invokes the
provider nor loads a model during binding. Detached fixed metadata is structural
information, not provider authentication or verification of storage/cleanup
semantics. Select trusted packaged modules; importing them runs initialization.
The existing checkpoint binding still returns exactly `save_new`, `load_file`
and `report`, so older two-operation providers remain compatible.

## Result and ownership contract

The callback must return `map{ok:true,value:...}` or `map{ok:false,error:...}`.
The returned scope report follows the existing `resource_scope` contract:

| Field | Meaning |
| --- | --- |
| `ok` | Body returned a valid success envelope and cleanup succeeded |
| `value` | Successful body payload, even if cleanup subsequently failed |
| `error` | Explicitly returned body error payload; never replaced by cleanup |
| `body_status` | `returned`, `exception`, or `invalid_result` |
| `body_result` | Snapshot of the valid body envelope, or nil |
| `original_error_available` | True for explicitly returned body errors |
| `cleanup_ok` | The single registered cleanup completed successfully |
| `cleanup_errors` | Separate cleanup exception or malformed-close result |
| `cleanups_attempted` | One after a loaded handle enters the callback scope |

Always inspect `ok`; the presence of `value` does not prove complete success.
A caught body exception reports `body_status:exception`, `error:nil` and
`original_error_available:false`. The retained runtime does not expose its
original exception message to catch expressions. Return an explicit error
envelope when the application needs to retain its own error object. Invalid
return values, including nil or false alone, are failures and still trigger
cleanup. Payload/error objects remain caller-owned aliases, not deep copies or
redacted data; the scope does not serialize or log them.

The close function is captured before application code runs. Replacing the
public `loaded.close` field cannot replace that cleanup. Closing the bundled
handle early is allowed: its later idempotent false result is valid. A nonboolean
close result is a cleanup failure with the fixed message
`invalid checkpoint close result`; a thrown close is separately classified as an
opaque cleanup exception. Cleanup is attempted once, without retry or fallback.
A failing or dishonest custom implementation cannot establish that resources
were released merely by returning a shaped result.

A saved prediction/evaluation/checkpoint method and a handle returned as the
body value refer to closed state once the scope returns. Earlier detached
predictions, evaluations and exported checkpoints remain usable. A returned
checkpoint can explicitly load another independent handle. Nested scopes own
independent handles; an inner failure must not close the outer one.

## Acquisition failures and limits

A noncallable body is rejected before any file access. Otherwise `load_file`
runs before entering the body scope. Acquisition failures preserve its existing
public exception, invoke no callback, and make no cleanup attempt on an absent
handle. Strict decoder errors remain distinct from bounded private-storage
errors. There is no retry, ordinary-filesystem fallback, automatic model
replacement, training, activation, deletion or checkpoint rewrite.

Cleanup applies to trusted synchronous callbacks and catchable runtime failures.
It is not a sandbox, cancellation supervisor, deadline, thread synchronization,
hard-kill/OOM recovery mechanism or guarantee against callbacks that never
return. It does not promise immediate garbage collection, reduced RSS, secure
zeroization or revocation of earlier exports. The file adapter retains its
Linux-only private-parent requirement and existing capability grants, byte
limits, quantization and lack of authentication/encryption/crash-durability
claims. Other device/platform support and representative AI evaluation remain
separate work.

## Example and verification

First create `student.json` using the existing checkpoint file example in a
fresh private directory. Then run `examples/pbai_student_checkpoint_scope.sapp`
(PBLang) or `demo/pbai_student_checkpoint_scope.sapp` (standalone) with the same
`PBAI_CHECKPOINT_TEST_DIR` and `PBAI_CHECKPOINT_SCOPE_PHASE=success|throw`.
Both paths expose their success/failure status and confirm that a saved
prediction alias has become terminal.

- `tests/semantics/pbai_checkpoint_scope_binding_test.sapp`: actual helper/public
  facade, invalid providers, no invocation during binding, detached reports and
  unchanged legacy checkpoint binding.
- `tests/semantics/pbai_checkpoint_scope_installed_test.sapp`: set `PBLANG_BIN`
  to a trusted retained runtime and choose one bounded `PBAI_SCOPE_PHASE`:
  `baseline`, `real`, `faults`, `errors`, `unavailable-scope` or
  `unavailable-resource`. Optional
  `PBAI_SOURCE_HIDE_ROOT` hides another source tree. The harness copies both
  products, hides original sources, creates a foreign caller with decoy modules,
  denies networking and removes its private fixture after each phase.
- `baseline` characterizes the ordinary unscoped failure pattern, then explicitly
  closes its test handle. `real` tests actual private files and native handles,
  callback success/errors/throws/malformed results, aliases, early/mutated close,
  returned handles/exports and nested scopes. `faults` replaces only the private
  copied file adapter to exercise invalid bodies, acquisition failure, thrown
  cleanup and malformed close results with exact call counts. That fixture is
  never a production backend-selection mechanism.
- `errors` checks exact acquisition errors and absence of callback execution.
  `unavailable-scope` / `unavailable-resource` check selected dependencies
  cannot fall back to caller decoys, while the original storage binding remains
  usable without these opt-in dependencies. Each deliberate shared-tree mutation
  rebuilds both private fixture locks; no automatic lock repair is added to the
  runtime API. Real file phases retain the backend's existing FFI/exec grants; fault
  and unavailable-dependency children deny them as well as network, HTTP and SQLite.

See [recorded evidence](evidence/pbai_checkpoint_scope-2026-10-05.json).
This is a lifecycle/correctness change, with no performance optimization claim.
The cycle8 benchmarks and rejected inline-guard experiment remain unchanged.
