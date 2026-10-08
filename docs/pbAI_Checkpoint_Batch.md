# Bounded checkpoint file groups

`stdlib:pbai_checkpoint_scope.sapp` now also exports `with_files(paths, body)`.
It owns one to eight independently loaded checkpoint handles for one synchronous
callback. Use it for an explicit comparison of a current and candidate model,
or another bounded operation that needs all files before it can start.
It neither replaces an active model nor selects a winner automatically.

```pblang
let AI=import("stdlib/pbai.sapp");
let S=import("stdlib:pbai_checkpoint_scope.sapp");
let batch=AI.neural_specialist_checkpoint_batch_bind(S);
let result=batch.with_files([current_path,candidate_path],fn(handles){
    return map{ok:true,value:[handles[0].predict([6,0]),handles[1].predict([6,0])]};
});
if result.ok {print result.value;}
```

Import trusted modules during application setup. The same shared owner serves
standalone and embedded pbAI. The new binding checks a native module's callable
`with_files` export without invoking it and exposes exactly `with_files` and a
fresh `report()`. The report declares `max_files:8`, not an authenticated provider
or verified storage guarantee. For a custom provider this is the bundled
contract, not a measured or enforced provider limit; binding does not wrap its
requests. The original single-file binding still accepts
providers exporting only `with_file`; its surface and exception contract remain
unchanged. The original save/load binding remains unchanged too.

## Preflight and partial acquisition

The body must be callable. The entire paths input must be an array of one to
eight nonempty strings, each at most 4096 characters and without NUL. These checks
finish before the first file access. The paths list is copied; caller input is
never used as a cleanup ledger. Invalid request shape throws a fixed error and
performs no loading. Absolute/private-parent and actual file guarantees remain
the existing file adapter's responsibility. Eight individually bounded files
are not a promise about total process RSS or total execution time.

The resource scope starts before acquisition. Files load in caller order; each
successful load immediately registers its original close function. The body is
called exactly once only after all loads succeed, with handles in that order.
If a later load throws, acquisition stops and the body is never called. Every
previously acquired handle gets one cleanup attempt, in reverse acquisition
order. A failure closing one handle does not prevent attempts on earlier ones.
There is no retry, fallback, partial callback or automatic model activation.

Duplicate paths are allowed and produce independent handles. They do not imply a
filesystem snapshot: an external writer can change files between acquisitions.
This API is not an atomic read of several checkpoints and does not authenticate
or authorize model promotion. Each actual file still passes the existing bounded
private-storage and strict checkpoint decoder checks.

## Result contract

The result contains the existing [single-file scope](pbAI_Checkpoint_Scope.md)
resource-scope fields plus:

| Field | Meaning |
| --- | --- |
| `stage` | `acquire` if acquisition stopped; `body` once every handle loaded |
| `acquisition_error` | nil, or a fixed adapter error with zero-based `index` of the rejected file (0..7) |
| `acquired_count` | Number of successfully loaded, registered handles, 0..8 |
| `cleanups_attempted` | Equals `acquired_count` on normal return from the scope |

A failed first load returns stage `acquire`, count 0 and cleanup success with
zero attempts: there was no acquired handle to release. A failed second load
reports count 1 and one attempt. Failed acquisitions and thrown body exceptions
report `body_status:exception`, nil primary error and
`original_error_available:false`, because the retained runtime does not expose
the original caught exception message. Unlike `with_file`, batch acquisition
errors are intentionally caught by the already-open resource scope. Use the
stage to distinguish acquisition from body failure. The additive
`acquisition_error` preserves known adapter failures before the opaque catch.
It contains only `index` and `error`, never a path or checkpoint content. Read
rejections deliberately do not distinguish missing files from permission/storage
failures. Invalid encoding and invalid checkpoint structure have distinct fixed
messages. Unexpected exceptions still have nil `acquisition_error`; this field
does not recover arbitrary runtime exception messages. All old fields retain
their previous values, including nil `error` and false
`original_error_available` for acquisition failures. Cleanup failures remain
separate and cannot replace the acquisition diagnostic.

The callback returns `map{ok:true,value:...}` or `map{ok:false,error:...}`. Its
explicit error is retained separately from cleanup errors. Invalid results are
failures and still close the group. Check overall `ok`: `value` can be present
when the body succeeded but cleanup failed. Payloads/errors remain aliases, not
deep copies or redacted data. The owner neither logs nor serializes them.

Original closers are registered before the application callback. Removing or
replacing entries in the exposed handle array or replacing a public close field
does not change the ledger. Early explicit close is allowed; boolean false from
the bundled idempotent close is valid. Cleanup failures retain separate entries
and registration IDs in reverse order. There is one attempt per registered
handle, with no assertion that a failing/dishonest custom closer released it.

Handles or saved methods retained by the body refer to terminal state after the
scope returns. Detached predictions/checkpoint exports remain usable. Nested
scopes own separate handles; an inner partial acquisition failure does not close
the outer model. Returning the handle list does not transfer live ownership.

## Verification and limits

Run `tests/semantics/pbai_checkpoint_batch_binding_test.sapp` in each product.
Run `tests/semantics/pbai_checkpoint_batch_installed_test.sapp` with trusted
`PBLANG_BIN` and one bounded `PBAI_BATCH_PHASE=real|faults|unavailable-scope|unavailable-resource`. Optional
`PBAI_SOURCE_HIDE_ROOT` hides another source tree. Copies of both products run
from a foreign caller with eight decoy modules, hidden original sources and
network denied. The fault adapter replaces only a private copied dependency.
It is not a production backend option. Every deliberate dependency mutation
regenerates both fixture package locks. The fixture is removed at completion.

The fault matrix covers every acquisition failure position 1..8, cleanup failure
at every registered handle, malformed requests/results, exact call counts and
reverse cleanup order. Native-file cases cover limits 1/8, duplicate-path
independence, saved/returned handles, detached exports, nested failure, early
close and mutation of exposed fields. They also run the comparison example with
missing, corrupt and valid candidate files and verify unchanged original bytes.

Create `student.json` with the existing file example, then explicitly supply a
`candidate.json` in the same fresh private directory. Run
`examples/pbai_student_checkpoint_batch.sapp` (PBLang) or
`demo/pbai_student_checkpoint_batch.sapp` (standalone) with
`PBAI_CHECKPOINT_TEST_DIR`. A missing or corrupt candidate yields a structured
acquisition failure, never a comparison result or implicit replacement.

Trusted synchronous callbacks only: no guarantee for nonreturning callbacks,
hard termination, OOM, deadlines, concurrent access or asynchronous ownership.
No secure zeroization, immediate GC/RSS reduction, revocation of prior exports,
authentication, encryption, durability, training or automatic replacement claim.
The native file backend remains Linux-only. This uses the previously tested
direct-capture cleanup factory; it is not a general fix for the retained
runtime's inherited-closure limitation. No performance optimization or AGI/ASI
claim is made. See [recorded evidence](evidence/pbai_checkpoint_batch-2026-10-05.json).

For result-based decoding/loading, reproducible before/after checks and the
three-case example, see [acquisition diagnostics](pbAI_Checkpoint_Diagnostics.md).
