# History-aware review of student continuation

`neural_specialist_student_continue_reviewed(checkpoint, history, training,
validation, options=nil)` is an optional strict path above the diagnostic
[continuation API](pbAI_Student_Continuation.md). Both the standalone and embedded
pbAI facades call the same PBLang implementation. Existing checkpoint formats and
continuation behavior are unchanged.

## Declared history and separation

Create a history explicitly with
`neural_specialist_student_history(checkpoint, previous_training, previous_holdout)`.
Each collection contains 1..128 exact `{x,y}` examples, with 1..4 integer tokens
in 0..7 and one label in 0..7. Contexts are unique within each collection and the
two collections must be disjoint. Numerical signed zero has one canonical identity.
The history contains exactly `format`, `checkpoint_digest`, `generation`,
`training`, `holdout`, and `digest`. Its format is `pbai.neural.student-history.v1`.
The digest covers the format, checkpoint identity, generation and ordered,
canonical labeled examples. Generation starts at zero.

A legacy checkpoint does not reveal its training history. This initial history
is a **caller declaration**, not reconstructed or independently authenticated
provenance. The caller must supply the actual prior student training inputs and
correct labels for the retained evaluation cases. The implementation cannot prove
that the prefix is complete, labels are true, or the declared training occurred.
Teacher training, upstream data and omitted runs remain unknown. No source name,
path, username, chat, external account or URL is collected.

New training and validation each have 1..32 examples, with the existing continuation
options and 4096 declared cumulative-step ceiling. All new contexts must be fresh:
no reuse from either historical split, duplicate within a new split, or overlap
between the new splits. Thus recorded holdouts cannot silently become training,
and previously used validation cannot masquerade as fresh validation. This strict
path intentionally disallows replay and corrections on previously observed inputs;
the original diagnostic API remains available for those experiments.

Both histories must still fit 128 examples after appending the new sets, and at
most eight declared reviewed generations are admitted. Capacity, binding, schema,
and split failures reject before training. Failed attempts return the unchanged
parent history; they do not create an authenticated global attempt ledger. A caller
can restart, rewrite a declaration and recompute its unkeyed digest. The limits
bound an invocation/declared chain, not all computation or experimentation.

## Review before local acceptance

The old holdout is fixed by history before training. Every old holdout example is
evaluated; callers cannot select a favorable subset after seeing the candidate.
The new validation and historical labels do not train the candidate or select
intermediate weights. The candidate follows exactly the diagnostic continuation
update rule. The review then exports and reloads its fixed-point factors and
compares **those serialized weights**, not only the unsaved floating-point head,
against the original checkpoint.

All of the following must pass:

- New training: finite loss/accuracy, no higher loss, no lower correct count,
  and zero baseline-correct regressions.
- New validation: the same conditions, with strictly lower loss.
- Entire historical holdout: no higher loss, no lower correct count, zero
  baseline-correct regressions, and at least one originally correct answer.

There is no caller-controlled relaxation or post-training threshold adjustment.
The nonempty-correct requirement prevents a vacuous claim of retained capability.
Average improvement cannot compensate for losing an individual previously correct
answer. Retention is limited to the declared examples; it does not establish
retention of all knowledge. Historical holdouts are reused and caller-selected,
so the report explicitly does not claim an independent test or unbiased benchmark.
Repeated attempts and any external hyperparameter search require separate control.

## Handle, authority and rollback

The returned single-threaded, in-memory handle exposes:

| Method | Result |
| --- | --- |
| `report()` | Detached paired metrics, pass flags, digests, counts and provenance limits |
| `status()` | Phase, active checkpoint/history identities and local-only authority flags |
| `predict(ids)` | Prediction from the active checkpoint, initially the parent |
| `checkpoint()` | Detached active checkpoint, initially the parent |
| `history()` | Detached active history, initially the original declaration |
| `accept()` | Once, only after a passing review; switches this handle to the exact evaluated checkpoint and appended history |
| `rollback()` | Once after acceptance; restores both original checkpoint and original history, then finishes the acceptance transaction while keeping the handle usable |
| `close()` | From any phase, clears handle-owned model/checkpoint/history references and enters terminal `closed`; returns `true` once, then `false` |

Phases are `reviewed` or `rejected`, then `accepted`, then `rolled-back`.
Any of these phases may explicitly transition to terminal `closed`.
A rejected handle cannot be accepted. Editing returned reports, status, histories,
checkpoints or original inputs confers no authority and cannot alter private state.
No evaluator callback, custom model implementation, file target or tool authority
is accepted. Acceptance changes only this handle; it does not install a model,
change global routes, invoke tools, save files or authorize actions.

A new reviewed continuation may explicitly receive the accepted `checkpoint()`
and `history()`. The accumulated training and holdout data remain bound to that
checkpoint. The next review evaluates the entire accumulated holdout, including
validation from accepted preceding runs. Keep the parent pair to restore or branch.

## Explicit terminal close

`close()` ends only the reviewed handle. After closing, `predict`, `checkpoint`,
`history`, `accept` and `rollback` throw `student review is closed` before
processing their inputs. Saved method aliases observe the same closed state.
`status()` and detached `report()` remain readable: active checkpoint/history
identities are `nil`, candidate activity is false, and authority flags remain false.
The retained report still contains aggregate metrics, counts and content digests.

The private method factory captures only the state owning its model, adapters,
checkpoints and histories. Closing clears those references; it does not promise
immediate garbage collection, physical memory erasure, a measured RAM reduction,
crash cleanup or thread safety. Previously exported checkpoints, histories and
predictions remain caller-owned and usable, as do other independently created
handles. Closing does not delete caller files or revoke exports. Ordinary loaded,
distilled and legacy continuation handles retain their existing lifecycle.
Rollback remains distinct: it restores usable parent state. To stop using that
rolled-back handle, call `close()` explicitly. There is no reopen operation;
creating another review is an explicit independent operation.

Run `tests/semantics/pbai_student_review_close_test.sapp` in each product for
direct-module, legacy public-facade and bound-facade routes across all four
pre-close phases (12 cases per product).
The runnable examples are `examples/pbai_student_review_close.sapp` in PBLang
and `demo/pbai_student_review_close.sapp` in standalone pbAI. The examples now use the
[explicit specialist binding](pbAI_Specialist_Binding.md), with the selected neural
module imported at application top level. This supported route executes from a
foreign cwd without loading a caller-directory provider. Its Linux installed
harness has separate `installed`, `unavailable` and `surface` phases for source-hidden
execution, decoy manifests and missing/broken selected dependencies.

The legacy `neural_specialist_*` convenience wrappers still require the trusted
product directory as cwd on the retained runtime. A foreign caller manifest can
redirect their lazy imports before a reviewed handle exists. Use the bound route
for foreign-cwd applications; the general runtime-origin defect is not repaired.

## Transport and privacy

Explicit `neural_specialist_student_history_encode(checkpoint, history)` and
`neural_specialist_student_history_decode(checkpoint, text)` provide bounded,
strict JSON transport. Decode admits at most 32768 bytes, depth 5, 4096 parser
items and 1024 lines; duplicate decoded keys, trailing garbage, unknown fields,
wrong types, invalid tokens, overlapping contexts and mismatched bindings fail.
A history does not replace or extend the seven-field student checkpoint.

**History exports contain the supplied token contexts and labels.** They are not
anonymous and may disclose training/evaluation information; content hashes also
are not anonymization. Export is an explicit call, with no automatic persistence,
telemetry, network, corpus/cache access or conversation ingestion. Default reports
contain only aggregate metrics, counts and identities. No new history file-saving
API is provided; the existing private checkpoint file API still accepts only
checkpoint artifacts. Encryption, authenticated lineage, cross-device support and
crash-safe history storage are outside this slice.

## Reproduction

Run `tests/semantics/pbai_student_review_test.sapp` in either product. The synthetic
fixture includes successful acceptance/rollback, a candidate that improves its new
labels but forgets an old correct answer, exact serialized evaluation, accumulated
split guards, malformed/oversized/cyclic inputs, and mutation isolation.
The runnable example is `examples/pbai_student_review.sapp` in PBLang and
`demo/pbai_student_review.sapp` in standalone pbAI.

This is a bounded tiny-token specialist control mechanism, not a representative
language benchmark or evidence of AGI/ASI. All platform/runtime claims must be
limited to the environments actually exercised in the accompanying check record.

`bench/pbai_student_review.sapp` measures the unchanged positive and forgetting
fixtures with one warmup and five samples each. Reports must be repeatable and
identical across products; shared-desktop timings are observational, and the
five-sample p95 is the largest sample. No speedup is claimed.

The fixture `tests/fixtures/pbai_student_review_file.sapp` also exercises separate
processes using `PBAI_REVIEW_TEST_DIR` and `PBAI_REVIEW_TEST_PHASE` (`seed`, `resume`,
`verify`). It deliberately uses ordinary file writes for synthetic test transport
under a runner-created mode-0700 directory and mode-0600 umask. This is not a new
production storage implementation or a secure-file guarantee. Run `seed` in
PBLang, `resume` in pbAI, then `verify` separately in both; verify source-file hashes
before and after. The fixture exports raw token histories only when explicitly run.

## Recorded local results (2026-09-28)

Both products returned exactly the same review reports and example output under
PBLang 0.6.0 on Linux x86_64. In the fixed positive case, validation loss changed
from 0.0022279596 to 0.0005434065, with the historical correct answer retained
(1/1, zero regressions). In the forgetting case, new-label validation loss improved
from 8.0569821353 to 0.0008668170, but historical correctness fell from 1/1 to 0/1;
acceptance was rejected. Neither result measures general language capability.

See [the measured reports](benchmarks/pbai_student_review_release-2026-09-28.json)
and [local verification](benchmarks/pbai_student_review_checks-2026-09-28.json).

Reviewed predictions now reuse the private evaluated adapters across calls and
follow acceptance/rollback without reloading. For bounded prediction/comparison
requests and measured inference behavior, see [student tools](pbAI_Student_Tools.md).

## Migrated example and benchmark ownership (2026-10-08)

The matching checkpoint, continuation and review examples now register captured
closers before export/use and close on synchronous catchable failure. Their
benchmarks close seeds, warmups and samples outside the timed work. See the exact
[migrated caller scope and reproduction](pbAI_Caller_Ownership.md); other legacy
callers remain separate work.
