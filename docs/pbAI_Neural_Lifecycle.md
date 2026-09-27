# Explicit pbAI neural lifecycle

Standalone and embedded pbAI share the PBLang implementation in
`stdlib/pbai_neural_adapter.sapp`. These entry points are available in both facades:

- `neural_specialist_train(training, validation, options=nil)` returns initial and
  trained artifacts plus before/after metrics.
- `neural_specialist_run(artifact, ids)` predicts with an explicitly supplied artifact.
- `neural_specialist_session(training, validation, options=nil)` retains private
  initial/candidate weights and exposes `status`, `predict`, `evaluate`, `promote`
  and `rollback`.

## Model and input contract

This is an experimental, deterministically initialized decoder, not a pretrained
language model: vocabulary 8, context 4, model width 8, FFN width 16, one attention
layer/head, seed 77. Only a rank-2 LM-head adapter (32 parameters, alpha 4) is trained.
The frozen decoder weights and default assistant routing remain unchanged.
Token IDs 0..7 need a caller-defined semantic contract; compact numbers do not
supply missing information or confer natural-language understanding.

Each split contains 1..32 examples, each exactly `{x,y}` with a nonempty context
of up to four integer token IDs and one integer target. Values must be finite.
Duplicate contexts within a split and exact train/validation overlap are rejected.
Signed zero is normalized for overlap checks. Options are exactly `{epochs,lr}`:
1..128 epochs and learning rate 0.000001..0.25, default 64 and 0.1. Work is bounded
by 4096 updates per call, not a wall-clock or global session quota.

Exact schemas check map width without enumerating keys. Nested/cyclic values are
rejected before serialization. These are local post-materialization checks; a
network adapter must separately enforce byte limits before decoding requests.
Low-level `llm_adapters` operations expect trusted shape-compatible tensors and
pure deterministic feature callbacks. The specialist facade is the bounded entry.

## Training and artifact correctness

Training and inference use the same `llm_attention.head_hidden` features, including
the binary token residual. Frozen features are computed once per training context.
`train_epochs` returns the actual final adapter and preserves cumulative counters
across resumed calls; per-call `steps` remains separate.

Artifacts encode both matrices as fixed-point integers at scale 1,000,000, with a
fixed profile, base digest, step count and content digest. Validation checks exact
fields, shapes, integer/range bounds and digests. Final metrics use the reconstructed
saved representation, matching subsequent inference and JSON roundtrips.
`neural_weights_changed` compares the saved weight matrices, excluding step metadata.
An update smaller than the quantization threshold cannot claim changed saved weights.

`evaluate` scores final weights. `evaluate_pair` reuses the same frozen features for
both heads and counts formerly correct answers that become wrong. These metrics
are not the online loss averaged over changing weights during training.

## Private session transitions

A session starts in `candidate` at revision 0 with the initial artifact active.
One well-formed test set, disjoint from both training and validation, consumes the
evaluation allowance. Invalid input does not consume it. Test results must have
lower loss, nondecreasing accuracy and no lost baseline-correct answers. The same
retention rule applies to training and validation; training loss must not increase
and validation loss must decrease. Failed quality produces terminal `rejected`.

A passing result yields `evaluated` (revision 1). Explicit `promote()` switches only
this handle's active weights (`promoted`, revision 2). Explicit `rollback()` restores
the initial artifact and exact logits (`rolled-back`, revision 3, terminal).
The private decision cannot be changed by editing a returned report. Caller changes
to original datasets, prediction arrays or status objects cannot rewrite private
weights or remembered split membership. Use a handle on one thread only.

## Example: curriculum to session

```pblang
let AI=import("stdlib/pbai.sapp");
let groups=[map{family:"token-map",split:"training",examples:[map{x:[0],y:4}]},
 map{family:"token-map",split:"validation",examples:[map{x:[7,0],y:4}]},
 map{family:"token-map",split:"test",examples:[map{x:[6,0],y:4}]}];
let data=AI.neural_task_curriculum_split(8,4,groups);
let session=AI.neural_specialist_session(data.training,data.validation);
let evaluation=session.evaluate(data.test_families[0].examples);
if evaluation.passed {session.promote();}
print session.predict([6,0]).token;
if session.status().session_adapter_active {session.rollback();}
```

The runnable example is `examples/pbai_neural_session.sapp` in PBLang and
`demo/pbai_neural_session.sapp` in standalone pbAI. The three splits must also fit
the specialist's smaller fixed vocabulary/context limits.

## Privacy, authority and evaluation limits

These operations do not read chats, corpora or caches, write files, contact a
provider, register models or authorize tools. Weights, example digests and model
outputs may reveal training information; they are not anonymous or safe for
public logging merely because raw examples are absent. Digests identify content,
not author identity, consent, dataset rights or independent provenance.

Session promotion is local model selection, not persistent Max checkpoint promotion
or PBRuler authorization. Exact context disjointness does not establish semantic
independence. Repeated new sessions can still overfit a reused test set; the session
is not a hidden-test service. Rollback restores behavior, not secure memory erasure.

## Verification and next work

Tests cover finite-difference gradients, unchanged base weights, head-input parity,
resumed counters, deterministic artifacts, JSON reload, malformed and cyclic input,
split overlap, private state, rejection, promotion and exact rollback. The fixed
benchmark trains on four examples, validates on four and tests sixteen related
synthetic contexts; it reports one warmup and seven samples. p95 is the maximum of
seven sample averages, not individual-request tail latency. It is a numerical
fixture, not independent language understanding or an AGI/ASI benchmark.

Run `pblang run bench/pbai_neural_session.sapp` and the `pbai_neural_adapter_test`,
`pbai_neural_session_test`, `pbai_neural_facade_test` and `llm_adapter_pair_eval_test`
scripts under `tests/semantics`. Results are recorded in
[the lifecycle measurements](benchmarks/pbai_neural_lifecycle-2026-09-27.json).

Broader candidate architectures, supervised distillation, independent datasets,
representative language tasks, persistent model governance and device-specific
execution remain separate work. This release makes no universal-platform or
production-readiness claim.

See the [verification and review record](benchmarks/pbai_neural_lifecycle-checks-2026-09-27.json) for exact commands and coverage.
