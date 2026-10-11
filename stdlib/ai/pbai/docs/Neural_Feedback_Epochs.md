# Bounded full-batch reconstruction training (HF-13C)

`stdlib/ai/pbai/stdlib/pbai_feedback_train.sapp` exports
`train_epochs(encoder, decoder, train_batch, eval_batch, learning_rate, epochs)`.
It repeats the existing detached joint SGD `step` exactly the requested number
of times and returns new compatible `pbai.feedback.stack.v1` models. All original
models and samples remain unchanged, including if a later epoch fails.

```sapp
let T = import("stdlib:ai/pbai/stdlib/pbai_feedback_train.sapp");
let trained = T.train_epochs(encoder, decoder,
    [[0.2, 0.8], [0.7, 0.1]], [[0.15, 0.85]], 0.01, 5);
// Adoption remains an explicit caller decision, not an automatic side effect.
encoder = trained.encoder;
decoder = trained.decoder;
```

This trains the one-pass reconstruction `D(E(x))`, not the iterative inference
`cycle` recurrence. The loss is **half mean squared reconstruction error**:
`sum((D(E(x))-x)^2)/(2*batch_size*input_width)`. Every step computes both models'
gradients at the same pre-step weights and reuses the verified SGD implementation.
There is no extra normalization or separate encoder/decoder optimization.

## Admission and bounded work

- `epochs` is an integer in **1..16**; no zero, fractional, boolean, string or
  nonfinite value is accepted. `learning_rate` is finite in **[0.000001,0.1]**.
- Existing training model contracts are unchanged: 1..4 layers per direction,
  widths 1..16, exact supported map fields, matching reconstruction dimensions,
  finite parameters in ±10000, at most 4096 combined parameters (structural
  limits currently permit at most 2176), and normalized finite samples in [0,1].
- Both batches must contain **1..16** samples of the input width. Both are fully
  validated and copied before any training or held-out forward pass.
- With P parameters, N epochs, Bt training samples and Be evaluation samples,
  require **`P * (3*N*Bt + 2*Be) <= 32768`**. The three counted traversals per
  epoch are original forward, backward and candidate forward; the other two
  are initial and final held-out evaluation.
- This is a deterministic **traversal accounting proxy**, not an exact operation,
  CPU, memory, energy or elapsed-time count. Backward work, tensor copying,
  repeated validation inside `step`/`gradients`, allocation and runtime overhead
  also cost resources. External deadlines and memory limits remain appropriate.
- Returned trace has exactly N entries and hence at most **16**. Each entry has
  only four numeric fields: `epoch`, `loss_before`, `loss_after`,
  `max_abs_update`. It retains no model, batch, activation or gradient history.
  Model/batch shapes and per-step temporary structures remain independently
  bounded; physical reclamation still depends on the host runtime.

The public `step` contracts still apply at every epoch: requested and represented
parameter changes have absolute value at most one, and all returned parameters
stay in ±10000. No clipping, rate fallback or partial acceptance is performed.
An error anywhere throws the fixed existing
`pbai.feedback.gradients: invalid or unsupported request` message. No partial
training result is returned and no caller objects are mutated. This family name
is retained for compatibility. Host error rendering can separately include a
source excerpt; the API does not promise to redact the runtime's diagnostics.

## Held-out evaluation and truthful reporting

Evaluation samples are used **only for forward loss at the initial and final
weights**. They never enter gradient computation, choose a model, trigger early
stopping, modify the learning rate or reorder samples. A worsening candidate is
returned truthfully if all numeric bounds hold. The caller explicitly decides
whether to adopt it. Identical, rounded-away or saturated zero updates are also
reported without claiming that useful learning occurred.

The function does not automatically detect duplicates, split datasets or prove
that caller-supplied evaluation data are independent. Tests use distinct fixed
training/evaluation samples and verify that changing held-out values cannot
change the trained parameters or training trace. That is narrower than proving
absence of every possible form of data leakage.

The result has format `pbai.feedback.epochs.v1` and contains:

- `objective`, `encoder`, `decoder`, `learning_rate`, `epochs`;
- `parameters`, `input_width`, `train_batch_size`, `eval_batch_size`;
- `initial_train_loss`, `final_train_loss`, `initial_eval_loss`, `final_eval_loss`;
- `parameter_sample_traversals` and the bounded scalar `trace`.

Initial/final training loss reuse the first `step.loss_before` and last
`step.loss_after`. Held-out loss is evaluated exactly twice. Returned model
structures and trace are detached from caller objects and from each other.

## Acceptance and remaining scope

`pbai_feedback_epochs_test.sapp` uses a separate scalar inference/objective and
central finite differences for every reference update. It compares 1, 2 and 5
epochs across two architectures, every final parameter, each epoch trace and
initial/final losses. It also covers malformed/nonfinite inputs, both batches,
epoch bounds, exact work admission, caller/result isolation, truthful saturation,
reachable worsening and a last-bias failure in the second epoch after the first
successfully completes. The companion
`pbai_feedback_epochs_negative_cases.sapp` exposes 34 uncaught negative fixtures
for bounded external verification of the exact fixed diagnostic payload. The
retained language catch form returns a fallback without binding the exception;
catch-based ownership assertions alone do not establish that payload. A mutated
error-family message must fail the external diagnostic check. The shared numeric
admission checks `x==x` before ordered comparisons, so NaN uses the fixed family
for `gradients`, `step` and `train_epochs` instead of escaping as a VM comparison
error. Existing finite numeric behavior is unchanged.

The exact cap fixture uses a 9→13 / 13→9 pair (P=256), three epochs, 14 training
samples and one evaluation sample: `256*(3*3*14+2)=32768`. Increasing evaluation
count by one adds 512 and is rejected. For this fixed shape the count is always a
multiple of 256, so an artificial cap+1 fixture is not claimed.

This is an embedded subtree source implementation, not a global installation,
canonical core upgrade, distributed trainer, checkpoint manager or new pretrained general model.
It has no callbacks, file/network/device I/O, shuffling, automatic adoption,
logging, early stopping or accelerator implementation. General convergence,
generalization, superiority and platform/resource guarantees remain unproven.

For embedded invocation, provenance and a staged adoption checklist, see
[Embedded training adoption](Embedded_Training_Adoption.md).
