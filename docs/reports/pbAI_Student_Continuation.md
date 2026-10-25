# Explicit continuation from a student checkpoint

Both products expose `neural_specialist_student_continue(artifact, training,
validation, options=nil)` through `stdlib/pbai.sapp`. The PBLang implementation
lives in `stdlib/pbai_neural_adapter.sapp`, alongside checkpoint validation and
student inference. It creates a new diagnostic candidate from caller-supplied
labeled examples. The supplied checkpoint, loaded parent handle, and frozen base
remain unchanged. Training does not write files or activate a model.

## Starting state and update rule

The input is the validated rank-1 `pbai.neural.student-checkpoint.v1` artifact,
including its fixed base/profile binding and 16 fixed-point adapter parameters.
The first update starts from those dequantized weights, not a fresh random head.
This is a hard-label continuation path: each supplied `y` is the optimization
target. It reuses the existing verified hard-only endpoint of
`train_supervised_step_clipped`, applying a single L2 gradient bound across both
factors. It does not reconstruct a missing teacher or reuse validation labels for
training, tuning, stopping or checkpoint selection.

Options have exactly three fields:

| Field | Default | Bounds |
| --- | ---: | --- |
| `epochs` | 16 | integer 1..128 |
| `lr` | 0.05 | 0.000001..0.25 |
| `max_grad_norm` | 1 | 0.000001..1000 |

Training and validation each contain 1..32 exact `{x,y}` examples. Contexts contain
1..4 integer tokens in 0..7; `y` is an integer in 0..7. Duplicates and overlaps are
rejected with signed-zero normalization. Malformed, cyclic, extra-field and
nonfinite inputs fail validation. The fixed matrix magnitude bound remains active.

Added updates equal `epochs * training example count`. Before any update, the
operation rejects a batch for which `artifact.steps + added_updates > 4096`.
Counters are cumulative and never silently reset. This is a limit on declared
checkpoint history, not an authenticated global compute quota: a caller can
construct another valid artifact and recompute its unkeyed content digest.
The report makes that limitation explicit.

## Candidate, metrics and reversibility

The returned handle exposes `report`, `predict`, `evaluate`, `checkpoint` and `close`.
It has no promotion or automatic-save operation. Reports include the parent
digest, prior/added/total steps, options, online hard-label loss, clipping counts,
maximum gradient norm, and paired baseline/candidate train and validation metrics.
Paired evaluation always uses the source checkpoint as the baseline. It reports
`baseline_correct_regressions` so new learning cannot hide lost prior answers.
Improvement on one supplied label set is not automatic acceptance for use.

`weights_changed` compares actual floating-point factors. Separately,
`checkpoint_weights_changed` compares exported integer factors and excludes step
metadata. The live `student_digest` and serialized `checkpoint_digest` identify
different representations; incrementing counters alone changes checkpoint identity
but does not prove a retained weight change. Each export is detached from the
private candidate. Editing inputs, reports or returned matrices cannot change it.

The new candidate's `evaluate` rejects its current training and validation
contexts. `current_run_input_disjointness_checked` describes only this run;
`prior_training_overlap_checked` and independent provenance remain false.
Training metrics explicitly do not claim holdout disjointness. The checkpoint
contains no prior examples, so the API cannot verify all historical overlap or
prevent forgetting without caller-supplied evaluation/replay data.

Keeping the parent checkpoint permits explicit restoration of the earlier behavior
with `neural_specialist_student_load(parent)`. Saving the candidate requires a
separate caller-invoked `neural_specialist_student_save_new` with a new filename.
Parent and candidate files are independent. A subsequent continuation starts from
the candidate's exported six-decimal factors; repeated export/resume cycles are
not claimed bitwise-equivalent to uninterrupted floating-point SGD. Checkpoints
retain weights/base/steps, not authenticated ancestry, datasets or optimizer history.

## Privacy and platform scope

New product code is PBLang only. The continuation operation has no filesystem,
network, subprocess, telemetry, implicit chat ingestion or cache-write operations.
Examples are supplied explicitly and remain local; exported checkpoints contain
no raw examples or labels. Weights, hashes and predictions may still encode
information and should not be treated as anonymous. Existing file storage retains
its Linux private-directory, no-replacement and bounded-read contract; see
[student checkpoints](pbAI_Student_Checkpoints.md).
Other-platform execution, encrypted storage and crash durability are not established
by this local training change. Activation authority remains outside these handles.

## Reproduction and evidence

Run `tests/semantics/pbai_student_continuation_test.sapp` for the focused regression.
It verifies a one-step numeric oracle from the serialized factors, cumulative
limits including the exact ceiling, label revision, unchanged parent/base, old
answer regressions, validation-label independence, private ownership and malformed
inputs. Existing checkpoint, supervised-distillation and session tests protect
compatibility. The separately invoked fixture
`tests/fixtures/pbai_student_continuation_file.sapp` uses an isolated directory and
phases `write`, `continue`, `read` to verify continuation across processes/products
while preserving the original file. Its environment names are
`PBAI_CONTINUATION_TEST_DIR` and `PBAI_CONTINUATION_TEST_PHASE`.

`bench/pbai_student_continuation.sapp` fixes the parent, 16 new examples, four
validation examples and 16 test examples. It runs the default 16 epochs at 0.05
with clipping limit 1, one warmup and five measured runs. It records both live and
reloaded quality, baseline-correct regressions, unchanged source checkpoint, and
median/p95 sample wall times. Five-sample p95 is the maximum sampled duration;
shared desktop load is uncontrolled. There is no test-label hyperparameter search
and no performance or general-intelligence claim.

See [measurements](benchmarks/pbai_student_continuation_release-2026-09-27.json)
and [checks](benchmarks/pbai_student_continuation_release-checks-2026-09-27.json).
Runnable examples are `examples/pbai_student_continuation.sapp` in PBLang and
`demo/pbai_student_continuation.sapp` in standalone pbAI. Representative language
benchmarks, authenticated lineage, history-aware replay, production
promotion and validation on other devices remain separate work.

Measured result: the original checkpoint declared 1024 updates; continuation added
256 and exported 1280. Held-out loss decreased from 0.7690471660 to 0.7333827668,
accuracy stayed 8/16, and baseline-correct regressions were zero. Both products
returned identical reports and live/reloaded quality across all five samples.
The original checkpoint file hash stayed unchanged across the cross-product
fresh-process continuation test. These are fixture-specific results.

## Optional history and retention review

For explicit declared history, fresh-input admission, serialized-weight retention
gates and reversible local acceptance, use the new
[student review API](pbAI_Student_Review.md). The diagnostic API above retains its
original behavior and provenance limitations. A declared history is not authenticated
ancestry or proof of complete upstream data.

## Explicit continuation ownership (2026-10-08)

The caller owns each handle returned by `student_continue`. Call `close()` when
finished: the first call returns `true`, subsequent calls return `false`.
`predict`, `evaluate` and `checkpoint` become terminal, including previously
saved method references. Public map edits cannot reopen the private state.
Already exported checkpoints, reports, predictions and metrics remain caller-owned.
Export a checkpoint before close if later restoration is needed. The supplied
parent artifact and independently loaded parent handles remain unchanged.

The reviewed constructor owns its intermediate continuation handle. It captures
its close method immediately, exports the checkpoint, and closes the intermediate
before reconstruction or evaluation. A catchable checkpoint export exception also
attempts to close it, then throws `student continuation checkpoint export failed`; no partial
review is returned. Constructor failure before a handle exists has no handle to
close. The returned reviewed handle has its own independent lifetime.

Method factories capture only a private state map. Close clears the model,
baseline/candidate factors, base identity, context ledger and internal report
reference. This establishes explicit reference removal and terminal methods,
not physical allocator reclamation, zeroization, RSS reduction, or cleanup after
OOM/hard termination. Ordinary neural training sessions now have an explicit
[close contract](pbAI_Neural_Lifecycle.md#explicit-lifetime-and-failed-use-2026-10-08);
legacy callers still need ownership migration. Distillation handles have their own
[close contract](pbAI_Distillation_Contract.md#explicit-lifetime-and-failed-use-2026-10-08).

`tests/semantics/pbai_continuation_close_test.sapp` exercises original saved methods,
malformed inputs, detached exports and independent active ownership through direct,
legacy-facade and explicit-binding routes. `pbai_continuation_lifetime_test.sapp`
uses private copied providers with Linux `bwrap`, denied network and hidden source
roots, following the prerequisites of the [tool lifetime test](pbAI_Student_Tools.md).
Set `PBLANG_BIN` to the trusted executable and `PBAI_RELOAD_PRODUCT=pblang|pbai`.
The default gate requires closure; `PBAI_CONTINUATION_LIFETIME_PHASE=baseline` only
characterizes the historical implementation. Three routes each exercise successful
review, checkpoint-export exception, rejected review and later recovery, with an
independent caller model preserved throughout. Original methods are inspected
before diagnostic cleanup. See [evidence](evidence/pbai_continuation_lifetime-2026-10-08.json).

## Migrated example and benchmark ownership (2026-10-08)

The matching checkpoint, continuation and review examples now register captured
closers before export/use and close on synchronous catchable failure. Their
benchmarks close seeds, warmups and samples outside the timed work. See the exact
[migrated caller scope and reproduction](pbAI_Caller_Ownership.md); other legacy
callers remain separate work.

## Export failure and cleanup precedence (2026-10-08, cycle 22)

The reviewed constructor makes exactly one attempt to close its intermediate
continuation, including when checkpoint export throws or returns nil. A failed
export retains the existing fixed `student continuation checkpoint export failed`
message even if cleanup also throws or returns an invalid value. Cleanup cannot
replace that known primary failure. The retained runtime still cannot recover
arbitrary original caught exception text.

When checkpoint export succeeds, a catchable cleanup exception or nonboolean
close result throws `student continuation cleanup failed` before reconstruction,
review evaluation or return of a reviewed handle. No backend payload is formatted
or exposed. The bundled idempotent boolean results remain compatible, including
false for an already closed handle. A return shape alone does not prove closure
for an untrusted or modified implementation. There is no retry, fallback, global
activation or new callback/provider API.

This deliberately tightens the previously silent acceptance of malformed cleanup
results. Normal successful/rejected reviews, artifacts, metrics and legacy
facade/binding outputs remain unchanged. When both operations fail, the existing
known export error takes precedence; the throwing API does not expose a separate
secondary cleanup report. If cleanup throws before releasing state, this change
does not manufacture successful release or solve unknown native cleanup failures.

The mirrored `tests/tooling/pbai_review_export_failure.py` gate copies the real
provider into a private fixture and wraps only continuation export/close methods.
It tests real owned handles through direct, legacy facade and explicit binding
routes. Saved original methods are checked before diagnostic cleanup. Cases where
a close throws before release are explicitly counted as still open, then closed
by the test; they are not automatic-cleanup successes. An independent model and
caller parent/history are checked after every case. Separate uncaught subprocess
cases check the exact fixed failure text and suppression of synthetic private
messages. This is test-only fault injection, not a new production extension hook
or proof over all native failures.

From either product, set `PBLANG_BIN` to the trusted executable and run:

```sh
python3 tests/tooling/pbai_review_export_failure.py --product pblang
# From the standalone sibling repository, use --product pbai.
```

Linux/Python/bubblewrap are test prerequisites. Child model computations deny net,
HTTP, exec, FFI, spawn, graphics and SQLite; the orchestrator creates/removes only
its private fixture. `--phase baseline --baseline-owner FILE` is an explicit
historical source comparison, not an installed runtime mode. The source gate's
`--provider-stdlib DIR --expected-owner-sha256 HASH` allows a separately pinned
installed-provider copy to be tested with the same fault overlay. Such a test
must be labeled an overlay qualification and cannot stand in for unmodified
installed-launcher acceptance. No performance, physical RAM release or accelerator
claim follows from the lifecycle counts.
