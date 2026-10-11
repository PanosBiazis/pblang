# One-pass reconstruction derivatives (HF-13A)

The embedded pbAI subtree provides the bounded derivative primitive. Run from the
PBLang project root with a compatible dependency containing this subtree:

```sapp
let T = import("stdlib:ai/pbai/stdlib/pbai_feedback_train.sapp");
let result = T.gradients(encoder, decoder, [[0.2, 0.8], [0.7, 0.1]]);
```

`encoder` and `decoder` use the existing `pbai.feedback.stack.v1` format,
including `w[output][input]` dense sigmoid layers. The `gradients` operation does
not update weights, run SGD, train a model, save checkpoints,
or differentiate the iterative `cycle` operation. Inference APIs are unchanged.
This embedded source addition does not activate a global installation or add a
canonical `stdlib:pbai_feedback_train.sapp` alias.

## Exact objective

For B normalized samples of input width d, with r = D(E(x)):

`L = sum_samples sum_coordinates (r - x)^2 / (2 * B * d)`.

The result has format `pbai.feedback.gradients.v1` and fields `objective`
(`half_mean_squared_reconstruction`), `loss`, `encoder`, `decoder`, `batch_size`,
`input_width`, `parameters`, and `parameter_sample_pairs`. `encoder` and `decoder`
are arrays of derivative layers with `n_in`, `n_out`, `w`, `b`; these are derivatives,
not trained model weights. Their shapes match the corresponding caller models.

The output delta includes the factors `1/B` and `1/d` exactly once. Both stacks
form one chain; the decoder input derivative propagates through the encoder's
output sigmoid exactly once. Every derivative uses the same original weights.
Targets are fixed input observations, not differentiable trainable parameters.
The returned loss is the objective of these unchanged weights. No online training
loss, validation improvement, convergence or generalization is implied.

Do not equate this objective with the legacy `NN.train_sample` return value: that
function reports SSE while its update corresponds to half-SSE. This addition
changes neither that function nor its historical learning-rate convention.

## Admission and ownership

- Each model contains exactly `format` and `layers`; each layer exactly `n_in`,
  `n_out`, `w`, and `b`. Unknown/missing fields and custom providers are rejected.
- 1–4 layers per direction; integer widths 1–16; finite parameters in ±10,000.
- Encoder output width equals decoder input width; decoder output equals the
  original input width. Batch size 1–16; finite sample coordinates in [0,1].
- At most 4,096 parameters and 32,768 parameter/sample pairs. The width/depth
  limits independently constrain the actual maximum to 2,176 parameters.
  A full-width eight-layer pair accepts 15 samples (32,640 pairs), rejects 16.
- Model copying is structurally bounded before work admission. Work admission
  precedes gradient allocation and all forward/backward passes. Shape/type
  checking does not recursively traverse arbitrary values, including cycles.
- Arithmetic is checked for finite bounded values. Stable sigmoid evaluates
  exponentials with nonpositive arguments. Finite saturation and zero derivatives
  are valid floating-point outcomes; they do not demonstrate learnability.
- Models and batches are copied; returned gradient arrays never alias inputs.
  There are no callbacks, I/O, logging, global mutation or automatic effects.
  Failure uses a fixed message without echoing model or sample content.

The pair-count limit bounds numerical loops, not elapsed time or host memory.
Validation/copying and VM/import overhead remain. External execution deadlines
and resource limits are still appropriate. No GPU/NPU/TPU acceleration is claimed.

## Independent numerical acceptance

`tests/semantics/pbai_feedback_gradients_test.sapp` defines its own scalar sigmoid
forward and objective, without production forward/loss/gradient helpers. It tests
all 19 parameters of a 2→2→1 / 1→2→2 pair and all 7 parameters of a 1→2 / 2→1 pair,
for batch sizes 1 and 3. Every weight and bias is checked by central differences
at epsilon 0.00001 and 0.000001, with tolerance `0.000001 + 0.0001*abs(reference)`.
Joint directional derivatives, nonzero encoder derivatives, caller/result
independence, deterministic results, malformed/nonfinite/cyclic inputs, dimension,
depth, batch/work limits and finite saturation are also checked. Numerical maxima
are printed for independent review; no threshold is relaxed to hide failure.

A separate [detached atomic SGD step](Neural_Feedback_Step.md) is available as
HF-13B. A separate [bounded trainer](Neural_Feedback_Epochs.md) supplies fixed
full-batch epochs and held-out evaluation (HF-13C). Installation/parity and
broader resource measurements remain separate work.

For embedded invocation, provenance and a staged adoption checklist, see
[Embedded training adoption](Embedded_Training_Adoption.md).
