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

The returned handle exposes `report`, `predict`, `evaluate` and `checkpoint`.
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
benchmarks, authenticated lineage, history-aware replay/retention gates, production
promotion and validation on other devices remain separate work.

Measured result: the original checkpoint declared 1024 updates; continuation added
256 and exported 1280. Held-out loss decreased from 0.7690471660 to 0.7333827668,
accuracy stayed 8/16, and baseline-correct regressions were zero. Both products
returned identical reports and live/reloaded quality across all five samples.
The original checkpoint file hash stayed unchanged across the cross-product
fresh-process continuation test. These are fixture-specific results.
