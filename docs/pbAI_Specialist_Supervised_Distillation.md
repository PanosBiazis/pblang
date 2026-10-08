# Supervised distillation of the fixed token specialist

Both products expose `neural_specialist_distill_supervised(teacher, training,
validation, options=nil)` through `stdlib/pbai.sapp`. It shares the bounded private
student implementation with `neural_specialist_distill`; the original soft-only
entry point retains its options, unclipped updates and report format.

## Explicit supervision and limits

The teacher must be a valid fixed-profile rank-2 artifact from
`neural_specialist_train`. The student remains rank 1: 16 trainable adapter
parameters versus 32 for the teacher, sharing the same frozen tiny base. This
ratio describes adapters only, not total model memory or pretrained capability.

Training and validation each contain 1..32 exact `{x,y}` examples. Context `x`
contains 1..4 integer token IDs in 0..7; label `y` is an integer in 0..7. Duplicate
contexts, signed-zero aliases, overlap between splits, extra fields, nonfinite
numbers and malformed/cyclic structures fail validation. Evaluation also rejects
student training and validation contexts. Teacher training overlap and independent
label provenance are not checked; caller labels are not certified ground truth.

Options must have exactly these five fields:

| Field | Default | Range |
| --- | ---: | --- |
| `epochs` | 64 | integer 1..128 |
| `lr` | 0.1 | 0.000001..0.25 |
| `temperature` | 2 | 0.25..8 |
| `hard_weight` | 0.5 | 0..1 |
| `max_grad_norm` | 1 | 0.000001..1000 |

There are at most 4096 updates per call. Bounds apply to materialized local values;
this does not implement a network parser, global compute quota or concurrent
training scheduler. Existing matrix magnitude limits reject oversized updates.

## Objective and stability

With weight w and temperature T, the objective is:

`(1-w) * T*T * CE(teacher probabilities at T, student logits / T)`
`+ w * CE(supplied label, student logits)`.

Weight 0 ignores labels for optimization (they must still be valid); weight 1 is
hard-label training. Intermediate weights mix both signals. The five options are
explicit; validation/test labels never select a weight, epoch or checkpoint.
The default 0.5 is a starting configuration, not a tuned recommendation.

The two factor gradients are combined before clipping. A single L2 norm across
both factors gives one scale, preserving their joint direction. Scaling components
by their largest magnitude before squaring avoids overflow. Nonfinite or oversized
gradients are rejected. Teacher weights and frozen hidden features never update.

`llm_adapters.train_supervised_step_clipped` and `train_soft_step_clipped` are
low-level kernels for trusted, shape-compatible tensors. Their scalar/distribution
validation does not replace the specialist's fixed-shape boundary.

## Results, privacy and interpretation

The handle exposes `report`, `predict(ids)`, diagnostic `evaluate(examples)` and
explicit `checkpoint()` plus terminal, idempotent `close()`. Training retains private in-memory weights; optional
[checkpoint export and file persistence](pbAI_Student_Checkpoints.md) are separate
caller-invoked operations, with no activation. Returned objects and later mutations of inputs cannot change them.
Training adds no file writes, network calls, chat ingestion, registry changes or
telemetry. Numeric
labels, model outputs, weights and hashes can still encode information: reports
are not automatically anonymous or suitable for public logging.

The report adds `online_soft_loss`, `online_hard_loss`, `online_combined_loss`,
`hard_weight`, `max_grad_norm`, `clipped_steps` and
`maximum_unclipped_gradient_norm`. Online losses are averages measured before each
update, not final held-out performance. `training_labels_authenticated` remains
false. Final evaluation retains paired teacher errors, student corrections,
regressions, loss, accuracy, agreement and KL divergence from the
[distillation contract](pbAI_Distillation_Contract.md).

## Reproduction and remaining work

Run `bench/pbai_specialist_supervised_distillation.sapp` in either product. It fixes
the same teacher, 16 student examples, four validation examples and 16 distinct
final examples. Weight 0, 0.5 and 1 all use rank 1, 64 epochs, learning rate 0.1,
temperature 2 and clipping limit 1. This controls clipping and compute when
comparing objectives; label-bearing variants receive additional information.
There is one warmup and seven measured samples per method. Timings are diagnostic
shared-desktop wall time with uncontrolled load and fixed method order. The p95
is the largest of seven sample averages, not request-level tail latency.

See [measurements](benchmarks/pbai_specialist_supervised_release-2026-09-27.json)
and [checks](benchmarks/pbai_specialist_supervised_release-checks-2026-09-27.json).
The executable example is `examples/pbai_specialist_supervised_distillation.sapp`
in PBLang and `demo/pbai_specialist_supervised_distillation.sapp` in standalone pbAI.

Finite-difference tests check both factors at T=0.25,1,2,8 and weights 0,0.5,1;
additional tests verify joint clipping, unchanged legacy updates, correction of a
wrong teacher, validation-label isolation, strict boundaries and private handles.
This synthetic tiny-token experiment does not establish language competence, AGI,
ASI, universal device support or production readiness. Representative datasets,
independent provenance and production replacement
gates remain separate work.

Measured on the recorded Linux runtime, all three variants obtained 8/16 correct.
Held-out cross entropy was 0.769306 for clipped soft-only, 0.807390 for the 50/50
mix and 0.758686 for hard-only. No teacher-correct answers were lost, and no teacher
errors were corrected on this fixture. Both products produced identical reports
and quality metrics across all repeated runs. These results do not support a
general accuracy improvement or selecting the mixed objective as superior. A
separate regression fixture verifies that explicit labels can correct a deliberately
wrong teacher in the hard-only endpoint.

For close semantics, detached exports and cleanup after failed use, see the
[shared lifetime contract](pbAI_Distillation_Contract.md#explicit-lifetime-and-failed-use-2026-10-08).

The runnable supervised example now closes its student after catchable evaluation failure as well as success, with unchanged training and output. See [caller ownership and regression evidence](pbAI_Caller_Ownership.md#tool-and-distillation-examples-cycle-20).
