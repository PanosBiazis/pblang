# Explicit teacher-to-student distillation

Both products expose `neural_specialist_distill(teacher_artifact, contexts,
validation, options=nil)` through `stdlib/pbai.sapp`. The shared implementation is
PBLang `stdlib/pbai_neural_adapter.sapp`; the soft-target training kernel is in
`stdlib/llm_adapters.sapp`.

## Model and work bounds

The teacher is the validated fixed-profile rank-2 artifact returned by
`neural_specialist_train`. A rank-1 student trains 16 adapter parameters, versus
32 for the teacher. Both use the same frozen, deterministically initialized tiny
decoder (vocabulary 8, context 4, width 8, FFN 16, one layer/head). The 0.5 ratio
is adapter-only; it is not a claim of halved total model memory or pretrained LLM
compression. Numeric tokens require caller-defined meaning.

Supply 1..32 contexts, each containing 1..4 integer token IDs in 0..7. Validation
contains 1..32 exact `{x,y}` examples. Training contexts and validation contexts
must be unique and disjoint after signed-zero normalization. Options are exactly
`{epochs,lr,temperature}`: epochs 1..128, learning rate 0.000001..0.25 and temperature
0.25..8, default 64/0.1/2. At most 4096 updates execute per call. Finite scalar,
shape and exact-field validation rejects malformed, oversized and cyclic data.
Bounds apply after values are materialized; this is a local API, not a bounded
network request parser or a global compute quota.

## Training objective and numerical checks

Teacher probabilities at temperature T supervise the student using T-squared
cross entropy. Stable log-sum-exp avoids probability clipping in the soft objective.
The analytic logit derivative is T times (student probability minus teacher
probability). The same factor-gradient implementation serves hard-label and
soft-target training, preserving the existing hard-label update order.

Frozen hidden vectors and teacher target distributions are computed once for each
training context. Only student factors update; teacher and base weights remain
unchanged. Validation labels do not enter the training loop. Evaluation uses the
actual private floating-point student weights; unlike the teacher artifact, this
student handle retains full-precision weights. Its explicit `checkpoint()` operation
exports a separate fixed-point representation; see [student checkpoints](pbAI_Student_Checkpoints.md).

The low-level `soft_target_loss` and `train_soft_step` validate bounded probability
vectors (up to 65536 entries), normalized mass, learning rate and temperature.
They expect trusted shape-compatible model/adapter tensors. Use the specialist
facade for the fixed, fully bounded model shape.

## Handle, reports and paired outcomes

The returned handle contains `report`, `predict(ids)`, `evaluate(examples)`,
`checkpoint()` and `close()`.
Final evaluation examples must be unique and disjoint from student training and
validation. Evaluation is repeatable and diagnostic: it does not enforce a hidden
one-shot test, activate the student or choose a production checkpoint.

Metrics distinguish teacher, initial-student and trained-student loss/accuracy,
teacher agreement, and KL divergence at temperature 1. Paired counts include:

- `teacher_correct_student_wrong`: correct teacher answers lost by the student.
- `teacher_wrong_student_correct`: supplied labels the student gets right where
  the teacher is wrong.
- `teacher_and_student_wrong`: examples both get wrong, whether or not they agree.
- `initial_student_correct_regressions`: correct initial-student answers lost by
  training, independently of teacher behavior.

For every evaluation, student correct = teacher correct - teacher answers lost +
student corrections. Teacher correct + student corrections + both wrong = total.
Perfect agreement can coexist with zero accuracy; tests explicitly cover that case.
Metrics use supplied labels, whose independent correctness is not authenticated.

Returned reports and prediction arrays are detached from private weights. Editing
inputs or reports cannot change later predictions. Reports contain hashes, counters
and aggregates; weights, hashes and outputs are not anonymous or automatically safe
for public logging. Teacher digests bind content, not provenance or authorization.

## Example

```pblang
let AI=import("stdlib/pbai.sapp");
let teacher=AI.neural_specialist_train([map{x:[0],y:4}],[map{x:[7,0],y:4}]);
let student=AI.neural_specialist_distill(
 teacher.artifact,[[0],[1,0]],[map{x:[7,0],y:4}]);
print json_stringify(student.evaluate([map{x:[6,0],y:4}]));
print student.predict([6,0]).token;
assert(student.close(),"release caller-owned distillation state");
```

Use `examples/pbai_neural_distillation.sapp` in PBLang or
`demo/pbai_neural_distillation.sapp` in standalone pbAI.

## Verification and remaining work

`llm_soft_distillation_test.sapp` checks analytic gradients against central finite
differences at four temperatures, hard-label/one-hot update parity, input ownership,
extreme logits and malformed probability distributions. `pbai_neural_distillation_test.sapp`
checks both product entry points, reproducibility, split boundaries, caller-mutation
isolation, paired counts and misleading teacher agreement. Existing neural adapter
and private-session tests protect the hard-label and rollback paths.

`bench/pbai_neural_distillation.sapp` uses four teacher training examples, sixteen
student training contexts, four validation and sixteen distinct final contexts.
One warmup and seven samples report all quality results plus median/p95 timings.
Timing excludes teacher training; inference samples average ten calls. Seven-sample
p95 is the maximum sample average, not request-level tail latency. Shared-desktop
load is uncontrolled; these are diagnostic timings, not a performance comparison.

See [measurements](benchmarks/pbai_distillation_release-2026-09-27.json) and
[verification coverage](benchmarks/pbai_distillation_release-checks-2026-09-27.json).

No chat ingestion, file/network access, model registration, persistent checkpoint
change or tool authorization occurs through this operation. Teacher training/test
overlap and independent provenance remain unverified. Semantic dataset separation,
production replacement gates,
representative language evaluation and other devices remain separate work. No AGI,
ASI, general language competence or universal-platform claim follows from this fixture.

Explicit labeled training with bounded joint gradients is available through
[`neural_specialist_distill_supervised`](pbAI_Specialist_Supervised_Distillation.md).

## Explicit lifetime and failed use (2026-10-08)

Both soft-only and supervised distillation now return an idempotent `close()`.
The first call returns true, subsequent calls false. It clears the private model,
teacher/initial/student adapters, context ledger and internal report reference.
Saved `predict`, `evaluate` and `checkpoint` aliases become terminal; mutable public
fields cannot reopen the private state. Reports, metrics, predictions and checkpoints
exported before close retain their independent caller-owned lifetimes. Export before
close when a later load or continuation is intended. Training and numerical output
formats are unchanged.

The caller must close each acquired handle. For automatic cleanup after catchable
use failures, register its captured close method in `resource_scope.with_scope`.
`examples/pbai_distillation_scope.sapp` (standalone: `demo/`) demonstrates success,
invalid prediction and overlapping evaluation while keeping an independent active
model usable. The callback uses a direct-parameter factory to avoid inherited
capture limitations in the retained runtime. Raw distillation calls do not
implicitly join an ambient scope. There is no returned handle to close when
construction itself fails, and registration/OOM/hard termination are not covered.

Run `tests/semantics/pbai_distillation_close_test.sapp` with isolated `PBAI_STATE_ROOT`.
It checks direct/facade/binding routes, soft/supervised variants, successful use,
actual invalid-prediction and evaluation-overlap failures, and early manual close
followed by scope cleanup. It inspects original methods before a diagnostic close,
preserves the caller's active model, checks detached exports and rejects malformed
post-close inputs. Default mode requires closure; `PBAI_DISTILL_LIFETIME_PHASE=baseline`
only characterizes old code. The before/after output digests must match exactly.
See [evidence](evidence/pbai_distillation_lifetime-2026-10-08.json).

Explicit ownership removal is not proof of physical GC/reclamation, zeroization or
RSS savings. The fixed numerical experiment does not establish AGI/ASI or all-device
support. Plain neural training sessions still lack terminal close; older callers
that discard these handles without closing them are not automatically migrated.
