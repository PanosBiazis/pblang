# Detached one-pass SGD step (HF-13B)

`stdlib/ai/pbai/stdlib/pbai_feedback_train.sapp` exports `step(encoder, decoder, batch, learning_rate)`
alongside the unchanged `gradients` API. This performs one real full-batch SGD
update and returns new compatible `pbai.feedback.stack.v1` models. It changes
neither the supplied models nor the supplied samples. The caller decides whether
to use the candidate. It does not train the iterative `cycle` recurrence.

```sapp
let T = import("stdlib:ai/pbai/stdlib/pbai_feedback_train.sapp");
let candidate = T.step(encoder, decoder, [[0.2, 0.8], [0.7, 0.1]], 0.01);
// Only an explicit caller decision adopts these weights:
encoder = candidate.encoder;
decoder = candidate.decoder;
```

The objective remains `sum((D(E(x))-x)^2)/(2*batch_size*input_width)`.
Every parameter uses `candidate = original - learning_rate * gradient`, with all
joint gradients computed from the same original encoder and decoder weights.
There is no sequential decoder-then-encoder retraining, no clipping and no
second rescaling by batch size or output width.

## Supported contract

Learning rate must be a finite number in **[0.000001, 0.1]**, inclusive. It is
required, with no implicit default or conversion from booleans/strings. This
interval is a supported operating contract, not a guarantee that every accepted
step improves the objective.

All HF-13A model, batch, finite-value and parameter/sample bounds apply. Complete
owned model/sample snapshots are admitted before gradient/update loops. The
underlying gradient call revalidates and copies these bounded owned snapshots;
copy/validation overhead is included in execution cost, not numerical pair counts.
Every requested delta and every represented parameter change must have absolute
value at most **1**, and candidate parameters must remain in **[-10000,10000]**.
There is no partial acceptance, clamping or automatic learning-rate reduction.

All new tensors are constructed separately. If even the last candidate bias
fails validation, the operation throws the existing fixed
`pbai.feedback.gradients: invalid or unsupported request` family message and
returns no candidate. Original arrays remain unchanged. The family message is
shared with gradient admission for compatibility; it does not expose samples or
weights. A host runtime may separately render caller source excerpts in its
error presentation; this API does not promise their redaction.

The returned map has format `pbai.feedback.step.v1`, `encoder`, `decoder`,
`objective`, `loss_before`, `loss_after`, `learning_rate`, `parameters`,
`batch_size`, `input_width`, `parameter_sample_pairs`, and `max_abs_update`.
Both losses use frozen respective weights and the same batch/objective.
`max_abs_update` is the largest represented difference between returned and
original parameter values, not a claim about physical memory or model quality.

There are two forward passes and one backward pass per sample: one original
forward/backward pair, followed by a forward-only candidate loss evaluation.
The pair-count admission remains at most 32,768; it is not an elapsed-time or
physical-resource guarantee. A finite saturated model can validly produce zero
changes. Small updates may also round away in floating-point representation.
This is reported honestly, without claiming learning.

## Verification and boundaries

The test `pbai_feedback_step_test.sapp` implements scalar reference inference and
finite-difference gradients independently. It checks all 52 candidate parameters
across four fixtures, including both learning-rate endpoints, and compares frozen
before/after losses. Declared unsaturated fixtures show changes in both models
and lower loss; this is deliberately not a universal convergence claim. Returned
stacks are also exercised through the existing inference API.

Two non-synthetic branch assumptions are exercised with reachable models:

- E(x)=sigmoid(0), D(z)=sigmoid(320*z-160), x=0, rate=0.1 gives exactly a unit
  encoder bias change. Changing 320 to 320.001 with matching bias makes the
  proposed delta exceed one and rejects the whole step.
- E(x)=sigmoid(10000), D(z)=sigmoid(-10000*z+10000), x=1 produces a valid inward
  decoder weight update, followed by an invalid final bias of 10000.0125.
  Even this late failure leaves every caller parameter unchanged.

Tests also cover invalid rates/types/nonfinite values, malformed shapes, cyclic
samples, batch/work admission, detached ownership and deterministic repeated
calls. Existing derivative/inference acceptance remains separate and required.

This is embedded subtree source delivery. No global installation, canonical core
module, checkpoint or bundled runtime changes automatically. A [bounded training loop with fixed held-out evaluation](Neural_Feedback_Epochs.md)
is available as HF-13C. Adoption remains explicit; parity/installation
qualification and broader resource measurements remain outstanding. There is no new pretrained general model,
accelerator implementation or claim of superiority over other models.

For embedded invocation, provenance and a staged adoption checklist, see
[Embedded training adoption](Embedded_Training_Adoption.md).
